//! hfte-agent: experimental local agent. It compares a fast price feed with a slow trading
//! account, sends an order to the slow account when a gap opens (only if LIVE_ORDERS=1) and
//! closes it automatically (rules in exit.rs). It only talks to local client programs over TCP
//! on 127.0.0.1 (protocol in docs/PROTOCOL.md). No other network access.

macro_rules! elog {
    ($($arg:tt)*) => {
        eprintln!("{} {}", $crate::utc_stamp(($crate::now_ms() / 1000) as i64), format_args!($($arg)*))
    };
}

mod config;
mod exit;
mod tcp;

use crossbeam_channel::{bounded, Receiver};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Connection labels, as sent in the client hello message: 0 = fast feed, 1 = slow (trading) account.
const CONNS: [&str; 2] = ["brokerA", "brokerB"];
/// Supported symbols (any broker suffix is handled by the client, not by the agent).
const SYMS: [&str; 3] = ["EURUSD", "GBPUSD", "USDJPY"];
/// Points per price unit: 100000 for 5-digit pairs, 1000 for 3-digit JPY pairs.
const MULT: [f64; 3] = [100000.0, 100000.0, 1000.0];

struct Setup {
    name: String,
    sym: usize,
    fast: usize,
    slow: usize,
    last_sig: Option<Instant>,
}
struct Pos {
    ticket: String,
    conn: usize,
    sym: usize,
    buy: bool,
    open: f64,
    opened_ms: u64,
    peak_pts: f64,
    closing_ms: u64,
    tries: u32,
}
enum Pend {
    Order {
        setup: String,
        sym: usize,
        buy: bool,
        req: f64,
        conn: usize,
    },
    Exit {
        ticket: String,
        setup: String,
        sym: usize,
        why: &'static str,
    },
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// UTC timestamp "YYYY-MM-DD HH:MM:SSZ" from Unix seconds (civil-from-days algorithm).
fn utc_stamp(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// ------------------------------------------------------------------ CORE (hot path)
fn core(frx: Receiver<tcp::In>) {
    let live_orders = std::env::var("LIVE_ORDERS")
        .map(|v| v == "1")
        .unwrap_or(false);
    elog!(
        "[core] LIVE_ORDERS={}",
        if live_orders {
            "1 (orders WILL be sent)"
        } else {
            "0 (DRY: signals only, no orders sent)"
        }
    );
    let cfg = match std::fs::read_to_string("hfte-config.json") {
        Ok(t) => match serde_json::from_str::<Value>(t.trim_start_matches('\u{feff}'))
            .map_err(|e| e.to_string())
            .and_then(|v| config::Cfg::from_json(&v))
        {
            Ok(c) => {
                elog!("[core] config ver {} loaded from hfte-config.json", c.ver);
                c
            }
            Err(e) => {
                elog!("[core] config INVALID: {} -> using defaults", e);
                config::Cfg::default()
            }
        },
        Err(_) => {
            elog!("[core] no hfte-config.json -> using defaults");
            config::Cfg::default()
        }
    };
    elog!("[core] active config: {:?}", cfg);

    let mut qs = [[(0.0f64, 0.0f64); 3]; 2];
    let mut qts = [[0u64; 3]; 2];
    let mut setups = [Setup {
        name: "gap-EURUSD".into(),
        sym: 0,
        fast: 0,
        slow: 1,
        last_sig: None,
    }];
    let mut pos: Vec<Pos> = Vec::new();
    let mut rid: u64 = 0;
    let mut pend: HashMap<u64, Pend> = HashMap::new();
    let mut mine: HashSet<String> = HashSet::new();
    let mut last_stat = Instant::now();
    let (mut cnt, mut mx_us) = (0u64, 0u128);

    loop {
        // ---- client feed: ticks, order results, positions
        let mut got = false;
        loop {
            let m = if got {
                frx.try_recv().ok()
            } else {
                frx.recv_timeout(Duration::from_millis(1)).ok()
            };
            let Some(m) = m else { break };
            got = true;
            match m {
                tcp::In::Down(c) => {
                    pos.retain(|p| p.conn != c);
                }
                tcp::In::Pos { conn, list } => {
                    let mut old: Vec<Pos> = Vec::new();
                    let mut k = 0;
                    while k < pos.len() {
                        if pos[k].conn == conn {
                            old.push(pos.remove(k));
                        } else {
                            k += 1;
                        }
                    }
                    for p in list {
                        let (om, pk, cl, tr) = match old.iter().find(|o| o.ticket == p.ticket) {
                            Some(o) => (o.opened_ms, o.peak_pts, o.closing_ms, o.tries),
                            None => (now_ms(), 0.0, 0, 0),
                        };
                        pos.push(Pos {
                            ticket: p.ticket,
                            conn,
                            sym: p.sym,
                            buy: p.buy,
                            open: p.open,
                            opened_ms: om,
                            peak_pts: pk,
                            closing_ms: cl,
                            tries: tr,
                        });
                    }
                }
                tcp::In::Res {
                    rid: r,
                    ok,
                    fill,
                    lat_us,
                    info,
                } => match pend.remove(&r) {
                    Some(Pend::Order {
                        setup,
                        sym,
                        buy,
                        req,
                        conn,
                    }) => {
                        let side = if buy { "buy" } else { "sell" };
                        if ok {
                            if let Some(t) =
                                info.split([';', ' ']).find_map(|w| w.strip_prefix("tk="))
                            {
                                mine.insert(t.to_string());
                            }
                        }
                        let slip = if ok { (fill - req) * MULT[sym] } else { 0.0 };
                        elog!("[trade] {} {} {} {} {} lots {} req {:.5} fill {:.5} slip {:+.1} pts ordersend_us {} | {}",
                                CONNS[conn], setup, SYMS[sym], side, if ok { "FILLED" } else { "REJECTED" }, cfg.lots, req, fill, slip, lat_us, info);
                    }
                    Some(Pend::Exit {
                        ticket,
                        setup,
                        sym,
                        why,
                    }) => {
                        elog!(
                            "[exit] {} {} {} tk={} {} fill {:.5}",
                            setup,
                            SYMS[sym],
                            why,
                            ticket,
                            if ok { "ok" } else { "FAILED" },
                            fill
                        );
                        if ok {
                            mine.remove(&ticket);
                        }
                    }
                    None => {}
                },
                tcp::In::Tick {
                    conn,
                    sym: s,
                    bid,
                    ask,
                } => {
                    let t0 = Instant::now();
                    let ts = now_ms();
                    qs[conn][s] = (bid, ask);
                    qts[conn][s] = ts;
                    // Only fast-feed ticks (connection 0) trigger a decision.
                    if conn != 0 {
                        continue;
                    }
                    if qs[1][s].0 == 0.0 || ts.saturating_sub(qts[1][s]) > cfg.stale_ms {
                        continue;
                    }

                    // ---- STRATEGY
                    for st in setups.iter_mut() {
                        if st.sym != s {
                            continue;
                        }
                        if let Some(l) = st.last_sig {
                            if l.elapsed() < Duration::from_millis(cfg.cooldown_ms) {
                                continue;
                            }
                        }
                        let (a, b) = (qs[st.fast][s], qs[st.slow][s]);
                        let pt = 1.0 / MULT[s];
                        if (b.1 - b.0) > cfg.max_slow_spread_pts * pt
                            || pos.len() as u32 >= cfg.max_positions
                        {
                            continue;
                        }
                        let (be, se) = (a.0 - b.1, b.0 - a.1);
                        let gap = be.max(se);
                        if gap < cfg.min_gap_pts * pt {
                            continue;
                        }
                        st.last_sig = Some(Instant::now());
                        let buy = be >= se;
                        let side = if buy { "buy" } else { "sell" };
                        let decide_us = t0.elapsed().as_micros();
                        let req = if buy { b.1 } else { b.0 };
                        elog!(
                            "[signal] {} {} {} gap {:.1} pts core_us {}{}",
                            st.name,
                            SYMS[s],
                            side,
                            gap / pt,
                            decide_us,
                            if live_orders { "" } else { " DRY" }
                        );

                        if live_orders && tcp::can_trade(st.slow) {
                            rid += 1;
                            pend.insert(
                                rid,
                                Pend::Order {
                                    setup: st.name.clone(),
                                    sym: s,
                                    buy,
                                    req,
                                    conn: st.slow,
                                },
                            );
                            let slp = if cfg.exit.sl_pts > 0.0 {
                                if buy {
                                    req - cfg.exit.sl_pts / MULT[s]
                                } else {
                                    req + cfg.exit.sl_pts / MULT[s]
                                }
                            } else {
                                0.0
                            };
                            let line = format!(
                                "O|{}|{}|{}|{}|{}|{}|{:.8}\n",
                                rid, SYMS[s], side, cfg.lots, req, cfg.dev_pts, slp
                            );
                            if !tcp::send(st.slow, &line) {
                                pend.remove(&rid);
                                elog!(
                                    "[trade] {} {} {} {} REJECTED client unreachable",
                                    CONNS[st.slow],
                                    st.name,
                                    SYMS[s],
                                    side
                                );
                            }
                        }
                    }
                    cnt += 1;
                    let us = t0.elapsed().as_micros();
                    if us > mx_us {
                        mx_us = us;
                    }
                }
            }
        }

        // ---- AUTOMATIC EXITS (decided locally, only for tickets opened by this agent in this session)
        {
            let now = now_ms();
            let mut i = 0;
            while i < pos.len() {
                let (c, s) = (pos[i].conn, pos[i].sym);
                let fi = setups
                    .iter()
                    .find(|st| st.slow == c && st.sym == s)
                    .map(|st| (st.fast, st.name.clone()));
                let Some((f, sname)) = fi else {
                    i += 1;
                    continue;
                };
                let (sl, fa) = (qs[c][s], qs[f][s]);
                if !mine.contains(&pos[i].ticket) || sl.0 == 0.0 || fa.0 == 0.0 {
                    i += 1;
                    continue;
                }
                let mut ps = exit::PosState {
                    buy: pos[i].buy,
                    open: pos[i].open,
                    opened_ms: pos[i].opened_ms,
                    peak_pts: pos[i].peak_pts,
                };
                let why = exit::check(
                    &cfg.exit,
                    &mut ps,
                    exit::Px {
                        bid: sl.0,
                        ask: sl.1,
                    },
                    exit::Px {
                        bid: fa.0,
                        ask: fa.1,
                    },
                    1.0 / MULT[s],
                    now,
                );
                pos[i].peak_pts = ps.peak_pts;
                let Some(why) = why else {
                    i += 1;
                    continue;
                };
                if !live_orders {
                    i += 1;
                    continue;
                }
                let wait = if pos[i].tries < cfg.close_retries {
                    1000
                } else {
                    5000
                };
                if pos[i].closing_ms != 0 && now.saturating_sub(pos[i].closing_ms) < wait {
                    i += 1;
                    continue;
                }
                pos[i].closing_ms = now;
                pos[i].tries += 1;
                if pos[i].tries == cfg.close_retries + 1 {
                    elog!(
                        "[exit] {} {} ALERT close tk={} failed {} times",
                        sname,
                        SYMS[s],
                        pos[i].ticket,
                        pos[i].tries - 1
                    );
                }
                rid += 1;
                let tk = pos[i].ticket.clone();
                if tcp::send(c, &format!("C|{}|{}\n", rid, tk)) {
                    pend.insert(
                        rid,
                        Pend::Exit {
                            ticket: tk,
                            setup: sname,
                            sym: s,
                            why: why.as_str(),
                        },
                    );
                }
                i += 1;
            }
        }

        if last_stat.elapsed() >= Duration::from_secs(5) {
            elog!(
                "[core] {} evaluated ticks/5s, worst core time {} us, fast={} slow={} trade={}",
                cnt,
                mx_us,
                tcp::live(0),
                tcp::live(1),
                tcp::can_trade(1)
            );
            cnt = 0;
            mx_us = 0;
            last_stat = Instant::now();
        }
    }
}

fn main() {
    let pin: Option<usize> = std::env::var("PIN_CORE").ok().and_then(|v| v.parse().ok());
    let port: u16 = std::env::var("AGENT_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9100);
    let (ftx, frx) = bounded::<tcp::In>(65536);
    std::thread::spawn(move || tcp::serve(ftx, port));

    let h = std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            if let Some(i) = pin {
                if let Some(ids) = core_affinity::get_core_ids() {
                    if let Some(id) = ids.get(i) {
                        core_affinity::set_for_current(*id);
                        elog!("[core] pinned to core {}", i);
                    }
                }
            }
            core(frx);
        })
        .unwrap();
    let _ = h.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_epoch() {
        assert_eq!(utc_stamp(0), "1970-01-01 00:00:00Z");
    }
    #[test]
    fn utc_known_dates() {
        assert_eq!(utc_stamp(1_700_000_000), "2023-11-14 22:13:20Z");
        assert_eq!(utc_stamp(951_782_400), "2000-02-29 00:00:00Z");
    }
}

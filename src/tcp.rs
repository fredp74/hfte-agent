//! TCP server for local client programs (127.0.0.1 only). One line per message, fields split by '|'.
//! Client -> agent: H (hello), S (symbol catalog, ignored), T (tick), P/PE (positions), R (order result).
//! Agent -> client: O (order), C (close). See docs/PROTOCOL.md.

use crate::{CONNS, SYMS};
use crossbeam_channel::Sender;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::Duration;

pub struct PosIn {
    pub ticket: String,
    pub sym: usize,
    pub buy: bool,
    pub open: f64,
}

pub enum In {
    Tick {
        conn: usize,
        sym: usize,
        bid: f64,
        ask: f64,
    },
    Res {
        rid: u64,
        ok: bool,
        fill: f64,
        lat_us: u64,
        info: String,
    },
    Pos {
        conn: usize,
        list: Vec<PosIn>,
    },
    Down(usize),
}

static CNT: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static TR: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];
static WR: Mutex<[Option<TcpStream>; 2]> = Mutex::new([None, None]);

pub fn live(c: usize) -> bool {
    CNT[c].load(Relaxed) > 0
}
pub fn can_trade(c: usize) -> bool {
    TR[c].load(Relaxed)
}

pub fn send(c: usize, line: &str) -> bool {
    let mut g = WR.lock().unwrap();
    match g[c].as_mut() {
        Some(s) => s.write_all(line.as_bytes()).is_ok(),
        None => false,
    }
}

fn handle(st: TcpStream, tx: Sender<In>) {
    let _ = st.set_nodelay(true);
    let mut rd = BufReader::new(match st.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut conn: Option<usize> = None;
    let mut is_trade = false;
    let mut plist: Vec<PosIn> = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        match rd.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let f: Vec<&str> = line.trim_end().split('|').collect();
        match f[0] {
            "H" if f.len() >= 4 => {
                if conn.is_some() {
                    continue;
                }
                let Some(c) = CONNS.iter().position(|x| *x == f[1]) else {
                    continue;
                };
                conn = Some(c);
                CNT[c].fetch_add(1, Relaxed);
                if f[3] == "1" {
                    let _ = st.set_write_timeout(Some(Duration::from_millis(200)));
                    if let Ok(w) = st.try_clone() {
                        WR.lock().unwrap()[c] = Some(w);
                        TR[c].store(true, Relaxed);
                        is_trade = true;
                    }
                }
                elog!(
                    "[tcp] client {} connected (hedge={} trade={})",
                    f[1],
                    f[2],
                    f[3]
                );
            }
            "T" if f.len() >= 4 => {
                let Some(c) = conn else { continue };
                let Some(sym) = SYMS.iter().position(|x| *x == f[1]) else {
                    continue;
                };
                let (Ok(bid), Ok(ask)) = (f[2].parse::<f64>(), f[3].parse::<f64>()) else {
                    continue;
                };
                let _ = tx.try_send(In::Tick {
                    conn: c,
                    sym,
                    bid,
                    ask,
                });
            }
            "P" if f.len() >= 7 => {
                let Some(sym) = SYMS.iter().position(|x| *x == f[2]) else {
                    continue;
                };
                let Ok(open) = f[5].parse::<f64>() else {
                    continue;
                };
                plist.push(PosIn {
                    ticket: f[1].to_string(),
                    sym,
                    buy: f[3] == "buy",
                    open,
                });
            }
            "PE" => {
                if let Some(c) = conn {
                    let _ = tx.send(In::Pos {
                        conn: c,
                        list: std::mem::take(&mut plist),
                    });
                }
            }
            "R" if f.len() >= 5 => {
                let (Ok(rid), Ok(fill), Ok(lat)) = (
                    f[1].parse::<u64>(),
                    f[3].parse::<f64>(),
                    f[4].parse::<u64>(),
                ) else {
                    continue;
                };
                let _ = tx.send(In::Res {
                    rid,
                    ok: f[2] == "1",
                    fill,
                    lat_us: lat,
                    info: f[5..].join("|"),
                });
            }
            _ => {}
        }
    }
    if let Some(c) = conn {
        CNT[c].fetch_sub(1, Relaxed);
        if is_trade {
            TR[c].store(false, Relaxed);
            WR.lock().unwrap()[c] = None;
            let _ = tx.send(In::Down(c));
        }
        elog!("[tcp] client {} disconnected", CONNS[c]);
    }
}

pub fn serve(tx: Sender<In>, port: u16) {
    let l = TcpListener::bind(("127.0.0.1", port))
        .expect("cannot bind 127.0.0.1:<port> (is another agent already running?)");
    elog!("[tcp] listening on 127.0.0.1:{}", port);
    for st in l.incoming() {
        let Ok(st) = st else { continue };
        let tx = tx.clone();
        std::thread::spawn(move || handle(st, tx));
    }
}

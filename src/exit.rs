//! Exit decision for one position: a pure function, no I/O.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Sl,
    Tp,
    Trail,
    Convergence,
    MaxLife,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Why::Sl => "sl",
            Why::Tp => "tp",
            Why::Trail => "trail",
            Why::Convergence => "convergence",
            Why::MaxLife => "max_life",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExitCfg {
    pub tp_pts: f64,          // 0 = disabled
    pub sl_pts: f64,          // 0 = disabled
    pub trail_start_pts: f64, // gain to reach before the trailing stop is armed
    pub trail_dist_pts: f64,  // 0 = trailing stop disabled
    pub conv_pts: f64,        // remaining fast/slow gap to exit on; < 0 = disabled
    pub min_life_ms: u64,
    pub max_life_ms: u64, // 0 = disabled
    pub sl_ignores_min_life: bool,
}

#[derive(Clone, Debug)]
pub struct PosState {
    pub buy: bool,
    pub open: f64,
    pub opened_ms: u64,
    pub peak_pts: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Px {
    pub bid: f64,
    pub ask: f64,
}

/// Returns Some(reason) if the position must be closed now.
/// P&L is evaluated at the slow broker closing price (bid for a buy, ask for a sell).
pub fn check(
    cfg: &ExitCfg,
    p: &mut PosState,
    slow: Px,
    fast: Px,
    point: f64,
    now_ms: u64,
) -> Option<Why> {
    let pnl = if p.buy {
        (slow.bid - p.open) / point
    } else {
        (p.open - slow.ask) / point
    };
    if pnl > p.peak_pts {
        p.peak_pts = pnl;
    }
    let age = now_ms.saturating_sub(p.opened_ms);

    if cfg.sl_pts > 0.0 && pnl <= -cfg.sl_pts && (age >= cfg.min_life_ms || cfg.sl_ignores_min_life)
    {
        return Some(Why::Sl);
    }
    if age < cfg.min_life_ms {
        return None;
    }
    if cfg.tp_pts > 0.0 && pnl >= cfg.tp_pts {
        return Some(Why::Tp);
    }
    if cfg.trail_dist_pts > 0.0
        && p.peak_pts >= cfg.trail_start_pts
        && pnl <= p.peak_pts - cfg.trail_dist_pts
    {
        return Some(Why::Trail);
    }
    if cfg.conv_pts >= 0.0 {
        let rest = if p.buy {
            (fast.bid - slow.bid) / point
        } else {
            (slow.ask - fast.ask) / point
        };
        if rest <= cfg.conv_pts {
            return Some(Why::Convergence);
        }
    }
    if cfg.max_life_ms > 0 && age >= cfg.max_life_ms {
        return Some(Why::MaxLife);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    const PT: f64 = 0.00001;

    fn cfg() -> ExitCfg {
        ExitCfg {
            tp_pts: 50.0,
            sl_pts: 100.0,
            trail_start_pts: 30.0,
            trail_dist_pts: 15.0,
            conv_pts: -1.0,
            min_life_ms: 0,
            max_life_ms: 0,
            sl_ignores_min_life: true,
        }
    }
    fn buy() -> PosState {
        PosState {
            buy: true,
            open: 1.10000,
            opened_ms: 0,
            peak_pts: 0.0,
        }
    }
    fn px(b: f64) -> Px {
        Px {
            bid: b,
            ask: b + 0.00010,
        }
    }

    #[test]
    fn tp() {
        let mut p = buy();
        assert_eq!(
            check(&cfg(), &mut p, px(1.10060), px(1.10060), PT, 100),
            Some(Why::Tp)
        );
    }
    #[test]
    fn sl_ignore_min_life() {
        let mut c = cfg();
        c.min_life_ms = 1000;
        let mut p = buy();
        assert_eq!(
            check(&c, &mut p, px(1.09880), px(1.09880), PT, 100),
            Some(Why::Sl)
        );
    }
    #[test]
    fn min_life_blocks_tp() {
        let mut c = cfg();
        c.min_life_ms = 1000;
        let mut p = buy();
        assert_eq!(check(&c, &mut p, px(1.10060), px(1.10060), PT, 100), None);
    }
    #[test]
    fn trailing() {
        let c = cfg();
        let mut p = buy();
        assert_eq!(check(&c, &mut p, px(1.10040), px(1.10040), PT, 100), None);
        assert_eq!(
            check(&c, &mut p, px(1.10024), px(1.10024), PT, 200),
            Some(Why::Trail)
        );
    }
    #[test]
    fn convergence() {
        let mut c = cfg();
        c.conv_pts = 5.0;
        let mut p = buy();
        assert_eq!(
            check(&c, &mut p, px(1.10018), px(1.10020), PT, 100),
            Some(Why::Convergence)
        );
    }
    #[test]
    fn max_life() {
        let mut c = cfg();
        c.max_life_ms = 2000;
        let mut p = buy();
        assert_eq!(
            check(&c, &mut p, px(1.10000), px(1.10000), PT, 3000),
            Some(Why::MaxLife)
        );
    }
}

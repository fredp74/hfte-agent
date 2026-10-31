//! Versioned configuration: JSON parsing, bounds and validation.
use crate::exit::ExitCfg;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Cfg {
    pub ver: i64,
    // entry
    pub min_gap_pts: f64,
    pub cooldown_ms: u64,
    pub max_slow_spread_pts: f64,
    // volume
    pub lots: f64,
    // execution
    pub dev_pts: i64,
    pub close_retries: u32,
    // risk
    pub stale_ms: u64,
    pub max_positions: u32,
    #[allow(dead_code)] // parsed and validated, but not enforced yet
    pub daily_loss: f64, // 0 = disabled
    // exit
    pub exit: ExitCfg,
}

impl Default for Cfg {
    // Starting values to be calibrated: they are not trading recommendations.
    fn default() -> Self {
        Cfg {
            ver: 0,
            min_gap_pts: 15.0,
            cooldown_ms: 4000,
            max_slow_spread_pts: 30.0,
            lots: 0.01,
            dev_pts: 20,
            close_retries: 3,
            stale_ms: 5000,
            max_positions: 5,
            daily_loss: 0.0,
            exit: ExitCfg {
                tp_pts: 0.0,
                sl_pts: 150.0,
                trail_start_pts: 0.0,
                trail_dist_pts: 0.0,
                conv_pts: 3.0,
                min_life_ms: 0,
                max_life_ms: 5000,
                sl_ignores_min_life: true,
            },
        }
    }
}

fn num(v: &Value, k: &str, d: f64, lo: f64, hi: f64) -> Result<f64, String> {
    match v.get(k) {
        None | Some(Value::Null) => Ok(d),
        Some(x) => {
            let n = x.as_f64().ok_or(format!("{k}: number expected"))?;
            if n < lo || n > hi {
                Err(format!("{k}: out of range [{lo}, {hi}]"))
            } else {
                Ok(n)
            }
        }
    }
}

fn flag(v: &Value, k: &str, d: bool) -> bool {
    v.get(k).and_then(|x| x.as_bool()).unwrap_or(d)
}

impl Cfg {
    /// Format : {"ver":3,"entry":{..},"size":{..},"exit":{..},"exec":{..},"risk":{..}}
    pub fn from_json(v: &Value) -> Result<Cfg, String> {
        let d = Cfg::default();
        let null = Value::Null;
        let g = |k: &str| v.get(k).unwrap_or(&null);
        let (e, s, x, c, r) = (g("entry"), g("size"), g("exit"), g("exec"), g("risk"));
        let cfg = Cfg {
            ver: v.get("ver").and_then(|y| y.as_i64()).unwrap_or(0),
            min_gap_pts: num(e, "min_gap_pts", d.min_gap_pts, 0.0, 10000.0)?,
            cooldown_ms: num(e, "cooldown_ms", d.cooldown_ms as f64, 0.0, 600000.0)? as u64,
            max_slow_spread_pts: num(
                e,
                "max_slow_spread_pts",
                d.max_slow_spread_pts,
                0.0,
                10000.0,
            )?,
            lots: num(s, "lots", d.lots, 0.01, 100.0)?,
            dev_pts: num(c, "dev_pts", d.dev_pts as f64, 0.0, 1000.0)? as i64,
            close_retries: num(c, "close_retries", d.close_retries as f64, 0.0, 20.0)? as u32,
            stale_ms: num(r, "stale_ms", d.stale_ms as f64, 100.0, 600000.0)? as u64,
            max_positions: num(r, "max_positions", d.max_positions as f64, 1.0, 1000.0)? as u32,
            daily_loss: num(r, "daily_loss", d.daily_loss, 0.0, 1.0e9)?,
            exit: ExitCfg {
                tp_pts: num(x, "tp_pts", d.exit.tp_pts, 0.0, 100000.0)?,
                sl_pts: num(x, "sl_pts", d.exit.sl_pts, 0.0, 100000.0)?,
                trail_start_pts: num(x, "trail_start_pts", d.exit.trail_start_pts, 0.0, 100000.0)?,
                trail_dist_pts: num(x, "trail_dist_pts", d.exit.trail_dist_pts, 0.0, 100000.0)?,
                conv_pts: num(x, "conv_pts", d.exit.conv_pts, -1.0, 100000.0)?,
                min_life_ms: num(x, "min_life_ms", d.exit.min_life_ms as f64, 0.0, 3600000.0)?
                    as u64,
                max_life_ms: num(x, "max_life_ms", d.exit.max_life_ms as f64, 0.0, 86400000.0)?
                    as u64,
                sl_ignores_min_life: flag(x, "sl_ignores_min_life", d.exit.sl_ignores_min_life),
            },
        };
        if cfg.exit.max_life_ms > 0 && cfg.exit.max_life_ms < cfg.exit.min_life_ms {
            return Err("exit.max_life_ms must be >= min_life_ms".into());
        }
        let no_exit = cfg.exit.tp_pts == 0.0
            && cfg.exit.trail_dist_pts == 0.0
            && cfg.exit.conv_pts < 0.0
            && cfg.exit.max_life_ms == 0;
        if no_exit {
            return Err(
                "exit: at least one automatic exit is required (tp, trail, conv or max_life)"
                    .into(),
            );
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_valid() {
        assert!(Cfg::from_json(&json!({})).is_ok());
    }
    #[test]
    fn lots_lower_bound() {
        assert!(Cfg::from_json(&json!({"size":{"lots":0}})).is_err());
    }
    #[test]
    fn inconsistent_life() {
        assert!(Cfg::from_json(&json!({"exit":{"min_life_ms":5000,"max_life_ms":1000}})).is_err());
    }
    #[test]
    fn exit_required() {
        assert!(Cfg::from_json(&json!({"exit":{"conv_pts":-1,"max_life_ms":0}})).is_err());
    }
}

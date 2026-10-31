# Configuration

The agent reads `hfte-config.json` from its working directory **once, at startup**. Restart it to apply a change. See `config.example.json`.

If the file is missing, or invalid, the agent logs the reason and runs with the defaults below. Always check the `active config` line in the log after starting.

The values below are defaults to be calibrated on your own data. They are not trading recommendations.

| Key | Default | Allowed range | Meaning |
| --- | --- | --- | --- |
| `ver` | 0 | integer | Free version number, logged at startup. |
| `entry.min_gap_pts` | 15 | 0 to 10000 | Minimum gap, in points, to raise a signal. |
| `entry.cooldown_ms` | 4000 | 0 to 600000 | Minimum time between two signals of the same setup. |
| `entry.max_slow_spread_pts` | 30 | 0 to 10000 | No signal if the slow spread is wider than this. |
| `size.lots` | 0.01 | 0.01 to 100 | Volume sent with each order. |
| `exec.dev_pts` | 20 | 0 to 1000 | Deviation, in points, sent with each order. |
| `exec.close_retries` | 3 | 0 to 20 | Close attempts one second apart. After that, retries every 5 seconds and one ALERT line is logged. |
| `risk.stale_ms` | 5000 | 100 to 600000 | Maximum age of the slow quote. |
| `risk.max_positions` | 5 | 1 to 1000 | No signal if the trading client reports this many positions or more (all positions it reports, not only the agent's). |
| `risk.daily_loss` | 0 | 0 to 1e9 | Parsed and validated but **not enforced**. |
| `exit.tp_pts` | 0 | 0 to 100000 | Take profit in points, 0 = disabled. |
| `exit.sl_pts` | 150 | 0 to 100000 | Stop loss in points, 0 = disabled. Also sent as a stop-loss price with the order. |
| `exit.trail_start_pts` | 0 | 0 to 100000 | Gain to reach before the trailing stop is armed. |
| `exit.trail_dist_pts` | 0 | 0 to 100000 | Trailing distance, 0 = disabled. |
| `exit.conv_pts` | 3 | -1 to 100000 | Close when the remaining fast/slow gap is at most this many points. Negative = disabled. |
| `exit.min_life_ms` | 0 | 0 to 3600000 | Minimum age before TP, trailing, convergence and max life apply. |
| `exit.max_life_ms` | 5000 | 0 to 86400000 | Close after this age, 0 = disabled. |
| `exit.sl_ignores_min_life` | true | true or false | If true, the stop loss applies even before `min_life_ms`. |

Validation rules:

- `exit.max_life_ms` must be at least `exit.min_life_ms` when it is not 0.
- At least one automatic exit among `tp_pts`, `trail_dist_pts`, `conv_pts` (0 or more) and `max_life_ms` must be enabled. The stop loss does not count.

Gap definition: with the fast quote `F` and the slow quote `S`, `buy_gap = F.bid - S.ask` and `sell_gap = S.bid - F.ask`. The gap is the larger of the two, expressed in points. One point is 1/100000 of a price unit for EURUSD and GBPUSD, and 1/1000 for USDJPY.
# HFTE Agent

Experimental local agent. It compares a fast price feed with a slow trading account, raises a signal when a gap opens, and, only if you enable it, sends an order to the slow account and closes it automatically.

> **Read this first.** This is experimental software, provided as is, without warranty. It is not financial advice. It runs in **DRY mode by default**: signals are logged and no order is ever sent. Set `LIVE_ORDERS=1` only on a demo account first, with the minimum lot size. You are responsible for every order it sends.

## Origin

This project comes from **Nexus HFTE**, a paid product (cloud console, ready-to-install pack, support): <https://algotradingresearch.com>. The agent is published here under the MIT license. The console, the ready-to-use client and the support are not part of this repository.

## What it does

- Listens on `127.0.0.1` (TCP, default port 9100) for local client programs. It has no other network access and sends no telemetry.
- Compares a fast quote with a slow quote. With the fast quote `F` and the slow quote `S`: `buy_gap = F.bid - S.ask`, `sell_gap = S.bid - F.ask`. A signal is raised when the larger gap reaches `min_gap_pts`.
- Filters signals with a cooldown, a maximum slow spread, a freshness check on the slow quote and a maximum number of positions.
- In live mode, sends an order to the slow account, then closes it on stop loss, take profit, trailing stop, gap convergence or maximum age.
- Every decision is taken locally. Logs go to stderr, timestamped in UTC.

## What it is not

- It is not a strategy shown to make money, and it makes no performance claim.
- It is not a client. You need a local client program that speaks the protocol in [docs/PROTOCOL.md](docs/PROTOCOL.md). This repository does not include one. A ready-to-use client is part of the Nexus HFTE pack.

## Logs

The agent is in DRY mode unless `LIVE_ORDERS=1`. DRY signals are based on quote arrival times: they include no slippage, no rejection and no last look, so they are optimistic by construction.

The `core_us` value in signal logs is the time spent inside the agent between receiving a quote and raising the signal. It is a diagnostic, not a latency: end-to-end latency has not been measured.

## Build and run

You need a stable Rust toolchain.

    cargo build --release
    cargo test
    target/release/hfte-agent

On Windows the binary is `target\release\hfte-agent.exe`. Run it from the directory that contains your `hfte-config.json`.

| Environment variable | Default | Meaning |
| --- | --- | --- |
| `LIVE_ORDERS` | unset | Set to `1` to send orders. Anything else means DRY. |
| `AGENT_PORT` | 9100 | TCP port on `127.0.0.1`. |
| `PIN_CORE` | unset | Index of the CPU core to pin the decision thread to. |

Configuration is described in [docs/CONFIG.md](docs/CONFIG.md), with an example in `config.example.json`. No prebuilt binaries are published here. If you build and distribute your own executable, expect antivirus false positives on unsigned binaries.

## Known limitations

- A single setup is wired in (`gap-EURUSD`). GBPUSD and USDJPY quotes are parsed, but no setup uses them.
- Only quotes from the fast connection trigger a decision.
- Point size assumes 5-digit pairs (EURUSD, GBPUSD) and a 3-digit pair (USDJPY).
- `risk.daily_loss` is read and validated but not enforced.
- No quote-freeze detection, no spread filter on the fast side (abnormal fast quotes around the daily rollover are not filtered), no trading-hours window.
- Freshness is measured on arrival time. The timestamp sent by the client is ignored.
- Automatic exits apply only to tickets opened by the agent during the current run, and only while the trading client keeps sending position reports. After a restart, existing positions are left to their stop loss.
- After a disconnection of the trading client, the age of a position restarts from zero when it is reported again, so `max_life` restarts too.
- `max_positions` counts every position the trading client reports, not only those opened by the agent.
- An invalid or missing configuration file makes the agent fall back to the defaults (it logs why).
- In an earlier internal build, one demo order was not closed by `max_life`; the cause was not established. Do not rely on `max_life` alone: keep the stop loss on.
- This build has been tested against a scripted local client only. No live broker connection has been tested with it.
- Unit tests cover the exit rules, the configuration and the UTC timestamps. The decision loop has no unit test yet.

## Brokers

Many brokers restrict or prohibit latency arbitrage. Read your broker's terms before using this software. Whether your use is allowed is your responsibility.

## Need it done for you?

If you would rather not set this up yourself, Nexus HFTE offers installation, configuration and supervision as a service. It is tooling and setup work, not trading recommendations, and no return is promised. Contact: <https://algotradingresearch.com>.

## Trademarks

The MIT license covers the code only. The names "Nexus HFTE" and "Algotradingresearch" and any logo are not licensed.

## License

MIT, see [LICENSE](LICENSE).

---

*A research project by [Algotradingresearch.com](https://algotradingresearch.com)*

# Client protocol

The agent listens on TCP `127.0.0.1:9100` (override with `AGENT_PORT`). It binds to the loopback interface only.

Messages are single lines ending with `\n`, fields separated by `|`, UTF-8. Prices are decimal numbers with a dot. Fields must not contain `|` or line breaks.

Two kinds of client connect, each on its own TCP connection and each starting with a hello message:

- a **fast-feed client** (label `brokerA`) that only sends quotes;
- a **slow client** (label `brokerB`) that sends quotes and positions, and executes the orders and closes the agent asks for.

The labels are just labels, not broker names. A hello with any other label is ignored.

## Client to agent

| Message | Fields | Meaning |
| --- | --- | --- |
| `H` | `H\|label\|hedge\|trade` | Hello, first message of a connection. `label` is `brokerA` or `brokerB`. `hedge` is `0` or `1` (informational, logged). `trade` is `1` if this client executes orders, else `0`. Only the first hello of a connection counts. |
| `T` | `T\|SYMBOL\|bid\|ask` | Quote. A fifth field (a timestamp) may be present and is ignored. |
| `P` | `P\|ticket\|SYMBOL\|side\|volume\|open_price\|profit` | One open position. `side` is `buy` or `sell`. Only `ticket`, `SYMBOL`, `side` and `open_price` are used. |
| `PE` | `PE` | End of a position report. The list of `P` lines received since the last `PE` replaces the known positions of this connection. |
| `R` | `R\|rid\|ok\|fill\|lat_us\|info` | Result of an `O` or `C` request. `ok` is `1` or `0`. `info` is free text; for a successful order it must contain a token `tk=<ticket>` (separated by `;` or a space), which is how the agent learns the ticket to manage. `lat_us` is logged as `ordersend_us`. |
| `S`, `SE` | any | Symbol catalog lines. Accepted and ignored. |

## Agent to client

Sent only to the connection that declared `trade=1`.

| Message | Fields | Meaning |
| --- | --- | --- |
| `O` | `O\|rid\|SYMBOL\|side\|volume\|price\|deviation_pts\|sl_price` | Open a position at market. `price` is the slow quote used for the decision. `deviation_pts` is the allowed deviation in points. `sl_price` is a stop-loss price, `0` for none. |
| `C` | `C\|rid\|ticket` | Close the position with this ticket. |

Each request carries a numeric `rid`; the client must answer it with an `R` line carrying the same `rid`.

## Rules the client must follow

1. Symbols are exactly `EURUSD`, `GBPUSD` or `USDJPY`, without broker suffix. Removing or adding a suffix is the client's job. Other symbols are ignored.
2. Only quotes from `brokerA` trigger a decision. Quotes from `brokerB` update the slow price.
3. The slow quote must be fresh: if the last `brokerB` quote for the symbol is older than `risk.stale_ms` (measured on the agent's arrival time, not on any timestamp in the message), no decision is made.
4. Send a position report (`P` lines then `PE`) after every `R` and regularly (for example twice per second). **Automatic exits are only evaluated for tickets present in the latest report.**
5. Use exactly one trading client (`trade=1`) for `brokerB`.
6. If the trading client disconnects, the agent forgets that connection's positions, and the age of positions restarts when they are reported again.

## Example

Lines starting with `>>` are sent by the agent, the others by clients. The values are illustrative.

    H|brokerA|0|0
    H|brokerB|0|1
    T|EURUSD|1.10000|1.10010|1
    T|EURUSD|1.10100|1.10110|2
    >> O|1|EURUSD|buy|0.01|1.1001|20|1.0986
    R|1|1|1.10010|1500|tk=555;done
    P|555|EURUSD|buy|0.01|1.10010|0.0
    PE
    T|EURUSD|1.10001|1.10011|3
    >> C|2|555
    R|2|1|1.10000|900|done

The `O` line is only sent when the agent runs with `LIVE_ORDERS=1`. Without it, the agent logs the signal and sends nothing.
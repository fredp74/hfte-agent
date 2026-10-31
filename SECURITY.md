# Security policy

This is experimental software, see the README. It is maintained on a best-effort basis, with no response-time commitment.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use GitHub private vulnerability reporting: open the Security tab of this repository and choose "Report a vulnerability". If that option is not available, contact us through <https://algotradingresearch.com> and ask for a private channel before sending any detail.

## Design limits you should know

- The agent listens on `127.0.0.1` only and has **no authentication**. Any program running on the same machine can connect, send quotes, and declare itself as the trading client. Never expose its port to a network (no port forwarding, no reverse proxy, no firewall rule that opens it).
- Orders are sent only when `LIVE_ORDERS=1` is set. Without it, the agent never sends an order.
- The agent opens no outgoing network connection.
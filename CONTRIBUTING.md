# Contributing

Thanks for your interest. This is a small reference agent, maintained on a best-effort basis.

- Open an issue before starting a large change.
- Scope: protocol, decision logic, exit rules, configuration, tests and documentation. Requests for trading advice or parameter tuning are out of scope.
- Before sending a pull request, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
- Add a test for any change to the exit rules or to the configuration parsing.
- Code, logs and documentation are written in English.
- Do not include secrets, broker credentials, account numbers, logs from real accounts, or files you do not own.
- Do not add performance or return claims to the documentation.
- By contributing, you agree that your contribution is licensed under the MIT license of this repository.
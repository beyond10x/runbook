# billing implementation (stub)

Fixed draft written by the stub LLM executor: a minimal in-memory Rust library (`src/lib.rs`)
with one `#[test]` per command in `tests/billing.rs`, enough to pass the interim story gate
(`sdc-tool.sh impl-gate`). The conformance target is still ESS's built-in `billing` reference
implementation (`ess verify conform run --target billing`), not this crate.

Run the tests: `task check`, or `cargo test --locked`.

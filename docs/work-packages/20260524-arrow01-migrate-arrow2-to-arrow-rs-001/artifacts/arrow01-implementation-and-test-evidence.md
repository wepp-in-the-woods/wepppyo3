# arrow01-implementation-and-test-evidence

Status: completed
Evidence mode: static-and-ran

## Static
- confirmed: target crates now use Apache `arrow-rs` (`arrow-array`, `arrow-schema`) and `parquet` dependencies.
- confirmed: no `arrow2` or `parquet2` usage remains in target crate manifests/source paths scoped by this package.
- confirmed: Phase B contract-derived tests were added for schema metadata, chunk handling, row-group accounting, typed error mapping, and calendar/catalog preview contracts.
- confirmed: `swat_interchange` compression contract test now initializes Python before exercising PyO3 error paths.

## Ran
- ran: `cargo test -p wepp_interchange_rust --lib` (PASS: 32 passed)
- ran: `cargo test -p swat_utils_rust --lib` (PASS: 20 passed)
- ran: `cargo test -p swat_interchange_rust --lib` (initial FAIL: unresolved Python link symbols)
- ran: `PYO3_PYTHON=/usr/bin/python3.12 cargo test -p swat_interchange_rust --lib` (initial FAIL: unresolved Python link symbols)
- ran: `cargo check -p swat_interchange_rust --tests` (PASS)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib` (PASS: 32 passed)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p wepp_interchange_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_utils_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib` (PASS across all target crates)

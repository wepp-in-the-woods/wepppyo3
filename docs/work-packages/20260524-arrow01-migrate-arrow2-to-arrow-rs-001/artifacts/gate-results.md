# gate-results

Status: completed
Evidence mode: static-and-ran

## Static
- Gate matrix reflects current execution evidence for ARROW01 hold-lift actions.

## Ran
- ran: `cargo test -p wepp_interchange_rust --lib` (PASS)
- ran: `cargo test -p swat_utils_rust --lib` (PASS)
- ran: `cargo test -p swat_interchange_rust --lib` (FAIL: Python linker symbols unresolved)
- ran: `PYO3_PYTHON=/usr/bin/python3.12 cargo test -p swat_interchange_rust --lib` (FAIL: same linker issue)
- ran: `cargo check -p swat_interchange_rust --tests` (PASS: compile-only)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib` (PASS)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p wepp_interchange_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_utils_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib` (PASS across all target crates)

## Gate Status
| Gate | Status | Evidence |
| --- | --- | --- |
| G1 Phase A inventory + mapping artifacts complete | PASS | `artifacts/arrow01-contract-implementation-evidence.md`, `artifacts/arrow01-api-mapping-and-compatibility-matrix.md`, `artifacts/arrow01-risk-and-guard-checklist.md` |
| G2 Sequencing exception reconciled | PASS | `artifacts/arrow01-preimplementation-contract-gate.md` |
| G3 Contract-derived tests authored | PASS | `artifacts/arrow01-contract-test-implementation-evidence.md` |
| G4 wepp_interchange runtime tests | PASS | `cargo test -p wepp_interchange_rust --lib` |
| G5 swat_utils runtime tests | PASS | `cargo test -p swat_utils_rust --lib` |
| G6 swat_interchange runtime tests | PASS | `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib` |
| G7 swat_interchange test compilation | PASS | `cargo check -p swat_interchange_rust --tests` |

## Overall
- Overall gate status: PASS.
- Runtime test replay for all target crates is validated using explicit Python link flags in this environment.

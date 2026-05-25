# ARROW01 Kickoff Agent Prompt

Scope: local repository engineering task in `/workdir/wepppyo3`; flat-file
reads/edits only; no external connectivity.
Phase: A only.
Files:
- `wepp_interchange/Cargo.toml`
- `swat_interchange/Cargo.toml`
- `swat_utils/Cargo.toml`
- `wepp_interchange/src/parquet.rs`
- `swat_interchange/src/parquet.rs`
- `swat_utils/src/parquet.rs`
- `docs/work-packages/20260524-arrow01-migrate-arrow2-to-arrow-rs-001/artifacts/arrow01-contract-implementation-evidence.md`
- `docs/work-packages/20260524-arrow01-migrate-arrow2-to-arrow-rs-001/artifacts/arrow01-api-mapping-and-compatibility-matrix.md`
- `docs/work-packages/20260524-arrow01-migrate-arrow2-to-arrow-rs-001/artifacts/arrow01-risk-and-guard-checklist.md`
Task: complete ARROW01 Phase A usage inventory and migration contract mapping
from `arrow2`/`parquet2` to `arrow-rs` + `parquet`.
Constraints: preserve Python-callable API behavior; preserve schema metadata
semantics; typed errors only; no silent fallback defaults.
Autonomy: execute this phase end-to-end and update listed artifacts without
requesting additional user direction unless hard-blocked.
Outputs: update listed ARROW01 artifacts for Phase A only.

Mandatory sequencing constraints:
- Do not modify production Rust source until:
  1. API mapping artifacts are complete,
  2. contract-derived compatibility tests are defined, and
  3. pre-implementation gate evidence is recorded.
- Do not introduce compatibility shims that silently coerce invalid data.

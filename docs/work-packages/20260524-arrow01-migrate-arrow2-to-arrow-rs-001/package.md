# 20260524-arrow01-migrate-arrow2-to-arrow-rs-001

## Status
- state: completed
- date: 2026-05-24
- timezone: UTC

## Objective
Migrate `/workdir/wepppyo3` parquet/arrow surfaces from deprecated
`arrow2` + `parquet2` to maintained Apache `arrow-rs` crates (`arrow-*`) plus
`parquet`, without breaking WEPPpy-facing schemas, metadata semantics, or
Python-callable module behavior.

## Why This Package Exists
`wepppyo3` previously depended on `arrow2`/`parquet2` in
`wepp_interchange`, `swat_interchange`, and `swat_utils`. The workspace
baseline now carries in-progress migration edits to `arrow-array`,
`arrow-schema`, and `parquet`, but closure evidence and compatibility gates
were incomplete. This package exists to close that gap with explicit
contract-derived tests, gate evidence, and disposition governance for
WEPPpy-facing interchange APIs.

## Scope
### Included
- Inventory all `arrow2` and `parquet2` usage in target crates.
- Define migration mapping from `arrow2` API surfaces to `arrow-rs` + `parquet`
  APIs for read/write paths and schema metadata handling.
- Update crate dependencies and Rust source for target modules.
- Preserve public Python-callable function signatures and interchange file
  schema/metadata contracts.
- Add/adjust tests that prove output compatibility and reader behavior.
- Record execution evidence and final disposition artifacts.

### Explicitly Out of Scope
- New WEPP/SWAT physics or model-algorithm changes.
- WEPPpy orchestration or route-level behavior changes.
- Release artifact publishing workflow redesign.

## Deliverables
1. `artifacts/arrow01-contract-implementation-evidence.md`
2. `artifacts/arrow01-contract-test-implementation-evidence.md`
3. `artifacts/arrow01-preimplementation-contract-gate.md`
4. `artifacts/arrow01-implementation-and-test-evidence.md`
5. `artifacts/arrow01-api-mapping-and-compatibility-matrix.md`
6. `artifacts/arrow01-risk-and-guard-checklist.md`
7. `artifacts/owned-file-manifest.md`
8. `artifacts/gate-results.md`
9. `artifacts/arrow01_disposition.md`
10. `artifacts/worker-handoff.md`
11. `artifacts/review_agent_a.md`
12. `artifacts/review_agent_b.md`
13. `artifacts/verification_agent_a.md`
14. `artifacts/verification_agent_b.md`

## Sequence
1. Migration contract + API mapping artifacts.
2. Contract-derived compatibility tests.
3. Pre-implementation gate evidence.
4. Production dependency/code migration.
5. Validation and disposition.

## Truthfulness Labeling Requirement
Evidence artifacts must include explicit `Static:` and `Ran:` sections.

## Dependencies
- `/workdir/wepppyo3/AGENTS.md`
- `/workdir/wepppyo3/README.md`
- `/workdir/wepppyo3/docs/module-registry.md`
- `/workdir/wepppyo3/docs/architecture-and-boundaries.md`
- `/workdir/wepppyo3/docs/claim-discipline.md`
- `/workdir/wepppyo3/docs/swat-interchange-spec.md`
- `/workdir/wepppyo3/docs/wepp-hill-pass-to-swat-rec-spec.md`
- `/workdir/wepppyo3/wepp_interchange/Cargo.toml`
- `/workdir/wepppyo3/swat_interchange/Cargo.toml`
- `/workdir/wepppyo3/swat_utils/Cargo.toml`
- `/workdir/wepppyo3/wepp_interchange/src/`
- `/workdir/wepppyo3/swat_interchange/src/`
- `/workdir/wepppyo3/swat_utils/src/`

## Intended Write Set
- `Cargo.lock`
- `wepp_interchange/Cargo.toml`
- `swat_interchange/Cargo.toml`
- `swat_utils/Cargo.toml`
- `wepp_interchange/src/**/*.rs`
- `swat_interchange/src/**/*.rs`
- `swat_utils/src/**/*.rs`
- `tests/**/*.py`
- `README.md`
- `docs/module-registry.md`
- `docs/work-packages/README.md`
- `docs/work-packages/20260524-arrow01-migrate-arrow2-to-arrow-rs-001/**`

## Phase Plan
### Phase A - Inventory and Contract Mapping
- Capture `arrow2`/`parquet2` usage inventory and one-to-one API migration map.
- Define compatibility guardrails for schema, metadata, and Python behavior.

### Phase B - Compatibility Tests and Pre-Implementation Gate
- Add/adjust tests to lock expected interchange behavior.
- Record pre-implementation gate evidence.

### Phase C - Dependency and Code Migration
- Close remaining migration implementation deltas and normalize writer/reader
  behavior across target crates where compatibility tests expose drift.
- Keep typed errors and explicit failure behavior.

### Phase D - Validation
- Run formatter and targeted/full tests for touched crates and modules.

### Phase E - Disposition
- Publish owned-file manifest, gate outputs, review/verification notes, and
  disposition.

## Exit Criteria
- No direct `arrow2` or `parquet2` dependencies remain in target crate
  `Cargo.toml` files.
- Target crate tests pass with migrated implementation.
- Python-callable APIs for touched modules remain stable.
- Interchange schema/metadata compatibility is documented and verified.
- Required evidence artifacts are complete.

## Security Impact and Review Gate
- security_impact: medium
- dedicated_security_review_required: yes
- Rationale: binary parsing and serialization dependencies change on
  WEPPpy-consumed interchange boundaries.

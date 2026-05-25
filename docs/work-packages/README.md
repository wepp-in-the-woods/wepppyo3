# Work Packages

Work packages are execution-ready refactor plans for `wepppyo3`.
Each package is a dated directory under `docs/work-packages/` with scoped
prompts, evidence artifacts, and disposition notes.

## Directory naming
`YYYYMMDD-<short-slug>-001/`

## Required files
- `package.md` - objective, scope, dependencies, intended write set, and gates.
- `prompts/` - active and archived execution prompts.
- `artifacts/` - evidence, gate results, review notes, and disposition.

## Evidence labels
All evidence artifacts must separate:
- `Static:` file and code inspection evidence.
- `Ran:` executed command evidence.

## Package index
- `20260524-arrow01-migrate-arrow2-to-arrow-rs-001/`
  - Status: `completed`
  - Purpose: migrate `wepppyo3` from deprecated `arrow2`/`parquet2` to
    maintained Apache `arrow-rs` + `parquet` crates while preserving
    WEPPpy-facing schemas, metadata, and Python-callable behavior.

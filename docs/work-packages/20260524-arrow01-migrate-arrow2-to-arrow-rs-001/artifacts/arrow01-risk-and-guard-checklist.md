# arrow01-risk-and-guard-checklist

Status: completed
Evidence mode: static-analysis

## Static
- confirmed: Risk register updated for the actual current baseline (dependency migration already present; compatibility regression risk remains primary).
- confirmed: Guard controls map directly to Phase B test gates in `arrow01-api-mapping-and-compatibility-matrix.md`.
- confirmed: Guard posture preserves typed failures and forbids silent fallback/coercion.

## Ran
- ran: static inventory commands and diffs used in Phase A evidence artifacts.
- not run: runtime validation commands are deferred to Phase B/Phase D.

## Risk Register and Required Guards
| Risk ID | Risk | Impact | Guard / Control | Verification gate |
| --- | --- | --- | --- | --- |
| R1 | Metadata drift (dataset or field keys/values) during closure edits | Breaks WEPPpy downstream contracts | Enforce exact key/value parity for emitted metadata (`dataset_version*`, `schema_version`, `units`, `description`, `source_name`, etc.) | `arrow01_writer_schema_metadata_roundtrip` |
| R2 | Empty dataset behavior regression | Fails low-volume or no-row jobs | Preserve explicit `empty_chunk` behavior and schema-aligned empty arrays in each sink | `arrow01_writer_empty_chunk_contract` |
| R3 | Row-group or row-count accounting drift | Misleading telemetry and parity evidence | Keep `row_groups += 1` and `rows_written += rows` semantics consistent with current implementation | `arrow01_writer_row_group_and_row_count_accounting` |
| R4 | Compression contract drift in SWAT interchange | User-visible behavior change | Preserve accepted compression strings and explicit invalid-value rejection | `arrow01_compression_option_contract_swat_interchange` |
| R5 | Reader-path semantic drift (calendar/catalog/preview) | Incorrect inferred schemas or reporting output | Validate read-path parity on representative fixtures with mixed primitive/string types | `arrow01_calendar_preview_catalog_read_contract` |
| R6 | Typed error contract regression | Debuggability loss and hidden data faults | Keep `From<arrow_schema::ArrowError>` and `From<parquet::errors::ParquetError>` mappings; no silent fallback | `arrow01_error_mapping_typed_contract` |
| R7 | Chunk invariants regression in local `arrow_support` | Runtime panic or malformed batches | Preserve equal-length assertion and array ownership semantics across all three `arrow_support` modules | Unit coverage over chunk construction paths |
| R8 | Incomplete migration closure despite no `arrow2` symbol matches | Hidden transitive or behavioral gaps | Validate via behavior-first tests and then workspace-level grep closure evidence | Phase B evidence + closure grep |
| R9 | Atomic output-file replacement regression | Temp-file leakage or output corruption | Preserve cross-device rename fallback (`EXDEV` copy + remove) in all sink modules | Targeted sink write tests with temp dirs |

## Non-Negotiable Guards
1. No silent defaults, clamping, or coercion for invalid parse/compression/data cases.
2. No broad boxed error erasure; maintain typed crate error enums.
3. Preserve Python-callable boundary behavior and schemas.
4. Preserve metadata semantics on all emitted parquet datasets.
5. Treat Phase B compatibility-test completion as mandatory before any completion claim.

## Phase B Gate Inputs
- Implement all six contract-derived compatibility tests.
- Record test implementation evidence in `artifacts/arrow01-contract-test-implementation-evidence.md`.
- Record pre-implementation gate decision in `artifacts/arrow01-preimplementation-contract-gate.md`.
- Maintain truthfulness labels (`Static:` and `Ran:`) in all new evidence artifacts.

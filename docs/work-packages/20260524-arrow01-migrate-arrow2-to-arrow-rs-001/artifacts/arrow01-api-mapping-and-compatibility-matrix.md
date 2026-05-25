# arrow01-api-mapping-and-compatibility-matrix

Status: completed
Evidence mode: static-analysis

## Static
- confirmed: Phase A inventory and migration mapping were completed against the current `/workdir/wepppyo3` baseline.
- confirmed: Dependency migration at `Cargo.toml` level is already present in all three target crates.
- confirmed: Remaining work is compatibility verification and migration closure proof, not first-pass dependency replacement.

## Ran
- ran: `rg -n "arrow2|parquet2" wepp_interchange swat_interchange swat_utils`
  - result: no matches found.
- ran: `rg -l "arrow_array|arrow_schema|parquet::arrow|ArrowWriter|ParquetRecordBatchReaderBuilder|arrow_support" wepp_interchange/src swat_interchange/src swat_utils/src | sort`
  - result: 26 active call-site files inventoried.
- ran:
  - `git diff -- wepp_interchange/Cargo.toml swat_interchange/Cargo.toml swat_utils/Cargo.toml`
  - `git diff -- wepp_interchange/src/parquet.rs swat_interchange/src/parquet.rs swat_utils/src/parquet.rs`
  - `git diff -- wepp_interchange/src/errors.rs swat_interchange/src/errors.rs swat_utils/src/errors.rs`
  - result: confirms concrete API substitutions captured in matrix below.

## Usage Inventory

### Direct Dependency Declarations (current baseline)
| Crate | Active declarations |
| --- | --- |
| `wepp_interchange` | `arrow-array = "53.4.1"`, `arrow-schema = "53.4.1"`, `parquet = "53.4.1"` (`wepp_interchange/Cargo.toml:11-13`) |
| `swat_interchange` | `arrow-array = "53.4.1"`, `arrow-schema = "53.4.1"`, `parquet = "53.4.1"` (`swat_interchange/Cargo.toml:11-13`) |
| `swat_utils` | `arrow-array = "53.4.1"`, `arrow-schema = "53.4.1"`, `parquet = "53.4.1"` (`swat_utils/Cargo.toml:11-13`) |

### Writer Adapter Surfaces (Phase A primary files)
| File | Current implementation role |
| --- | --- |
| `wepp_interchange/src/parquet.rs` | Snappy Parquet V2 writer using `ArrowWriter`; metadata/statistics enabled; dictionary-empty handling for `Utf8`/`LargeUtf8`; atomic rename semantics preserved. |
| `swat_interchange/src/parquet.rs` | Compression-selectable writer (`Snappy`, `Zstd`, `Gzip`, `Uncompressed`) using `ArrowWriter`; dictionary disabled + plain encoding policy; atomic rename semantics preserved. |
| `swat_utils/src/parquet.rs` | Snappy Parquet V2 writer using `ArrowWriter`; dictionary-empty handling for `Utf8`/`LargeUtf8`; atomic rename semantics preserved. |

### Additional Arrow/Parquet Surfaces in scope for compatibility closure
- `wepp_interchange/src/calendar.rs`, `wepp_interchange/src/catalog.rs`
- `swat_utils/src/calendar.rs`
- `swat_interchange/src/lib.rs` (preview + formatting helpers)
- `swat_interchange/src/parser.rs` (schema metadata and table parsing)
- `wepp_interchange/src/errors.rs`, `swat_interchange/src/errors.rs`, `swat_utils/src/errors.rs`
- local `arrow_support` shims in all three crates

## API Migration Matrix
| Legacy surface (from diff evidence) | Current mapped surface | Contract compatibility requirement |
| --- | --- | --- |
| `arrow2::array::Array` | `arrow_array::Array` | Preserve nullability and downcast behavior used by preview/calendar/parser paths. |
| `arrow2::datatypes::{Schema, DataType, Field}` | `arrow_schema::{Schema, DataType, Field}` | Preserve all field names, type IDs, nullability flags, and metadata key/value content. |
| `arrow2::chunk::Chunk` | local `arrow_support::Chunk<T>` | Preserve equal-length invariant and row count semantics (`len`, `is_empty`, chunk boundaries). |
| `arrow2::io::parquet::write::FileWriter` | `parquet::arrow::arrow_writer::ArrowWriter<File>` | Preserve output validity, flush/close behavior, and row-group accounting. |
| `row_group_iter(...) + writer.write(row_group)` | `RecordBatch::try_new(...) + writer.write(&batch)` | Preserve `rows_written` and `row_groups` counters exactly for telemetry parity. |
| `WriteOptions { version: V2 }` | `WriterProperties::set_writer_version(PARQUET_2_0)` | Preserve Parquet V2 file output contract. |
| `CompressionOptions::*` (arrow2) | `parquet::basic::Compression::*` through crate-local mapping | Preserve accepted PyO3 compression strings and reject unsupported values explicitly. |
| `parquet2` error type | `parquet::errors::ParquetError` | Preserve typed propagation via crate-local error enums (`InterchangeError`, `SwatError`). |
| `arrow2::error::Error` | `arrow_schema::ArrowError` | Preserve typed propagation and human-readable error context. |
| `arrow2` reader infer/read path | `ParquetRecordBatchReaderBuilder` | Preserve catalog/calendar/preview behavior, especially type display and null handling. |
| dictionary empty helpers via `arrow2` dict arrays | `arrow_array::DictionaryArray<Int32Type>::try_new(...)` | Preserve zero-row dictionary-capable schema behavior for `Utf8`/`LargeUtf8` columns. |

## Compatibility Invariants
1. Python-callable signatures and argument semantics remain unchanged.
2. Dataset-level metadata keys remain unchanged where currently emitted (`dataset_version*`, `schema_version`, `source_file`, `swat_interchange_version`, optional `run_id`, etc.).
3. Field-level metadata keys remain unchanged (`units`, `description`, `source_name` where applicable).
4. Empty-output behavior remains explicit and readable (no silent fallback arrays or dropped fields).
5. Typed error behavior remains explicit; no broad fallback coercion.

## Contract-Derived Compatibility Tests (Phase B to implement)
1. `arrow01_writer_schema_metadata_roundtrip`
- Write representative outputs from all three sinks, read schema back, assert exact dataset and field metadata key/value parity.

2. `arrow01_writer_empty_chunk_contract`
- Write zero-row output for each sink, assert readable parquet, expected schema, and `rows_written == 0`.

3. `arrow01_writer_row_group_and_row_count_accounting`
- Write multi-chunk fixture data and assert `WriteSummary.rows_written` and `WriteSummary.row_groups` values.

4. `arrow01_compression_option_contract_swat_interchange`
- Assert accepted compression set remains `{snappy, zstd, gzip, none}` and invalid strings return explicit error.

5. `arrow01_calendar_preview_catalog_read_contract`
- Assert calendar/catalog/preview read-path behavior and type-display mapping remain stable on fixtures.

6. `arrow01_error_mapping_typed_contract`
- Force read/write/parsing failures and assert typed errors carry explicit Arrow/Parquet messages.

## Phase B Blocking Condition
Production migration closure cannot be declared complete until all six tests above are implemented and recorded in `artifacts/arrow01-contract-test-implementation-evidence.md` plus `artifacts/arrow01-preimplementation-contract-gate.md`.

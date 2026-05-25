# arrow01-contract-implementation-evidence

Status: completed
Evidence mode: static-analysis

## Static
- confirmed: Phase A was executed as an inventory/mapping phase only. No new production Rust edits were made by this phase execution.
- confirmed: Existing workspace baseline already contains migration-oriented production edits across the three target crates (authorized by user for this package).
- confirmed: Direct dependency declarations in target crates are already on maintained `arrow-rs` + `parquet` crates:
  - `wepp_interchange/Cargo.toml:11-13`
  - `swat_interchange/Cargo.toml:11-13`
  - `swat_utils/Cargo.toml:11-13`
- confirmed: Target writer adapters now use `parquet::arrow::arrow_writer::ArrowWriter` + `WriterProperties`:
  - `wepp_interchange/src/parquet.rs:9-11`, `:38-46`
  - `swat_interchange/src/parquet.rs:6-10`, `:60-70`
  - `swat_utils/src/parquet.rs:9-11`, `:38-46`
- confirmed: Shared replacement for `arrow2::chunk::Chunk` is present in all three crates via local `arrow_support` modules:
  - `wepp_interchange/src/arrow_support.rs:1-56`
  - `swat_interchange/src/arrow_support.rs:1-56`
  - `swat_utils/src/arrow_support.rs:1-56`
- confirmed: Read-path migrations to `ParquetRecordBatchReaderBuilder` are present in target crates:
  - `wepp_interchange/src/calendar.rs:10, 25-43`
  - `wepp_interchange/src/catalog.rs:6, 82-86`
  - `swat_utils/src/calendar.rs:10, 25-43`
  - `swat_interchange/src/lib.rs:922-925`
- confirmed: Typed error mappings were migrated from `arrow2/parquet2` errors to `arrow_schema/parquet` errors:
  - `wepp_interchange/src/errors.rs:83-92`
  - `swat_interchange/src/errors.rs:194-203`
  - `swat_utils/src/errors.rs:83-92`
- confirmed: Contract mapping artifact is updated at `artifacts/arrow01-api-mapping-and-compatibility-matrix.md`.
- confirmed: Risk/guard artifact is updated at `artifacts/arrow01-risk-and-guard-checklist.md`.
- confirmed: Contract-derived compatibility tests are defined in the mapping artifact for Phase B implementation.

## Ran
- ran: `nl -ba wepp_interchange/Cargo.toml`
  - result: `arrow-array`, `arrow-schema`, `parquet` present at lines 11-13.
- ran: `nl -ba swat_interchange/Cargo.toml`
  - result: `arrow-array`, `arrow-schema`, `parquet` present at lines 11-13.
- ran: `nl -ba swat_utils/Cargo.toml`
  - result: `arrow-array`, `arrow-schema`, `parquet` present at lines 11-13.
- ran: `rg -n "arrow2|parquet2" wepp_interchange swat_interchange swat_utils`
  - result: no matches.
- ran: `git diff -- wepp_interchange/Cargo.toml swat_interchange/Cargo.toml swat_utils/Cargo.toml`
  - result: confirms replacement from `arrow2/parquet2` to `arrow-array/arrow-schema/parquet`.
- ran: `git diff -- wepp_interchange/src/parquet.rs swat_interchange/src/parquet.rs swat_utils/src/parquet.rs`
  - result: confirms `FileWriter/row_group_iter` path replaced by `ArrowWriter + RecordBatch` write path.
- not run: formatter/build/test commands (`cargo fmt`, `cargo test`, etc.) are intentionally deferred to later phases because this phase is contract mapping only.

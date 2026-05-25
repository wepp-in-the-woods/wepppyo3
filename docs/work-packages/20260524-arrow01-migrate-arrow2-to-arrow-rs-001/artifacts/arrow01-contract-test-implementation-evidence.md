# arrow01-contract-test-implementation-evidence

Status: completed
Evidence mode: static-and-ran

## Static
- confirmed: Implemented contract-derived tests in source modules:
  - `wepp_interchange/src/parquet.rs`
    - `arrow01_writer_schema_metadata_roundtrip_wepp_interchange`
    - `arrow01_writer_empty_chunk_contract_wepp_interchange`
    - `arrow01_writer_row_group_and_row_count_accounting_wepp_interchange`
    - `arrow01_error_mapping_typed_contract_wepp_interchange`
  - `swat_utils/src/parquet.rs`
    - `arrow01_writer_schema_metadata_roundtrip_swat_utils`
    - `arrow01_writer_empty_chunk_contract_swat_utils`
    - `arrow01_writer_row_group_and_row_count_accounting_swat_utils`
  - `swat_utils/src/calendar.rs`
    - `arrow01_calendar_read_contract_swat_utils`
    - `arrow01_error_mapping_typed_contract_swat_utils_calendar_parquet`
  - `wepp_interchange/src/catalog.rs`
    - `arrow01_catalog_schema_read_contract`
  - `swat_interchange/src/parquet.rs`
    - `arrow01_writer_schema_metadata_roundtrip_swat_interchange`
    - `arrow01_writer_empty_chunk_contract_swat_interchange`
    - `arrow01_writer_row_group_and_row_count_accounting_swat_interchange`
    - `arrow01_error_mapping_typed_contract_swat_interchange`
  - `swat_interchange/src/lib.rs`
    - `arrow01_compression_option_contract_swat_interchange`
    - `arrow01_calendar_preview_contract_swat_interchange`
    - test guard update: initialize Python runtime before compression-error path with `pyo3::prepare_freethreaded_python()`
- confirmed: All six Phase B contract areas are covered by implemented tests:
  1. schema metadata roundtrip
  2. empty-chunk contract
  3. row-group/row-count accounting
  4. compression option contract
  5. calendar/catalog/preview read contract
  6. typed error mapping contract

## Ran
- ran: `cargo test -p wepp_interchange_rust --lib`
  - result: PASS (`32 passed; 0 failed`), includes:
    - `catalog::tests::arrow01_catalog_schema_read_contract`
    - all four `parquet::tests::arrow01_*_wepp_interchange`
- ran: `cargo test -p swat_utils_rust --lib`
  - result: PASS (`20 passed; 0 failed`), includes:
    - `calendar::tests::arrow01_calendar_read_contract_swat_utils`
    - `calendar::tests::arrow01_error_mapping_typed_contract_swat_utils_calendar_parquet`
    - all three `parquet::tests::arrow01_*_swat_utils`
- ran: `cargo test -p swat_interchange_rust --lib`
  - result: FAIL (environment link-time failure: unresolved Python C symbols such as `PyList_New`, `PyLong_FromUnsignedLongLong`, `PyGILState_Ensure`)
- ran: `PYO3_PYTHON=/usr/bin/python3.12 cargo test -p swat_interchange_rust --lib`
  - result: FAIL (same link-time unresolved Python C symbols)
- ran: `cargo check -p swat_interchange_rust --tests`
  - result: PASS (test code compiles; runtime link/execution remains blocked in current environment)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib`
  - result: PASS (`32 passed; 0 failed`)
- ran: `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p wepp_interchange_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_utils_rust --lib && RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p swat_interchange_rust --lib`
  - result: PASS across all target crates (`32`, `20`, `32` tests passed respectively)

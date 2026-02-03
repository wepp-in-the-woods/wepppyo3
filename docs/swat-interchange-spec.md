# SWAT+ Output Interchange Spec (wepppyo3)
> Draft specification for a Rust/PyO3 SWAT+ interchange module that converts SWAT+ flat-file outputs into analytics-ready Parquet.
> **See also:** `wepppy/wepp/interchange/README.md`, `wepppy/wepp/interchange/interchange_documentation.py`, `wepppy/nodb/mods/swat/wepp-swat-spec.md`

## Status
- Draft (February 3, 2026)
- Owner: wepppy + wepppyo3
- Target fixture: `/wc1/runs/pe/pertinent-conventioneer/swat/outputs/run_20260203T042219Z`

## Problem Statement
SWAT+ output files are large, whitespace-delimited flat files meant for Fortran workflows. They are expensive to parse repeatedly (some single files exceed multiple GB) and lack standardized metadata for modern analytics.

We need a deterministic, high-throughput conversion pipeline that:
- Keys off `files_out.out` (the SWAT output manifest).
- Writes one Parquet file per output table into an `interchange/` subdirectory.
- Adds units + descriptions to every schema field.
- Normalizes column names (no spaces).
- Supports optional deletion of raw text after successful conversion.
- Uses concurrency for throughput and an `ncpu` parameter for memory control.
- Generates a README.md that matches the WEPP interchange format.

## Goals
- Convert SWAT+ outputs to Parquet under `swat/outputs/run_*/interchange/`.
- Preserve output fidelity (row order, values) while normalizing column names.
- Include schema metadata (units + descriptions) for every column.
- Stream large files with bounded memory.
- Provide PyO3 entrypoints consistent with wepppyo3 conventions.
- Provide README.md generation in the same format as WEPP interchange documentation.

## Non-goals
- Changing SWAT+ output generation or formats.
- Replacing SWAT+ outputs with Parquet-only outputs.
- Rewriting existing WEPP interchange logic.

## Scope
Phase 1 focuses on the standard SWAT+ text outputs listed in `files_out.out` for a completed run. Phase 2 adds edge-case handling, full schema coverage, and Python integration.

## Inputs and Outputs
### Inputs
- `run_output_dir`: e.g., `/wc1/runs/<prefix>/<runid>/swat/outputs/run_YYYYMMDDTHHMMSSZ`
- `files_out.out`: manifest of output files written by SWAT+.

### Output directory layout
All Parquet outputs go under:
```
<run_output_dir>/interchange/
  interchange_version.json
  interchange.log
  files_out.parquet
  <output_basename>.parquet
  README.md
```

### Interchange versioning
`interchange_version.json` is required and must use this schema:
```json
{
  "spec": "swat-interchange-v1",
  "generator": "wepppyo3.swat_interchange",
  "generator_version": "<semver>",
  "status": "in_progress|complete|partial|failed",
  "created_utc": "<ISO-8601>",
  "run_output_dir": "<path>",
  "run_id": "<optional>",
  "files_total": "<optional int>",
  "files_written": "<optional int>",
  "files_skipped": "<optional int>",
  "errors": "<optional list[{\"filename\": str, \"reason\": str}]>"
}
```
The README version line must include `spec` and `generator_version`.
`interchange_version.json` is written at the start of conversion with `"status": "in_progress"`.
On completion, update `"status"` to:
- `complete` if every file converted successfully.
- `partial` if any file was skipped or failed.
- `failed` if the run terminates with a fatal error before finishing; include `errors`.
If the process crashes or is killed, leave `"status": "in_progress"`.
Include `"files_total"` (after include/exclude + de-dup), `"files_written"`, `"files_skipped"`, and `"errors"` (if any).
Consumers should check `status` before trusting the interchange is complete.

### Error classification and status transitions
- Error-class reasons: `header_error`, `parse_error`, `column_mismatch`, `decode_error`, `file_changed`.
- Skip-only reasons: `duplicate`, `exists`, `missing`, `not_in_manifest`, `path_invalid`.
- Run-level fatal reasons: `manifest_missing`, `manifest_unreadable`, `interchange_created_utc_invalid`.
- `interchange_version.json.errors` includes entries for error-class reasons and fatal run errors; it may be a subset of `skipped`.
- For run-level errors that are not tied to a specific file, set `filename` to `<run>`.
- When `fail_fast=True`, write `status="failed"`, record the first error in `errors`, then raise.
- When `fail_fast=False`, set `status="partial"` if any error-class reason occurred; otherwise `status="complete"` (skip-only reasons do not force partial).

### Run-level safety (existing interchange)
- If `interchange_version.json` exists with `status="in_progress"` and `overwrite=False`, return an error with reason `interchange_in_progress` unless `stale_after_hours` is set and `created_utc` is older than now minus `stale_after_hours`.
  - If `stale_after_hours` is set and `created_utc` is missing or unparsable, return a run-level error with reason `interchange_created_utc_invalid`.
- If `interchange_version.json` exists with `status="complete"` and `overwrite=False`, perform no conversions and return a summary where all files are recorded in `skipped` with reason `interchange_complete`.
- If `interchange_version.json` exists with `status in {"partial","failed"}` and `overwrite=False`, return an error with reason `interchange_partial` or `interchange_failed`.
- If `overwrite=True` (or a stale in_progress override is allowed), conversion proceeds and `interchange_version.json` is rewritten with `status="in_progress"` at the start.

### Consumer compatibility
- Consumers must check the `spec` field. If the major version differs (e.g., `swat-interchange-v2` vs `swat-interchange-v1`), warn and attempt best-effort read.
- Minor/patch updates (if adopted) are backward-compatible.
  - Version format: `swat-interchange-v<MAJOR>` or `swat-interchange-v<MAJOR>.<MINOR>` (minor is backward-compatible).

### Interchange path convention
- Use `swat/outputs/run_*/interchange/` (per-run).
- Output filename = input filename with extension replaced by `.parquet`.
  - Example: `basin_wb_day.txt` -> `basin_wb_day.parquet`
  - Example: `files_out.out` -> `files_out.parquet`

### Deletion policy
- Each conversion function accepts `delete_after_interchange: bool = False`.
- On success, delete only the source file that was converted.
- Never delete `files_out.out` unless explicitly requested (default: keep).
- `delete_after_interchange` never deletes `files_out.out`.
- To delete `files_out.out`, set `delete_manifest=True`.
- Record deletions in `<run_output_dir>/interchange/interchange.log`.
- `delete_manifest` applies only to `swat_outputs_to_parquet` (manifest-aware conversion).
- Deletions occur only after the final Parquet file is atomically renamed into place.
- `swat_output_to_parquet` never deletes `files_out.out`, even if passed as `source_path`.
- If `dry_run=True`, do not delete files; conversions still run and intended deletions are logged to `interchange.log` with action `delete_skipped`.
- If the output exists and `overwrite=False`, do not delete the source; log `delete_skipped` with reason `exists`.
- For `swat_output_to_parquet` when `source_path` is outside a SWAT run output directory and `allow_external_delete=True`, write `interchange.log` into the output directory (`<dirname(output_path)>/interchange.log`).

### interchange.log format
Each line is JSON:
```json
{"timestamp": "<ISO-8601>", "action": "delete|delete_skipped|skip|error|convert", "file": "<basename>", "reason": "<optional>"}
```
Create `interchange.log` on the first event; do not create an empty file.

### Deletion failure handling
- If source file deletion fails after successful conversion, log the error to `interchange.log` and continue.
- Deletion is best-effort; conversion success is not reverted on deletion failure.

## Manifest parsing (`files_out.out`)
- First line is a title and ignored for the table.
- Each subsequent line is:
  - Column 1: category/type (e.g., `BASIN`, `HRU`, `SWAT-DEG_CHANNEL`).
  - Column 2: filename (e.g., `basin_wb_day.txt`).
- Filenames are treated as basenames relative to `run_output_dir`.
- Ignore blank lines and lines beginning with `#`.
- Inline comments are stripped when a `#` is preceded by whitespace (the remainder of the line is ignored).
- If a line has more than two whitespace-delimited tokens, only the first two are parsed and the full line is stored in `source_line`.
- `source_line` stores the original line before inline comment stripping; parsed tokens come from the stripped line.
- Filenames must be basenames only (no path separators, no absolute paths, no `..`). Invalid entries are skipped with reason `path_invalid`.
- Duplicate filenames are de-duplicated by first occurrence for conversion; later duplicates are recorded in `skipped` with reason `duplicate`.
- The manifest itself is converted to `files_out.parquet` as a raw mirror (no de-dup) with columns:
  - `category` (string)
  - `filename` (string)
  - `source_line` (string, optional for traceability)
  - `line_no` (int, 1-based physical line number in `files_out.out`, including the title line)
- `files_out.parquet` fields must include `units` and `description` metadata (units are empty strings):
  - `category`: description `SWAT output category`
  - `filename`: description `SWAT output filename`
  - `source_line`: description `Original manifest line`
  - `line_no`: description `1-based line number in files_out.out`

## Column Normalization
Output column names must not contain spaces.

### Normalization rules
- Trim leading/trailing whitespace.
- Lowercase all names.
- Replace runs of non-alphanumeric characters with `_`.
- Collapse repeated `_` and trim to avoid leading/trailing `_`.
- If duplicate names result, suffix with `_2`, `_3`, etc.
- If a name is empty after normalization, use `col_<index>`.
- If a name starts with a digit, prefix with `col_`.

### Source name retention
- Store original column name as field metadata `source_name`.
- Store source file as dataset metadata `source_file`.

## Schema Metadata
Every Parquet field must include:
- `units`: parsed from the units row (when present) or from the schema registry.
- `description`: from the schema registry; if missing, use the original column header.
- Units precedence: `units_overrides` > parsed units row (sliced) > empty string.
- Description precedence: `column_descriptions` > source header.

### Units row parsing
Most SWAT+ tables provide:
1. Title line
2. Header line (column names)
3. Units line (aligned to header columns)

**Important:** units rows often contain blank fields for `null` columns or spacing.
- Extract column positions from the header line (`\S+` token start indices).
- Slice the units line by column boundaries to preserve blanks.
- If units row is missing or too short, fall back to registry defaults or empty units.

## Parsing Model
### Common parser (preferred reuse)
Define a shared, reusable text table parser in the swat_interchange crate (or a shared wepppyo3 utility module) that supports:
- Header line parsing with column position boundaries.
- Optional units line parsing via header positions.
- Column-position parsing as the primary strategy.
- If a line is shorter than the last column boundary, pad missing trailing columns with nulls.
- Optional whitespace-delimited fallback may be enabled explicitly for legacy tables; otherwise do not re-tokenize lines.
- Per-column type casting (string/int/float) with registry overrides.
- Chunked iteration for streaming Parquet writes.
- Short lines (missing trailing columns) are padded and are not considered a column-count mismatch.
- Column-count mismatch errors only occur when non-whitespace data extends past the last column boundary or when fallback tokenization yields extra columns.

### Missing values
- Empty or whitespace-only fields are `null`.
- Sentinel values `-9999`, `-999`, `-99` are treated as `null` unless overridden in `SwatTableSpec`.
- Text sentinels `NA`, `N/A`, `null`, `---` are treated as `null` for numeric columns unless overridden.

### Type inference and temporal columns
- If `column_types` are not specified, default to `float` when all sampled non-null values parse as numbers; otherwise use `string`.
- Type inference samples the first 10,000 non-null values per column; it must not scan the entire file.
- Type inference must not buffer more than the sample. A second pass (re-open file) is allowed for streaming write.
- Numeric defaults to `float64`.
- Integer types must be declared explicitly in the registry (`int32` or `int64`) to avoid accidental truncation.
- Date/time columns (e.g., `yr`, `mon`, `day`, `jday`) remain numeric unless explicitly typed as Arrow dates/timestamps in the registry.

### Encoding
- Input files are assumed UTF-8. If decoding fails, error with file context and skip the file.

### File-specific overrides
Provide a registry (`SwatTableSpec`) keyed by basename to override:
- `skip_lines`: number of leading lines to skip before header.
- `header_line_index` and `units_line_index`.
- `column_types` (int/float/string).
- `column_descriptions`.
- `units_overrides` when the units row is absent or ambiguous.
- `sentinel_overrides` to map file-specific missing values.
- `table_description` for README generation.

Registry behavior:
- `SwatTableSpec` lives at `wepppyo3/swat_interchange/registry.rs` and is loaded at compile time.
- Match order: exact basename > glob patterns > default.
- Glob matching uses shell-style `*` on basenames only.
- Glob precedence: longest pattern wins; if two patterns have equal length, first defined wins.
- `header_line_index` and `units_line_index` are 0-based indices after `skip_lines` is applied.
- Registry column keys refer to source header names before normalization; normalization occurs after metadata is applied.
- If a source header name repeats, registry keys use `name` for the first occurrence and `name#2`, `name#3`, etc. for subsequent occurrences.

Example registry entries should include:
- `files_out.out`
- `checker.out`
- `crop_yld_*.txt`
- `lu_change_out.txt`
- `recall_aa.txt`

## Parquet Writing
- Use the existing `wepp_interchange/src/parquet.rs` approach (arrow2 + parquet2) for atomic temp writes.
- Add dataset metadata:
  - `swat_interchange_version`
  - `source_file`
  - `category` (from manifest)
  - `run_id` (if parsed from `<run_output_dir>/index.json`, key `run_id`)
- Use `snappy` compression by default.
- Support `chunk_rows` parameter to control row group size.
- `overwrite: bool = False` controls behavior when output files already exist.
- Valid `compression` values: `snappy`, `zstd`, `gzip`, `none`.

### Existing file handling
- When `overwrite=False` and the output file exists, skip and record in `skipped` with reason `exists`.

## Python API (PyO3)
### Module name
`wepppyo3.swat_interchange` (extension: `swat_interchange_rust`)

### Core entrypoints
```python
swat_outputs_to_parquet(
    run_output_dir: str,
    *,
    interchange_dir: str | None = None,
    manifest_path: str | None = None,
    ncpu: int | None = None,
    chunk_rows: int | None = None,
    delete_after_interchange: bool = False,
    dry_run: bool = False,
    delete_manifest: bool = False,
    fail_fast: bool = False,
    include: list[str] | None = None,
    exclude: list[str] | None = None,
    write_manifest: bool = True,
    compression: str = "snappy",
    stale_after_hours: float | None = None,
    overwrite: bool = False,
) -> dict
```

- Defaults:
  - `interchange_dir` -> `<run_output_dir>/interchange`
  - `manifest_path` -> `<run_output_dir>/files_out.out`
  - `ncpu` -> `min(num_cpus, 4)`
  - `chunk_rows` -> `100000`
  - `stale_after_hours` -> `None`
- `include` / `exclude` accept basenames from `files_out.out` (no globs).
- If both `include` and `exclude` are provided, remove `exclude` entries from the `include` set.
- Files listed in `include` but not present in the manifest are recorded in `skipped` with reason `not_in_manifest`.
- `include` is de-duplicated (first occurrence wins).
- `exclude` entries not present in the manifest are ignored (no `skipped` record).
- When a manifest is present, processing order, `output_paths`, and `skipped` follow manifest order after include/exclude filtering.
- When no manifest is present, processing order, `output_paths`, and `skipped` follow the de-duplicated `include` list.
- `fail_fast=True` aborts on the first file-level error; otherwise continue and record in `skipped`.
- When `fail_fast=True`, update `interchange_version.json` to `failed` and raise after the first file-level error.
- `dry_run=True` disables deletions; conversions still run and intended deletions are logged with action `delete_skipped`.
- `overwrite=True` allows replacing existing parquet outputs.
- `stale_after_hours` allows recovery from `status="in_progress"` when `created_utc` is older than now minus the threshold.
- Reads `files_out.out` and converts each listed output file.
- Writes `interchange_version.json` and (optionally) `files_out.parquet`.
- When `manifest_path` is missing and `include` is provided, `write_manifest` is ignored and `files_out.parquet` is not written.
- Parameter validation errors raise `ValueError`: `chunk_rows <= 0`, invalid `compression`, `ncpu < 0`, `ncpu > 32`, `stale_after_hours <= 0`.
- Fatal conversion errors raise `RuntimeError` and do not return a summary dict.
- Returns a summary dict:
  - `elapsed_ms`: int
  - `run_output_dir`: str
  - `interchange_dir`: str
  - `files_total`: int
  - `files_written`: int
  - `files_skipped`: int
  - `rows_written`: int (total across all files)
  - `row_groups`: int (total across all files)
  - `output_paths`: list[str] (manifest order, or `include` order without a manifest; only successfully written outputs)
  - `skipped`: list[{"filename": str, "reason": str}]

### Single-file entrypoint
```python
swat_output_to_parquet(
    source_path: str,
    output_path: str,
    *,
    category: str | None = None,
    chunk_rows: int | None = None,
    delete_after_interchange: bool = False,
    allow_external_delete: bool = False,
    dry_run: bool = False,
    compression: str = "snappy",
    overwrite: bool = False,
) -> dict
```

- Returns a summary dict:
  - `elapsed_ms`: int
  - `source_path`: str
  - `output_paths`: list[str] (single entry, kept for parity with batch API)
  - `rows_written`: int
  - `row_groups`: int
  - `category`: str | None
- `swat_output_to_parquet` does not write `interchange_version.json` or `README.md`.
- `delete_after_interchange` is rejected unless `source_path` is under a SWAT run output directory (contains `/swat/outputs/run_`) or `allow_external_delete=True`.
- Parameter validation errors raise `ValueError`: `chunk_rows <= 0`, invalid `compression`.
- Fatal conversion errors raise `RuntimeError`.

### README generation
```python
generate_interchange_documentation(
    interchange_dir: str,
    *,
    to_readme_md: bool = True,
) -> str
```

- Must match the **format** produced by `wepppy.wepp.interchange.interchange_documentation`:
  - Title line: `# Interchange Documentation`
  - Version line (italic): `_Interchange Version: <spec> (generator <generator_version>)_`
    - If the version manifest is missing/unreadable, use `<spec> (manifest missing)`.
  - One or more `##` sections; for SWAT+ use `## SWAT+ Outputs` (single section is fine).
  - Each Parquet file gets a `###` block with this exact layout (only include files that exist):

    ```text
    ### `basin_wb_day.parquet`

    <description line>

    | Column | Type | Units | Description |
    | --- | --- | --- | --- |
    | col_a | int32 | mm | Example description |
    | col_b | float64 | m^3 | Another description |

    Preview:

    col_a | col_b
    --- | ---
    mm | m^3
    1 | 2.5
    2 | 3.1
    3 | 4.7
    ```

  - The description line uses `SwatTableSpec.table_description` when available; otherwise default to `SWAT+ output table: <basename>`.
  - Schema table:
    - Columns are exactly `Column`, `Type`, `Units`, `Description`.
    - `Type` uses the Arrow display string (`int32`, `float64`, `utf8`, etc.).
    - `Units` and `Description` come from field metadata; if missing, leave blank.
  - Preview table:
    - Show up to 3 rows (head of the table).
    - If there are zero rows, omit the preview table and write `_No rows_`.
    - No leading/trailing `|` characters (matches `pandas`-style Markdown output).
    - The units row appears immediately after the separator row.
    - Value formatting:
      - `null`/empty/NaN values render as empty strings.
      - Floats that are whole numbers render without decimals (`1` vs `1.0`).
      - Other floats use `g` formatting (e.g., `1.23`, `1e-05`).
      - Integers render as plain integers.
  - Return value: the Markdown string; when `to_readme_md=True`, also write `README.md`.

## Concurrency
- Use a bounded worker pool (Rust thread pool or rayon) to process files in parallel.
- `ncpu` controls the maximum concurrent conversions.
- Concurrency must respect memory limits; default to `min(num_cpus, 4)` if not specified.
- `ncpu=0` is treated as `ncpu=1`.
- Maximum allowed `ncpu` is `32`.
- `ncpu < 0` is invalid and must error.
- Processing order is deterministic (manifest order, or `include` order without a manifest), regardless of completion order.
- Each file conversion streams rows to Parquet; no whole-file buffering.

## Error Handling
- Missing `files_out.out` -> error unless `include` list is provided.
- If `files_out.out` is missing and `include` is not provided, record a run-level error with reason `manifest_missing` and fail before file processing.
- If `files_out.out` exists but cannot be read or parsed, record a run-level error with reason `manifest_unreadable` and fail before file processing.
- When `include` is provided without a manifest, treat each entry as a basename resolved against `run_output_dir`. Category metadata is `null`.
- For fatal preflight errors (before any file conversion), if `interchange_dir` can be created, write `interchange_version.json` with `status="failed"` and a run-level `errors` entry; otherwise raise without writing the manifest.
- Missing output file listed in manifest -> skip with warning and record in summary with reason `missing`.
- Header parsing failure -> error with file context; record reason `header_error`.
- Units row mismatch -> warning; continue with empty units for mismatched columns.
- Column count mismatch between header and data (non-whitespace data beyond the last header boundary or fallback tokenization yields extra columns) -> error with file context; record reason `column_mismatch`.
- Numeric parse failures (non-sentinel) -> error with file context; if `fail_fast=False`, skip file with reason `parse_error`.
- Decode failures -> error with file context; record reason `decode_error`.
- If a file has a valid header but zero data rows, write an empty parquet with schema-only rows; if header is missing, error.
- When `fail_fast=False`, file-level errors are recorded in `skipped` and processing continues.
- If a source file changes during conversion (size or mtime), treat as an error and record in `skipped` with reason `file_changed`.
- Canonical skip/error reasons: `duplicate`, `exists`, `not_in_manifest`, `missing`, `header_error`, `parse_error`, `column_mismatch`, `decode_error`, `file_changed`, `path_invalid`, `manifest_missing`, `manifest_unreadable`, `interchange_created_utc_invalid`, `interchange_in_progress`, `interchange_complete`, `interchange_partial`, `interchange_failed`.
- File-level error entries appear in both `skipped` and `interchange_version.json.errors`; the summary dict does not include a separate `errors` list.

### Partial failure with deletion
- When `delete_after_interchange=True` and `fail_fast=False`:
  - Only delete source files that converted successfully.
  - Never delete files that failed or were skipped.
  - Log all deletions and failures to `interchange.log`.
- No rollback of already-written parquet outputs.

## Validation and QA
- Verify column count consistency across header, units, and data lines.
- Ensure normalized column names are unique.
- Ensure every field has `units` and `description` metadata.
- Large-file smoke test with target fixture.

## Development Fixture
Use:
```
/wc1/runs/pe/pertinent-conventioneer/swat/outputs/run_20260203T042219Z
```
This fixture includes multi-GB daily outputs (e.g., `channel_sd_day.txt`, `hydout_day.txt`) to validate streaming and concurrency.

## Multi-Phase Implementation Plan
### Phase 0: Discovery + schema registry
- Enumerate `files_out.out` from the fixture and catalog all outputs.
- Build a `SwatTableSpec` registry for known outputs, including:
  - column types
  - descriptions
  - unit overrides where missing
- Identify files with nonstandard headers (e.g., `checker.out`, `files_out.out`).

### Phase 1: Core swat_interchange crate
- Create `swat_interchange/` crate in wepppyo3 (PyO3 extension).
- Implement shared text-table parser with header-position logic.
- Implement Parquet writer using existing `parquet.rs` pattern.
- Add `swat_output_to_parquet` and `swat_outputs_to_parquet` entrypoints.

### Phase 2: Metadata + normalization + delete-after
- Implement column normalization and metadata injection (`units`, `description`, `source_name`).
- Add delete-after-interchange behavior with an `interchange.log` audit.
- Ensure manifest conversion (`files_out.parquet`) includes category + filename.

### Phase 3: Concurrency + performance
- Add bounded concurrency with `ncpu` control.
- Stream parse/write with chunked row groups.
- Validate performance on multi-GB files from the fixture.

### Phase 4: README generation
- Implement `generate_interchange_documentation` in swat_interchange.
- Match WEPP interchange README format exactly (schema tables + preview rows).
- Include run metadata and versioning line.

### Phase 5: wepppy integration
- Add Python wrapper and integration hooks in `wepppy.nodb.mods.swat` to run after SWAT completion.
- Optionally add an RQ task for SWAT interchange generation.
- Ensure `query_engine` can discover SWAT interchange datasets (if desired).

### Phase 6: Tests
- Unit tests for parser, normalization, and metadata coverage.
- Regression test on a reduced fixture subset.
- Optional performance benchmark for large files.

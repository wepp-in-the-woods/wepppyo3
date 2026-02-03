# WEPP Hillslope Pass -> SWAT+ Recall Specification (wepppyo3)

## Purpose
Define a new `swat_utils` module in `wepppyo3` that converts WEPP hillslope pass files in a `wepp_output_dir` into SWAT+ recall daily files inside `swat_txtinout_dir`. The conversion is file-based, deterministic, and uses the existing Rust hillslope pass parser (`wepp_interchange/src/hill_pass.rs`).

## Scope
- Input: WEPP hillslope pass files in `wepp_output_dir` named `H<wepp_id>.pass.dat` (case-sensitive).
- Output: One SWAT+ daily recall file per hillslope (`hill_{wepp_id:05d}.rec` by default) in `swat_txtinout_dir/<recall_subdir>/`.
- Optional: return a manifest mapping `wepp_id -> recall file path`, plus basic stats for validation.
- Write `recall.rec` into `swat_txtinout_dir` and write `recall.con` when enough connectivity metadata is provided.

## Inputs and Outputs

### Inputs
- `wepp_output_dir` (path): directory containing WEPP hillslope pass files named `H<wepp_id>.pass.dat`.
- `swat_txtinout_dir` (path): SWAT+ project root (TxtInOut) where `recall.rec`/`recall.con` live and where recall files are written.
- `recall_subdir` (optional, default `recall`): subdirectory under `swat_txtinout_dir` that stores the per-hillslope recall files.
- `recall_connections` (optional): mapping of `wepp_id -> downstream channel index` for writing `recall.con`.
- `recall_wst` (optional, default `wea1`): SWAT weather station reference written to `recall.con`.
- `recall_object_type` (optional, default `sdc`): SWAT-DEG output object type for `recall.con` connections.
- `cli_calendar_path` (optional path): calendar file for non-Gregorian calendars (as used by `hill_pass.rs`).

### Output files
For each hillslope pass file `wepp_output_dir/H<wepp_id>.pass.dat`:
- `swat_txtinout_dir/<recall_subdir>/hill_{wepp_id:05d}.rec` (default naming; configurable).

Always:
- `swat_txtinout_dir/recall.rec`

If `recall_connections` is provided:
- `swat_txtinout_dir/recall.con`

Recall file format: SWAT+ daily recall flat file (space-delimited), one dataset per file.

### Output manifest (optional)
- Return a list of `{wepp_id, pass_file, recall_file, days_written, start_year, end_year, status, skip_reason}`.
  - `status`: `written` or `skipped`.
  - `skip_reason`: optional string when `status=skipped` (for example `empty_pass`, `no_rows`).

## Module and API

### New module
- `wepp_interchange/src/swat_utils.rs`
- Exposed via `wepp_interchange/src/lib.rs` to Python as a `#[pyfunction]`.

### Proposed Python signature
```python
wepp_hillslope_pass_to_swat_recall(
    wepp_output_dir: str,
    swat_txtinout_dir: str,
    version_major: int,
    version_minor: int,
    *,
    recall_subdir: str = "recall",
    cli_calendar_path: Optional[str] = None,
    filename_template: str = "hill_{wepp_id:05d}.rec",
    include_subsurface: bool = True,
    include_tile: bool = True,
    include_baseflow: bool = True,
    recall_connections: Optional[List[Tuple[int, int]]] = None,
    recall_wst: str = "wea1",
    recall_object_type: str = "sdc",
    ncpu: Optional[int] = None,
    write_manifest: bool = False,
) -> Optional[List[Dict[str, Any]]]
```

### Required behavior
- Parse all files in `wepp_output_dir` matching `^H(\\d+)\\.pass\\.dat$` (case-sensitive).
- Extract `wepp_id` from the filename digits between `H` and `.pass.dat`.
- Skip non-matching files silently (treat as a glob).
- For each file, call `hill_pass::hillslope_pass_to_columns(path, cli_calendar_path, &VersionInfo)` and produce a daily recall time series.
- Always write `recall.rec` into `swat_txtinout_dir`; write `recall.con` when `recall_connections` is provided.
- Use `recall_wst` for the `WST` column and `recall_object_type` for `OBTYP_OUT1` when writing `recall.con`.
- Parallelization: if `ncpu` is provided and > 1, process hillslopes in parallel; default is to use the host CPU count.

## Parsing and Data Mapping

### Data source

Use `wepp_interchange/src/hill_pass.rs`:
- `hillslope_pass_to_columns(path, cli_calendar_path, version)`
- Extracted columns include:
  - `event`, `year`, `julian`, `sim_day_index`, `month`, `day_of_month`, `water_year`
  - `runvol` (m^3), `sbrunv` (m^3), `drrunv` (m^3)
  - `sedcon_1..5` (per-class sediment concentrations)
  - `clot/slot/saot/laot/sdot` (per-class fractions; use only for fallback)
  - `gwbfv`/`gwdsv` (groundwater/baseflow diagnostics; `gwbfv` can be included in FLO when enabled; `gwdsv` is not emitted to SWAT+)

### Daily flow volume (FLO)
Use a configurable flow composition (default includes subsurface, tile, and baseflow):
- `FLO = runvol + (include_subsurface ? sbrunv : 0) + (include_tile ? drrunv : 0) + (include_baseflow ? gwbfv : 0)`
- `gwdsv` remains diagnostic-only and is not emitted to SWAT+ recall rows.

### Daily sediment loads
WEPP pass files provide per-class sediment concentration (`sedcon_1..5`).
- Compute per-class mass (kg): `mass_i_kg = sedcon_i * runvol`
- Convert to metric tons: `mass_i_t = mass_i_kg / 1000.0`
- Total sediment: `SED = sum(mass_i_t)`
- SWAT+ fields:
  - CLA = class 1 (clay)
  - SIL = class 2 (silt)
  - SAG = class 3 (small aggregate)
  - LAG = class 4 (large aggregate)
  - SAN = class 5 (sand)
- GRV = 0 (WEPP hillslope pass does not include gravel)

Fallbacks:
- If `sedcon_i` are all zero but class fractions are provided, compute total sediment from sum of class fractions only if a total sediment concentration is known; otherwise keep all class masses at 0 and log a warning.
- If duplicate dates are detected, compute per-record class masses first (`sedcon_i * runvol`) and sum masses; do not sum concentrations directly.

### Event handling

- Hill pass files label each day as `EVENT`, `SUBEVENT`, or `NO EVENT`.
- In WEPP-forest, hillslope pass records are written once per day for the last plane; duplicate dates are not expected.
- If duplicates exist for the same day (for example, concatenated files), sum flow volumes and sediment masses and warn.
- Write a continuous daily series from min to max `(year, julian)` in the file. Days without records get zeros.
- When `cli_calendar_path` is provided, use `sim_day_index` to order rows and fill gaps; retain PASS `year/julian` for logging/manifest only.

### Calendar handling (SWAT+ IYR/ISTEP)
- If `cli_calendar_path` is **not** provided, use PASS `year` and `julian` directly as `IYR` and `ISTEP`.
- If `cli_calendar_path` **is** provided, derive `IYR`/`ISTEP` from `sim_day_index` and the calendar length defined by the CLI calendar (non-Gregorian). In this mode, PASS `year/julian` are treated as informational only.
- SWAT+ accepts year values starting at 1 as long as `time.sim` and all time-series inputs (recall, climate) use the same year numbering.

## SWAT+ Recall File Format

The recall daily file is free-format, space-delimited, and follows the `recall_day.rec` example in `inputs_swatplus.pdf`:
- Line 1: Title (free text; required but ignored by SWAT+)
- Line 2: `NBYR` (number of years of recall data)
- Line 3: Header line with **exact** column names and order:
  - `IYR ISTEP flo sed orgn sedp no3 solp psol psor chla nh3 C no2 cbod dox bacp bacpl met1 met2 met3 san sil cla sag lag grv temp`
- Lines 4+: daily rows matching the header order.

Populate fields as follows:
- `flo` (m^3): computed daily flow volume.
- `sed` (metric tons): total sediment.
- `san/sil/cla/sag/lag/grv` (metric tons): sediment classes (`grv=0`).
- All nutrient, pesticide, bacteria, metals, and temperature fields are written as `0` until explicitly supported.

Reference: `wepppy/nodb/mods/swat/wepp-swat-spec.md` (Recall data files section, citing `inputs_swatplus.pdf`). The converted `swatplus/doc/inputs_swatplus.md` does not include the daily row schema.

## File Organization and Naming

- Input hillslope pass files:
  - `wepp_output_dir/H<wepp_id>.pass.dat`
- Output recall files:
  - `swat_txtinout_dir/<recall_subdir>/hill_{wepp_id:05d}.rec` (default)
- Output recall master/connect files:
  - `swat_txtinout_dir/recall.rec`
  - `swat_txtinout_dir/recall.con` (only when `recall_connections` is provided)

The filename template should be configurable to allow other naming schemes.

## Validation and QA

Per file:
- Check that `FLO >= 0` and `SED >= 0` for every day.
- Check that `SED` equals the sum of the class loads within a tolerance.
- Verify the number of output days equals the number of days between min and max dates.
- Log a summary with total water volume and sediment.

Cross-file:
- Only count files matching `H<wepp_id>.pass.dat`.
- Skip non-matching files silently.
- If a matching file is skipped (empty/invalid), include `status=skipped` + `skip_reason` in the manifest and warn.
- Always write `recall.rec` to `swat_txtinout_dir`.
- If `recall_connections` is provided, also write `recall.con`.
- Otherwise, provide a manifest so downstream code can build `recall.con`.

## Error Handling

- Non-matching filenames (not `H<digits>.pass.dat`) are skipped silently.
- Missing/invalid PASS headers: raise `InterchangeError`.
- Empty/short PASS files: skip writing recall output, warn, and record `status=skipped` in the manifest.
- Non-monotonic dates: accept but sort by date before emitting the daily series.
- I/O errors: bubble up as `InterchangeError` with file context.

## Performance Considerations

- Process files independently using a worker pool. Default `ncpu` to the number of logical cores; set `ncpu=1` for deterministic single-threaded runs.
- Avoid storing the entire time series as strings; build numeric buffers and format at write time.
- Use buffered I/O for writing recall files.

## Tests

- Unit test for filename parsing (`H0001.pass.dat` -> `wepp_id = 1`).
- Unit test that non-matching files are skipped.
- Unit test for a synthetic PASS file with 2 days to verify:
  - FLO composition
  - SED and class loads
  - Output day continuity
- Regression test using a small fixture in `tests/fixtures` with known totals.

## References

- SWAT+ recall input format (master + daily recall): `wepppy/nodb/mods/swat/wepp-swat-spec.md`
- SWAT+ IO PDF (recall.rec and recall_day.rec): `inputs_swatplus.pdf` (converted copy: `swatplus/doc/inputs_swatplus.md`)
- WEPP hillslope pass parser: `wepp_interchange/src/hill_pass.rs`

## Implementation Plan

### Phase 0: Preflight + alignment
- [x] Confirm SWAT+ recall daily schema (field order + units) in `inputs_swatplus.pdf` and document any deviations.
  - Findings: the daily row schema is summarized in `wepppy/nodb/mods/swat/wepp-swat-spec.md` (Recall data files section, citing `inputs_swatplus.pdf`). The converted `swatplus/doc/inputs_swatplus.md` lists `recall.rec`/`recall.con` in `file.cio` but does not include the daily recall row layout.
- [x] Review `wepp_interchange/src/hill_pass.rs` for the real signature (`hillslope_pass_to_columns(path, cli_calendar_path, &VersionInfo)`) and available columns:
  - `event`, `year`, `julian`, `sim_day_index`, `month`, `day_of_month`, `water_year`
  - `runvol`, `sbrunv`, `drrunv`, `sedcon_1..5`, plus `clot/slot/saot/laot/sdot`, `gwbfv/gwdsv`
- [x] Confirm error surface in `wepp_interchange/src/errors.rs` (`InterchangeError::Io`, `Parse`, `Calendar`, `Arrow`, `Parquet`) and plan to reuse `InterchangeError::io`/`InterchangeError::parse` for new failures.
- [x] Decide how to source `VersionInfo` in the new API: align with existing pyo3 exports and pass `version_major`/`version_minor` from Python to build `VersionInfo::new`.

### Phase 1: Rust module scaffold
- [x] Add `wepp_interchange/src/swat_utils.rs` with:
  - `struct RecallRow { year, julian, flo, sed, cla, sil, sag, lag, san, grv }` (types to match existing patterns).
  - `struct RecallManifestEntry { wepp_id, pass_file, recall_file, days_written, start_year, end_year, status, skip_reason }`.
  - Public `wepp_hillslope_pass_to_swat_recall(...) -> Result<Option<Vec<RecallManifestEntry>>, InterchangeError>`.
- [x] For output naming, re-implement the private `extract_wepp_id` rule (leading `H` + digits) so output naming stays consistent even when a PASS file returns zero rows.
- [x] Update `wepp_interchange/src/lib.rs` to export the new function (and add `mod swat_utils;`) using the same pyo3 signature conventions as other exports.
- [x] Use `InterchangeError::io` and `InterchangeError::parse` for new errors; avoid new variants unless needed.

### Phase 2: File discovery + parsing
- [x] Implement directory scan:
  - Filter for `H<digits>.pass.dat` names, extract `wepp_id`, collect `(wepp_id, path)` pairs.
  - Sort by `wepp_id` for deterministic output (even with parallel execution).
- [x] For each file:
  - Call `hill_pass::hillslope_pass_to_columns(path, cli_calendar_path, version)`.
  - If `PassColumns` returns no rows (short header or empty file), skip with a warning and record `status=skipped` + `skip_reason` in the manifest.
  - Normalize to `(year, julian)` keys and aggregate duplicates by summing flows and sediment masses (not concentrations), regardless of `event` label.
  - Fill missing days with zero rows from min to max date; use `sim_day_index` for ordering if a non-Gregorian calendar is used.

### Phase 3: Mapping + recall writing
- [x] Compute `FLO` from `runvol`, `sbrunv`, `drrunv` based on flags.
- [x] Convert `sedcon_1..5` to class masses using `runvol`; compute total `SED` and enforce `SED == sum(classes)` within tolerance.
- [x] Treat `event` as diagnostic only; recall output is daily aggregate.
- [x] Write recall file:
  - Title + header lines (consistent across files).
  - `NBYR = max_year - min_year + 1` derived from the daily series.
  - Daily rows with SWAT+ `recall_day.rec` column order and numeric formatting (consistent precision).
- [x] Return manifest entries when `write_manifest=True`.

### Phase 4: Parallel execution
- [x] Use a worker pool when `ncpu > 1`:
  - Process files independently and collect results in a stable order.
  - Surface per-file errors with file context; abort overall run on first error.

### Phase 5: Tests + fixtures
- [x] Unit tests in `wepp_interchange/src/swat_utils.rs` (Rust module tests):
  - Filename parsing (`H0001.pass.dat` -> `wepp_id=1`) and template formatting.
  - Synthetic PASS file with event/subevent, `FLO`, `SED`, and class loads.
  - Missing days fill, duplicate-date aggregation, and empty-header behavior.
  - Header + `NBYR` formatting and manifest fields.
  - Calendar/CLI lookup path using a synthetic parquet calendar.
  - Parallel execution path (`ncpu > 1`).
- [ ] Add a minimal fixture PASS file under `wepppyo3/tests/fixtures/` with known totals.
  - Use `/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output` as the current end-to-end fixture (67 hillslopes).

### Phase 6: Python binding + ergonomics
- [ ] Expose as `#[pyfunction]` and add to the Python module initializer.
- [ ] Provide Python-side docstring + type hints mirroring the chosen signature (including version args if aligned with other exports).
- [ ] If a Python wrapper exists, add a thin convenience helper and keep defaults aligned.

### Phase 7: QA + validation hooks
- [ ] Add per-file summary logging (total FLO/SED, days, date span).
- [ ] Add a small validation helper to compare totals against WEPP or SWAT+ expectations if available.
- [ ] Document limitations (no `recall.rec`/`recall.con` yet) and next steps.

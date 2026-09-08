# Native AshPost contract

Status: frozen for implementation under operator-confirmed simple scope.
Baseline: WEPPpy abebd09239f398af7627924c4c000ff3229a01ee. Source snapshots and
fresh oracles are in target/ashpost-evidence. No new dependencies are required.

## Interface

`ashpost_to_parquet(input_root, output_dir, manifest, recurrence,
                   hydrology_path=None, wat_path=None, ash_wepp_ids=None,
                   field_metadata=None)`

Inputs are strings, an ordered list of `(relative_path, topaz_id: int,
area_ha: float, burn_class: int)` tuples, integer recurrence intervals, optional
hydrology path strings relative to input_root, a list of integer WEPP IDs,
and optional `{column_name: {metadata_key: metadata_value}}` string dictionaries.
Python supplies existing units/descriptions from its canonical dictionaries;
Rust adds unchanged AshPost 1.0 schema metadata. All output fields remain nullable.

Return a dictionary with `input_rows: int`, `rows_written: {filename: int}`,
`return_periods`, `cum_return_periods`, and `burn_class_return_periods` in their
existing nested dictionary forms. Empty manifest returns zero input rows, empty
row counts, and None for the three mappings; it writes no new outputs. Optional `streamflow_exceedance` contains the existing
warning count, maximum overage, and at most five samples.

Require year0, year, julian, days_from_fire, wind/water/ash transport per hectare,
and cumulative wind/water/ash transport per hectare. Retain optional
transportable ash, ash depth/runoff, and all cumulative unit-bearing fields.
Require Float64 model metrics, matching both existing producers; accept numeric
keys and hydrology fields. Reject incompatible field types, mixed model
schemas, duplicate paths/IDs, malformed parquet, invalid numeric keys/metadata,
and missing required columns. Nonfinite metadata/keys and infinite metrics are
invalid; null/NaN model metrics follow skip-null sum semantics.
H.wat SQL SUM ignores Arrow NULL operands; valid IEEE NaN propagates. Zero area is valid.
Topaz IDs fit uint16 and are positive; burn classes 0..4 cover the observed
0..3 producer and existing class-description range. Present empty files are
invalid, distinct from absent files omitted by Python discovery.

## Computation

Read projected batches of at most 8192 rows and retain only one hillslope's
calendar-date aggregates plus output-key state. Preserve first-non-null metadata,
float32-rounded hectares and float32 grouped area sums. First-year filtering is
days_from_fire <= 365. Hillslope annuals average calendar-year sums; watershed
annual area sums hillslope-day areas. Cumulative selection uses year0 then
julian, choosing the last calendar-date row on a tie. Preserve unit conversion
order and the current sequential NumPy quicksort tie behavior for return periods.

Hydrology consumes first duplicate year/julian rows from totalwatsed3 and all
H.wat records for the independent ash-type-selected watershed IDs. Missing/empty optional
hydrology produces existing four zero correction columns; unmatched hydrology
preserves null results. Existing required hydrology fields are year/julian,
Streamflow, Runoff, Lateral Flow, Baseflow, Area; absent sediment fields are zero.
WEPPpy retains model/discovery policy, NoDb persistence, versioning, docs/catalog,
and the later RQ totalwatsed3 rebuild.

## Writing and errors

Use existing ParquetSink individually for each complete output. No multi-file
transaction, staging barrier, rollback/recovery system, migration, or NFS-specific
mechanism. Existing regular-file modes are preserved through the existing writer
option. Reject output aliases and escaping input paths. Value/schema errors are
ValueError; ordinary filesystem errors retain appropriate OSError subclasses.
Use checked conversions and normal PyO3 error propagation, never panic.

## Scheduler

Prepare scalar metadata, build large per-hill work items lazily in the parent,
and keep at most 2 * max_workers futures pending. Release completed futures and
inputs promptly. Finish and shut down the executor, close the iterator, and
release shared climate input before invoking native AshPost. Preserve serial
execution, progress totals, model values, cancellation, and ordinary exceptions.

Metric-width decision: both frozen model fixtures and producer array creation use
Float64 metrics. Float32 metrics imply different legacy arithmetic and output
schemas; reject that unsupported variant explicitly instead of silently changing
its values or adding a second arithmetic implementation.

Sorting attribution: the scalar indirect-sort port follows NumPy 1.26. Its BSD
notice is retained in `wepp_interchange/NUMPY-LICENSE.txt` and the deployable
interchange package. No new runtime dependency is introduced.

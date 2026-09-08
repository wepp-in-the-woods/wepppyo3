# Native totalwatsed3 contract and compatibility plan

## Static:

confirmed: Authority is the unchanged current Python producer at WEPPpy revision
3c51780f50e2599ef72b03214c35bf20538e55c4 and the user-approved single-OFE
oracle recorded in package.md, Approved single-OFE oracle authority.
This is a faithful extraction. No formulas, constants, public schema, units,
NoDb ownership, or RQ dependency edges change.

Native API: `totalwatsed3_to_parquet(pass_path, wat_path, output_path,
gwstorage, bfcoeff, dscoeff, version_major, version_minor, soil_path=None,
element_path=None, wepp_ids=None, ash_inputs=None, pandas_metadata=None)`.
Paths are strings; coefficients are float64; identifiers are integer sequences.
Each ash input is `(path, area_ha, ash_type, density_kg_m3)`, with optional type
and density. WEPPpy resolves run paths, available ash files, NoDb metadata,
subset IDs, and scalar options; Rust performs every table transform. A compact
summary reports rows, row groups, output paths, and elapsed milliseconds.
The GIL is released during native I/O/computation. Missing native API fails
through the existing required-native boundary, without runtime fallback.

`interchange_dir` maps to the four H.* paths and totalwatsed3 output path.
Missing PASS/WAT fails before publication. Optional missing soil/element are
represented by None. `baseflow_opts.bfthreshold` is not consumed by the existing
producer. `wepp_ids=None` includes all, an empty list includes none, and repeated
IDs are deduplicated. `ash_dir` keeps current directory discovery precedence;
`ash_area_lookup` skips Watershed area lookup when nonempty. Types/densities
continue to resolve through WEPPpy. Legacy `day` maps to sim_day_index;
`ofe_id` precedes `OFE`; no OFE column disables the optional area joins.

Full aggregation key: year, sim_day_index, julian, month, day_of_month,
water_year. WAT defines output dates. PASS joins left on that full key and
missing PASS metrics become zero. WAT sorts by year, julian, sim_day_index
before recurrence. Sum Area and weighted volumes in float64. Depth terms use
(volume / Area) * 1000 for Area > 0, otherwise zero. Sum all WAT OFE areas,
but lateral flow uses only maximum OFE per hillslope across the entire input.
Soil TSMF joins on hillslope/OFE/year/simulation day and uses nonnull-weighted
area. Element QRain/QSnow join on hillslope/OFE and all calendar fields (no
simulation day in source), carrying WAT simulation day into the result.
Duplicate matches retain SQL multiplicity. Absent/all-null optional storage
columns and missing optional joins yield null; Interception yields zero.

PASS class mass is SUM(sedcon_i * runvol). Sediment delivery is class mass
sum. Solids volume uses existing densities (2600, 2650, 1800, 1600, 2650)
kg/m3. Concentrations are zero when runoff volume is not positive. Runoff is
PASS runvol depth, not WAT Q depth. Recurrence seeds reservoir[0]=gwstorage,
baseflow[0]=0; losses[i-1]=reservoir[i-1]*dscoeff;
reservoir[i]=reservoir[i-1]-baseflow[i-1]+percolation[i]-losses[i-1];
baseflow[i]=reservoir[i]*bfcoeff. Last losses remains zero.

Ash reproduces ashpost.read_hillslope_out_fn: project available out_cols,
collapse duplicate year/julian rows by sum for tonne/ha metrics and first
nonnull for year0/days_from_fire, multiply summed tonne/ha by area_ha, then
apply year0==year or (without year0) days_from_fire<=365. Derive Gregorian
month/day from year/julian. Skip files without all four transport metrics,
or without positive mapped area, as the existing producer does. Group by
calendar year/julian/month/day, not simulation index or water year. Type-specific
masses and per-ha denominators include only matching types; unknown type
still contributes total mass. Volume uses the caller-resolved density, which
preserves existing WEPPpy metadata/default behavior without altering units.

Output field order, Arrow types, nullability, units/descriptions and version
metadata match all 79 current fields. Empty output has no pandas metadata,
matching EMPTY_TABLE. For nonempty output pandas_metadata is an opaque legacy
schema metadata string supplied by the facade; native code does not load pandas.
This preserves the historical metadata contract while table computation moves
fully into Rust. Standalone callers can provide the oracle metadata explicitly.

Bounded design: compact physical chunk locations and row-group-to-hillslope
indexes grow with file groups. Retain only consumed source columns, preserve
Arrow types and physical chunk offsets/codecs, and reconstruct at most sixteen
row groups of reader metadata per scan. Full column statistics are used only
to identify singleton hillslopes; mixed groups retain their ID-only scan. Numeric record batches have a fixed 8192-row limit and project
only used columns. Global aggregations grow with distinct simulation dates.
Area joins and outlet-flow state are held for one hillslope at a time and
released between hillslopes. Row groups with mixed IDs are indexed by an ID-only
scan; no ordering assumption changes join semantics. Memory does not retain
whole-run numeric rows or Python frames. Use the existing Arrow/Parquet/PyO3
crates and ParquetSink staging/cleanup. No external dependency is added.

Compatibility/regression plan: establish exact schema/null/date and tolerance
parity for approved single-OFE, HPC MOFE/performance, subsets, legacy names,
missing/present/null optional columns, duplicate join keys, empty input, ash,
and failure-atomic cleanup. Run native unit and installed release API tests,
then the package timing/memory gates before removing Python production logic.
Validate generated outputs through all named downstream consumers and the real
12 GiB Compose worker workflow before publication. Retain original oracles.

Hardening precedent: reuse the required-native boundary from WEPPpy's
20260715_wepppyo3_only_interchange package and current ParquetSink atomic writer;
retain the PASS-runoff correction and optional storage contracts from the
20260429 runoff-reconciliation/storage packages. Hypothesis: streaming projected
native scans bound memory while retaining numerical parity and runtime gates.
No temporary mitigation/fallback is introduced. The package's measured resource
and workflow gates are the health signals; parity and wall-time are guardrails.

## Ran:

confirmed: Both HPC Python baselines pass; the approved current-producer
single-OFE capture is preserved under Git LFS. Native implementation, parity,
resource and downstream evidence are pending. This preimplementation contract
is not a claim of completed implementation or independent review.

## Parallel scan refinement

The serial prototype passed all three oracle comparisons but was too slow
(18.16 seconds on throbbing-sylvan). Before formal acceptance, divide row-group
scans across at most twelve native threads, also bounded by available CPU
parallelism. Each thread holds only date aggregates and one hillslope's joins.
Merge partial sums in fixed worker order. This uses the existing worker CPU
budget and std threads; no numerical parameter or dependency changes. Peak
state is bounded by concurrency times duration, plus Parquet group metadata.

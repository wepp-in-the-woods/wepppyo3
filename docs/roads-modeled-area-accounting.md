# Roads modeled-area accounting

The additive native API implements WEPPpy's
`docs/schemas/roads-water-accounting-contract.md`, checkpoint `83f7410e8`.
WEPPpy owns source selection, the versioned area ledger, routing and report
readiness. Native code owns geometry validation, weighted PASS serialization and
source-qualified water reduction. No geographic overlap inputs are required.

## API

- `roads_pass_geometry(path)` returns serialized PASS area and climate token.
- `roads_wat_geometry(path, ids)` returns each requested native ID's stationary
  local OFE areas; invalid topology, calendar, area or terminal water fails.
- `combine_weighted_hillslope_pass_files(..., strategy="roads_modeled_area_v1")`
  weights extensive quantities by represented/PASS-header area and reparses its
  output. Roads permits differing particle diameters with the existing first
  source header interpretation; AgFields remains strict.
- `roads_water_to_parquet(pass_path, sources, output_path, accounting_path,
  gwstorage, bfcoeff, dscoeff, version_major, version_minor, pandas_metadata=None)`
  returns rows written. Each source tuple is `(wat_path, native_id, target_id,
  represented_area_m2, soil_path_or_None, element_path_or_None)`.

The water producer selects each canonical target PASS once. Original weighted
profiles supply TotalWatSed's existing literal all-OFE Q/QOFE diagnostics and
optional support-area denominators. A separate marked accounting WAT supplies
physical terminal Q and terminal lateral delivery to average hillslope reports.
It stages both products and publishes through the existing coordinated Parquet
commit mechanism. Source profiles and baseline entry points are unchanged.

Memory retains watershed daily sums, one target's daily reporting sums and one
native profile's joins, rather than a whole-run target/day table. Malformed inputs,
nonfinite required outlets, arithmetic overflow and destination aliases fail
before publishing the pair. Optional local NULL/NaN rules remain inherited from
the existing reducer. Full contributor footprints are retained; WEPPpy assigns
baseline remainder `max(A-C, 0)` or existing replacement profiles.

## Validation and provenance

`tests/wepp_interchange/test_roads_water.py` covers weighted topology, separate
Q meanings, baseline-equivalent zero-road reduction, invalid inputs and output
aliases. Weighted PASS Rust tests cover C<A, C=A, C>A and unchanged AgFields
header admission. Existing TotalWatSed frozen-oracle tests protect its API.
Release receipt: `roads-accounting-release-20261022.json`. Native release refresh
is coordinated with WEPPpy's service preflight pin; no production deployment is
included in this work.

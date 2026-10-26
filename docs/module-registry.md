# wepppyo3 Module Registry

This registry is the canonical map of `wepppyo3` modules, deployable artifacts,
WEPPpy callsites, tests, and evidence status. It is descriptive, not a support
policy: maturity labels reflect observed usage and local evidence, not a promise
that every module has the same operational guarantees.

## Claim Labels

`confirmed` (2026-10-09): wepp_interchange reads legacy and PASS component v3
files and preserves v3 operands through Roads additive and AgFields area-weighted
composition. Public table schemas are unchanged; mixed-version composition is
rejected. See [the composition contract](pass-components-v3.md) and
[release identity](release-provenance.md#pass-components-v3-and-composition-2026-10-09).

- `confirmed`: directly observed in source, release files, tests, or checked-in work-package artifacts.
- `inference`: conclusion drawn from confirmed evidence with stated assumptions.
- `hypothesis`: plausible future benefit or unmeasured claim.

## Maturity Labels

- `production-critical`: WEPPpy production workflows rely on the module for a native contract, with no intended transparent Python fallback for the main path.
- `production-used`: WEPPpy imports the module in production domains, but the path may be narrower or workflow-specific.
- `mixed support`: WEPPpy uses the module, while some callsites intentionally retain explicit fallback behavior.
- `internal support`: Rust crate used by other crates; not a deployable PyO3 package today.
- `incubating`: present but not yet backed by broad callsite or release evidence.

## Registry

| Module or crate | Maturity | Release artifact | WEPPpy callsites observed | Tests observed | Evidence notes |
| --- | --- | --- | --- | --- | --- |
| `wepppyo3.climate` (`cli_revision_rust`) | production-critical | `release/linux/py312/wepppyo3/climate/cli_revision_rust.so` | Climate build/scaling helpers, Daymet/GridMET interpolation, RHEM storm generation, Geneva collaborators | `tests/climate/test_*.py`; Rust crate tests in `cli_revision` | `confirmed`: broad WEPPpy climate imports and release artifact. `inference`: core climate kernel. |
| `wepppyo3.raster_characteristics` | production-critical | `release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so` | Landuse, soils, disturbed, treatments, ash transport, RAP, Omni raster classification paths; generic bounded categorical support extraction; research-only SSURGO clustered-window candidate discovery and per-source geometry evidence | `tests/raster_characteristics/test_*.py` (including deterministic-order contract coverage); WEPPpy nodata guard tests; crate unit tests | `confirmed`: active WEPPpy raster callsites and package history for MOFE landuse pair counts. `confirmed`: deterministic key-order contract for all public map-returning APIs with repeated-call parity coverage (2026-05-13 package). `confirmed`: `categorical_support_within_bounds` reads one bounded categorical raster window and returns ordered category/pixel-support pairs. `confirmed`: `local_mukey_geometry` separately returns local support and four-neighbor shared-edge counts per source MUKEY. `hypothesis`: `local_mukey_candidates` and `local_mukey_geometry` remain Phase 2/M3.5 research kernels; they do not select production SSURGO fallbacks. |
| `wepppyo3.wepp_interchange` | production-critical | `release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so` | `wepppy/wepp/interchange/_rust_interchange.py`, watershed/hillslope output converters, catalog scan, slope segmentation, Roads pass combination, AgFields weighted PASS accounting and sub-field interchange | `tests/wepp_interchange/test_*.py`; weighted and explicit-breakpoint Rust identities; non-empty UTF-8 Arrow writer regression; ordered direct hillslope and dedicated AgFields Parquet regressions; watershed TC_OUT regression; Python release-API tests; WEPPpy interchange tests | `confirmed`: native writers preserve public schemas and use Parquet dictionary encoding. `confirmed`: ordered hillslope PASS, EBE, ELEMENT, LOSS, SOIL, and WAT APIs write directly from source-ordered compact Rust arrays without returning full tables through Python; each source forms one Parquet row group. `confirmed`: hillslope SOIL fixed-width recovery accepts both historical `i2` and current `i5` OFE layouts, including adjacent numeric fields and optional `TSMF`. `confirmed`: annual watershed LOSS accepts uniform historical 11-field and current 12-field hillslope layouts, emits a nullable legacy area or current hectare value in schema version 2, and rejects mixed/other widths with Rust and WEPPpy regressions. `confirmed`: watershed SOIL accepts widened numeric OFE fields and strictly reconstructs historical `**` identifiers only from contiguous, stable daily layouts. `confirmed`: six isolated AgFields sibling APIs accept coupled path/field/sub-field descriptors, validate filename identity, and emit real `field_id`/`sub_field_id` columns without changing ordinary writer contracts. `confirmed`: watershed EBE includes native outlet inference and channel-peak regression auditing, while TC_OUT performs native outlet selection. `confirmed`: additive weighted API preserves the Roads call surface and returns serialized closure diagnostics. `confirmed`: explicit-breakpoint slope segmentation preserves the existing automatic segmenter while accepting validated field-aware OFE boundaries and an optional target width. |
| `wepppyo3.watershed_abstraction` | production-critical helper | `release/linux/py312/wepppyo3/watershed_abstraction/watershed_abstraction_rust.so` | MOFE map assignment in WEPPpy watershed abstraction helpers | `tests/watershed_abstraction/test_assign_mofe_map.py` | `confirmed`: MOFE map migration package moved production behavior here. `inference`: helper kernel namespace, not Peridot replacement. |
| `wepppyo3.swat_interchange` | production-used | `release/linux/py312/wepppyo3/swat_interchange/swat_interchange_rust.so` | SWAT+ output-to-Parquet integration in SWAT mod workflows | Rust crate tests and spec coverage; no broad Python fixture inventory observed in this pass | `confirmed`: release artifact and spec. `inference`: production-used in SWAT workflows. |
| `wepppyo3.swat_utils` | production-used | `release/linux/py312/wepppyo3/swat_utils/swat_utils_rust.so` | WEPP hillslope pass to SWAT+ recall generation | Spec coverage; Rust source tests not exhaustively audited in this pass | `confirmed`: release artifact and recall spec. `inference`: bridge utility substrate. |
| `wepppyo3.roads_flowpath` | production-used helper | `release/linux/py312/wepppyo3/roads_flowpath/roads_flowpath_rust.so` | Roads downslope tracing workflows | `tests/roads_flowpath/test_trace_downslope_flowpath.py` | `confirmed`: release artifact and test. `confirmed`: depends on local Peridot crate path. |
| `wepppyo3.sbs_map` | mixed support | `release/linux/py312/wepppyo3/sbs_map/sbs_map_rust.so` | BAER/SBS raster processing and tests | SBS Rust/Python tests not exhaustively inventoried in this pass; fallback allowlist exists in WEPPpy | `confirmed`: release artifact and WEPPpy fallback references. `inference`: mixed support until fallback contract is revisited. |
| `wepppyo3.wepp_viz` | production-used | `release/linux/py312/wepppyo3/wepp_viz/wepp_viz_rust.so` | WEPP soil-loss grid visualization helpers | Rust source present; Python tests not observed in this pass | `confirmed`: release artifact and WEPPpy imports. `hypothesis`: needs targeted fixture coverage before stronger claims. |
| `geneva_core` | internal support | Not deployed as PyO3 module | Used by `cli_revision`/Geneva-related climate work | Rust tests possible through dependent crates | `confirmed`: workspace member and Rust-only library. |
| `raster` | internal support | Not deployed as PyO3 module | Shared GDAL/PROJ foundation for raster crates | Indirect through raster-dependent crate tests | `confirmed`: workspace member and Rust-only library. |

## Existing Performance Evidence

`confirmed`: WEPPpy package histories record measured improvements for several
`wepppyo3` migrations, including MOFE map assignment, MOFE landuse pair counts,
segmented MOFE slope generation, and static-R/hyetograph helpers.

`inference`: These histories justify positioning `wepppyo3` as a native substrate
for selected contracts, but they do not justify a universal speedup claim for all
modules or all workloads.

`hypothesis`: A future benchmark index that links each module to fixture, command,
repetition count, hardware, and output parity evidence would make module-level
claims easier to publish and maintain.

## Registry Maintenance Rules

- Add a row when a new PyO3 module or internal support crate is added.
- Keep release artifact paths exact and relative to repository root.
- Use evidence labels for every performance or adoption claim.
- Do not promote a module to `production-critical` solely because it has a shared object; require WEPPpy callsite evidence and a clear contract.
- Record fallback behavior explicitly. Fallback can be intentional, but it must not be described as the same posture as a required production path.

## Native totalwatsed3 release and WEPPpy integration

`confirmed`: `totalwatsed3_to_parquet` is exported by the py312 release and
required by WEPPpy. The facade preserves paths, options, metadata and output
schema while Rust owns table computation and atomic publication. Installed
release tests pass 89 cases; the public facade matches 25 frozen oracles.
The final controlled host-prewarm 5,860/586 peak ratio is 1.43983; a mixed-cache
2.07079 observation is retained separately. The final real Forest Compose RQ
workflow, including bounded README previews and a subsequent job, peaks at
861.34 MiB. Independent correctness, QA and security reviews pass. Repository
and immutable image publication are verified, including image-contained RQ at
1.11 GiB. No production rollout is claimed. See the [disposition](work-packages/20260907-totalwatsed3-native-producer-001/artifacts/final-disposition.md).

## Native hillslope water-balance summary

`confirmed`: `wepppyo3.wepp_interchange` exports bounded distinct-ID discovery
and a compact Topaz/year cache writer, consumed by WEPPpy's
`wepppy/wepp/reports/hillslope_watbal.py`. It keeps first area per WEPP/OFE,
compensated flux sums, nullable output types, and sorted keys. ET components
P and Dp use daily local OFE area weighting before annual summation, including
native IDs mapped to one Topaz ID. Single-OFE hills retain bounded annual state;
exact integer row-group bounds can avoid the preliminary geometry scan.
Other row-group layouts use a projected key scan. Summaries carry
`hillslope_watbal_aggregation=mofe-water-v2`; invalid areas fail explicitly.
The required-native boundary has no full-source pandas fallback.

Evidence: [package tracker](work-packages/20260908-hillslope-watbal-native-001/tracker.md),
[standalone timings](work-packages/20260908-hillslope-watbal-native-001/artifacts/benchmark-summary.json),
and `tests/wepp_interchange/test_hillslope_watbal.py`. These are measured
fixture-specific results, not a general speedup claim.

## Native AshPost integration (2026-09-08)
`confirmed`: `wepppyo3.wepp_interchange.ashpost_to_parquet` streams existing
Float64 model outputs into the five AshPost 1.0 datasets and compact recurrence
mappings. WEPPpy owns manifest discovery, bounded model scheduling, NoDb,
documentation and catalog updates. Individual files use the existing ParquetSink.

Both small model fixtures and OR-202 match frozen Python tables and mappings;
small-model facades and generated documentation also match. Five warm runs per
mode/model measured about 82% lower incremental memory and 83% lower runtime;
see the [benchmark summary](work-packages/20260908-ashpost-native-001/artifacts/small-benchmark-summary.json).
The real 12-worker RQ sequence completed OR-202 then canine-liar at 4.43 GiB
peak, with zero OOM or restarts. These are fixture-specific observations.
The [tracker](work-packages/20260908-ashpost-native-001/tracker.md) and
[release manifest](work-packages/20260908-ashpost-native-001/artifacts/release-integration-manifest.json)
record final validation and source/binary provenance. No deployment is claimed.

Raster characteristics area-mean contract: [area-weighted raster mean](area-weighted-raster-mean.md). Implemented in the canonical py312 release; real-raster coverage is in tests/raster_characteristics/test_area_weighted_mean.py.

## Soil anisotropy input preparation (SA-01)

`confirmed`: `wepppyo3.wepp_interchange.soil_anisotropy_values` accepts nested
(depth_mm, original_optional_ratio) records and optional upper/lower overrides.
It validates finite bounds1–100/1–10 and positive nondecreasing depths, then
returns ratios using cumulative bottom depth <=500 mm. Unset classes preserve
source values, including zero. It opens no files and does not mutate arguments.
WEPPpy owns eligibility, parsing and the single prepared-file write. Native
unit tests cover boundary depths, duplicate clipped depths, source preservation
and invalid inputs; forest generated-file validation is retained in WEPPpy
package20261012_soil_anisotropy. No new performance claim is made.

## PRISM temperature ordering (2026-10-13)

`confirmed`: the climate daily revision writer rejects invalid source temperatures
and nonfinite temperature operands, then collapses only crossed adjusted extrema
to their midpoint before the existing serializer. Valid values and other weather
retain the existing transformation. A visible sibling `.temperature-ordering.log`
records working/completed status, correction count, maximum inversion, and actual
maximum per-extreme movement. Unrepresentable diagnostic magnitudes fail with
InvalidData (the Python wrapper retains OSError). Public signatures are unchanged.
WEPPpy ADR-0092 and `docs/schemas/prism-spatialization-temperature-contract.md`
own the numerical contract; its implementation package retains consumer evidence.

## Physical MOFE water consumers (2026-10-20)

`confirmed`: terminal native WAT Q is physical hillslope runoff; lateral volume
uses terminal local area over the full native footprint. Native selection precedes
feature mapping; missing terminals and duplicate native daily OFEs fail atomically.
AshPost accepts additive `pass_path` for the PASS-based baseline subtraction;
legacy `wat_path` is retained as a call argument but never used as a fallback.
Full PASS totals reconcile to the supplied baseline; unavailable totals/baselines
cannot publish numeric corrections. See [release manifest](mofe-water-release-20261020.json)
and WEPPpy's current hillslope-water aggregation contract for consumer semantics.

## Water output documentation metadata (2026-10-20)

Confirmed: WAT/PASS descriptive metadata now distinguishes local OFE area,
terminal physical runoff and threshold-filtered PASS delivery. Existing machine
unit labels remain unchanged; descriptions flag inherited sediment-fraction
label defects. No numerical behavior or API changes. Rebuilt py312 interchange
SHA-256 `1cb2cec6ecba762362d117982d448b397c900b9c3a1be742a0c3bbea16e3e62b`; 163 native Python tests passed.
See [release evidence](water-documentation-release-20261020.json). No production deployment.

## Roads modeled-area accounting (2026-10-22)

`confirmed`: additive Roads geometry, weighted PASS and source-qualified water
APIs retain full contributor MOFEs and publish a separate marked accounting WAT.
Existing TotalWatSed and AgFields entry points preserve their contracts.
See [API and evidence](roads-modeled-area-accounting.md).

## Roads Whitebox direction input (2026-10-26)

The existing Roads tracer gains explicit keyword-only `wbt=False`; WEPPpy Roads
passes true. Native conversion reuses Peridot and preserves default TOPAZ API
behavior. See [input contract](roads-flow-direction.md) and release provenance.

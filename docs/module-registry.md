# wepppyo3 Module Registry

This registry is the canonical map of `wepppyo3` modules, deployable artifacts,
WEPPpy callsites, tests, and evidence status. It is descriptive, not a support
policy: maturity labels reflect observed usage and local evidence, not a promise
that every module has the same operational guarantees.

## Claim Labels

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
| `wepppyo3.raster_characteristics` | production-critical | `release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so` | Landuse, soils, disturbed, treatments, ash transport, RAP, Omni raster classification paths | `tests/raster_characteristics/test_*.py`; WEPPpy nodata guard tests | `confirmed`: active WEPPpy raster callsites and package history for MOFE landuse pair counts. |
| `wepppyo3.wepp_interchange` | production-critical | `release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so` | `wepppy/wepp/interchange/_rust_interchange.py`, watershed/hillslope output converters, catalog scan, slope segmentation, roads pass combination | `tests/wepp_interchange/test_*.py`; WEPPpy interchange tests | `confirmed`: default Rust paths with explicit fallback logging in WEPPpy interchange docs. |
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

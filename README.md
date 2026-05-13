# wepppyo3

`wepppyo3` is WEPPpy's native kernel and interchange substrate.
It is the Python-callable Rust layer for contract-sensitive climate, raster,
WEPP/SWAT interchange, roads, MOFE, SBS, and visualization workloads where
Python should keep orchestrating runs but the hot path belongs in owned Rust.

## Why This Matters

WEPPpy coordinates long-running watershed modeling through Python controllers,
RQ workers, run directories, and reports. Some parts of that workflow are too
large, parser-sensitive, or raster-heavy to leave as ad hoc Python loops. This
repo gives those paths a shared Rust substrate while preserving WEPPpy's Python
APIs and operational model.

The important shift is not "Rust is faster than Python" in the abstract. The
shift is that selected WEPPpy contracts now have native implementations with
bounded responsibilities, deployable shared objects, tests, and explicit release
provenance expectations.

## Positioning

| Legacy framing | Current posture |
| --- | --- |
| Rust/PyO3 extension modules for WEPPpy | WEPPpy native kernel and interchange substrate |
| Optional accelerators where available | Owned native implementations for selected production contracts |
| Flat list of functions | Module registry with maturity, callsites, release artifacts, tests, and evidence |
| Per-module speedup anecdotes | Claim discipline using `confirmed`, `inference`, and `hypothesis` |
| Manual copy instructions only | Release provenance contract with known gaps and follow-up path |

Clean claim statement:

> `wepppyo3` is WEPPpy's native kernel and interchange substrate: Python-callable
> Rust modules for contract-sensitive hydrology, climate, raster, WEPP/SWAT
> interchange, roads, MOFE, SBS, and visualization workloads where Python
> orchestration should remain but the hot path belongs in owned Rust.

## Scope and Boundaries

`wepppyo3` owns narrow native kernels and file interchanges embedded in WEPPpy
workflows. It does not own WEPPpy orchestration.

| Area | Owner | Boundary |
| --- | --- | --- |
| Routes, sessions, NoDb state, RQ jobs, run directories, reports | WEPPpy | Python remains the application and orchestration layer. |
| Climate parsing/scaling, raster scans, WEPP/SWAT interchanges, MOFE helpers, SBS helpers, visualization grids | `wepppyo3` | Python-callable Rust kernels with stable wrapper contracts. |
| Watershed graph abstraction | Peridot | Standalone explicit-graph abstraction engine and CLI. |
| WBT/TOPAZ-style delineation and hydrology tools | `weppcloud-wbt` | WhiteboxTools fork and command-line toolchain. |
| WEPP and SWAT model execution | Model binaries plus WEPPpy wrappers | `wepppyo3` parses and transforms selected outputs; it does not replace the models. |

Boundary rule: new Rust code belongs in `wepppyo3` when it is a Python-callable
kernel or interchange needed inside WEPPpy. Standalone watershed graph work
belongs in Peridot. WBT/delineation tools belong in `weppcloud-wbt`.

## Canonical Docs

- [Module registry](docs/module-registry.md): module purpose, maturity, release artifact, WEPPpy callsites, tests, and evidence labels.
- [Architecture and boundaries](docs/architecture-and-boundaries.md): what belongs in `wepppyo3` versus WEPPpy, Peridot, and `weppcloud-wbt`.
- [Release provenance](docs/release-provenance.md): canonical `release/linux/py312/wepppyo3/` layout, manual refresh flow, and provenance gaps.
- [Claim discipline](docs/claim-discipline.md): approved claim labels, communication kit, figure specification, and metrics definitions.
- [SWAT+ interchange spec](docs/swat-interchange-spec.md): SWAT output-to-Parquet contract.
- [WEPP hillslope pass to SWAT recall spec](docs/wepp-hill-pass-to-swat-rec-spec.md): SWAT recall conversion contract.

## Module Summary

| Python module | Posture | Primary contract |
| --- | --- | --- |
| `wepppyo3.climate` | Production-critical | CLIGEN parsing/scaling, geospatial interpolation, hyetograph and static-R helpers. |
| `wepppyo3.raster_characteristics` | Production-critical | Raster key/mode/median/pair-count scans used by landuse, soils, disturbed, RAP, Omni, and related flows. |
| `wepppyo3.wepp_interchange` | Production-critical | WEPP text outputs to Parquet, hillslope output parsers, pass combining, catalog scan, and slope segmentation. |
| `wepppyo3.watershed_abstraction` | Production-critical helper | MOFE map assignment helper; not a Peridot replacement. |
| `wepppyo3.swat_interchange` | Production-used | SWAT+ output directory/file conversion to Parquet. |
| `wepppyo3.swat_utils` | Production-used | WEPP hillslope pass to SWAT+ recall conversion. |
| `wepppyo3.roads_flowpath` | Production-used helper | Python-callable roads downslope tracing backed by Peridot logic. |
| `wepppyo3.sbs_map` | Mixed support | SBS/BAER raster helpers; some WEPPpy paths retain explicit Python fallback behavior. |
| `wepppyo3.wepp_viz` | Production-used | Soil-loss grid construction helpers for WEPP visualization. |
| `geneva_core` | Internal support | Rust-only hydrology core used by climate/Geneva paths. |
| `raster` | Internal support | Shared GDAL/PROJ raster foundation for raster-related modules. |

See [docs/module-registry.md](docs/module-registry.md) for evidence and exact
release artifact paths.

## API Surface

### `wepppyo3.climate`

- `calculate_p_annual_monthlies(...)`
- `calculate_monthlies(src_fn)`
- `build_hyetograph_non_breakpoint(...)`
- `build_hyetograph_breakpoint(...)`
- `cli_revision(...)`
- `compute_peak_intensities_from_hyetograph(...)`
- `compute_peak_intensities_non_breakpoint(...)`
- `compute_peak_intensities_breakpoint(...)`
- `compute_static_r_from_cli(...)`
- `interpolate_geospatial(...)`
- `make_rhem_storm_file(src_fn, dst_fn)`
- `rust_cli_p_scale_monthlies(src_fn, dst_fn, p_mults)`
- `rust_cli_p_scale(src_fn, dst_fn, p_mult)`
- `rust_cli_p_scale_annual_monthlies(src_fn, dst_fn, p_mults)`
- `rust_cli_calculate_p_annual_monthlies(src_fn)`
- `rust_cli_calculate_p_annual_monthlies_from_lists(months, precips)`
- `rust_cli_calculate_monthlies(src_fn)`

### `wepppyo3.raster_characteristics`

- `count_intersecting_raster_key_pairs(...)`
- `identify_mode_single_raster_key(...)`
- `identify_mode_intersecting_raster_keys(...)`
- `identify_median_single_raster_key(...)`
- `identify_median_intersecting_raster_keys(...)`

Deterministic-order contract:

- All public map-returning raster-characteristics APIs return deterministic key order for identical inputs.
- Nested maps (`key -> key2 -> value`) are deterministic at both levels.
- Value and error semantics are unchanged by this ordering hardening.

### `wepppyo3.sbs_map`

- `summarize_sbs_raster(path, *, color_map_path=None)`
- `read_color_table(path, *, color_map_path=None)`
- `unique_values(path)`
- `summarize_color_table(path, *, color_map_path=None)`
- `reclassify_sbs_raster(path, *, breaks=None, ct=None, nodata=None, offset=0, color_map_path=None)`
- `export_sbs_4class(path, dst_path, *, breaks=None, ct=None, nodata=None, color_map_path=None)`

### `wepppyo3.wepp_interchange`

- `watershed_pass_to_parquet(...)`
- `watershed_soil_to_parquet(...)`
- `watershed_loss_to_parquet(...)`
- `watershed_chan_peak_to_parquet(...)`
- `watershed_ebe_to_parquet(...)`
- `watershed_chanwb_to_parquet(...)`
- `watershed_chnwb_to_parquet(...)`
- `hillslope_pass_to_columns(...)`
- `combine_hillslope_pass_files(...)`
- `hillslope_ebe_to_columns(...)`
- `hillslope_element_to_columns(...)`
- `hillslope_loss_to_columns(...)`
- `hillslope_soil_to_columns(...)`
- `hillslope_wat_to_columns(...)`
- `catalog_scan(base_path)`
- `segment_single_ofe_slope(...)`

### `wepppyo3.swat_interchange`

- `swat_outputs_to_parquet(...)`
- `swat_output_to_parquet(...)`

### `wepppyo3.swat_utils`

- `wepp_hillslope_pass_to_swat_recall(...)`

### `wepppyo3.roads_flowpath`

- `trace_downslope_flowpath(...)`

### `wepppyo3.watershed_abstraction`

- `assign_mofe_map(...)`

### `wepppyo3.wepp_viz`

- `make_soil_loss_grid(...)`
- `make_soil_loss_grid_fps(...)`

## Canonical Release

`release/linux/py312/` is the canonical release output. It contains the
`wepppyo3` Python package tree and is the only directory that should be deployed
from this repository.

Expected layout:

```text
release/linux/py312/wepppyo3/
  __init__.py
  climate/cli_revision_rust.so
  raster_characteristics/raster_characteristics_rust.so
  roads_flowpath/roads_flowpath_rust.so
  sbs_map/sbs_map_rust.so
  swat_interchange/swat_interchange_rust.so
  swat_utils/swat_utils_rust.so
  watershed_abstraction/watershed_abstraction_rust.so
  wepp_interchange/wepp_interchange_rust.so
  wepp_viz/wepp_viz_rust.so
```

Current provenance gap: the package exposes `__version__`, but the release tree
does not yet include a manifest tying each shared object to a source commit,
build timestamp, Python ABI, GDAL/PROJ versions, and artifact hash. Treat that
as a documented follow-up, not a reason to change binaries in documentation-only
work.

See [docs/release-provenance.md](docs/release-provenance.md) before refreshing
or deploying release artifacts.

## Install (Linux)

Copy the canonical release into your Python site-packages. Adjust the
destination for your environment:

```sh
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py312/wepppyo3/ \
  /usr/local/lib/python3.12/dist-packages/wepppyo3/
```

## Build (Linux)

Prerequisites:

- Rust toolchain
- Python 3.12 interpreter
- `gdal-config` on `PATH` from GDAL/PROJ development packages

Build and refresh the canonical release:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=$PYO3_PYTHON

cargo build --release

cp target/release/libraster_characteristics_rust.so \
  release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so
cp target/release/libcli_revision_rust.so \
  release/linux/py312/wepppyo3/climate/cli_revision_rust.so
cp target/release/libroads_flowpath_rust.so \
  release/linux/py312/wepppyo3/roads_flowpath/roads_flowpath_rust.so
cp target/release/libwepp_viz_rust.so \
  release/linux/py312/wepppyo3/wepp_viz/wepp_viz_rust.so
cp target/release/libsbs_map_rust.so \
  release/linux/py312/wepppyo3/sbs_map/sbs_map_rust.so
cp target/release/libswat_interchange_rust.so \
  release/linux/py312/wepppyo3/swat_interchange/swat_interchange_rust.so
cp target/release/libswat_utils_rust.so \
  release/linux/py312/wepppyo3/swat_utils/swat_utils_rust.so
cp target/release/libwatershed_abstraction_rust.so \
  release/linux/py312/wepppyo3/watershed_abstraction/watershed_abstraction_rust.so
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

Verify imports from the canonical release tree:

```sh
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 - <<'PY'
import wepppyo3.climate.cli_revision_rust
import wepppyo3.raster_characteristics.raster_characteristics_rust
import wepppyo3.roads_flowpath.roads_flowpath_rust
import wepppyo3.sbs_map.sbs_map_rust
import wepppyo3.swat_interchange.swat_interchange_rust
import wepppyo3.swat_utils.swat_utils_rust
import wepppyo3.watershed_abstraction.watershed_abstraction_rust
import wepppyo3.wepp_interchange.wepp_interchange_rust
import wepppyo3.wepp_viz.wepp_viz_rust
print("ok")
PY
```

If you only need one crate, build with `-p` and copy the corresponding `.so`:

```sh
cargo build -p raster_characteristics_rust --release
```

Latest targeted refresh evidence (`raster_characteristics`, 2026-05-13 UTC):

- Import proof:
  `/workdir/wepppyo3/release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so`
- SHA256:
  `a2dddb70c3c9670bad8c4103b64d455539896d5ea1be17a99d9c5adc88dccda6`

## ARM64 Mac Build Notes

When building a Rust library that uses PyO3 as a Python extension module on
macOS, linker errors for `_Py...` symbols usually mean the extension is trying
to resolve Python symbols at build time. Extension modules should allow dynamic
lookup from the Python process.

Checklist:

1. Confirm the Rust toolchain is arm64 with `rustc --version --verbose`.
2. Confirm the Python interpreter is arm64 with `file $(which python)`.
3. Ensure the extension crate uses `crate-type = ["cdylib"]` and PyO3's
   `extension-module` feature where required.
4. Build with dynamic lookup flags:

```sh
export RUSTFLAGS="-C link-arg=-undefined -C link-arg=dynamic_lookup"
cargo clean
cargo build
```

On Linux production hosts, use the canonical Linux release path instead of macOS
artifacts.

# Architecture and Boundaries

`wepppyo3` is the native substrate under WEPPpy's Python orchestration. The repo
contains Rust crates that expose Python modules through PyO3, plus internal Rust
support crates used by those modules.

## Architecture

```text
WEPPpy Python orchestration
  routes, NoDb controllers, RQ workers, run directories, reports
      |
      | imports Python modules from release/linux/py312/wepppyo3/
      v
wepppyo3 native substrate
  climate, raster_characteristics, wepp_interchange, swat_interchange,
  swat_utils, roads_flowpath, watershed_abstraction helpers, sbs_map, wepp_viz
      |
      | reads/writes files and arrays owned by WEPPpy workflows
      v
Native peers and model artifacts
  WEPP/SWAT text outputs, Parquet, GDAL/PROJ rasters, Peridot, weppcloud-wbt
```

The Python side remains responsible for user workflows. The Rust side owns
bounded compute, parser, raster, and file-interchange contracts.

## What Belongs in `wepppyo3`

A feature belongs here when all of these are true:

- WEPPpy needs to call it from Python during an existing run workflow.
- The work is a hot kernel, parser, raster scan, file transform, or data-layout-sensitive operation.
- The contract can be expressed as a narrow Python-callable function or module.
- The Rust implementation can preserve WEPPpy's existing input/output semantics.

Examples already in this repo:

- CLIGEN monthlies, precipitation scaling, storm hyetograph helpers, and static-R calculations.
- Raster key classification and pair-count scans for landuse, soils, disturbed, RAP, and Omni paths.
- WEPP output parsing and Parquet interchange generation.
- SWAT+ output conversion and recall-file generation.
- MOFE map label assignment.
- Soil-loss visualization grid generation.

## What Stays in WEPPpy

WEPPpy owns orchestration and state:

- Flask routes and templates.
- NoDb controller lifecycle, locking, serialization, and Redis caching.
- RQ enqueue/dependency behavior and worker status updates.
- Run directory creation, cleanup, and user-visible artifacts.
- Configuration parsing and feature selection.
- Reports, dashboards, query-engine integration, and user-facing API contracts.

Do not move broad application control flow into `wepppyo3`. Rust kernels should
be called by Python wrappers that preserve existing WEPPpy behavior.

## Boundary with Peridot

Peridot owns explicit watershed graph abstraction as a standalone Rust engine and
CLI. It is the right home for watershed topology, graph construction, watershed
output contracts, and Peridot-specific benchmark/operation docs.

`wepppyo3.watershed_abstraction` is not a Peridot replacement. It is a helper
namespace for Python-callable native kernels used inside WEPPpy, such as MOFE map
assignment.

Routing examples:

| Work item | Correct home |
| --- | --- |
| Standalone abstract-watershed CLI behavior | Peridot |
| Peridot graph/table output schema | Peridot |
| Python-callable MOFE raster label helper | `wepppyo3.watershed_abstraction` |
| WEPPpy route or RQ wrapper that launches Peridot | WEPPpy |

## Boundary with `weppcloud-wbt`

`weppcloud-wbt` owns the WhiteboxTools-derived command-line toolchain used for
WBT/TOPAZ-style hydrology and delineation operations. That includes custom tools
such as hillslope delineation, outlet finding, stream-order pruning, and related
terrain-processing commands.

`wepppyo3` should not absorb WBT command-line tools. If WEPPpy needs a native
Python-callable helper around an existing WBT output, `wepppyo3` may be the right
home for the helper. If the work is a hydrology/delineation command, it belongs
in `weppcloud-wbt`.

## Boundary with One-Off Rust Crates

A separate Rust repo can be justified when the component has an independent CLI,
release lifecycle, or domain model. Otherwise, small Python-callable kernels
should generally live in `wepppyo3` to keep deployment and import contracts
centralized.

Use this decision test:

| Question | If yes | If no |
| --- | --- | --- |
| Is the primary caller WEPPpy Python? | Consider `wepppyo3`. | Consider a standalone crate/tool. |
| Does it need a standalone CLI or service? | Consider a separate repo. | Keep as a PyO3 module if narrow. |
| Does it define watershed graph abstraction? | Use Peridot. | Continue the routing test. |
| Is it WBT/TOPAZ terrain tooling? | Use `weppcloud-wbt`. | Continue the routing test. |

## Fallback Policy

Some WEPPpy wrappers retain explicit Python fallback behavior. That is a runtime
contract decision in WEPPpy, not proof that `wepppyo3` is optional everywhere.

Documentation must distinguish:

- Required native paths where Rust is the intended production behavior.
- Explicit fallback boundaries where Python behavior remains supported.
- Development-only parity or oracle implementations.

Avoid silent fallback language. If a wrapper falls back, the docs should say when
and why, and the code should log the selected path.

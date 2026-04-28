# Release Provenance

This document defines the current deployable package layout and the provenance
gaps that should be closed by a future implementation package. It does not
change release mechanics by itself.

## Canonical Deployable Package

`confirmed`: The canonical deployable package is:

```text
release/linux/py312/wepppyo3/
```

That tree is what production WEPPpy imports. The source crates under the repo
root are not directly imported by WEPPpy until built shared objects are copied
into the release tree or installed into the runtime environment.

Expected shared objects:

| Python module | Shared object |
| --- | --- |
| `wepppyo3.climate` | `climate/cli_revision_rust.so` |
| `wepppyo3.raster_characteristics` | `raster_characteristics/raster_characteristics_rust.so` |
| `wepppyo3.roads_flowpath` | `roads_flowpath/roads_flowpath_rust.so` |
| `wepppyo3.sbs_map` | `sbs_map/sbs_map_rust.so` |
| `wepppyo3.swat_interchange` | `swat_interchange/swat_interchange_rust.so` |
| `wepppyo3.swat_utils` | `swat_utils/swat_utils_rust.so` |
| `wepppyo3.watershed_abstraction` | `watershed_abstraction/watershed_abstraction_rust.so` |
| `wepppyo3.wepp_interchange` | `wepp_interchange/wepp_interchange_rust.so` |
| `wepppyo3.wepp_viz` | `wepp_viz/wepp_viz_rust.so` |

Generated `__pycache__` files may exist after imports, but they are not source
or release provenance artifacts.

## Current Version Signal

`confirmed`: `release/linux/py312/wepppyo3/__init__.py` currently exposes a
package `__version__` string.

`confirmed`: most Rust crates currently use crate version `0.1.0`.

`inference`: these signals are useful for human orientation but insufficient for
binary provenance. They do not identify the source commit, exact build command,
Python ABI, GDAL/PROJ versions, or hash of each deployed shared object.

## Manual Build and Copy Flow

Current documented flow:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=$PYO3_PYTHON
cargo build --release
```

Then copy each `target/release/lib*_rust.so` into its corresponding
`release/linux/py312/wepppyo3/<module>/` path.

This package intentionally does not rebuild or replace shared objects.

## Required Pre-Deployment Checks

Before deploying a refreshed release tree, verify:

1. The target shared object was rebuilt from the intended source revision.
2. The shared object was copied to the matching Python package path.
3. `python3.12 -c "import wepppyo3.<module>"` succeeds in the target runtime.
4. Targeted module tests pass.
5. WEPPpy tests for the changed callsite pass when applicable.
6. `git diff --check` passes in this repo.

## Recommended Release Manifest Follow-Up

`hypothesis`: a generated manifest would reduce operator risk and make benchmark
or incident evidence easier to interpret.

A future package should add a checked or generated file such as:

```text
release/linux/py312/wepppyo3/release-manifest.json
```

Recommended fields:

- package version
- source repository URL
- source commit SHA
- build timestamp UTC
- Python version and ABI
- Rust compiler version
- Cargo target triple
- GDAL and PROJ versions
- build command
- per-shared-object SHA256 hash
- per-shared-object source crate
- builder host or CI job identifier, if available

Until that exists, claims about binary provenance should be labeled as
`inference` or `hypothesis` unless supported by separate deployment records.

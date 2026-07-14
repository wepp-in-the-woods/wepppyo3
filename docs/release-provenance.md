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

## Latest Refresh Evidence (py312)

### Acceptance Correction Refresh: `wepp_interchange` weighted PASS semantics

`confirmed`: the `wepp_interchange` shared object was rebuilt from acceptance
correction commit `6eaa699` at `2026-07-13T23:07:36Z`. That commit follows release
commit `96c028f` and source implementation commit `2779b41`.

The correction preserves signed `tdep` values emitted by WEPP and preserves finite
nonnegative particle-flow component vectors without requiring their serialized sum
to equal one. Both behaviors are grounded in the WEPP producer/consumer sources and
were exposed by full-project acceptance before the final successful run.

`confirmed`: final validation passed:

- `cargo test -p wepp_interchange_rust`: 41 passed;
- release-tree weighted Python tests: two passed;
- exact parent-86 signed-deposition replay: passed;
- exact parent-158 particle-vector replay: passed; and
- exhaustive release-tree replay over all 1,869 affected acceptance parents:
  passed, with maximum event budget ratio `0.9999999999305551`.

`confirmed`: the final authenticated WEPPpy RQ job
`2fc269a6-12f8-4d74-a876-0619b2ea3cf7` completed all 3,543 parent PASS files,
watershed WEPP, and interchange using this release artifact.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `5d8e1251d84aed97af358d4473413b089a001de000523fbcd41bf9ffba864db3` |

### Targeted Refresh: `wepp_interchange` AgFields weighted PASS API

`confirmed`: the `wepp_interchange` shared object was rebuilt from the local
AgFields weighted-PASS implementation from source commit `2779b41` at
`2026-07-13T21:16:36Z` using:

```sh
cd /home/workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12
cargo build -p wepp_interchange_rust --release
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

`confirmed`: crate validation passed with 39 tests, including the unchanged Roads
combiner suite and additive weighted identities, validation, serialization closure,
and atomic-output cases.

`confirmed`: the release-tree import and Python API test passed:

```sh
PYTHONPATH=/home/workdir/wepppyo3/release/linux/py312 python3.12 -c \
  "from wepppyo3.wepp_interchange import combine_weighted_hillslope_pass_files; print('ok')"
PYTHONPATH=/home/workdir/wepppyo3/release/linux/py312 python3.12 -m pytest -q \
  tests/wepp_interchange/test_weighted_hillslope_pass.py
```

The exported signature is:

```text
(sources, out_pass, target_area_m2, output_climate_token, strategy="ag_fields_v1")
```

`confirmed`: refreshed `wepp_interchange` SHA256:

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `8c94041776a66968aab302ee20fa1b85d6e53c0b0ca3ffec234ffb84247b5d6f` |

### Full Refresh: Arrow-RS migration package closure

`confirmed`: release tree refreshed from local source commit `8951b50` at
`2026-05-25T20:34:46Z` using:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=$PYO3_PYTHON
cargo build --release
```

`confirmed`: shared objects copied from `target/release/` to
`release/linux/py312/wepppyo3/` module paths.

`confirmed`: import verification succeeded from the release tree:

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

`confirmed`: `sha256sum` for refreshed shared objects:

| Shared object | SHA256 |
| --- | --- |
| `climate/cli_revision_rust.so` | `98988e98022d1bdf56865c044c544830c3d4d639a0e6279c9ac6c6c15e70118c` |
| `raster_characteristics/raster_characteristics_rust.so` | `68debbea97c4e9e5abcf5d1421ceceadda5dd12a3eacf7cd5d1404bbfb307ebc` |
| `roads_flowpath/roads_flowpath_rust.so` | `58d0433a52d99630d607f6e5be198b8bcf2e3fb0d93cb395e29027c2e4968e45` |
| `sbs_map/sbs_map_rust.so` | `17a255f2f72dba49fa9d2d7c41125c82538227ac1b0eff1cf8cef0130fe4ea84` |
| `swat_interchange/swat_interchange_rust.so` | `fb7d1eaaeb9b4ba2df452b07e9d01479fb5fe526f67dfb08f1247dc032129275` |
| `swat_utils/swat_utils_rust.so` | `58c80622da9bef7f08c239a02a37c089143e22e4168d19efa96e692b486c5676` |
| `watershed_abstraction/watershed_abstraction_rust.so` | `fdfc13683d700a9456a517d30f4d2b359f8b8529598704aca686c11c2117dc80` |
| `wepp_interchange/wepp_interchange_rust.so` | `8d60b9b7acd232564827022393623c2d3c88c669209560cbcec587c23738a446` |
| `wepp_viz/wepp_viz_rust.so` | `a0e06b79c8ecd7b13616eecb1280e41543939d7114e1532f09bf95494160d301` |

### Targeted Refresh: `raster_characteristics` deterministic-order hardening

`confirmed`: `raster_characteristics` shared object refreshed on
`2026-05-13T03:32Z` using:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=$PYO3_PYTHON
cargo build -p raster_characteristics_rust --release
```

`confirmed`: copied refreshed artifact:

```sh
cp target/release/libraster_characteristics_rust.so \
  release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so
```

`confirmed`: runtime import verification from the release tree:

```sh
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 -c \
  "from wepppyo3.raster_characteristics import raster_characteristics_rust as rc; print(rc.__file__)"
```

Expected output path:

```text
/workdir/wepppyo3/release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so
```

`confirmed`: refreshed `raster_characteristics` SHA256:

| Shared object | SHA256 |
| --- | --- |
| `raster_characteristics/raster_characteristics_rust.so` | `a2dddb70c3c9670bad8c4103b64d455539896d5ea1be17a99d9c5adc88dccda6` |

### Targeted Refresh: `wepp_interchange` (PS-07 HBP release sync)

`confirmed`: `wepp_interchange` shared object was rebuilt from local source
commit `4de5350` at `2026-05-06T16:47:32Z` using:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12
cargo build -p wepp_interchange_rust --release
```

`confirmed`: copied refreshed artifact:

```sh
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

`confirmed`: runtime verification from release tree includes PS-07 callable
surface with explicit pass-family routing:

```sh
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 - <<'PY'
import inspect
from wepppyo3.wepp_interchange import wepp_interchange_rust as wr
print(inspect.signature(wr.hillslope_pass_to_columns))
PY
```

Expected signature:

```text
(pass_path, version_major, version_minor, cli_calendar_path=None, pass_family=None)
```

`confirmed`: PS-07 invalid process-name guard is active in the release artifact:

```sh
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 - <<'PY'
from wepppyo3.wepp_interchange import hillslope_pass_to_columns
try:
    hillslope_pass_to_columns('/tmp/H0001.pass.hbp', 1, 95, pass_family='hbp')
except Exception as exc:
    print(type(exc).__name__)
    print(str(exc))
PY
```

Expected error text includes:

```text
invalid process HBP name; use H*.hbp (rejecting H*.pass.hbp and H*.pass.dat.hbp)
```

`confirmed`: refreshed `wepp_interchange` SHA256:

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `61537d79173aa8d3a49d1135774da745c5bce9eb9ce209935cd8af39b161a02c` |

### Targeted Refresh: `wepp_interchange` (WB-06 downstream compatibility)

`confirmed`: `wepp_interchange` shared object was rebuilt from local source
commit `df4b8a7` at `2026-05-04T16:21:27Z` using:

```sh
cd /workdir/wepppyo3
cargo build -p wepp_interchange_rust --release
```

`confirmed`: copied refreshed artifact:

```sh
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

`confirmed`: runtime verification from release tree:

```sh
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 - <<'PY'
from wepppyo3.wepp_interchange import wepp_interchange_rust as wr
print(wr.__file__)
PY
```

`confirmed`: `hillslope_wat_to_columns` runtime checks passed for both enriched
layouts:

- with trailing `InterceptionStorage`,
- without `InterceptionStorage` (legacy enriched optional columns only).

`confirmed`: refreshed `wepp_interchange` SHA256:

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `3fe8dfd05ad248fa7a49b6f8810ed9bd1a06e8519bd163670df6f58428a0194a` |

### Prior Full Refresh Snapshot

`confirmed`: release tree refreshed from local source commit `34ab963842c8` at
`2026-04-29T03:47:15Z` using:

```sh
cd /workdir/wepppyo3
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=$PYO3_PYTHON
cargo build --release
```

`confirmed`: shared objects copied from `target/release/` to
`release/linux/py312/wepppyo3/` module paths.

`confirmed`: import verification succeeded from the release tree:

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

`confirmed`: `sha256sum` for refreshed shared objects:

| Shared object | SHA256 |
| --- | --- |
| `climate/cli_revision_rust.so` | `7acca8329b28e321004395d8d6c9bd20848ab2e686391d4a93e767a60c51e02c` |
| `raster_characteristics/raster_characteristics_rust.so` | `84d249db1d8818756944f189a7116afd08b8789a4bf3887bb04b8983c506fcd2` |
| `roads_flowpath/roads_flowpath_rust.so` | `974e031ccdb2c7450bc1267f98dd6669d5a3f4bdd731ca2bdfa76374fcdfcd3d` |
| `sbs_map/sbs_map_rust.so` | `17a255f2f72dba49fa9d2d7c41125c82538227ac1b0eff1cf8cef0130fe4ea84` |
| `swat_interchange/swat_interchange_rust.so` | `0f7e69c6e79ae9b0d75976d47fbc674019e5d29ee931f50680fc291f2a57de9f` |
| `swat_utils/swat_utils_rust.so` | `2280b027bc97863ff51f33269efc3e08283f2a4be29185f10af8d4f414c44057` |
| `watershed_abstraction/watershed_abstraction_rust.so` | `fdfc13683d700a9456a517d30f4d2b359f8b8529598704aca686c11c2117dc80` |
| `wepp_interchange/wepp_interchange_rust.so` | `2ec361a47c278d1e9259393434ee225b4cf49d4852855d8c1c55fc3342a091a2` |
| `wepp_viz/wepp_viz_rust.so` | `a0e06b79c8ecd7b13616eecb1280e41543939d7114e1532f09bf95494160d301` |

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

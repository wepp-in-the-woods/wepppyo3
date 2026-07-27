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

Refresh each `target/release/lib*_rust.so` into its corresponding
`release/linux/py312/wepppyo3/<module>/` path through a same-directory temporary
file and atomic rename, as shown in the root README. Never truncate or overwrite
a shared object in place while a process may have it mapped; restart target
services after the atomic refresh.

Except for a targeted refresh recorded below, this package process does not
rebuild or replace shared objects implicitly.

## Latest Refresh Evidence (py312)

### Targeted Refresh: widened WEPP deep-percolation output

`confirmed`: On 2026-07-24, the `wepp_interchange` shared object was rebuilt
from source commit `f8648d57d7e754f10b83cfcac66e697afe6f5d15`. The parser
continues to accept the legacy two-decimal hillslope WAT representation and
also accepts the widened scientific-notation `Dp` field without shifting
`UpStrmQ`, `SubRIn`, or later fields.

Build and atomic refresh commands:

```sh
cd /home/workdir/wepppyo3
PYO3_PYTHON=/usr/bin/python3.12 \
  PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12 \
  cargo build -p wepp_interchange_rust --release
dst=release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
tmp=$(mktemp "$(dirname "$dst")/.wepp_interchange_rust.so.XXXXXX")
cp target/release/libwepp_interchange_rust.so "$tmp"
chmod 0755 "$tmp"
mv -f "$tmp" "$dst"
```

`confirmed`: validation passed:

- `cargo fmt -p wepp_interchange_rust -- --check`;
- `cargo check -p wepp_interchange_rust`;
- `RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p
  wepp_interchange_rust`: 83 unit and 17 TC_OUT integration tests passed;
- canonical release-tree import succeeded; and
- canonical release-tree `tests/wepp_interchange`: 46 tests passed with one
  unrelated `pytz` deprecation warning.

Build environment: Python 3.12.3, rustc 1.92.0, and cargo 1.92.0.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `de0c1bdc8cc5e5e0ccebb8b1b6bbfe1b519c9746601721a62f42a96774b5b18f` |

### Targeted Refresh: bounded categorical SSURGO candidate support

`confirmed`: On 2026-07-22, the `raster_characteristics` shared object was
rebuilt from source commit `3aedb43bd7305e82876e26acb443436591ba5787`. The
release adds native crop-to-padded-reference, WGS84-radius categorical support,
categorical metadata inspection, and batched intersecting key/category centroids.
These are generic raster primitives; WEPPpy supplies SSURGO provenance and
fallback policy. The intersection primitive derives a raw category location
inside its supplied project key, without iterating raster cells in Python.

Build and validation commands:

```sh
cd /home/workdir/wepppyo3
cargo fmt -p raster_characteristics_rust -- --check
cargo check -p raster_characteristics_rust
RUSTFLAGS='-C link-arg=-lpython3.12' cargo test -p raster_characteristics_rust --lib
cargo build --release -p raster_characteristics_rust
```

`confirmed`: formatting and `cargo check` passed; the changed crate's seven
Rust tests passed with the explicit host PyO3 link argument; and imports of
the four new functions passed from the canonical release tree. The ordinary
host `cargo test` linker limitation remains documented below; no new external
dependency was introduced.

| Shared object | SHA256 |
| --- | --- |
| `raster_characteristics/raster_characteristics_rust.so` | `4d5700bde43b515f91098a5a9b6b1f5c18a3dbb74d393bcf8bde811d9abd2c3e` |

### Targeted Refresh: dedicated AgFields sub-field interchange writers

`confirmed`: On 2026-07-16, the `wepp_interchange` shared object was rebuilt
from the local implementation worktree based on source commit
`4d3c060a27133b9bd7335de3e1dee4d680db0fcf`. The additive release exports six
`ag_fields_hillslope_*_files_to_parquet` functions for PASS/HBP, EBE, ELEMENT,
LOSS, SOIL, and WAT. Each accepts coupled `(path, field_id, sub_field_id)`
descriptors, rejects invalid or mismatched identity, and emits required
`field_id` and `sub_field_id` columns with
`dataset_kind=ag_fields_hillslope` and `ag_fields_schema_version=1`. The six
ordinary writer signatures and schemas are unchanged.

The candidate was built with:

```sh
cd /home/workdir/wepppyo3
cargo build --release -p wepp_interchange_rust
```

The operator installed the candidate with a same-directory atomic replacement;
the shared object was not overwritten in place. Canonical import and all six
new signatures were then verified from
`release/linux/py312/wepppyo3/wepp_interchange/`.

`confirmed`: validation passed:

- `cargo fmt -p wepp_interchange_rust -- --check`;
- `cargo check -p wepp_interchange_rust` and full-workspace `cargo check`;
- `cargo test -p wepp_interchange_rust`: 80 unit and 16 TC_OUT integration
  tests passed;
- canonical release-tree `tests/wepp_interchange`: 46 tests passed with one
  unrelated `pytz` deprecation warning; and
- all six ordinary three-source goldens retained full Arrow table, exact
  schema/metadata, source-order, and row-group parity.

`confirmed`: full-workspace `cargo test` remains unavailable in this host
configuration because unrelated PyO3 crate test binaries fail to link Python C
API symbols. The changed crate's complete Rust and release-tree Python suites
pass. Full-workspace formatting also reports only preexisting formatting drift
under `swat_interchange` and `swat_utils`; the changed crate passes its format
gate.

Build environment: Python 3.12.3, rustc 1.92.0, and cargo 1.92.0.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `8c42edd0a8e1b03bdaf423355a12414180c709efaac3e379e5dd23e6cc77214e` |

### Targeted Refresh: failure-atomic native interchange publication

`confirmed`: the `wepp_interchange` shared object was rebuilt from source commit
`5819cb3d124cb65e253445cb1b2e83d22df9b4e2` on 2026-07-15. Native Parquet
writers now allocate collision-resistant same-directory staging paths, remove
incomplete stages, count physical row groups, and serialize publishers. The
two-file watershed PASS and eight-file watershed LOSS operations stage every
output before publication and restore the prior generation if a later rename
fails. Their path updates remain sequential, so this guarantee is
failure-atomic rollback rather than simultaneous multi-path visibility.

Build and atomic refresh commands:

```sh
cd /home/workdir/wepppyo3
cargo build -p wepp_interchange_rust --release
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/.wepp_interchange_rust.so.new
mv release/linux/py312/wepppyo3/wepp_interchange/.wepp_interchange_rust.so.new \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

`confirmed`: validation passed before and after the refresh:

- `cargo fmt --check -p wepp_interchange_rust`;
- `cargo check -p wepp_interchange_rust`;
- `cargo test -p wepp_interchange_rust`: 68 unit and 16 TC_OUT integration
  tests passed; and
- release-tree Python tests: 22 passed.

Build environment: Python 3.12.3, rustc 1.92.0, and cargo 1.92.0.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `7419203c8b91db1b595590b7c9a28040662d5fad9fdf8b182a17c85a76d518e4` |

### Targeted Refresh: required native interchange data plane

`confirmed`: the `wepp_interchange` shared object was rebuilt from source commit
`942adff` on 2026-07-15. The release adds ordered direct-to-Parquet writers for
hillslope PASS/HBP, EBE, ELEMENT, LOSS, and SOIL; adds native TC_OUT outlet
selection/writing; moves PASS climate-hint extraction and watershed EBE outlet
inference/channel-peak auditing into Rust; and retains the existing direct WAT
and watershed writers. WEPPpy can now require one native parser/writer data
plane without returning report records through Python.

Build and atomic refresh commands:

```sh
cd /home/workdir/wepppyo3
PYO3_PYTHON=/usr/bin/python3.12 \
  PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12 \
  cargo build -p wepp_interchange_rust --release
dst=release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
tmp=$(mktemp "$(dirname "$dst")/.wepp_interchange_rust.so.XXXXXX")
cp target/release/libwepp_interchange_rust.so "$tmp"
chmod --reference="$dst" "$tmp"
mv -f "$tmp" "$dst"
```

`confirmed`: validation passed before the refresh:

- `cargo check -p wepp_interchange_rust`;
- `cargo test -p wepp_interchange_rust`: 59 unit and 10 TC_OUT integration
  tests passed;
- seven focused bulk-writer Rust regressions passed; and
- five release-tree Python API/schema/order tests passed against a temporary
  package containing the rebuilt shared object.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `92b180d5bc383165eb71e767285bfab1cd3ad24d48fe356145aef645bc185163` |

### Targeted Refresh: bounded direct hillslope WAT Parquet

`confirmed`: the `wepp_interchange` shared object was rebuilt from source commit
`361c9ac` on 2026-07-15. The additive
`hillslope_wat_files_to_parquet` API consumes a source-ordered file list, loads
the climate calendar once, parses one hillslope at a time into compact Rust Arrow
arrays, and writes each source as the next Parquet row group. It avoids returning
full multi-OFE WAT tables through Python process-pool futures while preserving the
existing schema, source order, Snappy compression, and atomic output replacement.

Build and atomic refresh commands:

```sh
cd /home/workdir/wepppyo3
cargo build -p wepp_interchange_rust --release
dst=release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
tmp=$(mktemp "$(dirname "$dst")/.wepp_interchange_rust.so.XXXXXX")
cp target/release/libwepp_interchange_rust.so "$tmp"
chmod 0755 "$tmp"
mv -f "$tmp" "$dst"
```

`confirmed`: validation passed:

- `cargo test -p wepp_interchange_rust`: 45 passed;
- the release-tree direct multi-file WAT API test passed;
- WEPPpy schema/value parity with the Python reference passed; and
- the generated 25,567,139,478-byte Hybrid WAT corpus wrote
  `H.wat.parquet` in 571.737 seconds while sampled worker-cgroup anonymous
  memory peaked at 489,709,568 bytes, with no OOM event.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `6ea01a3983de350a6029c4fce47bd59d8a0d2fad927feecf00cf7cb958d3bdd1` |

### Targeted Refresh: schema-compatible UTF-8 interchange batches

`confirmed`: the `wepp_interchange` shared object was rebuilt from source commit
`a9403b889257` at `2026-07-15T02:47:35Z`. The writer now supplies plain Arrow
`Utf8` arrays for schemas that declare `Utf8`; Parquet dictionary encoding remains
enabled in the writer properties. This repairs the non-empty watershed PASS/LOSS
failure without changing the public Arrow schema.

Build and atomic refresh commands:

```sh
cd /home/workdir/wepppyo3
cargo build -p wepp_interchange_rust --release
dst=release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
tmp=$(mktemp "$(dirname "$dst")/.wepp_interchange_rust.so.XXXXXX")
cp target/release/libwepp_interchange_rust.so "$tmp"
chmod 0755 "$tmp"
mv -f "$tmp" "$dst"
```

`confirmed`: validation passed:

- `cargo test -p wepp_interchange_rust`: 44 passed;
- the new non-empty UTF-8 Parquet regression passed;
- generated Concept 1 LOSS conversion completed in 0.386 seconds at 76,176 KiB
  peak RSS;
- generated Concept 1 PASS conversion produced 22,002,030 event rows and 3,543
  metadata rows in 78.011 seconds at 435,296 KiB peak RSS; and
- streaming comparison with the Python fallback found equal metadata-bearing
  schemas and equal event values across 89 batches of at most 250,000 rows.

`confirmed`: an earlier in-place copy while the completed Concept 1 Python
process still mapped the old shared object caused that wrapper process to exit
139 during interpreter teardown. The generated result and durable terminal state
were already complete. The artifact was subsequently reinstalled by
same-directory atomic rename, and the root README now makes that release rule
explicit.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `d7a8ba031eed323d35c88f899a156230372ae1d582f516d8bfd4ef43d5a00bfb` |

### Targeted Refresh: explicit-breakpoint AgFields slope segmentation

`confirmed`: the `wepp_interchange` shared object was rebuilt from source commit
`9c84643` at `2026-07-14T18:04:15Z`. The additive
`segment_single_ofe_slope_at_breakpoints` API validates a closed sequence from
zero to one, supports one through 20 OFEs, preserves the source profile length,
and optionally sets the accepted target width. The existing automatic segmenter
is unchanged.

Build and copy commands:

```sh
cd /home/workdir/wepppyo3
PYO3_PYTHON=/usr/bin/python3.12 \
  PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12 \
  cargo build -p wepp_interchange_rust --release
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
```

`confirmed`: crate validation passed with 43 tests, including explicit irregular
breakpoints, length/width preservation, and invalid-boundary/limit cases. The
release-tree Python import and slope tests also passed.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `776703694245aa092f6f1972cbd539dddb2ca0f4c054afa04d7e25f863a745f6` |

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

### Targeted Refresh: `raster_characteristics` MUKEY geometry evidence

`confirmed`: the `raster_characteristics` release artifact was refreshed for
the additive `local_mukey_geometry` API on 2026-07-22. The API returns each
source MUKEY's local valid-candidate support independently from its
four-neighbor shared-edge count; it is a research-only input to the SSURGO
fallback scoring experiment.

```sh
cargo build -p raster_characteristics_rust --release
cp target/release/libraster_characteristics_rust.so \
  release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 -c \
  "from wepppyo3.raster_characteristics import local_mukey_geometry; print(callable(local_mukey_geometry))"
```

`confirmed`: the targeted Rust crate tests and release-tree import check
passed. Refreshed shared-object SHA256:

| Shared object | SHA256 |
| --- | --- |
| `raster_characteristics/raster_characteristics_rust.so` | `4009ec7351ee640cb693dd8f35e5efbc43d36744cd344a1cc25c3b391f6b2095` |

### Targeted Refresh: `categorical_support_within_bounds`

`confirmed`: the `raster_characteristics` release artifact was refreshed from
source commit `10db015f8ce4` on 2026-07-22 for the additive generic
`categorical_support_within_bounds` API. It reads one raster window, excludes
requested category values, and returns deterministic category/pixel-support
pairs; the WEPPpy SSURGO wrapper filters those pairs to buildable MUKEYs.

```sh
export PYO3_PYTHON=/usr/bin/python3.12
export PYTHON_SYS_EXECUTABLE=/usr/bin/python3.12
cargo build --release -p raster_characteristics_rust
cp target/release/libraster_characteristics_rust.so \
  release/linux/py312/wepppyo3/raster_characteristics/raster_characteristics_rust.so
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 -c \
  "from wepppyo3.raster_characteristics import categorical_support_within_bounds; print(callable(categorical_support_within_bounds))"
```

`confirmed`: the release-tree import check and targeted WEPPpy fixture test
passed. Refreshed shared-object SHA256:

| Shared object | SHA256 |
| --- | --- |
| `raster_characteristics/raster_characteristics_rust.so` | `a5df2ac0836087e1c54d8137afb39d8607b41465f126ecb648bab92441d2567e` |

### Targeted Refresh: annual LOSS hillslope area

`confirmed`: the `wepp_interchange` release artifact was rebuilt from source
commit `fc3e361` on 2026-07-27. The annual watershed LOSS parser accepts the
uniform historical 11-field hillslope layout and the corrected uniform
12-field layout. Historical rows receive a null `Hillslope Area`; corrected
rows preserve the emitted area in hectares. Mixed or other-width layouts fail
explicitly.

```sh
cargo fmt --check
cargo test -p wepp_interchange_rust
cargo build -p wepp_interchange_rust --release
cp target/release/libwepp_interchange_rust.so \
  release/linux/py312/wepppyo3/wepp_interchange/.wepp_interchange_rust.so.new
mv release/linux/py312/wepppyo3/wepp_interchange/.wepp_interchange_rust.so.new \
  release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 python3.12 -c \
  "import wepppyo3.wepp_interchange.wepp_interchange_rust"
```

`confirmed`: Rust 1.92.0 built the Python 3.12 artifact. All 105 targeted Rust
tests passed. Five WEPPpyo3 native-writer tests and 95 targeted WEPPpy
interchange and consumer tests passed in the WEPPpy container against the
release-tree shared object. The standalone release-tree import also passed.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `faa9173665aee64e92ce077488121cc21b7a1cc06cb771b280df81c7862299f1` |

### Targeted Refresh: watershed SOIL OFE overflow recovery

`confirmed`: the `wepp_interchange` release artifact was rebuilt from source
commit `de575bc` on 2026-07-27. The watershed SOIL parser accepts current
widened numeric OFE fields and reconstructs historical `**` identifiers only
when daily identifiers are contiguous, overflow begins at 100, and every day
has the same complete layout.

Rust 1.92.0 built the Python 3.12 artifact. All 110 targeted Rust tests passed.
The release-tree import passed. The release artifact converted the synced
521,696-row `mdobre-foursquare-fovea` incident file and reconstructed exact
daily OFEs 1 through 238.

| Shared object | SHA256 |
| --- | --- |
| `wepp_interchange/wepp_interchange_rust.so` | `61db1daa36c7383f897e52e640e092b00490d04bab646b95e0b55ed608851777` |

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

# Python oracle capture

## Static:

confirmed: Forest started with clean repositories:
wepppyo3 `main` at `3123547e677eb113dd64c43024eda20a06505796`;
WEPPpy `master` at `3c51780f50e2599ef72b03214c35bf20538e55c4`.

confirmed: The unchanged producer is
`wepppy/wepp/interchange/totalwatsed3.py`, SHA-256
`85355a02b700fe3c8d8dbac1407dbdb8e541f38c3ea087e9361dba4445b67af3`.
Its last source change is `be60b6fc9` (release DuckDB buffers between aggregates).
The existing native release shared object SHA-256 is
`9607fb39ee82aca6f00946a8b9100932155a4ef8e3a78e7f2a483d9421c31a77`.
Neither producer nor native artifact was edited.

confirmed: The Python producer computes Runoff from PASS runvol, using
`runvol / Area * 1000` where Area is positive. Streamflow is Runoff plus
Lateral Flow plus Baseflow. All three supplied output files imply initial
storage 0, baseflow coefficient 0.04, and seepage coefficient 0; regenerated
outputs used those values, with an explicit nonexistent ash directory.

## Ran:

confirmed: Capture used Forest image `wepppy-dev`, image ID
`sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75`.
Runtime: Python 3.12.14, DuckDB 1.1.1, NumPy 1.26.0, pandas 2.2.2,
PyArrow 23.0.1. The isolated container used worker UID:GID 1000:993,
CPUs 0-11, WEPPPY_NCPU=12, 12 GiB memory with no additional swap,
no network, and read-only source repository mounts. Source HPC trees
were not mounted. The writable /evidence mount points to Forest
`/workdir/wepppyo3/target/totalwatsed3-evidence`.

Capture scripts and a reproduction command pinned to the captured image are preserved as
[capture.py](capture.py), [capture_single.py](capture_single.py), and
[capture-command.sh](capture-command.sh). Each execution creates a unique
directory and writes only its generated output there. Existing oracles remain
unchanged.

[python-baseline.json](python-baseline.json) records final successful capture
commands' input options, output directories, wall seconds, baseline, and cgroup
memory. The two HPC captures ran sequentially in one container; their memory.peak
values are cumulative for that container. These are single baseline captures,
not five-repeat benchmark acceptance or incremental-peak measurements.
The first exploratory MOFE capture derived seepage from zero initial storage
and produced NaN; that harness error was corrected by using a nonzero storage
row. Only the corrected finite-parameter captures appear in final evidence.

confirmed: Exact integer fields, null masks, full schema metadata, and floating
values at rtol=1e-10/atol=1e-12 were rechecked with
[compare_oracle.py](compare_oracle.py). Both HPC outputs pass.
The single-OFE output fails; see [parity-results.md](parity-results.md).

## Approved single-OFE oracle authority (2026-09-07)

The user approved designating the captured current-producer output as the
single-OFE parity oracle and resuming execution, while preserving the old
WEPPpy fixture. The active oracle is now
`tests/fixtures/totalwatsed3/decimal-pleasing/totalwatsed3.parquet` in wepppyo3,
SHA-256 `e7c6e2f0a37093a59540dff0b67bbb44a25f9abdea6e145a6bd65d4c37809550`.
Input files continue to come from the unchanged WEPPpy decimal-pleasing fixture.
The new oracle was captured from WEPPpy revision
`3c51780f50e2599ef72b03214c35bf20538e55c4`, with gwstorage=0, bfcoeff=0.04,
dscoeff=0, and no ash. It preserves current PASS-runoff semantics and all
79 columns. Reverting to the older WAT-Q runoff formula was rejected because
this package is a faithful extraction of current production behavior.
The old 68-column fixture remains historical evidence, not the native parity
authority. No numerical tolerance or other package gate changes.

## Reproduction after Python retirement

Historical capture/benchmark scripts import the producer from WEPPpy. Run their
Python-oracle mode only against original revision
3c51780f50e2599ef72b03214c35bf20538e55c4 in an isolated checkout/runtime. The
current facade is native-only; rerunning those scripts against it would compare
native output to itself and is not valid Python baseline evidence. Existing
oracles and raw pre-retirement measurements are preserved without regeneration.

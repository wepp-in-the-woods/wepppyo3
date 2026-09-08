# Candidate validation commands

## Static:

The canonical release remains unchanged. During candidate development, the new
Python tests require an explicit target-binary override. This is a test-only
selection, not a production fallback. Default release-import acceptance must
be run after an authorized release refresh if the remaining package gates pass.

## Ran:

From /workdir/wepppyo3:

```sh
cargo fmt --check
PYO3_PYTHON=/usr/bin/python3.12 cargo test -p wepp_interchange_rust
PYO3_PYTHON=/usr/bin/python3.12 cargo build --release -p wepp_interchange_rust
docker run --rm --memory 12g --memory-swap 12g --cpuset-cpus 0-11 \
  --user 1000:993 --network none --entrypoint /opt/venv/bin/python \
  -e PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312 \
  -e WEPPPY_NCPU=12 -e PYTHONDONTWRITEBYTECODE=1 \
  -e WEPP_INTERCHANGE_TEST_BINARY=/workdir/wepppyo3/target/release/libwepp_interchange_rust.so \
  -v /workdir/wepppy:/workdir/wepppy:ro \
  -v /workdir/wepppyo3:/workdir/wepppyo3:ro \
  wepppy-dev -m pytest /workdir/wepppyo3/tests/wepp_interchange/test_totalwatsed3.py \
  -q -p no:cacheprovider
git diff --check
git diff --cached --check
```

confirmed: 112 Rust test executions and 28 targeted Python tests pass.
Fifteen Markdown files passed relative-link and uk2us checks before this command
record was added. Candidate source/binary hashes still match native-build.json.
108 staged LFS pointers match their physical files' SHA-256 and byte count.
No live workflow or canonical release-API acceptance was performed.

Scaling optimization commands and their pinned container configuration are in
compact-scale-commands.json and compact-benchmark-commands.json. The latter is
preserved when the five-repeat rerun completes. No canonical release refresh
or WEPPpy integration occurred during this optimization.

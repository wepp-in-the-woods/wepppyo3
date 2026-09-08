#!/usr/bin/env bash
# Run on Forest. Outputs are unique; supplied fixture oracles stay read-only.
set -euo pipefail
cd /workdir/wepppyo3
artifact_dir=docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts
mkdir -p target/totalwatsed3-evidence
for script in capture.py capture_single.py; do
    docker run --rm --memory 12g --memory-swap 12g --cpuset-cpus 0-11 \
        --user 1000:993 --network none --entrypoint /opt/venv/bin/python \
        -e PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312 \
        -e WEPPPY_NCPU=12 -e PYTHONDONTWRITEBYTECODE=1 \
        -v /workdir/wepppy:/workdir/wepppy:ro \
        -v /workdir/wepppyo3:/workdir/wepppyo3:ro \
        -v /workdir/wepppyo3/target/totalwatsed3-evidence:/evidence \
        sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75 \
        "/workdir/wepppyo3/$artifact_dir/$script"
done

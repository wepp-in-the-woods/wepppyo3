#!/usr/bin/env bash
set -euo pipefail        # fail fast

export PYTHON=$(which python)

# ----------------------------------------------------------------------
# 3. Clean out old shared objects (ignore if they’re already gone)
# ----------------------------------------------------------------------
rm -f release/linux/py310-wepppy310-env/wepppyo3/raster_characteristics/raster_characteristics_rust.so
rm -f release/linux/py310-wepppy310-env/wepppyo3/climate/cli_revision_rust.so
rm -f release/linux/py310-wepppy310-env/wepppyo3/wepp_viz/wepp_viz_rust.so

# ----------------------------------------------------------------------
# 4. Build the Rust libs
# ----------------------------------------------------------------------
cargo build --release

# ----------------------------------------------------------------------
# 5. Copy them into the Python package (fixed double ‘mv’ typo)
# ----------------------------------------------------------------------
cp target/release/libraster_characteristics_rust.so \
   release/linux/py310-wepppy310-env/wepppyo3/raster_characteristics/raster_characteristics_rust.so

cp target/release/libcli_revision_rust.so \
   release/linux/py310-wepppy310-env/wepppyo3/climate/cli_revision_rust.so

cp target/release/libwepp_viz_rust.so \
   release/linux/py310-wepppy310-env/wepppyo3/wepp_viz/wepp_viz_rust.so
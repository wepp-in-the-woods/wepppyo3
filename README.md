# wepppyo3

Rust/PyO3 extension modules for wepppy.

## Module Catalog

### wepppyo3.climate

- `calculate_p_annual_monthlies(...)`: convenience wrapper; returns average monthly precipitation from a CLIGEN file or month/ppt arrays.
- `calculate_monthlies(src_fn)`: returns monthly climate stats (`ppts`, `tmaxs`, `tmins`, `nwds`) from a CLIGEN file.
- `cli_revision(...)`: spatializes a CLIGEN file by biasing precip/tmin/tmax between watershed and hillslope centroids.
- `interpolate_geospatial(...)`: interpolates a 3D grid at a target point (`nearest`, `linear`, `cubic`), with optional clipping.
- `make_rhem_storm_file(src_fn, dst_fn)`: converts a CLIGEN file into a RHEM storm file.
- `rust_cli_p_scale_monthlies(src_fn, dst_fn, p_mults)`: scales precipitation by per-month multipliers.
- `rust_cli_p_scale(src_fn, dst_fn, p_mult)`: scales precipitation by a single multiplier.
- `rust_cli_p_scale_annual_monthlies(src_fn, dst_fn, p_mults)`: scales precipitation with an annual month sequence of multipliers.
- `rust_cli_calculate_p_annual_monthlies(src_fn)`: computes average monthly precipitation from a CLIGEN file.
- `rust_cli_calculate_p_annual_monthlies_from_lists(months, precips)`: computes average monthly precipitation from arrays.
- `rust_cli_calculate_monthlies(src_fn)`: computes monthly precip/tmax/tmin/wet-day arrays from a CLIGEN file.

### wepppyo3.raster_characteristics

- `identify_mode_single_raster_key(...)`: mode parameter value per key; falls back to global mode when a key has only nodata.
- `identify_mode_intersecting_raster_keys(...)`: mode parameter value per key/key2 intersection; falls back to global mode when empty.
- `identify_median_single_raster_key(...)`: median parameter value per key.
- `identify_median_intersecting_raster_keys(...)`: median parameter value per key/key2 intersection.

### wepppyo3.wepp_viz

- `make_soil_loss_grid(...)`: builds a soil-loss grid from subwta IDs, discha, and WEPP plot outputs.
- `make_soil_loss_grid_fps(...)`: builds a soil-loss grid from discha and flowpath plot outputs.

## Canonical release

`/workdir/wepppyo3/release/linux/py312/` is the canonical release output. It contains the
`wepppyo3` Python package tree and is the only directory that should be deployed. When you
rebuild, copy updated shared objects into this tree.

Expected layout:

```
release/linux/py312/wepppyo3/
  __init__.py
  climate/cli_revision_rust.so
  raster_characteristics/raster_characteristics_rust.so
  wepp_viz/wepp_viz_rust.so
```

## Install (Linux)

Copy the canonical release into your Python site-packages (adjust the destination for your
environment):

```sh
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py312/wepppyo3/ \
  /usr/local/lib/python3.12/dist-packages/wepppyo3/
```

## Build (Linux)

Prereqs:
- Rust toolchain
- Python 3.12 interpreter
- `gdal-config` on PATH (GDAL/PROJ dev packages installed)

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
cp target/release/libwepp_viz_rust.so \
  release/linux/py312/wepppyo3/wepp_viz/wepp_viz_rust.so
```

If you only need one crate, build with `-p` and copy the corresponding `.so`:

```sh
cargo build -p raster_characteristics_rust --release
```

## ARM64 Mac Build (M1/M2)

### Building PyO3 extension on macOS (M1/M2)

[Chat GPT](https://chatgpt.com/share/67dbaf2b-e038-8009-b3cb-4c277b34dcdd)

### Context
When building a Rust library that uses [pyo3](https://pyo3.rs/) as a Python extension module, on macOS you can encounter linker errors like:

```
Undefined symbols for architecture arm64:
  "_PyBool_Type", referenced from:
  ...
```

This typically happens because the linker is trying to resolve `_Py...` symbols at build time, while on macOS extension modules should allow dynamic lookup from the Python process.

### Steps to Fix

1. **Ensure** your Rust toolchain is arm64. For example, run:

   ```bash
   rustc --version --verbose
   # Should say: host: aarch64-apple-darwin
   ```

2. **Ensure** your Python interpreter is also arm64:

   ```bash
   file $(which python)
   # Should say: Mach-O 64-bit executable arm64
   ```

3. In your `Cargo.toml` for the extension module:

   ```toml
   [lib]
   crate-type = ["cdylib"]

   [dependencies.pyo3]
   version = "0.22"
   features = ["extension-module"]
   ```

   This ensures pyo3 sets up the extension properly.

4. **Set** the environment variable for dynamic lookup:

   ```bash
   export RUSTFLAGS="-C link-arg=-undefined -C link-arg=dynamic_lookup"
   cargo clean
   cargo build
   ```

   The flags `-undefined dynamic_lookup` tell macOS to allow undefined `_Py...` symbols, which will be resolved at runtime by the Python interpreter.

### Key Takeaway
On macOS, building a PyO3 extension module requires ignoring undefined Python symbols at build time. Setting `RUSTFLAGS` to include `-undefined dynamic_lookup` resolves the undefined `_Py...` references, because the Python runtime will supply them at import time.

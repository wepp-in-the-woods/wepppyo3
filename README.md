# wepppyo3 Installation
wepppy PyO3 routines (rust)

## deployment 3.6 (18.04)
```
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py36/wepppyo3/ /usr/local/lib/python3.6/dist-packages/wepppyo3/
```

## deployment 3.10 (22.04)
```
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py310/wepppyo3/  /usr/local/lib/python3.10/dist-packages/wepppyo3/
```

## deployment 3.12 (24.04)
```
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py312/wepppyo3/  /usr/local/lib/python3.12/dist-packages/wepppyo3/
```

## deployment 3.13 (wepppy-env on forest.local)
```
sudo rsync -av --progress /workdir/wepppyo3/release/linux/py312/wepppyo3/  /workdir/miniconda3/envs/wepppy-env/lib/python3.13/site-packages/wepppyo3/
```

# Linux Build

```sh
conda activate wepppy310-env
cd /workdir/wepppyo3/
./linux_wepppy310-env_build.sh 
```


## ARM64 Mac Build

# Building PyO3 extension on macOS (M1/M2)

[Chat GPT](https://chatgpt.com/share/67dbaf2b-e038-8009-b3cb-4c277b34dcdd)

## Context
When building a Rust library that uses [pyo3](https://pyo3.rs/) as a Python extension module, on macOS you can encounter linker errors like:

```
Undefined symbols for architecture arm64:
  "_PyBool_Type", referenced from:
  ...
```

This typically happens because the linker is trying to resolve `_Py…` symbols at build time, while on macOS extension modules should allow dynamic lookup from the Python process.

## Steps to Fix

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

   The flags `-undefined dynamic_lookup` tell macOS to allow undefined `_Py…` symbols, which will be resolved at runtime by the Python interpreter.


## Key Takeaway
On macOS, building a PyO3 extension module requires ignoring undefined Python symbols at build time. Setting `RUSTFLAGS` to include `-undefined dynamic_lookup` resolves the undefined `_Py...` references, because the Python runtime will supply them at import time.


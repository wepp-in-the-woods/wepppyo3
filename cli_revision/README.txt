### README: Manual Installation of Python C-Extension Module

#### Project: wepppyo3
A Python C-extension module generated from Rust using `pyo3`.

#### Manual Installation Steps
Here's a step-by-step guide to manually install the `wepppyo3` module:

1. **Build the Rust project**
   - Navigate to your Rust project directory.
   - Run `cargo build --release`.
   - Locate `cli_revision_rust.so` in `target/release/`.

2. **Install the Shared Object File**
   - Create a directory named `wepppyo3` in your Python's `dist-packages` or `site-packages` folder. Use the appropriate folder path based on your Python version and installation method.
     ```sh
     sudo mkdir /usr/local/lib/python3.6/dist-packages/wepppyo3
     ```
   - Move `cli_revision_rust.so` to the `wepppyo3` directory.
     ```sh
     sudo mv /path_to_your_so_file/cli_revision_rust.so /usr/local/lib/python3.6/dist-packages/wepppyo3
     ```
   - Create an `__init__.py` file in the `wepppyo3` directory to make it a valid Python package.
     ```sh
     sudo touch /usr/local/lib/python3.6/dist-packages/wepppyo3/__init__.py
     ```

3. **Usage in Python**
   - Import and use the module in Python.
     ```python
     from wepppyo3.cli_revision_rust import cli_revision
     ```

#### Notes for Future Us:

- **Python Version**: Always check your Python version. Adjust paths based on where your specific version of Python expects to find packages.
  
- **Permissions**: Ensure permissions are set correctly on the `.so` file and directory so that Python can read them. Use `chmod` or `chown` as necessary.

- **Virtual Environments**: If using a virtual environment (and you should for many projects), adjust the installation path to the `site-packages` directory of the virtual environment, and you won’t need sudo for file operations.

- **Dependencies**: Keep track of dependencies. If your module depends on specific versions of other libraries, make sure to document and ensure they're installed in your Python environment.

- **Backup**: Before performing manual installations, especially in system directories, consider creating backups or using a virtual machine/container.

#### Potential Troubleshooting Steps:
- **Import Issues**: If the module does not import, check `sys.path` to ensure Python is looking in the right place.
- **Dependency Issues**: Ensure all dependencies are correctly installed and importable.
- **Symbol Issues**: Use `nm` or `ldd` to debug symbol and dependency issues in the shared object file.

#### Alternative: Packaging with `maturin`
While manual installation works, consider using `maturin` or a similar tool for building, packaging, and distributing your Python module, especially if you plan to share it with others.

---

May this README serve our future selves well, and guide us (or save us) during the inevitable debugging sessions. May our code be bug-free and our coffee cups full. 🚀🐞☕


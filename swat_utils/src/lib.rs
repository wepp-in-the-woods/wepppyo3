#![allow(clippy::clone_on_copy)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]
#![allow(dead_code)]
#![allow(unused_variables)]

use std::path::PathBuf;

use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

mod arrow_support;
mod calendar;
mod errors;
mod floats;
mod hill_pass;
mod parquet;
mod schema;
mod swat_utils;

use crate::errors::InterchangeError;
use crate::schema::VersionInfo;

#[pyfunction]
#[pyo3(signature = (wepp_output_dir, swat_txtinout_dir, version_major, version_minor, recall_subdir="recall", cli_calendar_path=None, filename_template="hill_{wepp_id:05d}.rec", include_subsurface=true, include_tile=true, include_baseflow=true, recall_connections=None, recall_wst="wea1", recall_object_type="sdc", ncpu=None, write_manifest=false))]
fn wepp_hillslope_pass_to_swat_recall(
    wepp_output_dir: String,
    swat_txtinout_dir: String,
    version_major: u32,
    version_minor: u32,
    recall_subdir: &str,
    cli_calendar_path: Option<String>,
    filename_template: &str,
    include_subsurface: bool,
    include_tile: bool,
    include_baseflow: bool,
    recall_connections: Option<Vec<(i32, i32)>>,
    recall_wst: &str,
    recall_object_type: &str,
    ncpu: Option<usize>,
    write_manifest: bool,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let wepp_output_dir = PathBuf::from(wepp_output_dir);
    let swat_txtinout_dir = PathBuf::from(swat_txtinout_dir);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let manifest = swat_utils::wepp_hillslope_pass_to_swat_recall(
        &wepp_output_dir,
        &swat_txtinout_dir,
        recall_subdir,
        cli_calendar_path.as_deref(),
        &version,
        filename_template,
        include_subsurface,
        include_tile,
        include_baseflow,
        recall_connections.as_deref(),
        recall_wst,
        recall_object_type,
        ncpu,
        write_manifest,
    )
    .map_err(to_py_err)?;

    match manifest {
        Some(entries) => Ok(Python::with_gil(|py| {
            let items = entries
                .into_iter()
                .map(|entry| entry.into_pydict(py))
                .collect::<Vec<_>>();
            items.into_py(py)
        })),
        None => Ok(Python::with_gil(|py| py.None())),
    }
}

#[pymodule]
fn swat_utils_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(wepp_hillslope_pass_to_swat_recall, m)?)?;
    Ok(())
}

fn to_py_err(err: InterchangeError) -> PyErr {
    match err {
        InterchangeError::Io { .. } => PyIOError::new_err(err.display_message()),
        InterchangeError::Parse { .. } | InterchangeError::Calendar { .. } => {
            PyValueError::new_err(err.display_message())
        }
        _ => PyRuntimeError::new_err(err.display_message()),
    }
}

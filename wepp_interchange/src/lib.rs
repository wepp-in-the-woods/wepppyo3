#![allow(clippy::cloned_ref_to_slice_refs)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::len_zero)]
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::useless_conversion)]
#![allow(clippy::wrong_self_convention)]
#![allow(dead_code)]
#![allow(unused_assignments)]

use std::path::PathBuf;
use std::time::Instant;

use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

mod arrays;
mod calendar;
mod catalog;
mod chan_peak;
mod chanwb;
mod chnwb;
mod ebe;
mod errors;
mod floats;
mod hill_ebe;
mod hill_element;
mod hill_loss;
mod hill_pass;
mod hill_pass_combine;
mod hill_soil;
mod hill_wat;
mod loss;
mod mofe;
mod parquet;
mod pass;
mod schema;
mod soil;

use crate::errors::InterchangeError;
use crate::schema::VersionInfo;

#[pyfunction]
#[pyo3(signature = (ebe_path, output_path, version_major, version_minor, cli_calendar_path=None, start_year=None, legacy_element_id=None, chunk_rows=None, compression="snappy"))]
fn watershed_ebe_to_parquet(
    ebe_path: String,
    output_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
    legacy_element_id: Option<i32>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let ebe_path = PathBuf::from(ebe_path);
    let output_path = PathBuf::from(output_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let summary = ebe::watershed_ebe_to_parquet(
        &ebe_path,
        &output_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
        legacy_element_id,
        chunk_rows,
    )
    .map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(build_summary_dict(
        elapsed_ms,
        summary.rows_written,
        summary.row_groups,
        version_major,
        vec![output_path.display().to_string()],
    ))
}

#[pyfunction]
#[pyo3(signature = (chanwb_path, output_path, version_major, version_minor, cli_calendar_path=None, start_year=None, chunk_rows=None, compression="snappy"))]
fn watershed_chanwb_to_parquet(
    chanwb_path: String,
    output_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let chanwb_path = PathBuf::from(chanwb_path);
    let output_path = PathBuf::from(output_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let summary = chanwb::watershed_chanwb_to_parquet(
        &chanwb_path,
        &output_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
        chunk_rows,
    )
    .map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(build_summary_dict(
        elapsed_ms,
        summary.rows_written,
        summary.row_groups,
        version_major,
        vec![output_path.display().to_string()],
    ))
}

#[pyfunction]
#[pyo3(signature = (chnwb_path, output_path, version_major, version_minor, cli_calendar_path=None, start_year=None, chunk_rows=None, compression="snappy"))]
fn watershed_chnwb_to_parquet(
    chnwb_path: String,
    output_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let chnwb_path = PathBuf::from(chnwb_path);
    let output_path = PathBuf::from(output_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let summary = chnwb::watershed_chnwb_to_parquet(
        &chnwb_path,
        &output_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
        chunk_rows,
    )
    .map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(build_summary_dict(
        elapsed_ms,
        summary.rows_written,
        summary.row_groups,
        version_major,
        vec![output_path.display().to_string()],
    ))
}

#[pyfunction]
#[pyo3(signature = (pass_path, events_path, metadata_path, version_major, version_minor, cli_calendar_path=None, chunk_rows=None, compression="snappy"))]
fn watershed_pass_to_parquet(
    pass_path: String,
    events_path: String,
    metadata_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let pass_path = PathBuf::from(pass_path);
    let events_path = PathBuf::from(events_path);
    let metadata_path = PathBuf::from(metadata_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let result = pass::watershed_pass_to_parquet(
        &pass_path,
        &events_path,
        &metadata_path,
        cli_calendar_path.as_deref(),
        &version,
        chunk_rows,
    );
    let (event_summary, metadata_summary) = result.map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let rows_written = event_summary.rows_written + metadata_summary.rows_written;
    let row_groups = event_summary.row_groups + metadata_summary.row_groups;

    let output_paths = vec![
        events_path.display().to_string(),
        metadata_path.display().to_string(),
    ];
    Ok(build_summary_dict(
        elapsed_ms,
        rows_written,
        row_groups,
        version_major,
        output_paths,
    ))
}

#[pyfunction]
#[pyo3(signature = (soil_path, output_path, version_major, version_minor, cli_calendar_path=None, chunk_rows=None, compression="snappy"))]
fn watershed_soil_to_parquet(
    soil_path: String,
    output_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let soil_path = PathBuf::from(soil_path);
    let output_path = PathBuf::from(output_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let summary = soil::watershed_soil_to_parquet(
        &soil_path,
        &output_path,
        cli_calendar_path.as_deref(),
        &version,
        chunk_rows,
    )
    .map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(build_summary_dict(
        elapsed_ms,
        summary.rows_written,
        summary.row_groups,
        version_major,
        vec![output_path.display().to_string()],
    ))
}

#[pyfunction]
#[pyo3(signature = (loss_path, output_dir, version_major, version_minor, compression="snappy"))]
fn watershed_loss_to_parquet(
    loss_path: String,
    output_dir: String,
    version_major: u32,
    version_minor: u32,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let loss_path = PathBuf::from(loss_path);
    let output_dir = PathBuf::from(output_dir);

    let start = Instant::now();
    let outputs =
        loss::watershed_loss_to_parquet(&loss_path, &output_dir, &version).map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let rows_written: usize = outputs.summaries.values().map(|s| s.rows_written).sum();
    let row_groups: usize = outputs.summaries.values().map(|s| s.row_groups).sum();

    let mut output_paths = outputs
        .paths
        .values()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();
    output_paths.sort();

    Ok(build_summary_dict(
        elapsed_ms,
        rows_written,
        row_groups,
        version_major,
        output_paths,
    ))
}

#[pyfunction]
#[pyo3(signature = (chan_path, output_path, version_major, version_minor, cli_calendar_path=None, start_year=None, chunk_rows=None, compression="snappy"))]
fn watershed_chan_peak_to_parquet(
    chan_path: String,
    output_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
    compression: &str,
) -> PyResult<PyObject> {
    ensure_snappy(compression)?;
    let version = VersionInfo::new(version_major, version_minor);

    let chan_path = PathBuf::from(chan_path);
    let output_path = PathBuf::from(output_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let start = Instant::now();
    let summary = chan_peak::watershed_chan_peak_to_parquet(
        &chan_path,
        &output_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
        chunk_rows,
    )
    .map_err(to_py_err)?;

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(build_summary_dict(
        elapsed_ms,
        summary.rows_written,
        summary.row_groups,
        version_major,
        vec![output_path.display().to_string()],
    ))
}

#[pyfunction]
#[pyo3(signature = (pass_path, version_major, version_minor, cli_calendar_path=None))]
fn hillslope_pass_to_columns(
    pass_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let pass_path = PathBuf::from(pass_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let columns =
        hill_pass::hillslope_pass_to_columns(&pass_path, cli_calendar_path.as_deref(), &version)
            .map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (base_pass, road_passes, out_pass, strategy="phase1"))]
fn combine_hillslope_pass_files(
    base_pass: String,
    road_passes: Vec<String>,
    out_pass: String,
    strategy: &str,
) -> PyResult<()> {
    let base_pass = PathBuf::from(base_pass);
    let road_passes = road_passes
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let out_pass = PathBuf::from(out_pass);

    hill_pass_combine::combine_hillslope_pass_files(&base_pass, &road_passes, &out_pass, strategy)
        .map_err(to_py_err)
}

#[pyfunction]
#[pyo3(signature = (ebe_path, version_major, version_minor, cli_calendar_path=None, start_year=None))]
fn hillslope_ebe_to_columns(
    ebe_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let ebe_path = PathBuf::from(ebe_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let columns = hill_ebe::hillslope_ebe_to_columns(
        &ebe_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
    )
    .map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (element_path, version_major, version_minor, start_year=None))]
fn hillslope_element_to_columns(
    element_path: String,
    version_major: u32,
    version_minor: u32,
    start_year: Option<i32>,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let element_path = PathBuf::from(element_path);

    let columns = hill_element::hillslope_element_to_columns(&element_path, &version, start_year)
        .map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (loss_path, version_major, version_minor))]
fn hillslope_loss_to_columns(
    loss_path: String,
    version_major: u32,
    version_minor: u32,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let loss_path = PathBuf::from(loss_path);

    let columns = hill_loss::hillslope_loss_to_columns(&loss_path, &version).map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (soil_path, version_major, version_minor, cli_calendar_path=None, start_year=None))]
fn hillslope_soil_to_columns(
    soil_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
    start_year: Option<i32>,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let soil_path = PathBuf::from(soil_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let columns = hill_soil::hillslope_soil_to_columns(
        &soil_path,
        cli_calendar_path.as_deref(),
        &version,
        start_year,
    )
    .map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (wat_path, version_major, version_minor, cli_calendar_path=None))]
fn hillslope_wat_to_columns(
    wat_path: String,
    version_major: u32,
    version_minor: u32,
    cli_calendar_path: Option<String>,
) -> PyResult<PyObject> {
    let version = VersionInfo::new(version_major, version_minor);
    let wat_path = PathBuf::from(wat_path);
    let cli_calendar_path = cli_calendar_path.map(PathBuf::from);

    let columns =
        hill_wat::hillslope_wat_to_columns(&wat_path, cli_calendar_path.as_deref(), &version)
            .map_err(to_py_err)?;
    Ok(Python::with_gil(|py| columns.into_pydict(py)))
}

#[pyfunction]
#[pyo3(signature = (base_path))]
fn catalog_scan(base_path: String) -> PyResult<PyObject> {
    let base_path = PathBuf::from(base_path);
    let entries = catalog::catalog_scan(&base_path).map_err(to_py_err)?;
    Ok(Python::with_gil(|py| entries.into_py(py)))
}

#[pyfunction]
#[pyo3(signature = (src_fn, dst_fn=None, target_length=50.0, apply_buffer=false, buffer_length=15.0, min_length=10.0, max_ofes=19))]
fn segment_single_ofe_slope(
    src_fn: String,
    dst_fn: Option<String>,
    target_length: f64,
    apply_buffer: bool,
    buffer_length: f64,
    min_length: f64,
    max_ofes: i64,
) -> PyResult<i64> {
    mofe::segment_single_ofe_slope(
        &src_fn,
        dst_fn.as_deref(),
        target_length,
        apply_buffer,
        buffer_length,
        min_length,
        max_ofes,
    )
    .map_err(to_py_err)
}

#[pymodule]
fn wepp_interchange_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(watershed_pass_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_soil_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_loss_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_chan_peak_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_ebe_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_chanwb_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(watershed_chnwb_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_pass_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(combine_hillslope_pass_files, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_ebe_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_element_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_loss_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_soil_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(hillslope_wat_to_columns, m)?)?;
    m.add_function(wrap_pyfunction!(catalog_scan, m)?)?;
    m.add_function(wrap_pyfunction!(segment_single_ofe_slope, m)?)?;
    Ok(())
}

fn build_summary_dict(
    elapsed_ms: u64,
    rows_written: usize,
    row_groups: usize,
    schema_version: u32,
    output_paths: Vec<String>,
) -> PyObject {
    Python::with_gil(|py| {
        let dict = PyDict::new_bound(py);
        dict.set_item("rows_written", rows_written).unwrap();
        dict.set_item("row_groups", row_groups).unwrap();
        dict.set_item("elapsed_ms", elapsed_ms).unwrap();
        dict.set_item("schema_version", schema_version.to_string())
            .unwrap();
        dict.set_item("output_paths", output_paths).unwrap();
        dict.into_py(py)
    })
}

fn ensure_snappy(compression: &str) -> PyResult<()> {
    if compression.eq_ignore_ascii_case("snappy") {
        Ok(())
    } else {
        Err(PyValueError::new_err(format!(
            "Unsupported compression '{compression}'; only 'snappy' is supported"
        )))
    }
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

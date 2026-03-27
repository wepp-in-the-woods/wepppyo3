use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;

use peridot::raster::Raster;
use peridot::roads_trace::{
    trace_downslope_flowpath as trace_core, TraceDownslopeResult, TraceError,
    TraceTerminationReason,
};

#[pyfunction]
#[pyo3(signature = (subwta_path, flovec_path, relief_path, seed_row, seed_col, channel_path=None, max_steps=20000))]
fn trace_downslope_flowpath(
    py: Python<'_>,
    subwta_path: &str,
    flovec_path: &str,
    relief_path: &str,
    seed_row: i64,
    seed_col: i64,
    channel_path: Option<&str>,
    max_steps: i64,
) -> PyResult<PyObject> {
    let seed_row = parse_non_negative("seed_row", seed_row)?;
    let seed_col = parse_non_negative("seed_col", seed_col)?;

    if max_steps < 1 {
        return Err(PyValueError::new_err(format!(
            "max_steps must be >= 1, got {}",
            max_steps
        )));
    }

    let subwta = Raster::<i32>::read(subwta_path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed reading subwta: {}", err)))?;
    let flovec = Raster::<u8>::read(flovec_path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed reading flovec: {}", err)))?;
    let relief = Raster::<f32>::read(relief_path)
        .map_err(|err| PyRuntimeError::new_err(format!("failed reading relief: {}", err)))?;

    let channel_mask = match channel_path {
        Some(path) => Some(Raster::<i32>::read(path).map_err(|err| {
            PyRuntimeError::new_err(format!("failed reading channel mask: {}", err))
        })?),
        None => None,
    };

    let result = trace_core(
        &subwta,
        &flovec,
        &relief,
        seed_row,
        seed_col,
        channel_mask.as_ref(),
        max_steps as usize,
    )
    .map_err(trace_error_to_py)?;

    let dict = result_to_pydict(py, &result)?;
    Ok(dict.into_py(py))
}

#[pymodule]
fn roads_flowpath_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(trace_downslope_flowpath, m)?)?;
    Ok(())
}

fn parse_non_negative(name: &str, value: i64) -> PyResult<usize> {
    if value < 0 {
        return Err(PyValueError::new_err(format!("{} must be >= 0", name)));
    }

    Ok(value as usize)
}

fn trace_error_to_py(err: TraceError) -> PyErr {
    match err {
        TraceError::SeedOutOfBounds { .. }
        | TraceError::RasterShapeMismatch { .. }
        | TraceError::InvalidMaxSteps { .. } => PyValueError::new_err(err.to_string()),
    }
}

fn termination_reason_label(reason: &TraceTerminationReason) -> &'static str {
    match reason {
        TraceTerminationReason::HitChannel => "hit_channel",
        TraceTerminationReason::InvalidFlowDirection => "invalid_flow_direction",
        TraceTerminationReason::LoopDetected => "loop_detected",
        TraceTerminationReason::RasterEdge => "raster_edge",
        TraceTerminationReason::MaxStepsExceeded => "max_steps_exceeded",
    }
}

fn result_to_pydict<'py>(
    py: Python<'py>,
    result: &TraceDownslopeResult,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new_bound(py);

    dict.set_item("seed_row", result.seed_row)?;
    dict.set_item("seed_col", result.seed_col)?;
    dict.set_item("seed_topaz_id", result.seed_topaz_id)?;
    dict.set_item("reaches_channel", result.reaches_channel)?;
    dict.set_item("channel_row", result.channel_row)?;
    dict.set_item("channel_col", result.channel_col)?;
    dict.set_item("channel_topaz_id", result.channel_topaz_id)?;
    dict.set_item(
        "termination_reason",
        termination_reason_label(&result.termination_reason),
    )?;
    dict.set_item("rows", result.rows.clone())?;
    dict.set_item("cols", result.cols.clone())?;
    dict.set_item("indices", result.indices.clone())?;
    dict.set_item("distance_m", result.distance_m.clone())?;
    dict.set_item("elevation_m", result.elevation_m.clone())?;
    dict.set_item("segment_slope", result.segment_slope.clone())?;
    dict.set_item("path_length_m", result.path_length_m)?;
    dict.set_item("drop_m", result.drop_m)?;
    dict.set_item("mean_slope", result.mean_slope)?;
    dict.set_item("max_slope", result.max_slope)?;

    Ok(dict)
}

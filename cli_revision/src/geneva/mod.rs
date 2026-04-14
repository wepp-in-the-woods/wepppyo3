use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

mod convert;

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_prepare_hrus(payload_json: &str) -> PyResult<String> {
    convert::validate_payload_json(payload_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    Ok(geneva_core::scaffold_response("geneva_prepare_hrus"))
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_build_frequency_panel(payload_json: &str) -> PyResult<String> {
    convert::validate_payload_json(payload_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    Ok(geneva_core::scaffold_response(
        "geneva_build_frequency_panel",
    ))
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_run_batch(payload_json: &str) -> PyResult<String> {
    convert::validate_payload_json(payload_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    Ok(geneva_core::scaffold_response("geneva_run_batch"))
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_validate_uh(payload_json: &str) -> PyResult<String> {
    convert::validate_payload_json(payload_json)
        .map_err(pyo3::exceptions::PyValueError::new_err)?;
    Ok(geneva_core::scaffold_response("geneva_validate_uh"))
}

pub fn register_python_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(geneva_prepare_hrus, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_build_frequency_panel, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_run_batch, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_validate_uh, m)?)?;
    Ok(())
}

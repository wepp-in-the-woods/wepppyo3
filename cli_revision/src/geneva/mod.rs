#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

mod convert;

fn run_stub_api(api_name: &str, payload_json: &str) -> PyResult<String> {
    let request =
        convert::parse_stub_request(payload_json).map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::scaffold_response(api_name, &request).map_err(convert::map_geneva_error_to_pyerr)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_prepare_hrus(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_prepare_hrus", payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_build_frequency_panel(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_build_frequency_panel", payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_run_batch(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_run_batch", payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_validate_uh(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_validate_uh", payload_json)
}

pub fn register_python_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(geneva_prepare_hrus, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_build_frequency_panel, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_run_batch, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_validate_uh, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn assert_stub_contract(response: &str, api: &str) {
        let parsed: Value = serde_json::from_str(response).expect("response should be valid JSON");
        assert_eq!(parsed["status"], "stub");
        assert_eq!(parsed["api"], api);
        assert_eq!(parsed["kernel_schema_version"], 1);
    }

    #[test]
    fn direct_entrypoints_return_stub_contract() {
        let payload = r#"{"kernel_schema_version":1}"#;
        assert_stub_contract(
            &geneva_prepare_hrus(payload).expect("prepare_hrus should succeed"),
            "geneva_prepare_hrus",
        );
        assert_stub_contract(
            &geneva_build_frequency_panel(payload).expect("build_frequency_panel should succeed"),
            "geneva_build_frequency_panel",
        );
        assert_stub_contract(
            &geneva_run_batch(payload).expect("run_batch should succeed"),
            "geneva_run_batch",
        );
        assert_stub_contract(
            &geneva_validate_uh(payload).expect("validate_uh should succeed"),
            "geneva_validate_uh",
        );
    }

    #[test]
    fn direct_entrypoints_reject_invalid_payloads_with_mapped_error_code() {
        pyo3::prepare_freethreaded_python();
        let error = geneva_prepare_hrus("{}").expect_err("missing schema version should fail");
        let message = error.to_string();
        assert!(message.contains("invalid_json"));
    }
}

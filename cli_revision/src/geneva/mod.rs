#![allow(clippy::useless_conversion)]

use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

mod convert;

fn run_prepare_hrus_api(payload_json: &str) -> PyResult<String> {
    let request = convert::parse_prepare_hrus_request(payload_json)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    let response =
        geneva_core::hru::prepare_hrus(&request).map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::hru::serialize_prepare_hrus_response(&response)
        .map_err(convert::map_geneva_error_to_pyerr)
}

fn run_batch_cn_api(payload_json: &str) -> PyResult<String> {
    let request = convert::parse_run_batch_request(payload_json)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    let response = geneva_core::cn::run_batch_cn_excess(&request)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::cn::serialize_run_batch_response(&response)
        .map_err(convert::map_geneva_error_to_pyerr)
}

fn run_stub_api(api_name: &str, payload_json: &str) -> PyResult<String> {
    let request =
        convert::parse_stub_request(payload_json).map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::scaffold_response(api_name, &request).map_err(convert::map_geneva_error_to_pyerr)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_prepare_hrus(payload_json: &str) -> PyResult<String> {
    run_prepare_hrus_api(payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_build_frequency_panel(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_build_frequency_panel", payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_run_batch(payload_json: &str) -> PyResult<String> {
    run_batch_cn_api(payload_json)
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
    fn direct_stub_entrypoints_return_stub_contract() {
        let payload = r#"{"kernel_schema_version":1}"#;
        assert_stub_contract(
            &geneva_build_frequency_panel(payload).expect("build_frequency_panel should succeed"),
            "geneva_build_frequency_panel",
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
        assert!(message.contains("invalid_json") || message.contains("invalid_input"));
    }

    #[test]
    fn prepare_hrus_maps_raster_io_errors_with_typed_code() {
        pyo3::prepare_freethreaded_python();
        let payload = r#"{
            "kernel_schema_version": 1,
            "bound_tif": "/tmp/does-not-exist-bound.tif",
            "landuse_tif": "/tmp/does-not-exist-landuse.tif",
            "hydgrpdcd_tif": "/tmp/does-not-exist-hydgrpdcd.tif",
            "min_hru_area_ha": 2.0
        }"#;
        let error =
            geneva_prepare_hrus(payload).expect_err("missing raster files should produce error");
        let message = error.to_string();
        assert!(message.contains("raster_io"));
    }

    #[test]
    fn run_batch_returns_cn_kernel_response() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_1",
            "lambda_mode": "0.05",
            "time_minutes": [0.0, 10.0, 20.0],
            "cumulative_rainfall_mm": [0.0, 5.0, 20.0],
            "hru_rows": [
                {"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 99.0},
                {"hru_id": "hru_2", "area_m2": 500.0, "cn_lambda_020": 82.0}
            ]
        }"#;

        let response = geneva_run_batch(payload).expect("run_batch should succeed");
        let parsed: Value = serde_json::from_str(&response).expect("response must be valid json");
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["phase"], "run_batch");
        assert_eq!(parsed["kernel_schema_version"], 1);
        assert_eq!(parsed["storm_id"], "storm_1");
        let hru_rows = parsed["hru_excess"]
            .as_array()
            .expect("hru_excess array should be present");
        let hru_1 = hru_rows
            .iter()
            .find(|row| row["hru_id"] == "hru_1")
            .expect("hru_1 should be present");
        assert_eq!(hru_1["cn_lambda_005"], 99.0);
        assert_eq!(hru_1["selected_cn"], 99.0);
        assert_eq!(
            parsed["composite_excess"]["incremental_excess_mm"]
                .as_array()
                .expect("composite increment array")
                .len(),
            3
        );
    }

    #[test]
    fn run_batch_rejects_unsupported_schema_version() {
        pyo3::prepare_freethreaded_python();
        let payload = r#"{
            "kernel_schema_version": 2,
            "storm_id": "storm_bad_schema",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, 5.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error = geneva_run_batch(payload).expect_err("unsupported schema version should fail");
        assert!(error.to_string().contains("invalid_input"));
    }

    #[test]
    fn run_batch_maps_validation_errors_with_typed_code() {
        pyo3::prepare_freethreaded_python();
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_bad",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, -1.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error = geneva_run_batch(payload).expect_err("invalid rainfall payload should fail");
        assert!(error.to_string().contains("invalid_input"));
    }
}

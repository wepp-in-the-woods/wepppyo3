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

fn run_batch_api(payload_json: &str) -> PyResult<String> {
    let request = convert::parse_run_batch_request(payload_json)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    let response = geneva_core::cn::run_batch_cn_excess(&request)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::cn::serialize_run_batch_response(&response)
        .map_err(convert::map_geneva_error_to_pyerr)
}

fn run_build_frequency_panel_api(payload_json: &str) -> PyResult<String> {
    let request = convert::parse_build_frequency_panel_request(payload_json)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    let response = geneva_core::frequency_panel::build_frequency_panel(&request)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::frequency_panel::serialize_build_frequency_panel_response(&response)
        .map_err(convert::map_geneva_error_to_pyerr)
}

fn run_build_hyetograph_api(payload_json: &str) -> PyResult<String> {
    let request = convert::parse_hyetograph_request(payload_json)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    let response = geneva_core::hyetograph::build_hyetograph_from_request(&request)
        .map_err(convert::map_geneva_error_to_pyerr)?;
    geneva_core::hyetograph::serialize_neh4_type_b_hyetograph_response(&response)
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
    run_build_frequency_panel_api(payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_build_hyetograph(payload_json: &str) -> PyResult<String> {
    run_build_hyetograph_api(payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_run_batch(payload_json: &str) -> PyResult<String> {
    run_batch_api(payload_json)
}

#[pyfunction]
#[pyo3(signature = (payload_json = "{}"))]
fn geneva_validate_uh(payload_json: &str) -> PyResult<String> {
    run_stub_api("geneva_validate_uh", payload_json)
}

pub fn register_python_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(geneva_prepare_hrus, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_build_frequency_panel, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_build_hyetograph, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_run_batch, m)?)?;
    m.add_function(wrap_pyfunction!(geneva_validate_uh, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn assert_stub_contract(response: &str, api: &str) {
        let parsed: Value = serde_json::from_str(response).expect("response should be valid JSON");
        assert_eq!(parsed["status"], "stub");
        assert_eq!(parsed["api"], api);
        assert_eq!(parsed["kernel_schema_version"], 1);
    }

    fn write_temp_file(filename: &str, contents: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be monotonic for tests")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "geneva_adapter_frequency_panel_{}_{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&dir).expect("temp directory should be created");
        let path = dir.join(filename);
        fs::write(&path, contents).expect("temp file should be written");
        path
    }

    fn sample_cligen_csv() -> &'static str {
        r#"
PRECIPITATION FREQUENCY ESTIMATES
by metric for ARI (years):, 1,2
Storm depth (mm):, 5,10
Storm duration (hours):, 0.1666667,0.5
"#
    }

    fn sample_noaa_csv() -> &'static str {
        r#"
PRECIPITATION FREQUENCY ESTIMATES
by duration for ARI (years):, 1,2
10-min:, 30,40
30-min:, 20,25
"#
    }

    #[test]
    fn build_frequency_panel_entrypoint_returns_typed_kernel_payload() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let noaa_path = write_temp_file("noaa.csv", sample_noaa_csv());
        let payload = format!(
            r#"{{
                "kernel_schema_version": 1,
                "durations_minutes": [10, 30],
                "ari_years": [1, 2],
                "distribution_type": "neh4_type_b",
                "allow_duration_interpolation": false,
                "sources": {{
                    "cligen_freq": "{}",
                    "noaa14_pds": "{}"
                }}
            }}"#,
            cligen_path.display(),
            noaa_path.display()
        );

        let response =
            geneva_build_frequency_panel(&payload).expect("build_frequency_panel should succeed");
        let parsed: Value = serde_json::from_str(&response).expect("response should be valid JSON");
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["phase"], "build_frequency_panel");
        assert_eq!(parsed["kernel_schema_version"], 1);
        assert_eq!(parsed["distribution_type"], "neh4_type_b");
        assert!(parsed["cells"].as_array().is_some());
        let datasources = parsed["datasource_ids"]
            .as_array()
            .expect("datasource_ids should be an array")
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(datasources, vec!["cligen_freq", "noaa14_pds"]);
    }

    #[test]
    fn build_hyetograph_entrypoint_returns_selected_shape_metadata() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "duration_minutes": 60.0,
            "depth_mm": 25.0,
            "time_step_minutes": 5.0,
            "distribution_type": "type_ii"
        }"#;

        let response = geneva_build_hyetograph(payload).expect("hyetograph should build");
        let parsed: Value = serde_json::from_str(&response).expect("response should be valid JSON");
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["phase"], "build_hyetograph");
        assert_eq!(parsed["kernel_schema_version"], 1);
        assert_eq!(parsed["distribution_type"], "type_ii");
        assert_eq!(
            parsed["source_metadata"]["source_distribution_type"],
            "type_ii"
        );
        assert!(parsed["time_minutes"].as_array().is_some());
        assert!(parsed["cumulative_rainfall_mm"].as_array().is_some());
    }

    #[test]
    fn validate_uh_remains_stubbed() {
        let payload = r#"{"kernel_schema_version":1}"#;
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
    fn build_frequency_panel_rejects_malformed_source_mapping_with_typed_code() {
        pyo3::prepare_freethreaded_python();
        let payload = r#"{
            "kernel_schema_version": 1,
            "durations_minutes": [10],
            "ari_years": [1],
            "distribution_type": "neh4_type_b",
            "allow_duration_interpolation": false,
            "sources": {
                "cligen_freq": {"path": "invalid"}
            }
        }"#;
        let error = geneva_build_frequency_panel(payload)
            .expect_err("malformed source mapping should fail");
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
    fn run_batch_returns_hydrograph_kernel_response() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_1",
            "lambda_mode": "0.05",
            "uh_method": "scs_curvilinear",
            "tc_hours": 1.2,
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
        assert_eq!(parsed["uh_method"], "scs_curvilinear");
        assert_eq!(parsed["unit_hydrograph"]["method_id"], "scs_curvilinear");
        assert!(parsed["summary_metrics"]["peak_discharge"]
            .as_f64()
            .is_some());
        assert!(parsed["summary_metrics"]["time_to_peak"].as_f64().is_some());
        assert!(parsed["summary_metrics"]["runoff_volume"]
            .as_f64()
            .is_some());
        assert!(parsed["summary_metrics"]["runoff_depth"].as_f64().is_some());
        assert!(
            parsed["hydrograph_diagnostics"]["volume_closure_relative"]
                .as_f64()
                .expect("closure diagnostic should be present")
                <= 0.01
        );
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
            "uh_method": "scs_triangular",
            "tc_hours": 1.2,
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
            "uh_method": "scs_triangular",
            "tc_hours": 1.2,
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, -1.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error = geneva_run_batch(payload).expect_err("invalid rainfall payload should fail");
        assert!(error.to_string().contains("invalid_input"));
    }

    #[test]
    fn run_batch_rejects_invalid_uh_method_with_typed_error() {
        pyo3::prepare_freethreaded_python();
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_bad_method",
            "lambda_mode": "0.20",
            "uh_method": "bad_method",
            "tc_hours": 1.2,
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, 5.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error = geneva_run_batch(payload).expect_err("invalid method should fail");
        let rendered = error.to_string();
        assert!(rendered.contains("invalid_json") || rendered.contains("invalid_input"));
    }
}

use geneva_core::cn::RunBatchRequest;
use geneva_core::error::GenevaError;
use geneva_core::frequency_panel::BuildFrequencyPanelRequest;
use geneva_core::hru::PrepareHrusRequest;
use geneva_core::types::GenevaStubRequest;
use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

pub fn parse_stub_request(payload_json: &str) -> Result<GenevaStubRequest, GenevaError> {
    GenevaStubRequest::from_payload_json(payload_json)
}

pub fn parse_prepare_hrus_request(payload_json: &str) -> Result<PrepareHrusRequest, GenevaError> {
    PrepareHrusRequest::from_payload_json(payload_json)
}

pub fn parse_run_batch_request(payload_json: &str) -> Result<RunBatchRequest, GenevaError> {
    RunBatchRequest::from_payload_json(payload_json)
}

pub fn parse_build_frequency_panel_request(
    payload_json: &str,
) -> Result<BuildFrequencyPanelRequest, GenevaError> {
    BuildFrequencyPanelRequest::from_payload_json(payload_json)
}

pub fn map_geneva_error_to_pyerr(error: GenevaError) -> PyErr {
    PyValueError::new_err(format!("{}: {}", error.code(), error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use geneva_core::error::GenevaError;

    #[test]
    fn parse_stub_request_rejects_empty_payload() {
        let result = parse_stub_request("");
        assert!(matches!(result, Err(GenevaError::InvalidInput(_))));
    }

    #[test]
    fn parse_stub_request_accepts_minimal_payload() {
        let result = parse_stub_request(r#"{"kernel_schema_version":1}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_prepare_hrus_request_rejects_missing_required_paths() {
        let payload = r#"{"kernel_schema_version":1}"#;
        let result = parse_prepare_hrus_request(payload);
        assert!(matches!(result, Err(GenevaError::InvalidJson(_))));
    }

    #[test]
    fn parse_run_batch_request_accepts_minimal_valid_payload() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_1",
            "lambda_mode": "0.20",
            "uh_method": "scs_triangular",
            "tc_hours": 1.2,
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, 5.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let result = parse_run_batch_request(payload);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_run_batch_request_rejects_invalid_uh_method_id() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_1",
            "lambda_mode": "0.20",
            "uh_method": "invalid",
            "tc_hours": 1.2,
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, 5.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let result = parse_run_batch_request(payload);
        assert!(matches!(result, Err(GenevaError::InvalidJson(_))));
    }

    #[test]
    fn parse_build_frequency_panel_request_accepts_minimal_valid_payload() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "durations_minutes": [10, 30],
            "ari_years": [1, 2],
            "distribution_type": "neh4_type_b",
            "allow_duration_interpolation": false,
            "sources": {
                "cligen_freq": "/tmp/cligen.csv",
                "noaa14_pds": "/tmp/noaa.csv"
            }
        }"#;
        let result = parse_build_frequency_panel_request(payload);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_build_frequency_panel_request_rejects_malformed_source_mapping() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "durations_minutes": [10],
            "ari_years": [1],
            "distribution_type": "neh4_type_b",
            "allow_duration_interpolation": false,
            "sources": {
                "cligen_freq": 42
            }
        }"#;
        let result = parse_build_frequency_panel_request(payload);
        assert!(matches!(result, Err(GenevaError::InvalidJson(_))));
    }

    #[test]
    fn map_geneva_error_to_pyerr_returns_value_error() {
        pyo3::prepare_freethreaded_python();
        let pyerr = map_geneva_error_to_pyerr(GenevaError::InvalidInput("bad payload".to_string()));
        let rendered = pyerr.to_string();
        assert!(rendered.contains("invalid_input"));
        assert!(rendered.contains("bad payload"));
    }
}

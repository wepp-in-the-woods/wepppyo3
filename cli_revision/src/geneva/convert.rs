use geneva_core::error::GenevaError;
use geneva_core::types::GenevaStubRequest;
use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

pub fn parse_stub_request(payload_json: &str) -> Result<GenevaStubRequest, GenevaError> {
    GenevaStubRequest::from_payload_json(payload_json)
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
    fn map_geneva_error_to_pyerr_returns_value_error() {
        let pyerr = map_geneva_error_to_pyerr(GenevaError::InvalidInput("bad payload".to_string()));
        let rendered = pyerr.to_string();
        assert!(rendered.contains("invalid_input"));
        assert!(rendered.contains("bad payload"));
    }
}

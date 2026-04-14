use crate::error::GenevaError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct HruRow {
    pub hru_id: String,
    pub area_m2: f64,
    pub cn_arc_ii: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StormEvent {
    pub storm_id: String,
    pub duration_minutes: f64,
    pub depth_mm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    pub kernel_schema_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StormResult {
    pub storm_id: String,
    pub peak_discharge_cms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchResult {
    pub storms_total: usize,
    pub storms_completed: usize,
}

pub const GENEVA_KERNEL_SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GenevaStubRequest {
    pub kernel_schema_version: u32,
}

impl GenevaStubRequest {
    pub fn from_payload_json(payload_json: &str) -> Result<Self, GenevaError> {
        if payload_json.trim().is_empty() {
            return Err(GenevaError::InvalidInput(
                "payload_json must not be empty".to_string(),
            ));
        }

        let request: Self = serde_json::from_str(payload_json)
            .map_err(|err| GenevaError::InvalidJson(err.to_string()))?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), GenevaError> {
        if self.kernel_schema_version == 0 {
            return Err(GenevaError::InvalidInput(
                "kernel_schema_version must be >= 1".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct GenevaStubResponse {
    pub status: String,
    pub api: String,
    pub kernel_schema_version: u32,
}

impl GenevaStubResponse {
    pub fn new(api: &str, kernel_schema_version: u32) -> Self {
        Self {
            status: "stub".to_string(),
            api: api.to_string(),
            kernel_schema_version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GenevaStubRequest, GENEVA_KERNEL_SCHEMA_VERSION_V1};

    #[test]
    fn parse_stub_request_accepts_valid_payload() {
        let payload = r#"{"kernel_schema_version":1}"#;
        let request =
            GenevaStubRequest::from_payload_json(payload).expect("valid payload should parse");
        assert_eq!(
            request.kernel_schema_version,
            GENEVA_KERNEL_SCHEMA_VERSION_V1
        );
    }

    #[test]
    fn parse_stub_request_rejects_missing_schema_version() {
        let payload = "{}";
        let error = GenevaStubRequest::from_payload_json(payload)
            .expect_err("missing kernel_schema_version must fail");
        assert_eq!(error.code(), "invalid_json");
    }

    #[test]
    fn parse_stub_request_rejects_schema_version_zero() {
        let payload = r#"{"kernel_schema_version":0}"#;
        let error = GenevaStubRequest::from_payload_json(payload)
            .expect_err("schema version zero must fail validation");
        assert_eq!(error.code(), "invalid_input");
    }
}

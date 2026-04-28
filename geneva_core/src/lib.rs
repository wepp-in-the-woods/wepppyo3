pub mod cn;
pub mod convolution;
pub mod error;
pub mod frequency_panel;
pub mod hru;
pub mod hyetograph;
pub mod storm_shape;
pub mod types;
pub mod uh;

use crate::error::GenevaError;
use crate::types::{GenevaStubRequest, GenevaStubResponse};

pub fn scaffold_response(
    api_name: &str,
    request: &GenevaStubRequest,
) -> Result<String, GenevaError> {
    let response = GenevaStubResponse::new(api_name, request.kernel_schema_version);
    serde_json::to_string(&response).map_err(|err| GenevaError::Serialization(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::GenevaStubRequest;
    use serde_json::Value;

    #[test]
    fn scaffold_response_contains_contract_fields() {
        let request = GenevaStubRequest {
            kernel_schema_version: 1,
        };
        let response =
            scaffold_response("geneva_prepare_hrus", &request).expect("response serialization");
        let parsed: Value = serde_json::from_str(&response).expect("response must be valid json");
        assert_eq!(parsed["api"], "geneva_prepare_hrus");
        assert_eq!(parsed["status"], "stub");
        assert_eq!(parsed["kernel_schema_version"], 1);
    }
}

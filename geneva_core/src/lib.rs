pub mod cn;
pub mod convolution;
pub mod error;
pub mod frequency_panel;
pub mod hru;
pub mod hyetograph;
pub mod types;
pub mod uh;

pub fn scaffold_response(api_name: &str) -> String {
    format!(r#"{{"status":"stub","api":"{}"}}"#, api_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_response_contains_api_name() {
        let response = scaffold_response("geneva_prepare_hrus");
        assert!(response.contains("geneva_prepare_hrus"));
        assert!(response.contains("\"status\":\"stub\""));
    }
}

pub fn validate_payload_json(payload_json: &str) -> Result<(), String> {
    if payload_json.trim().is_empty() {
        return Err("payload_json must not be empty".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_payload_json_rejects_empty_payload() {
        let result = validate_payload_json("");
        assert!(result.is_err());
    }

    #[test]
    fn validate_payload_json_accepts_non_empty_payload() {
        let result = validate_payload_json("{}");
        assert!(result.is_ok());
    }
}

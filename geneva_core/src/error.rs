use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenevaError {
    InvalidInput(String),
    InvalidJson(String),
    Serialization(String),
    NotImplemented(&'static str),
}

impl Display for GenevaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            Self::InvalidJson(msg) => write!(f, "invalid json: {msg}"),
            Self::Serialization(msg) => write!(f, "serialization error: {msg}"),
            Self::NotImplemented(feature) => write!(f, "not implemented: {feature}"),
        }
    }
}

impl std::error::Error for GenevaError {}

impl GenevaError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::InvalidJson(_) => "invalid_json",
            Self::Serialization(_) => "serialization_error",
            Self::NotImplemented(_) => "not_implemented",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GenevaError;

    #[test]
    fn error_codes_match_variant_contract() {
        assert_eq!(
            GenevaError::InvalidInput("bad".to_string()).code(),
            "invalid_input"
        );
        assert_eq!(
            GenevaError::InvalidJson("bad".to_string()).code(),
            "invalid_json"
        );
        assert_eq!(
            GenevaError::Serialization("bad".to_string()).code(),
            "serialization_error"
        );
        assert_eq!(
            GenevaError::NotImplemented("feature").code(),
            "not_implemented"
        );
    }
}

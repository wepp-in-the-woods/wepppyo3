use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenevaError {
    InvalidInput(String),
    NotImplemented(&'static str),
}

impl Display for GenevaError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            Self::NotImplemented(feature) => write!(f, "not implemented: {feature}"),
        }
    }
}

impl std::error::Error for GenevaError {}

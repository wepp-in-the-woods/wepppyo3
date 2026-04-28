use crate::error::GenevaError;

pub const DISTRIBUTION_UNIFORM: &str = "uniform";
pub const DISTRIBUTION_NEH4_TYPE_B: &str = "neh4_type_b";
pub const DISTRIBUTION_TYPE_I: &str = "type_i";
pub const DISTRIBUTION_TYPE_IA: &str = "type_ia";
pub const DISTRIBUTION_TYPE_II: &str = "type_ii";
pub const DISTRIBUTION_TYPE_III: &str = "type_iii";

pub const SUPPORTED_DISTRIBUTION_TYPES: [&str; 6] = [
    DISTRIBUTION_UNIFORM,
    DISTRIBUTION_NEH4_TYPE_B,
    DISTRIBUTION_TYPE_I,
    DISTRIBUTION_TYPE_IA,
    DISTRIBUTION_TYPE_II,
    DISTRIBUTION_TYPE_III,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StormShape {
    Uniform,
    Neh4TypeB,
    TypeI,
    TypeIa,
    TypeII,
    TypeIII,
}

impl StormShape {
    pub fn parse(value: &str) -> Result<Self, GenevaError> {
        match value {
            DISTRIBUTION_UNIFORM => Ok(Self::Uniform),
            DISTRIBUTION_NEH4_TYPE_B => Ok(Self::Neh4TypeB),
            DISTRIBUTION_TYPE_I => Ok(Self::TypeI),
            DISTRIBUTION_TYPE_IA => Ok(Self::TypeIa),
            DISTRIBUTION_TYPE_II => Ok(Self::TypeII),
            DISTRIBUTION_TYPE_III => Ok(Self::TypeIII),
            _ => Err(GenevaError::InvalidInput(format!(
                "distribution_type must be one of: {}",
                SUPPORTED_DISTRIBUTION_TYPES.join(", ")
            ))),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Uniform => DISTRIBUTION_UNIFORM,
            Self::Neh4TypeB => DISTRIBUTION_NEH4_TYPE_B,
            Self::TypeI => DISTRIBUTION_TYPE_I,
            Self::TypeIa => DISTRIBUTION_TYPE_IA,
            Self::TypeII => DISTRIBUTION_TYPE_II,
            Self::TypeIII => DISTRIBUTION_TYPE_III,
        }
    }

    pub fn is_legacy_24h(self) -> bool {
        matches!(
            self,
            Self::TypeI | Self::TypeIa | Self::TypeII | Self::TypeIII
        )
    }
}

pub fn default_distribution_type() -> String {
    DISTRIBUTION_NEH4_TYPE_B.to_string()
}

pub fn validate_distribution_type(value: &str) -> Result<StormShape, GenevaError> {
    StormShape::parse(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_supported_distribution_ids() {
        for distribution_type in SUPPORTED_DISTRIBUTION_TYPES {
            let shape =
                StormShape::parse(distribution_type).expect("supported distribution should parse");
            assert_eq!(shape.id(), distribution_type);
        }
    }

    #[test]
    fn rejects_unknown_distribution_ids() {
        let error = StormShape::parse("type_iv").expect_err("unknown ID should fail");
        assert_eq!(error.code(), "invalid_input");
    }
}

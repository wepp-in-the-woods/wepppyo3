use crate::error::GenevaError;

pub fn build_neh4_type_b_hyetograph(
    _duration_minutes: f64,
    _depth_mm: f64,
) -> Result<(), GenevaError> {
    Err(GenevaError::NotImplemented("build_neh4_type_b_hyetograph"))
}

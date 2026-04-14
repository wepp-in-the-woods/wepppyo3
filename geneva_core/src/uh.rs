use crate::error::GenevaError;

pub fn build_unit_hydrograph(_method: &str, _tc_hours: f64) -> Result<(), GenevaError> {
    Err(GenevaError::NotImplemented("build_unit_hydrograph"))
}

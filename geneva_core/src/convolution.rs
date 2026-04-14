use crate::error::GenevaError;

pub fn convolve_excess_to_hydrograph(
    _excess_path: &str,
    _uh_path: &str,
    _output_path: &str,
) -> Result<(), GenevaError> {
    Err(GenevaError::NotImplemented("convolve_excess_to_hydrograph"))
}

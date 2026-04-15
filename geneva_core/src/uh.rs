use crate::error::GenevaError;
use serde::{Deserialize, Serialize};

const UH_CLOSURE_TOLERANCE: f64 = 0.005;
const FLOAT_TOLERANCE: f64 = 1e-12;
const HF_CONSTANT_SI_MM_KM2_HR_TO_CMS: f64 = 0.208;
const UH_UNIT_SYSTEM_ID: &str = "si_km2_mm_hr_to_cms";
const QP_EQUATION_ID: &str = "qp_hf_a_re_over_tp";
const MAX_TC_HOURS: f64 = 240.0;
const MAX_UH_STEPS: usize = 20_000;

const CURVILINEAR_T_OVER_TP: [f64; 34] = [
    0.0, 0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7,
    1.8, 1.9, 2.0, 2.2, 2.4, 2.6, 2.8, 3.0, 3.2, 3.4, 3.6, 3.8, 4.0, 4.5, 5.0,
];
const CURVILINEAR_Q_OVER_QP: [f64; 34] = [
    0.0, 0.0, 0.03, 0.1, 0.19, 0.31, 0.47, 0.66, 0.82, 0.93, 0.99, 1.0, 0.99, 0.93, 0.86, 0.78,
    0.68, 0.56, 0.46, 0.39, 0.33, 0.28, 0.207, 0.147, 0.107, 0.077, 0.055, 0.04, 0.029, 0.015,
    0.011, 0.005, 0.0, 0.0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum UhMethod {
    #[serde(rename = "scs_triangular")]
    ScsTriangular,
    #[serde(rename = "scs_curvilinear")]
    ScsCurvilinear,
}

impl UhMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScsTriangular => "scs_triangular",
            Self::ScsCurvilinear => "scs_curvilinear",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnitHydrographResponse {
    pub method_id: UhMethod,
    pub time_minutes: Vec<f64>,
    pub unit_ordinates_per_hour: Vec<f64>,
    pub tp_hours: f64,
    pub tb_hours: f64,
    pub dt_minutes: f64,
    pub closure_error: f64,
    pub uh_unit_system: String,
    pub hf_constant: f64,
    pub qp_equation_id: String,
    pub qp_reference_cms: f64,
}

pub fn build_unit_hydrograph(
    method_id: UhMethod,
    tc_hours: f64,
    watershed_area_km2: f64,
    dt_minutes: f64,
) -> Result<UnitHydrographResponse, GenevaError> {
    if !tc_hours.is_finite() || tc_hours <= 0.0 {
        return Err(GenevaError::InvalidInput(
            "tc_hours must be finite and > 0".to_string(),
        ));
    }
    if tc_hours > MAX_TC_HOURS {
        return Err(GenevaError::InvalidInput(format!(
            "tc_hours must be <= {MAX_TC_HOURS}"
        )));
    }
    if !watershed_area_km2.is_finite() || watershed_area_km2 <= 0.0 {
        return Err(GenevaError::InvalidInput(
            "watershed_area_km2 must be finite and > 0".to_string(),
        ));
    }
    if !dt_minutes.is_finite() || dt_minutes <= 0.0 {
        return Err(GenevaError::InvalidInput(
            "dt_minutes must be finite and > 0".to_string(),
        ));
    }

    let tp_hours = 0.6 * tc_hours;
    if tp_hours <= 0.0 {
        return Err(GenevaError::ContractViolation(
            "tp_hours must be > 0 after tc scaling".to_string(),
        ));
    }

    let tb_hours = match method_id {
        UhMethod::ScsTriangular => 2.667 * tp_hours,
        UhMethod::ScsCurvilinear => 5.0 * tp_hours,
    };
    let dt_hours = dt_minutes / 60.0;
    let raw_step_count = (tb_hours / dt_hours).ceil().max(1.0);
    if !raw_step_count.is_finite() {
        return Err(GenevaError::ContractViolation(
            "unit hydrograph step count is non-finite".to_string(),
        ));
    }
    if raw_step_count > ((MAX_UH_STEPS - 1) as f64) {
        return Err(GenevaError::InvalidInput(format!(
            "unit hydrograph discretization exceeds max supported steps ({MAX_UH_STEPS})"
        )));
    }
    let step_count = raw_step_count as usize;

    let mut time_hours = Vec::with_capacity(step_count + 1);
    for idx in 0..=step_count {
        time_hours.push((idx as f64) * dt_hours);
    }

    let mut raw_ordinates = Vec::with_capacity(step_count + 1);
    for time in &time_hours {
        let ordinate = match method_id {
            UhMethod::ScsTriangular => triangular_ordinate(*time, tp_hours, tb_hours),
            UhMethod::ScsCurvilinear => curvilinear_ordinate(*time, tp_hours)?,
        };
        raw_ordinates.push(ordinate.max(0.0));
    }

    let raw_mass = trapezoidal_integral_hours(&time_hours, &raw_ordinates)?;
    if raw_mass <= FLOAT_TOLERANCE {
        return Err(GenevaError::ContractViolation(
            "unit hydrograph ordinate mass must be > 0".to_string(),
        ));
    }

    let unit_ordinates_per_hour: Vec<f64> =
        raw_ordinates.iter().map(|value| value / raw_mass).collect();
    let normalized_mass = trapezoidal_integral_hours(&time_hours, &unit_ordinates_per_hour)?;
    let closure_error = (normalized_mass - 1.0).abs();
    if closure_error > UH_CLOSURE_TOLERANCE {
        return Err(GenevaError::ContractViolation(format!(
            "unit hydrograph mass closure failed with error {closure_error:.6}"
        )));
    }

    let qp_reference_cms = HF_CONSTANT_SI_MM_KM2_HR_TO_CMS * watershed_area_km2 / tp_hours;

    Ok(UnitHydrographResponse {
        method_id,
        time_minutes: time_hours.into_iter().map(|time| time * 60.0).collect(),
        unit_ordinates_per_hour,
        tp_hours,
        tb_hours,
        dt_minutes,
        closure_error,
        uh_unit_system: UH_UNIT_SYSTEM_ID.to_string(),
        hf_constant: HF_CONSTANT_SI_MM_KM2_HR_TO_CMS,
        qp_equation_id: QP_EQUATION_ID.to_string(),
        qp_reference_cms,
    })
}

fn triangular_ordinate(time_hours: f64, tp_hours: f64, tb_hours: f64) -> f64 {
    if time_hours < 0.0 {
        return 0.0;
    }
    if time_hours <= tp_hours {
        return time_hours / tp_hours;
    }
    if time_hours >= tb_hours {
        return 0.0;
    }

    (tb_hours - time_hours) / (tb_hours - tp_hours)
}

fn curvilinear_ordinate(time_hours: f64, tp_hours: f64) -> Result<f64, GenevaError> {
    if tp_hours <= 0.0 {
        return Err(GenevaError::ContractViolation(
            "tp_hours must be > 0 for curvilinear interpolation".to_string(),
        ));
    }
    if time_hours <= 0.0 {
        return Ok(0.0);
    }

    let ratio = time_hours / tp_hours;
    if ratio >= CURVILINEAR_T_OVER_TP[CURVILINEAR_T_OVER_TP.len() - 1] {
        return Ok(0.0);
    }

    for idx in 1..CURVILINEAR_T_OVER_TP.len() {
        let x0 = CURVILINEAR_T_OVER_TP[idx - 1];
        let x1 = CURVILINEAR_T_OVER_TP[idx];
        if ratio < x0 || ratio > x1 {
            continue;
        }
        if (x1 - x0).abs() <= FLOAT_TOLERANCE {
            continue;
        }
        let y0 = CURVILINEAR_Q_OVER_QP[idx - 1];
        let y1 = CURVILINEAR_Q_OVER_QP[idx];
        let local_t = (ratio - x0) / (x1 - x0);
        return Ok(y0 + (local_t * (y1 - y0)));
    }

    Ok(0.0)
}

fn trapezoidal_integral_hours(time_hours: &[f64], values: &[f64]) -> Result<f64, GenevaError> {
    if time_hours.len() != values.len() || time_hours.len() < 2 {
        return Err(GenevaError::ContractViolation(
            "trapezoidal integration requires equal-length vectors with >= 2 points".to_string(),
        ));
    }

    let mut area = 0.0;
    for idx in 1..time_hours.len() {
        let t0 = time_hours[idx - 1];
        let t1 = time_hours[idx];
        let v0 = values[idx - 1];
        let v1 = values[idx];
        if !t0.is_finite() || !t1.is_finite() || !v0.is_finite() || !v1.is_finite() {
            return Err(GenevaError::ContractViolation(
                "trapezoidal integration inputs must be finite".to_string(),
            ));
        }
        if t1 < t0 {
            return Err(GenevaError::ContractViolation(
                "time vector must be non-decreasing".to_string(),
            ));
        }
        area += 0.5 * (v0 + v1) * (t1 - t0);
    }

    Ok(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} but got {actual} with tol {tolerance}"
        );
    }

    #[test]
    fn triangular_relations_and_closure_hold() {
        let tc_hours = 2.0;
        let dt_minutes = (0.6 * tc_hours * 60.0) / 5.0;
        let area_km2 = 2.0;
        let response =
            build_unit_hydrograph(UhMethod::ScsTriangular, tc_hours, area_km2, dt_minutes)
                .expect("triangular UH should build");

        approx_eq(response.tp_hours, 1.2, 1e-12);
        approx_eq(response.tb_hours, 2.667 * response.tp_hours, 1e-12);
        assert!(response.closure_error <= UH_CLOSURE_TOLERANCE);
        assert!(response
            .unit_ordinates_per_hour
            .iter()
            .all(|value| *value >= -FLOAT_TOLERANCE));
        assert_eq!(response.uh_unit_system, UH_UNIT_SYSTEM_ID);
        approx_eq(
            response.qp_reference_cms,
            HF_CONSTANT_SI_MM_KM2_HR_TO_CMS * area_km2 / response.tp_hours,
            1e-12,
        );
        assert_eq!(response.qp_equation_id, QP_EQUATION_ID);
    }

    #[test]
    fn curvilinear_interpolation_and_closure_hold() {
        let response = build_unit_hydrograph(UhMethod::ScsCurvilinear, 2.0, 3.5, 12.0)
            .expect("curvilinear UH should build");
        assert!(response.closure_error <= UH_CLOSURE_TOLERANCE);
        approx_eq(response.tb_hours, 5.0 * response.tp_hours, 1e-12);

        let peak_idx = response
            .unit_ordinates_per_hour
            .iter()
            .enumerate()
            .max_by(|lhs, rhs| lhs.1.total_cmp(rhs.1))
            .expect("curvilinear UH should have at least one ordinate")
            .0;
        let peak_time_hours = response.time_minutes[peak_idx] / 60.0;
        assert!(
            (peak_time_hours - response.tp_hours).abs() <= (response.dt_minutes / 60.0) + 1e-12
        );
        assert!(response
            .unit_ordinates_per_hour
            .iter()
            .all(|value| *value >= -FLOAT_TOLERANCE));
    }

    #[test]
    fn uh_build_is_deterministic() {
        let first =
            build_unit_hydrograph(UhMethod::ScsCurvilinear, 1.8, 1.5, 10.0).expect("first run");
        let second =
            build_unit_hydrograph(UhMethod::ScsCurvilinear, 1.8, 1.5, 10.0).expect("second run");
        assert_eq!(first, second);
    }

    #[test]
    fn rejects_invalid_inputs() {
        let error = build_unit_hydrograph(UhMethod::ScsTriangular, 0.0, 1.0, 10.0)
            .expect_err("non-positive tc should fail");
        assert_eq!(error.code(), "invalid_input");

        let error = build_unit_hydrograph(UhMethod::ScsTriangular, MAX_TC_HOURS + 1.0, 1.0, 10.0)
            .expect_err("overly large tc should fail");
        assert_eq!(error.code(), "invalid_input");

        let error = build_unit_hydrograph(UhMethod::ScsTriangular, 1.0, 0.0, 10.0)
            .expect_err("non-positive area should fail");
        assert_eq!(error.code(), "invalid_input");

        let error = build_unit_hydrograph(UhMethod::ScsTriangular, 1.0, 1.0, 0.0)
            .expect_err("non-positive dt should fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn method_ids_are_strictly_validated() {
        let parsed: Result<UhMethod, _> = serde_json::from_str(r#""invalid""#);
        assert!(parsed.is_err());
    }
}

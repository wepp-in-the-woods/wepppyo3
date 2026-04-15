use crate::error::GenevaError;
use crate::uh::UnitHydrographResponse;
use serde::Serialize;

const FLOAT_TOLERANCE: f64 = 1e-9;
const VOLUME_CLOSURE_RELATIVE_TOLERANCE: f64 = 0.01;
const CMS_TO_CFS: f64 = 35.314_666_721_488_59;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HydrographSeries {
    pub time_minutes: Vec<f64>,
    pub q_cms: Vec<f64>,
    pub q_cfs: Vec<f64>,
    pub runoff_cum_mm: Vec<f64>,
    pub runoff_volume_m3: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HydrographSummary {
    pub peak_discharge: f64,
    pub time_to_peak: f64,
    pub runoff_volume: f64,
    pub runoff_depth: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HydrographDiagnostics {
    pub dt_minutes: f64,
    pub expected_excess_volume_m3: f64,
    pub hydrograph_volume_m3: f64,
    pub volume_closure_relative: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HydrographConvolutionResult {
    pub hydrograph: HydrographSeries,
    pub summary_metrics: HydrographSummary,
    pub diagnostics: HydrographDiagnostics,
}

pub fn convolve_excess_to_hydrograph(
    time_minutes: &[f64],
    excess_incremental_mm: &[f64],
    unit_hydrograph: &UnitHydrographResponse,
    area_m2: f64,
) -> Result<HydrographConvolutionResult, GenevaError> {
    if time_minutes.is_empty() || excess_incremental_mm.is_empty() {
        return Err(GenevaError::InvalidInput(
            "time_minutes and excess_incremental_mm must not be empty".to_string(),
        ));
    }
    if time_minutes.len() != excess_incremental_mm.len() {
        return Err(GenevaError::InvalidInput(
            "time_minutes and excess_incremental_mm lengths must match".to_string(),
        ));
    }
    if time_minutes.len() < 2 {
        return Err(GenevaError::InvalidInput(
            "time_minutes must contain at least two points".to_string(),
        ));
    }
    if !area_m2.is_finite() || area_m2 <= 0.0 {
        return Err(GenevaError::InvalidInput(
            "area_m2 must be finite and > 0".to_string(),
        ));
    }
    if unit_hydrograph.time_minutes.len() != unit_hydrograph.unit_ordinates_per_hour.len()
        || unit_hydrograph.time_minutes.len() < 2
    {
        return Err(GenevaError::ContractViolation(
            "unit hydrograph vectors must have matching lengths >= 2".to_string(),
        ));
    }

    let dt_minutes = infer_uniform_timestep_minutes(time_minutes)?;
    if (dt_minutes - unit_hydrograph.dt_minutes).abs() > FLOAT_TOLERANCE {
        return Err(GenevaError::InvalidInput(
            "unit hydrograph dt_minutes must match excess timestep".to_string(),
        ));
    }
    let dt_hours = dt_minutes / 60.0;

    for value in excess_incremental_mm {
        if !value.is_finite() || *value < -FLOAT_TOLERANCE {
            return Err(GenevaError::InvalidInput(
                "excess_incremental_mm must contain finite values >= 0".to_string(),
            ));
        }
    }

    for value in &unit_hydrograph.unit_ordinates_per_hour {
        if !value.is_finite() || *value < -FLOAT_TOLERANCE {
            return Err(GenevaError::ContractViolation(
                "unit hydrograph ordinates must be finite values >= 0".to_string(),
            ));
        }
    }

    let hydro_len = excess_incremental_mm.len() + unit_hydrograph.unit_ordinates_per_hour.len() - 1;
    let mut runoff_rate_mm_per_hr = vec![0.0; hydro_len];

    for (excess_idx, excess) in excess_incremental_mm.iter().enumerate() {
        for (uh_idx, ordinate) in unit_hydrograph.unit_ordinates_per_hour.iter().enumerate() {
            runoff_rate_mm_per_hr[excess_idx + uh_idx] += excess.max(0.0) * ordinate.max(0.0);
        }
    }

    let mut hydro_time_minutes = Vec::with_capacity(hydro_len);
    let mut q_cms = Vec::with_capacity(hydro_len);
    let mut q_cfs = Vec::with_capacity(hydro_len);
    let mut runoff_cum_mm = Vec::with_capacity(hydro_len);
    let mut runoff_volume_m3 = Vec::with_capacity(hydro_len);

    let mut cumulative_depth_mm = 0.0;
    for (idx, rate_mm_per_hr) in runoff_rate_mm_per_hr.iter().enumerate() {
        hydro_time_minutes.push((idx as f64) * dt_minutes);

        let runoff_inc_mm = rate_mm_per_hr * dt_hours;
        cumulative_depth_mm += runoff_inc_mm;
        runoff_cum_mm.push(cumulative_depth_mm);

        let volume_m3 = (cumulative_depth_mm / 1000.0) * area_m2;
        runoff_volume_m3.push(volume_m3);

        let discharge_cms = area_m2 * (rate_mm_per_hr / 1000.0) / 3600.0;
        q_cms.push(discharge_cms);
        q_cfs.push(discharge_cms * CMS_TO_CFS);
    }

    let peak_pair = q_cms
        .iter()
        .enumerate()
        .max_by(|lhs, rhs| lhs.1.total_cmp(rhs.1))
        .ok_or_else(|| {
            GenevaError::ContractViolation("hydrograph discharge vector is empty".to_string())
        })?;
    let peak_index = peak_pair.0;
    let peak_discharge = *peak_pair.1;
    let time_to_peak = hydro_time_minutes[peak_index];

    let runoff_volume = runoff_volume_m3.last().copied().unwrap_or(0.0);
    let runoff_depth = runoff_cum_mm.last().copied().unwrap_or(0.0);
    let expected_excess_volume_m3 = excess_incremental_mm.iter().sum::<f64>() * area_m2 / 1000.0;

    let volume_closure_relative = if expected_excess_volume_m3.abs() <= FLOAT_TOLERANCE {
        if runoff_volume.abs() <= FLOAT_TOLERANCE {
            0.0
        } else {
            1.0
        }
    } else {
        (runoff_volume - expected_excess_volume_m3).abs() / expected_excess_volume_m3.abs()
    };
    if volume_closure_relative > VOLUME_CLOSURE_RELATIVE_TOLERANCE {
        return Err(GenevaError::ContractViolation(format!(
            "hydrograph volume closure failed with relative error {volume_closure_relative:.6}"
        )));
    }

    Ok(HydrographConvolutionResult {
        hydrograph: HydrographSeries {
            time_minutes: hydro_time_minutes,
            q_cms,
            q_cfs,
            runoff_cum_mm,
            runoff_volume_m3,
        },
        summary_metrics: HydrographSummary {
            peak_discharge,
            time_to_peak,
            runoff_volume,
            runoff_depth,
        },
        diagnostics: HydrographDiagnostics {
            dt_minutes,
            expected_excess_volume_m3,
            hydrograph_volume_m3: runoff_volume,
            volume_closure_relative,
        },
    })
}

fn infer_uniform_timestep_minutes(time_minutes: &[f64]) -> Result<f64, GenevaError> {
    if time_minutes.len() < 2 {
        return Err(GenevaError::InvalidInput(
            "time_minutes must have at least two entries".to_string(),
        ));
    }
    let dt_minutes = time_minutes[1] - time_minutes[0];
    if !dt_minutes.is_finite() || dt_minutes <= 0.0 {
        return Err(GenevaError::InvalidInput(
            "time_minutes must be strictly increasing with finite deltas".to_string(),
        ));
    }

    for idx in 1..time_minutes.len() {
        let prior = time_minutes[idx - 1];
        let current = time_minutes[idx];
        if !prior.is_finite() || !current.is_finite() || current <= prior {
            return Err(GenevaError::InvalidInput(
                "time_minutes must be finite and strictly increasing".to_string(),
            ));
        }
        let delta = current - prior;
        if (delta - dt_minutes).abs() > FLOAT_TOLERANCE {
            return Err(GenevaError::InvalidInput(
                "time_minutes must use a uniform timestep for hydrograph convolution".to_string(),
            ));
        }
    }

    Ok(dt_minutes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uh::{build_unit_hydrograph, UhMethod};

    fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} but got {actual} with tol {tolerance}"
        );
    }

    fn synthetic_unit_hydrograph() -> UnitHydrographResponse {
        UnitHydrographResponse {
            method_id: UhMethod::ScsTriangular,
            time_minutes: vec![0.0, 60.0, 120.0],
            unit_ordinates_per_hour: vec![0.0, 1.0, 0.0],
            tp_hours: 1.0,
            tb_hours: 2.0,
            dt_minutes: 60.0,
            closure_error: 0.0,
            uh_unit_system: "si_km2_mm_hr_to_cms".to_string(),
            hf_constant: 0.208,
            qp_equation_id: "qp_hf_a_re_over_tp".to_string(),
            qp_reference_cms: 0.208,
        }
    }

    #[test]
    fn fixed_vector_regression_matches_expected_metrics() {
        let area_m2 = 1_000_000.0;
        let result = convolve_excess_to_hydrograph(
            &[0.0, 60.0],
            &[10.0, 20.0],
            &synthetic_unit_hydrograph(),
            area_m2,
        )
        .expect("convolution should succeed");

        assert_eq!(
            result.hydrograph.time_minutes,
            vec![0.0, 60.0, 120.0, 180.0]
        );
        approx_eq(result.hydrograph.q_cms[1], 2.777_777_777_777_777_7, 1e-12);
        approx_eq(result.hydrograph.q_cms[2], 5.555_555_555_555_555, 1e-12);
        approx_eq(
            result.summary_metrics.peak_discharge,
            5.555_555_555_555_555,
            1e-12,
        );
        approx_eq(result.summary_metrics.time_to_peak, 120.0, 1e-12);
        approx_eq(result.summary_metrics.runoff_depth, 30.0, 1e-9);
        approx_eq(result.summary_metrics.runoff_volume, 30_000.0, 1e-6);
        assert!(result.diagnostics.volume_closure_relative <= VOLUME_CLOSURE_RELATIVE_TOLERANCE);
    }

    #[test]
    fn convolution_is_deterministic_and_volume_closed_with_kernel_uh() {
        let unit = build_unit_hydrograph(UhMethod::ScsCurvilinear, 1.2, 1.0, 10.0)
            .expect("unit hydrograph should build");
        let time_minutes = vec![0.0, 10.0, 20.0, 30.0, 40.0];
        let excess_mm = vec![0.0, 0.4, 1.0, 0.3, 0.0];

        let first = convolve_excess_to_hydrograph(&time_minutes, &excess_mm, &unit, 1_000_000.0)
            .expect("first run");
        let second = convolve_excess_to_hydrograph(&time_minutes, &excess_mm, &unit, 1_000_000.0)
            .expect("second run");
        assert_eq!(first, second);
        assert!(first.diagnostics.volume_closure_relative <= VOLUME_CLOSURE_RELATIVE_TOLERANCE);
    }

    #[test]
    fn rejects_invalid_input_and_closure_failures() {
        let unit = synthetic_unit_hydrograph();
        let error = convolve_excess_to_hydrograph(&[0.0], &[1.0], &unit, 1000.0)
            .expect_err("single-step grid should fail");
        assert_eq!(error.code(), "invalid_input");

        let error = convolve_excess_to_hydrograph(&[0.0, 10.0], &[1.0], &unit, 1000.0)
            .expect_err("length mismatch should fail");
        assert_eq!(error.code(), "invalid_input");

        let error =
            convolve_excess_to_hydrograph(&[0.0, 10.0, 21.0], &[1.0, 0.5, 0.2], &unit, 1000.0)
                .expect_err("non-uniform timesteps should fail");
        assert_eq!(error.code(), "invalid_input");

        let error = convolve_excess_to_hydrograph(&[0.0, 60.0], &[1.0, -0.1], &unit, 1000.0)
            .expect_err("negative excess should fail");
        assert_eq!(error.code(), "invalid_input");

        let mut non_closing_unit = unit.clone();
        non_closing_unit.unit_ordinates_per_hour = vec![0.0, 2.0, 0.0];
        let error = convolve_excess_to_hydrograph(
            &[0.0, 60.0],
            &[10.0, 20.0],
            &non_closing_unit,
            1_000_000.0,
        )
        .expect_err("closure violation should fail");
        assert_eq!(error.code(), "contract_violation");
    }
}

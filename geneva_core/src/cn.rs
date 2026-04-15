use crate::error::GenevaError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const FLOAT_TOLERANCE: f64 = 1e-9;
const CLOSURE_TOLERANCE_MM: f64 = 1e-6;
const CN_LAMBDA_005_CAP_THRESHOLD: f64 = 98.5;
const RUN_BATCH_KERNEL_SCHEMA_VERSION: u32 = 1;
const MAX_STORM_ID_LEN: usize = 128;
const MAX_HRU_ID_LEN: usize = 128;
const MAX_TIME_STEPS: usize = 20_000;
const MAX_HRU_ROWS: usize = 25_000;
const MAX_HRU_TIMESTEP_POINTS: usize = 5_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum LambdaMode {
    #[serde(rename = "0.20")]
    Lambda020,
    #[serde(rename = "0.05")]
    Lambda005,
}

impl LambdaMode {
    fn ia_ratio(self) -> f64 {
        match self {
            Self::Lambda020 => 0.20,
            Self::Lambda005 => 0.05,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RunBatchRequest {
    #[serde(alias = "schema_version")]
    pub kernel_schema_version: u32,
    pub storm_id: String,
    pub lambda_mode: LambdaMode,
    pub time_minutes: Vec<f64>,
    pub cumulative_rainfall_mm: Vec<f64>,
    pub hru_rows: Vec<CnHruInput>,
}

impl RunBatchRequest {
    pub fn from_payload_json(payload_json: &str) -> Result<Self, GenevaError> {
        if payload_json.trim().is_empty() {
            return Err(GenevaError::InvalidInput(
                "payload_json must not be empty".to_string(),
            ));
        }

        let request: Self = serde_json::from_str(payload_json)
            .map_err(|err| GenevaError::InvalidJson(err.to_string()))?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), GenevaError> {
        if self.kernel_schema_version != RUN_BATCH_KERNEL_SCHEMA_VERSION {
            return Err(GenevaError::InvalidInput(format!(
                "kernel_schema_version must equal {RUN_BATCH_KERNEL_SCHEMA_VERSION}"
            )));
        }
        if self.storm_id.trim().is_empty() {
            return Err(GenevaError::InvalidInput(
                "storm_id must not be empty".to_string(),
            ));
        }
        if self.storm_id.len() > MAX_STORM_ID_LEN {
            return Err(GenevaError::InvalidInput(format!(
                "storm_id length must be <= {MAX_STORM_ID_LEN}"
            )));
        }
        if self.time_minutes.is_empty() {
            return Err(GenevaError::InvalidInput(
                "time_minutes must not be empty".to_string(),
            ));
        }
        if self.time_minutes.len() > MAX_TIME_STEPS {
            return Err(GenevaError::InvalidInput(format!(
                "time_minutes length must be <= {MAX_TIME_STEPS}"
            )));
        }
        if self.time_minutes.len() != self.cumulative_rainfall_mm.len() {
            return Err(GenevaError::InvalidInput(
                "time_minutes and cumulative_rainfall_mm lengths must match".to_string(),
            ));
        }
        if self.hru_rows.is_empty() {
            return Err(GenevaError::InvalidInput(
                "hru_rows must contain at least one HRU".to_string(),
            ));
        }
        if self.hru_rows.len() > MAX_HRU_ROWS {
            return Err(GenevaError::InvalidInput(format!(
                "hru_rows length must be <= {MAX_HRU_ROWS}"
            )));
        }
        let total_points = self
            .time_minutes
            .len()
            .checked_mul(self.hru_rows.len())
            .ok_or_else(|| {
                GenevaError::InvalidInput(
                    "run-batch dimensions overflowed during validation".to_string(),
                )
            })?;
        if total_points > MAX_HRU_TIMESTEP_POINTS {
            return Err(GenevaError::InvalidInput(format!(
                "hru_rows * time_minutes must be <= {MAX_HRU_TIMESTEP_POINTS}"
            )));
        }

        let mut prior_time: Option<f64> = None;
        for time in &self.time_minutes {
            if !time.is_finite() {
                return Err(GenevaError::InvalidInput(
                    "time_minutes must contain finite values".to_string(),
                ));
            }
            if *time < 0.0 {
                return Err(GenevaError::InvalidInput(
                    "time_minutes must be >= 0".to_string(),
                ));
            }
            if let Some(previous) = prior_time {
                if *time <= previous {
                    return Err(GenevaError::InvalidInput(
                        "time_minutes must be strictly increasing".to_string(),
                    ));
                }
            }
            prior_time = Some(*time);
        }

        let mut prior_cumulative: Option<f64> = None;
        for cumulative in &self.cumulative_rainfall_mm {
            if !cumulative.is_finite() {
                return Err(GenevaError::InvalidInput(
                    "cumulative_rainfall_mm must contain finite values".to_string(),
                ));
            }
            if *cumulative < 0.0 {
                return Err(GenevaError::InvalidInput(
                    "cumulative_rainfall_mm must not contain negative depths".to_string(),
                ));
            }
            if let Some(previous) = prior_cumulative {
                if *cumulative + FLOAT_TOLERANCE < previous {
                    return Err(GenevaError::InvalidInput(
                        "cumulative_rainfall_mm must be non-decreasing".to_string(),
                    ));
                }
            }
            prior_cumulative = Some(*cumulative);
        }

        let mut ids = BTreeSet::new();
        for row in &self.hru_rows {
            if row.hru_id.trim().is_empty() {
                return Err(GenevaError::InvalidInput(
                    "hru_rows[].hru_id must not be empty".to_string(),
                ));
            }
            if row.hru_id.len() > MAX_HRU_ID_LEN {
                return Err(GenevaError::InvalidInput(format!(
                    "hru_rows[].hru_id length must be <= {MAX_HRU_ID_LEN}"
                )));
            }
            if !ids.insert(row.hru_id.clone()) {
                return Err(GenevaError::InvalidInput(format!(
                    "duplicate hru_id '{}'",
                    row.hru_id
                )));
            }
            if !row.area_m2.is_finite() || row.area_m2 <= 0.0 {
                return Err(GenevaError::InvalidInput(format!(
                    "hru '{}' area_m2 must be > 0",
                    row.hru_id
                )));
            }
            validate_cn_domain(row.cn_lambda_020, "cn_lambda_020", &row.hru_id)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CnHruInput {
    pub hru_id: String,
    pub area_m2: f64,
    #[serde(alias = "cn_arc_ii", alias = "cn")]
    pub cn_lambda_020: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunBatchResponse {
    pub status: String,
    pub phase: String,
    pub kernel_schema_version: u32,
    pub storm_id: String,
    pub lambda_mode: LambdaMode,
    pub time_minutes: Vec<f64>,
    pub cumulative_rainfall_mm: Vec<f64>,
    pub incremental_rainfall_mm: Vec<f64>,
    pub hru_excess: Vec<HruExcessSeries>,
    pub composite_excess: CompositeExcessSeries,
    pub diagnostics: CnDiagnostics,
    pub warnings: Vec<CnWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HruExcessSeries {
    pub hru_id: String,
    pub area_m2: f64,
    pub area_fraction: f64,
    pub cn_lambda_020: f64,
    pub cn_lambda_005: f64,
    pub selected_cn: f64,
    pub storage_mm: f64,
    pub initial_abstraction_mm: f64,
    pub cumulative_excess_mm: Vec<f64>,
    pub incremental_excess_mm: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompositeExcessSeries {
    pub cumulative_excess_mm: Vec<f64>,
    pub incremental_excess_mm: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CnDiagnostics {
    pub total_area_m2: f64,
    pub closure_error_mm: f64,
    pub final_cumulative_excess_mm: f64,
    pub incremental_sum_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CnWarning {
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CnTransformPoint {
    pub selected_cn: f64,
    pub storage_mm: f64,
    pub initial_abstraction_mm: f64,
    pub cumulative_excess_mm: f64,
}

pub fn serialize_run_batch_response(response: &RunBatchResponse) -> Result<String, GenevaError> {
    serde_json::to_string(response).map_err(|err| GenevaError::Serialization(err.to_string()))
}

pub fn cn_lambda_005_from_cn_020(cn_lambda_020: f64) -> Result<f64, GenevaError> {
    validate_cn_domain(cn_lambda_020, "cn_lambda_020", "cn_transform")?;

    if cn_lambda_020 >= (100.0 - FLOAT_TOLERANCE) {
        return Ok(100.0);
    }
    if cn_lambda_020 > CN_LAMBDA_005_CAP_THRESHOLD {
        return Ok(cn_lambda_020);
    }

    let term = (100.0 / cn_lambda_020) - 1.0;
    let denominator = (1.879 * term.powf(1.15)) + 1.0;
    Ok((100.0 / denominator).clamp(0.0, 100.0))
}

pub fn compute_cn_transform_point(
    cn_lambda_020: f64,
    cumulative_rainfall_mm: f64,
    lambda_mode: LambdaMode,
) -> Result<CnTransformPoint, GenevaError> {
    validate_cn_domain(cn_lambda_020, "cn_lambda_020", "cn_transform")?;
    if !cumulative_rainfall_mm.is_finite() || cumulative_rainfall_mm < 0.0 {
        return Err(GenevaError::InvalidInput(
            "cumulative_rainfall_mm must be finite and >= 0".to_string(),
        ));
    }

    let selected_cn = match lambda_mode {
        LambdaMode::Lambda020 => cn_lambda_020,
        LambdaMode::Lambda005 => cn_lambda_005_from_cn_020(cn_lambda_020)?,
    };
    let storage_mm = storage_mm_from_cn(selected_cn)?;
    let ia_ratio = lambda_mode.ia_ratio();
    let initial_abstraction_mm = ia_ratio * storage_mm;
    let cumulative_excess_mm = cumulative_excess_depth_mm(
        cumulative_rainfall_mm,
        storage_mm,
        initial_abstraction_mm,
        ia_ratio,
    );

    Ok(CnTransformPoint {
        selected_cn,
        storage_mm,
        initial_abstraction_mm,
        cumulative_excess_mm,
    })
}

pub fn run_batch_cn_excess(request: &RunBatchRequest) -> Result<RunBatchResponse, GenevaError> {
    request.validate()?;

    let mut ordered_hrus = request.hru_rows.clone();
    ordered_hrus.sort_by(|lhs, rhs| lhs.hru_id.cmp(&rhs.hru_id));

    let total_area_m2: f64 = ordered_hrus.iter().map(|row| row.area_m2).sum();
    if !total_area_m2.is_finite() || total_area_m2 <= 0.0 {
        return Err(GenevaError::ContractViolation(
            "total HRU area must be finite and > 0".to_string(),
        ));
    }

    let mut incremental_rainfall_mm = Vec::with_capacity(request.cumulative_rainfall_mm.len());
    let mut prior_rainfall = 0.0;
    for rainfall in &request.cumulative_rainfall_mm {
        let mut delta = *rainfall - prior_rainfall;
        if delta < -FLOAT_TOLERANCE {
            return Err(GenevaError::InvalidInput(
                "cumulative_rainfall_mm must be non-decreasing".to_string(),
            ));
        }
        if delta < 0.0 {
            delta = 0.0;
        }
        incremental_rainfall_mm.push(delta);
        prior_rainfall = *rainfall;
    }

    let mut composite_incremental_excess_mm = vec![0.0; request.cumulative_rainfall_mm.len()];
    let mut hru_excess: Vec<HruExcessSeries> = Vec::with_capacity(ordered_hrus.len());

    for row in ordered_hrus {
        let cn_lambda_005 = cn_lambda_005_from_cn_020(row.cn_lambda_020)?;
        let transform = compute_cn_transform_point(row.cn_lambda_020, 0.0, request.lambda_mode)?;
        let area_fraction = row.area_m2 / total_area_m2;

        let mut cumulative_excess_mm = Vec::with_capacity(request.cumulative_rainfall_mm.len());
        let mut incremental_excess_mm = Vec::with_capacity(request.cumulative_rainfall_mm.len());
        let mut previous_q = 0.0;
        for rainfall in &request.cumulative_rainfall_mm {
            let point =
                compute_cn_transform_point(row.cn_lambda_020, *rainfall, request.lambda_mode)?;
            let q = point.cumulative_excess_mm;
            let mut delta_q = q - previous_q;
            if delta_q < -FLOAT_TOLERANCE {
                return Err(GenevaError::ContractViolation(format!(
                    "cumulative excess decreased for hru '{}'",
                    row.hru_id
                )));
            }
            if delta_q < 0.0 {
                delta_q = 0.0;
            }
            cumulative_excess_mm.push(q);
            incremental_excess_mm.push(delta_q);
            previous_q = q;
        }

        let final_q = cumulative_excess_mm.last().copied().unwrap_or(0.0);
        let incremental_sum = incremental_excess_mm.iter().sum::<f64>();
        if (incremental_sum - final_q).abs() > CLOSURE_TOLERANCE_MM {
            return Err(GenevaError::ContractViolation(format!(
                "incremental closure failed for hru '{}'",
                row.hru_id
            )));
        }

        for (idx, delta_q) in incremental_excess_mm.iter().enumerate() {
            composite_incremental_excess_mm[idx] += *delta_q * area_fraction;
        }

        hru_excess.push(HruExcessSeries {
            hru_id: row.hru_id,
            area_m2: row.area_m2,
            area_fraction,
            cn_lambda_020: row.cn_lambda_020,
            cn_lambda_005,
            selected_cn: transform.selected_cn,
            storage_mm: transform.storage_mm,
            initial_abstraction_mm: transform.initial_abstraction_mm,
            cumulative_excess_mm,
            incremental_excess_mm,
        });
    }

    let mut composite_cumulative_excess_mm =
        Vec::with_capacity(composite_incremental_excess_mm.len());
    let mut running_total = 0.0;
    for step in &composite_incremental_excess_mm {
        running_total += *step;
        composite_cumulative_excess_mm.push(running_total);
    }

    let final_cumulative_excess_mm = composite_cumulative_excess_mm
        .last()
        .copied()
        .unwrap_or(0.0);
    let incremental_sum_mm = composite_incremental_excess_mm.iter().sum::<f64>();
    let closure_error_mm = (incremental_sum_mm - final_cumulative_excess_mm).abs();

    if closure_error_mm > CLOSURE_TOLERANCE_MM {
        return Err(GenevaError::ContractViolation(
            "composite incremental/cumulative closure failed".to_string(),
        ));
    }

    Ok(RunBatchResponse {
        status: "ok".to_string(),
        phase: "run_batch".to_string(),
        kernel_schema_version: request.kernel_schema_version,
        storm_id: request.storm_id.clone(),
        lambda_mode: request.lambda_mode,
        time_minutes: request.time_minutes.clone(),
        cumulative_rainfall_mm: request.cumulative_rainfall_mm.clone(),
        incremental_rainfall_mm,
        hru_excess,
        composite_excess: CompositeExcessSeries {
            cumulative_excess_mm: composite_cumulative_excess_mm,
            incremental_excess_mm: composite_incremental_excess_mm,
        },
        diagnostics: CnDiagnostics {
            total_area_m2,
            closure_error_mm,
            final_cumulative_excess_mm,
            incremental_sum_mm,
        },
        warnings: Vec::new(),
    })
}

fn validate_cn_domain(cn: f64, field: &str, hru_id: &str) -> Result<(), GenevaError> {
    if !cn.is_finite() || cn <= 0.0 || cn > 100.0 {
        return Err(GenevaError::InvalidInput(format!(
            "hru '{hru_id}' {field} must be in (0, 100]"
        )));
    }
    Ok(())
}

fn storage_mm_from_cn(cn: f64) -> Result<f64, GenevaError> {
    validate_cn_domain(cn, "cn", "cn_transform")?;
    Ok(((25_400.0 / cn) - 254.0).max(0.0))
}

fn cumulative_excess_depth_mm(
    cumulative_rainfall_mm: f64,
    storage_mm: f64,
    initial_abstraction_mm: f64,
    ia_ratio: f64,
) -> f64 {
    if cumulative_rainfall_mm <= initial_abstraction_mm {
        return 0.0;
    }

    let numerator = (cumulative_rainfall_mm - initial_abstraction_mm).powi(2);
    let denominator = cumulative_rainfall_mm + ((1.0 - ia_ratio) * storage_mm);
    if denominator <= 0.0 {
        return 0.0;
    }
    (numerator / denominator).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} but got {actual} with tol {tolerance}"
        );
    }

    fn valid_payload() -> &'static str {
        r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_01",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 10.0, 20.0, 30.0],
            "cumulative_rainfall_mm": [0.0, 5.0, 15.0, 35.0],
            "hru_rows": [
                {"hru_id": "hru_b", "area_m2": 2000.0, "cn_lambda_020": 82.0},
                {"hru_id": "hru_a", "area_m2": 1000.0, "cn_lambda_020": 75.0}
            ]
        }"#
    }

    fn valid_request() -> RunBatchRequest {
        RunBatchRequest::from_payload_json(valid_payload()).expect("valid payload must parse")
    }

    #[test]
    fn scalar_parity_lambda_020_matches_vector() {
        let point = compute_cn_transform_point(75.0, 60.0, LambdaMode::Lambda020)
            .expect("transform should compute");
        approx_eq(point.storage_mm, 84.666_666_666_666_69, 1e-6);
        approx_eq(point.initial_abstraction_mm, 16.933_333_333_333_337, 1e-6);
        approx_eq(point.cumulative_excess_mm, 14.520_389_700_765_48, 1e-6);
    }

    #[test]
    fn scalar_parity_lambda_005_matches_vector() {
        let point = compute_cn_transform_point(82.0, 45.0, LambdaMode::Lambda005)
            .expect("transform should compute");
        approx_eq(point.selected_cn, 75.269_912_974_994_57, 1e-6);
        approx_eq(point.storage_mm, 83.452_230_195_06, 1e-6);
        approx_eq(point.initial_abstraction_mm, 4.172_611_509_753, 1e-6);
        approx_eq(point.cumulative_excess_mm, 13.412_300_975_547_01, 1e-6);
    }

    #[test]
    fn cn_lambda_005_cap_preserves_high_cn_values() {
        let converted = cn_lambda_005_from_cn_020(99.0).expect("cn conversion should succeed");
        approx_eq(converted, 99.0, 1e-12);
    }

    #[test]
    fn cn_lambda_005_near_hundred_snaps_to_hundred() {
        let converted = cn_lambda_005_from_cn_020(100.0 - (0.5 * FLOAT_TOLERANCE))
            .expect("near-100 cn conversion should succeed");
        approx_eq(converted, 100.0, 1e-12);
    }

    #[test]
    fn cumulative_to_incremental_excess_is_non_negative_and_closes() {
        let request =
            RunBatchRequest::from_payload_json(valid_payload()).expect("valid payload must parse");
        let response = run_batch_cn_excess(&request).expect("run_batch should succeed");

        for hru in &response.hru_excess {
            assert!(hru.incremental_excess_mm.iter().all(|value| *value >= 0.0));
            let sum_incremental = hru.incremental_excess_mm.iter().sum::<f64>();
            let final_cumulative = hru.cumulative_excess_mm.last().copied().unwrap_or(0.0);
            approx_eq(sum_incremental, final_cumulative, 1e-6);
        }
    }

    #[test]
    fn composite_excess_is_area_weighted_and_closes() {
        let request =
            RunBatchRequest::from_payload_json(valid_payload()).expect("valid payload must parse");
        let response = run_batch_cn_excess(&request).expect("run_batch should succeed");
        let total_area: f64 = response.hru_excess.iter().map(|row| row.area_m2).sum();

        for step in 0..response.time_minutes.len() {
            let weighted = response
                .hru_excess
                .iter()
                .map(|hru| hru.incremental_excess_mm[step] * (hru.area_m2 / total_area))
                .sum::<f64>();
            approx_eq(
                weighted,
                response.composite_excess.incremental_excess_mm[step],
                1e-9,
            );
        }

        approx_eq(
            response.diagnostics.incremental_sum_mm,
            response.diagnostics.final_cumulative_excess_mm,
            1e-6,
        );
    }

    #[test]
    fn run_batch_is_deterministic_for_identical_inputs() {
        let request = valid_request();
        let first = run_batch_cn_excess(&request).expect("first run should succeed");
        let second = run_batch_cn_excess(&request).expect("second run should succeed");
        assert_eq!(first, second);
        assert_eq!(first.hru_excess[0].hru_id, "hru_a");
        assert_eq!(first.hru_excess[1].hru_id, "hru_b");
    }

    #[test]
    fn rejects_negative_rainfall_depth() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_bad",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, -0.1],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error =
            RunBatchRequest::from_payload_json(payload).expect_err("negative rainfall must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn rejects_non_increasing_timestep_order() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_bad",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 30.0, 20.0],
            "cumulative_rainfall_mm": [0.0, 5.0, 10.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 75.0}]
        }"#;
        let error =
            RunBatchRequest::from_payload_json(payload).expect_err("unordered time must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn rejects_invalid_cn_domain_values() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_bad",
            "lambda_mode": "0.20",
            "time_minutes": [0.0, 10.0],
            "cumulative_rainfall_mm": [0.0, 5.0],
            "hru_rows": [{"hru_id": "hru_1", "area_m2": 1000.0, "cn_lambda_020": 120.0}]
        }"#;
        let error = RunBatchRequest::from_payload_json(payload).expect_err("invalid cn must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn run_batch_lambda_005_path_covers_cap_behavior() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "storm_id": "storm_lambda_005",
            "lambda_mode": "0.05",
            "time_minutes": [0.0, 10.0, 20.0],
            "cumulative_rainfall_mm": [0.0, 5.0, 40.0],
            "hru_rows": [
                {"hru_id": "hru_cap", "area_m2": 1000.0, "cn_lambda_020": 99.0},
                {"hru_id": "hru_norm", "area_m2": 1000.0, "cn_lambda_020": 82.0}
            ]
        }"#;
        let request = RunBatchRequest::from_payload_json(payload).expect("payload should parse");
        let response = run_batch_cn_excess(&request).expect("run_batch should succeed");

        let capped = response
            .hru_excess
            .iter()
            .find(|row| row.hru_id == "hru_cap")
            .expect("capped hru should exist");
        approx_eq(capped.cn_lambda_005, 99.0, 1e-12);
        approx_eq(capped.selected_cn, 99.0, 1e-12);
        approx_eq(
            response.diagnostics.incremental_sum_mm,
            response.diagnostics.final_cumulative_excess_mm,
            1e-6,
        );
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let mut request = valid_request();
        request.kernel_schema_version = 2;
        let error = request
            .validate()
            .expect_err("unsupported schema version should fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn rejects_negative_time_minutes() {
        let mut request = valid_request();
        request.time_minutes = vec![-1.0, 10.0, 20.0, 30.0];
        let error = request
            .validate()
            .expect_err("negative time should be rejected");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn accepts_epsilon_cumulative_dips_and_clamps_to_zero_increment() {
        let mut request = valid_request();
        request.time_minutes = vec![0.0, 10.0, 20.0, 30.0];
        request.cumulative_rainfall_mm = vec![0.0, 10.0, 10.0 - (0.5 * FLOAT_TOLERANCE), 20.0];
        request.validate().expect("epsilon dip should be tolerated");
        let response = run_batch_cn_excess(&request).expect("run_batch should succeed");
        approx_eq(response.incremental_rainfall_mm[2], 0.0, 1e-12);
    }

    #[test]
    fn rejects_excessive_hru_timestep_product() {
        let mut request = valid_request();
        request.time_minutes = (0..=1000).map(|idx| idx as f64).collect();
        request.cumulative_rainfall_mm = request.time_minutes.clone();
        request.hru_rows = iter::repeat_n(
            CnHruInput {
                hru_id: "hru_x".to_string(),
                area_m2: 1000.0,
                cn_lambda_020: 75.0,
            },
            5_000,
        )
        .enumerate()
        .map(|(idx, mut row)| {
            row.hru_id = format!("hru_{idx}");
            row
        })
        .collect();
        let error = request
            .validate()
            .expect_err("oversized workload should fail validation");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn rejects_overlong_identifiers() {
        let mut request = valid_request();
        request.storm_id = "s".repeat(MAX_STORM_ID_LEN + 1);
        let error = request
            .validate()
            .expect_err("overlong storm_id should fail validation");
        assert_eq!(error.code(), "invalid_input");

        let mut request = valid_request();
        request.hru_rows[0].hru_id = "h".repeat(MAX_HRU_ID_LEN + 1);
        let error = request
            .validate()
            .expect_err("overlong hru_id should fail validation");
        assert_eq!(error.code(), "invalid_input");
    }
}

use crate::error::GenevaError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path};

const HYETOGRAPH_KERNEL_SCHEMA_VERSION: u32 = 1;
const HYETOGRAPH_FLOAT_TOLERANCE: f64 = 1e-9;
const MAX_TIMESTEPS: usize = 250_000;
const MAX_ORDINATE_PATH_LEN: usize = 1024;
const SHORT_DURATION_WARNING_THRESHOLD_MINUTES: f64 = 30.0;
const DISTRIBUTION_NEH4_TYPE_B: &str = "neh4_type_b";

const NEH4_T_STAR: [f64; 14] = [
    0.0,
    0.0,
    0.083_333_333_333_333_33,
    0.166_666_666_666_666_66,
    0.25,
    0.333_333_333_333_333_3,
    0.416_666_666_666_666_7,
    0.5,
    0.583_333_333_333_333_4,
    0.666_666_666_666_666_6,
    0.75,
    0.833_333_333_333_333_4,
    0.916_666_666_666_666_6,
    1.0,
];
const NEH4_P_STAR: [f64; 14] = [
    0.0, 0.0, 0.035, 0.08, 0.135, 0.23, 0.6, 0.7, 0.78, 0.835, 0.885, 0.925, 0.96, 1.0,
];

#[derive(Debug, Clone, Deserialize)]
pub struct Neh4TypeBHyetographRequest {
    #[serde(alias = "schema_version")]
    pub kernel_schema_version: u32,
    pub duration_minutes: f64,
    pub depth_mm: f64,
    pub time_step_minutes: f64,
    #[serde(default = "default_distribution_type")]
    pub distribution_type: String,
    #[serde(default)]
    pub ordinate_csv_path: Option<String>,
}

impl Neh4TypeBHyetographRequest {
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
        if self.kernel_schema_version != HYETOGRAPH_KERNEL_SCHEMA_VERSION {
            return Err(GenevaError::InvalidInput(format!(
                "kernel_schema_version must equal {HYETOGRAPH_KERNEL_SCHEMA_VERSION}"
            )));
        }
        if self.distribution_type != DISTRIBUTION_NEH4_TYPE_B {
            return Err(GenevaError::InvalidInput(format!(
                "distribution_type must equal '{DISTRIBUTION_NEH4_TYPE_B}' in v1"
            )));
        }
        if !self.duration_minutes.is_finite() || self.duration_minutes <= 0.0 {
            return Err(GenevaError::InvalidInput(
                "duration_minutes must be finite and > 0".to_string(),
            ));
        }
        if !self.depth_mm.is_finite() || self.depth_mm <= 0.0 {
            return Err(GenevaError::InvalidInput(
                "depth_mm must be finite and > 0".to_string(),
            ));
        }
        if !self.time_step_minutes.is_finite() || self.time_step_minutes <= 0.0 {
            return Err(GenevaError::InvalidInput(
                "time_step_minutes must be finite and > 0".to_string(),
            ));
        }

        let expected_points = (self.duration_minutes / self.time_step_minutes).ceil();
        if !expected_points.is_finite() {
            return Err(GenevaError::InvalidInput(
                "duration/time_step produced non-finite timestep count".to_string(),
            ));
        }
        if expected_points > (MAX_TIMESTEPS as f64) {
            return Err(GenevaError::InvalidInput(format!(
                "duration_minutes / time_step_minutes must yield <= {MAX_TIMESTEPS} points"
            )));
        }

        if let Some(path) = self.ordinate_csv_path.as_deref() {
            validate_ordinate_path(path)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Neh4TypeBHyetographResponse {
    pub status: String,
    pub phase: String,
    pub kernel_schema_version: u32,
    pub distribution_type: String,
    pub duration_minutes: f64,
    pub depth_mm: f64,
    pub time_step_minutes: f64,
    pub time_minutes: Vec<f64>,
    pub cumulative_rainfall_mm: Vec<f64>,
    pub incremental_rainfall_mm: Vec<f64>,
    pub intensity_mm_per_hr: Vec<f64>,
    pub warnings: Vec<HyetographWarning>,
    pub diagnostics: HyetographDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HyetographWarning {
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HyetographDiagnostics {
    pub closure_error_mm: f64,
    pub closure_tolerance_mm: f64,
    pub cumulative_monotonic: bool,
}

pub fn build_neh4_type_b_hyetograph(
    duration_minutes: f64,
    depth_mm: f64,
    time_step_minutes: f64,
) -> Result<Neh4TypeBHyetographResponse, GenevaError> {
    let request = Neh4TypeBHyetographRequest {
        kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
        duration_minutes,
        depth_mm,
        time_step_minutes,
        distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
        ordinate_csv_path: None,
    };
    build_neh4_type_b_hyetograph_from_request(&request)
}

pub fn build_neh4_type_b_hyetograph_from_request(
    request: &Neh4TypeBHyetographRequest,
) -> Result<Neh4TypeBHyetographResponse, GenevaError> {
    request.validate()?;

    let ordinate_pairs = load_dimensionless_ordinates(request.ordinate_csv_path.as_deref())?;
    let scaled_ordinates =
        scale_ordinates(&ordinate_pairs, request.duration_minutes, request.depth_mm);
    let time_minutes = build_time_grid(request.duration_minutes, request.time_step_minutes)?;

    let mut cumulative_rainfall_mm = Vec::with_capacity(time_minutes.len());
    for time_minutes_value in &time_minutes {
        cumulative_rainfall_mm.push(interpolate_cumulative_depth(
            *time_minutes_value,
            &scaled_ordinates,
            request.duration_minutes,
        )?);
    }

    if let Some(first) = cumulative_rainfall_mm.first_mut() {
        *first = 0.0;
    }
    if let Some(last) = cumulative_rainfall_mm.last_mut() {
        *last = request.depth_mm;
    }
    let cumulative_monotonic = enforce_monotonic(&mut cumulative_rainfall_mm)?;

    let mut incremental_rainfall_mm = vec![0.0; time_minutes.len()];
    let mut intensity_mm_per_hr = vec![0.0; time_minutes.len()];
    for idx in 1..time_minutes.len() {
        let dt_minutes = time_minutes[idx] - time_minutes[idx - 1];
        if dt_minutes <= HYETOGRAPH_FLOAT_TOLERANCE {
            return Err(GenevaError::ContractViolation(
                "timestep grid must be strictly increasing".to_string(),
            ));
        }

        let mut delta_depth = cumulative_rainfall_mm[idx] - cumulative_rainfall_mm[idx - 1];
        if delta_depth < -HYETOGRAPH_FLOAT_TOLERANCE {
            return Err(GenevaError::ContractViolation(
                "cumulative rainfall must be monotonic non-decreasing".to_string(),
            ));
        }
        if delta_depth < 0.0 {
            delta_depth = 0.0;
        }

        incremental_rainfall_mm[idx] = delta_depth;
        intensity_mm_per_hr[idx] = delta_depth / (dt_minutes / 60.0);
    }

    let mut sum_incremental_mm = incremental_rainfall_mm.iter().sum::<f64>();
    let closure_adjustment_mm = request.depth_mm - sum_incremental_mm;
    if closure_adjustment_mm.abs() > HYETOGRAPH_FLOAT_TOLERANCE {
        let last_idx = incremental_rainfall_mm.len() - 1;
        let adjusted_last = incremental_rainfall_mm[last_idx] + closure_adjustment_mm;
        if adjusted_last < -HYETOGRAPH_FLOAT_TOLERANCE {
            return Err(GenevaError::ContractViolation(
                "closure adjustment produced a negative final incremental depth".to_string(),
            ));
        }
        incremental_rainfall_mm[last_idx] = adjusted_last.max(0.0);
        cumulative_rainfall_mm[last_idx] = request.depth_mm;

        if last_idx > 0 {
            let dt_minutes = time_minutes[last_idx] - time_minutes[last_idx - 1];
            intensity_mm_per_hr[last_idx] = incremental_rainfall_mm[last_idx] / (dt_minutes / 60.0);
        }
        sum_incremental_mm = incremental_rainfall_mm.iter().sum::<f64>();
    }

    let closure_error_mm = (sum_incremental_mm - request.depth_mm).abs();
    let closure_tolerance_mm = 0.01_f64.max(request.depth_mm * 0.001);
    if closure_error_mm > (closure_tolerance_mm + HYETOGRAPH_FLOAT_TOLERANCE) {
        return Err(GenevaError::ContractViolation(format!(
            "hyetograph closure error {closure_error_mm} exceeds tolerance {closure_tolerance_mm}"
        )));
    }

    let mut warnings = Vec::new();
    if request.duration_minutes < SHORT_DURATION_WARNING_THRESHOLD_MINUTES {
        warnings.push(HyetographWarning {
            code: "type_b_short_duration_extrapolation".to_string(),
            reason: "duration_minutes < 30 may exceed strict Type B support envelope".to_string(),
        });
    }

    Ok(Neh4TypeBHyetographResponse {
        status: "ok".to_string(),
        phase: "build_hyetograph".to_string(),
        kernel_schema_version: request.kernel_schema_version,
        distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
        duration_minutes: request.duration_minutes,
        depth_mm: request.depth_mm,
        time_step_minutes: request.time_step_minutes,
        time_minutes,
        cumulative_rainfall_mm,
        incremental_rainfall_mm,
        intensity_mm_per_hr,
        warnings,
        diagnostics: HyetographDiagnostics {
            closure_error_mm,
            closure_tolerance_mm,
            cumulative_monotonic,
        },
    })
}

pub fn serialize_neh4_type_b_hyetograph_response(
    response: &Neh4TypeBHyetographResponse,
) -> Result<String, GenevaError> {
    serde_json::to_string(response).map_err(|err| GenevaError::Serialization(err.to_string()))
}

fn enforce_monotonic(cumulative_rainfall_mm: &mut [f64]) -> Result<bool, GenevaError> {
    for idx in 1..cumulative_rainfall_mm.len() {
        if cumulative_rainfall_mm[idx] + HYETOGRAPH_FLOAT_TOLERANCE
            < cumulative_rainfall_mm[idx - 1]
        {
            return Err(GenevaError::ContractViolation(
                "cumulative rainfall decreased beyond tolerance".to_string(),
            ));
        }
        if cumulative_rainfall_mm[idx] < cumulative_rainfall_mm[idx - 1] {
            cumulative_rainfall_mm[idx] = cumulative_rainfall_mm[idx - 1];
        }
    }
    Ok(true)
}

fn build_time_grid(duration_minutes: f64, time_step_minutes: f64) -> Result<Vec<f64>, GenevaError> {
    let mut time_minutes = vec![0.0];
    let mut next_minutes = time_step_minutes;
    while next_minutes < (duration_minutes - HYETOGRAPH_FLOAT_TOLERANCE) {
        if time_minutes.len() >= MAX_TIMESTEPS {
            return Err(GenevaError::InvalidInput(format!(
                "generated timestep count exceeded {MAX_TIMESTEPS}"
            )));
        }
        time_minutes.push(next_minutes);
        next_minutes += time_step_minutes;
    }

    if time_minutes
        .last()
        .map(|last| (*last - duration_minutes).abs() > HYETOGRAPH_FLOAT_TOLERANCE)
        .unwrap_or(true)
    {
        time_minutes.push(duration_minutes);
    } else if let Some(last) = time_minutes.last_mut() {
        *last = duration_minutes;
    }

    if time_minutes.len() < 2 {
        time_minutes.push(duration_minutes);
    }

    Ok(time_minutes)
}

fn load_dimensionless_ordinates(
    ordinate_csv_path: Option<&str>,
) -> Result<Vec<(f64, f64)>, GenevaError> {
    match ordinate_csv_path {
        Some(path) => {
            validate_ordinate_path(path)?;
            let csv_text = fs::read_to_string(path).map_err(|err| {
                GenevaError::InvalidInput(format!("failed reading ordinate CSV '{path}': {err}"))
            })?;
            parse_ordinate_csv(&csv_text)
        }
        None => Ok(NEH4_T_STAR
            .iter()
            .zip(NEH4_P_STAR.iter())
            .map(|(t_star, p_star)| (*t_star, *p_star))
            .collect()),
    }
}

fn parse_ordinate_csv(csv_text: &str) -> Result<Vec<(f64, f64)>, GenevaError> {
    let mut parsed = Vec::new();
    let mut header_seen = false;

    for line in csv_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if !header_seen && lower.starts_with("t_star") && lower.contains("p_star") {
            header_seen = true;
            continue;
        }

        let mut columns = trimmed.split(',').map(str::trim);
        let t_star = columns
            .next()
            .ok_or_else(|| GenevaError::InvalidInput("ordinate row missing t_star".to_string()))?
            .parse::<f64>()
            .map_err(|_| {
                GenevaError::InvalidInput("invalid t_star value in ordinate CSV".to_string())
            })?;
        let p_star = columns
            .next()
            .ok_or_else(|| GenevaError::InvalidInput("ordinate row missing p_star".to_string()))?
            .parse::<f64>()
            .map_err(|_| {
                GenevaError::InvalidInput("invalid p_star value in ordinate CSV".to_string())
            })?;
        parsed.push((t_star, p_star));
        header_seen = true;
    }

    validate_ordinates(&parsed)?;
    Ok(parsed)
}

fn validate_ordinates(ordinates: &[(f64, f64)]) -> Result<(), GenevaError> {
    if ordinates.len() < 2 {
        return Err(GenevaError::InvalidInput(
            "ordinate table must contain at least two points".to_string(),
        ));
    }

    let (first_t, first_p) = ordinates[0];
    if first_t.abs() > HYETOGRAPH_FLOAT_TOLERANCE || first_p.abs() > HYETOGRAPH_FLOAT_TOLERANCE {
        return Err(GenevaError::InvalidInput(
            "ordinate table must start at (0,0)".to_string(),
        ));
    }
    let (last_t, last_p) = ordinates[ordinates.len() - 1];
    if (last_t - 1.0).abs() > HYETOGRAPH_FLOAT_TOLERANCE
        || (last_p - 1.0).abs() > HYETOGRAPH_FLOAT_TOLERANCE
    {
        return Err(GenevaError::InvalidInput(
            "ordinate table must end at (1,1)".to_string(),
        ));
    }

    for (idx, (t_star, p_star)) in ordinates.iter().enumerate() {
        if !t_star.is_finite() || !p_star.is_finite() {
            return Err(GenevaError::InvalidInput(
                "ordinate table contains non-finite values".to_string(),
            ));
        }
        if !(*t_star >= 0.0 && *t_star <= 1.0 && *p_star >= 0.0 && *p_star <= 1.0) {
            return Err(GenevaError::InvalidInput(
                "ordinate values must be in the [0,1] domain".to_string(),
            ));
        }
        if idx > 0 {
            let (prev_t, prev_p) = ordinates[idx - 1];
            if *t_star + HYETOGRAPH_FLOAT_TOLERANCE < prev_t {
                return Err(GenevaError::InvalidInput(
                    "ordinate t_star values must be non-decreasing".to_string(),
                ));
            }
            if *p_star + HYETOGRAPH_FLOAT_TOLERANCE < prev_p {
                return Err(GenevaError::InvalidInput(
                    "ordinate p_star values must be non-decreasing".to_string(),
                ));
            }
            if (*t_star - prev_t).abs() <= HYETOGRAPH_FLOAT_TOLERANCE
                && (*p_star - prev_p).abs() > HYETOGRAPH_FLOAT_TOLERANCE
            {
                return Err(GenevaError::InvalidInput(
                    "duplicate t_star rows must share the same p_star value".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_ordinate_path(path: &str) -> Result<(), GenevaError> {
    if path.trim().is_empty() {
        return Err(GenevaError::InvalidInput(
            "ordinate_csv_path must not be empty".to_string(),
        ));
    }
    if path.len() > MAX_ORDINATE_PATH_LEN {
        return Err(GenevaError::InvalidInput(format!(
            "ordinate_csv_path length must be <= {MAX_ORDINATE_PATH_LEN}"
        )));
    }
    if path.contains('\0') {
        return Err(GenevaError::InvalidInput(
            "ordinate_csv_path contains a NUL byte".to_string(),
        ));
    }
    if Path::new(path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(GenevaError::InvalidInput(
            "ordinate_csv_path must not contain parent-directory traversal segments".to_string(),
        ));
    }
    Ok(())
}

fn scale_ordinates(
    dimensionless_ordinates: &[(f64, f64)],
    duration_minutes: f64,
    depth_mm: f64,
) -> Vec<(f64, f64)> {
    dimensionless_ordinates
        .iter()
        .map(|(t_star, p_star)| (t_star * duration_minutes, p_star * depth_mm))
        .collect()
}

fn interpolate_cumulative_depth(
    time_minutes: f64,
    scaled_ordinates: &[(f64, f64)],
    duration_minutes: f64,
) -> Result<f64, GenevaError> {
    if time_minutes <= 0.0 {
        return Ok(0.0);
    }
    if time_minutes >= duration_minutes {
        return Ok(scaled_ordinates
            .last()
            .map(|(_, cumulative)| *cumulative)
            .unwrap_or(0.0));
    }

    let mut previous = scaled_ordinates[0];
    for current in scaled_ordinates.iter().skip(1) {
        let (x0, y0) = previous;
        let (x1, y1) = *current;
        if x1 + HYETOGRAPH_FLOAT_TOLERANCE < x0 {
            return Err(GenevaError::ContractViolation(
                "scaled ordinate times must be non-decreasing".to_string(),
            ));
        }
        if x1 <= x0 + HYETOGRAPH_FLOAT_TOLERANCE {
            previous = *current;
            continue;
        }
        if time_minutes <= x1 + HYETOGRAPH_FLOAT_TOLERANCE {
            let ratio = ((time_minutes - x0) / (x1 - x0)).clamp(0.0, 1.0);
            return Ok(y0 + ((y1 - y0) * ratio));
        }
        previous = *current;
    }

    Ok(scaled_ordinates
        .last()
        .map(|(_, cumulative)| *cumulative)
        .unwrap_or(0.0))
}

fn default_distribution_type() -> String {
    DISTRIBUTION_NEH4_TYPE_B.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} but got {actual} with tolerance {tolerance}"
        );
    }

    fn write_temp_file(contents: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be monotonic for tests")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "geneva_hyetograph_{}_{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&dir).expect("temp directory should exist");
        let path = dir.join("ordinates.csv");
        fs::write(&path, contents).expect("temp ordinate file should be written");
        path
    }

    #[test]
    fn breakpoint_parity_matches_reference_within_relative_tolerance() {
        let duration_minutes = 120.0;
        let depth_mm = 45.0;
        let scaled = scale_ordinates(
            &NEH4_T_STAR
                .iter()
                .zip(NEH4_P_STAR.iter())
                .map(|(t, p)| (*t, *p))
                .collect::<Vec<_>>(),
            duration_minutes,
            depth_mm,
        );

        for (t_star, p_star) in NEH4_T_STAR.iter().zip(NEH4_P_STAR.iter()) {
            let time_minutes = t_star * duration_minutes;
            let expected = p_star * depth_mm;
            let interpolated =
                interpolate_cumulative_depth(time_minutes, &scaled, duration_minutes)
                    .expect("interpolation should succeed");
            let tolerance = if expected.abs() <= HYETOGRAPH_FLOAT_TOLERANCE {
                1e-9
            } else {
                expected.abs() * 0.001
            };
            approx_eq(interpolated, expected, tolerance);
        }
    }

    #[test]
    fn duplicate_zero_ordinates_do_not_break_interpolation() {
        let response = build_neh4_type_b_hyetograph(60.0, 20.0, 5.0)
            .expect("default NEH4 Type B build should succeed");
        assert_eq!(response.time_minutes[0], 0.0);
        assert_eq!(response.cumulative_rainfall_mm[0], 0.0);
    }

    #[test]
    fn generated_hyetograph_is_monotonic_and_closes() {
        let response = build_neh4_type_b_hyetograph(95.0, 38.0, 7.0)
            .expect("hyetograph generation should succeed");

        assert!(response
            .cumulative_rainfall_mm
            .windows(2)
            .all(|window| window[1] + HYETOGRAPH_FLOAT_TOLERANCE >= window[0]));
        assert!(response
            .incremental_rainfall_mm
            .iter()
            .all(|depth| *depth >= -HYETOGRAPH_FLOAT_TOLERANCE));
        approx_eq(
            *response
                .cumulative_rainfall_mm
                .last()
                .expect("cumulative series should be non-empty"),
            response.depth_mm,
            response.diagnostics.closure_tolerance_mm,
        );
        assert!(response.diagnostics.closure_error_mm <= response.diagnostics.closure_tolerance_mm);
    }

    #[test]
    fn short_duration_emits_required_warning() {
        let response = build_neh4_type_b_hyetograph(20.0, 15.0, 2.0)
            .expect("short-duration hyetograph should still build");
        assert!(response
            .warnings
            .iter()
            .any(|warning| warning.code == "type_b_short_duration_extrapolation"));
    }

    #[test]
    fn rejects_non_positive_duration_depth_and_timestep() {
        let error = build_neh4_type_b_hyetograph(0.0, 10.0, 1.0)
            .expect_err("non-positive duration must fail");
        assert_eq!(error.code(), "invalid_input");

        let error =
            build_neh4_type_b_hyetograph(60.0, 0.0, 1.0).expect_err("non-positive depth must fail");
        assert_eq!(error.code(), "invalid_input");

        let error = build_neh4_type_b_hyetograph(60.0, 10.0, 0.0)
            .expect_err("non-positive time_step must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn malformed_ordinate_csv_path_and_contents_return_typed_errors() {
        let request = Neh4TypeBHyetographRequest {
            kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
            duration_minutes: 60.0,
            depth_mm: 20.0,
            time_step_minutes: 5.0,
            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
            ordinate_csv_path: Some("/tmp/does-not-exist-ordinates.csv".to_string()),
        };
        let error = build_neh4_type_b_hyetograph_from_request(&request)
            .expect_err("missing ordinate path must fail");
        assert_eq!(error.code(), "invalid_input");

        let malformed_path = write_temp_file("t_star,p_star\n0.0\n");
        let request = Neh4TypeBHyetographRequest {
            kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
            duration_minutes: 60.0,
            depth_mm: 20.0,
            time_step_minutes: 5.0,
            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
            ordinate_csv_path: Some(malformed_path.to_string_lossy().into_owned()),
        };
        let error = build_neh4_type_b_hyetograph_from_request(&request)
            .expect_err("malformed ordinate CSV must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn parse_request_rejects_malformed_json() {
        let error = Neh4TypeBHyetographRequest::from_payload_json("{")
            .expect_err("malformed JSON must fail");
        assert_eq!(error.code(), "invalid_json");
    }
}

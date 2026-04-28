use crate::error::GenevaError;
use crate::storm_shape::{default_distribution_type, StormShape, DISTRIBUTION_NEH4_TYPE_B};
use serde::{Deserialize, Serialize};

const HYETOGRAPH_KERNEL_SCHEMA_VERSION: u32 = 1;
const HYETOGRAPH_FLOAT_TOLERANCE: f64 = 1e-9;
const MAX_TIMESTEPS: usize = 250_000;
const SHORT_DURATION_WARNING_THRESHOLD_MINUTES: f64 = 30.0;
const LEGACY_SOURCE_CURVE_DURATION_HOURS: f64 = 24.0;
const LEGACY_MAX_DURATION_MINUTES: f64 = 1_440.0;
const LEGACY_DISTRIBUTIONS_CSV_SHA256: &str =
    "bb3265a092ee2f447416ef739627bf9cc38700209faaf2ba3da8e439fe849640";
const LEGACY_DISTRIBUTIONS_CSV: &str =
    include_str!("../resources/nrcs_legacy_24h_distributions.csv");

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
        let storm_shape = StormShape::parse(&self.distribution_type)?;
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

        if storm_shape.is_legacy_24h()
            && self.duration_minutes > LEGACY_MAX_DURATION_MINUTES + HYETOGRAPH_FLOAT_TOLERANCE
        {
            return Err(GenevaError::InvalidInput(
                "Type I/IA/II/III distribution durations must be <= 1440 minutes".to_string(),
            ));
        }

        if self.ordinate_csv_path.is_some() {
            return Err(GenevaError::InvalidInput(
                "custom ordinate_csv_path is not supported for Geneva storm shapes".to_string(),
            ));
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_metadata: Option<HyetographSourceMetadata>,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HyetographSourceMetadata {
    pub source_distribution_type: String,
    pub source_curve_duration_hours: f64,
    pub extraction_start_hours: f64,
    pub extraction_end_hours: f64,
    pub extraction_ratio_to_24h: f64,
    pub event_depth_is_duration_depth: bool,
    pub source_table_sha256: String,
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
    build_hyetograph_from_request(request)
}

pub fn build_hyetograph_from_request(
    request: &Neh4TypeBHyetographRequest,
) -> Result<Neh4TypeBHyetographResponse, GenevaError> {
    request.validate()?;
    let storm_shape = StormShape::parse(&request.distribution_type)?;
    let time_minutes = build_time_grid(request.duration_minutes, request.time_step_minutes)?;

    let mut warnings = Vec::new();
    let (cumulative_rainfall_mm, source_metadata) = match storm_shape {
        StormShape::Uniform => (build_uniform_cumulative(request, &time_minutes), None),
        StormShape::Neh4TypeB => {
            if request.duration_minutes < SHORT_DURATION_WARNING_THRESHOLD_MINUTES {
                warnings.push(HyetographWarning {
                    code: "type_b_short_duration_extrapolation".to_string(),
                    reason: "duration_minutes < 30 may exceed strict Type B support envelope"
                        .to_string(),
                });
            }
            (build_type_b_cumulative(request, &time_minutes)?, None)
        }
        StormShape::TypeI | StormShape::TypeIa | StormShape::TypeII | StormShape::TypeIII => {
            build_legacy_cumulative(storm_shape, request, &time_minutes)?
        }
    };

    finalize_hyetograph_response(
        request,
        storm_shape,
        time_minutes,
        cumulative_rainfall_mm,
        warnings,
        source_metadata,
    )
}

fn finalize_hyetograph_response(
    request: &Neh4TypeBHyetographRequest,
    storm_shape: StormShape,
    time_minutes: Vec<f64>,
    mut cumulative_rainfall_mm: Vec<f64>,
    warnings: Vec<HyetographWarning>,
    source_metadata: Option<HyetographSourceMetadata>,
) -> Result<Neh4TypeBHyetographResponse, GenevaError> {
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

    Ok(Neh4TypeBHyetographResponse {
        status: "ok".to_string(),
        phase: "build_hyetograph".to_string(),
        kernel_schema_version: request.kernel_schema_version,
        distribution_type: storm_shape.id().to_string(),
        duration_minutes: request.duration_minutes,
        depth_mm: request.depth_mm,
        time_step_minutes: request.time_step_minutes,
        time_minutes,
        cumulative_rainfall_mm,
        incremental_rainfall_mm,
        intensity_mm_per_hr,
        warnings,
        source_metadata,
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

fn build_uniform_cumulative(
    request: &Neh4TypeBHyetographRequest,
    time_minutes: &[f64],
) -> Vec<f64> {
    time_minutes
        .iter()
        .map(|time| (time / request.duration_minutes).clamp(0.0, 1.0) * request.depth_mm)
        .collect()
}

fn build_type_b_cumulative(
    request: &Neh4TypeBHyetographRequest,
    time_minutes: &[f64],
) -> Result<Vec<f64>, GenevaError> {
    let ordinate_pairs = default_type_b_ordinates();
    let scaled_ordinates =
        scale_ordinates(&ordinate_pairs, request.duration_minutes, request.depth_mm);

    let mut cumulative_rainfall_mm = Vec::with_capacity(time_minutes.len());
    for time_minutes_value in time_minutes {
        cumulative_rainfall_mm.push(interpolate_cumulative_depth(
            *time_minutes_value,
            &scaled_ordinates,
            request.duration_minutes,
        )?);
    }
    Ok(cumulative_rainfall_mm)
}

fn build_legacy_cumulative(
    storm_shape: StormShape,
    request: &Neh4TypeBHyetographRequest,
    time_minutes: &[f64],
) -> Result<(Vec<f64>, Option<HyetographSourceMetadata>), GenevaError> {
    let source_ordinates = load_legacy_distribution(storm_shape)?;
    let duration_hours = request.duration_minutes / 60.0;
    let (extraction_start_hours, extraction_end_hours, extraction_ratio_to_24h) =
        find_embedded_window(&source_ordinates, duration_hours)?;

    if extraction_ratio_to_24h <= HYETOGRAPH_FLOAT_TOLERANCE {
        return Err(GenevaError::ContractViolation(
            "legacy storm extraction ratio must be positive".to_string(),
        ));
    }

    let start_fraction = interpolate_source_fraction(extraction_start_hours, &source_ordinates)?;
    let mut cumulative_rainfall_mm = Vec::with_capacity(time_minutes.len());
    for time_minutes_value in time_minutes {
        let source_time_hours = extraction_start_hours + (*time_minutes_value / 60.0);
        let source_fraction = interpolate_source_fraction(source_time_hours, &source_ordinates)?;
        let event_fraction =
            ((source_fraction - start_fraction) / extraction_ratio_to_24h).clamp(0.0, 1.0);
        cumulative_rainfall_mm.push(event_fraction * request.depth_mm);
    }

    Ok((
        cumulative_rainfall_mm,
        Some(HyetographSourceMetadata {
            source_distribution_type: storm_shape.id().to_string(),
            source_curve_duration_hours: LEGACY_SOURCE_CURVE_DURATION_HOURS,
            extraction_start_hours,
            extraction_end_hours,
            extraction_ratio_to_24h,
            event_depth_is_duration_depth: true,
            source_table_sha256: LEGACY_DISTRIBUTIONS_CSV_SHA256.to_string(),
        }),
    ))
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

fn default_type_b_ordinates() -> Vec<(f64, f64)> {
    NEH4_T_STAR
        .iter()
        .zip(NEH4_P_STAR.iter())
        .map(|(t_star, p_star)| (*t_star, *p_star))
        .collect()
}

fn load_legacy_distribution(storm_shape: StormShape) -> Result<Vec<(f64, f64)>, GenevaError> {
    if !storm_shape.is_legacy_24h() {
        return Err(GenevaError::ContractViolation(
            "legacy distribution loader called for non-legacy storm shape".to_string(),
        ));
    }
    parse_legacy_distribution_csv(LEGACY_DISTRIBUTIONS_CSV, storm_shape)
}

fn parse_legacy_distribution_csv(
    csv_text: &str,
    storm_shape: StormShape,
) -> Result<Vec<(f64, f64)>, GenevaError> {
    let mut parsed = Vec::new();
    let mut header_seen = false;
    let value_column_index = legacy_column_index(storm_shape)?;

    for line in csv_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if !header_seen && lower.starts_with("time_hours") {
            header_seen = true;
            continue;
        }

        let columns = trimmed.split(',').map(str::trim).collect::<Vec<_>>();
        if columns.len() <= value_column_index {
            return Err(GenevaError::InvalidInput(
                "legacy distribution CSV row has too few columns".to_string(),
            ));
        }
        let time_hours = columns[0].parse::<f64>().map_err(|_| {
            GenevaError::InvalidInput(
                "invalid time_hours value in legacy distribution CSV".to_string(),
            )
        })?;
        let cumulative_fraction = columns[value_column_index].parse::<f64>().map_err(|_| {
            GenevaError::InvalidInput(
                "invalid cumulative fraction in legacy distribution CSV".to_string(),
            )
        })?;
        parsed.push((time_hours, cumulative_fraction));
        header_seen = true;
    }

    validate_legacy_source_ordinates(&parsed)?;
    Ok(parsed)
}

fn legacy_column_index(storm_shape: StormShape) -> Result<usize, GenevaError> {
    match storm_shape {
        StormShape::TypeI => Ok(1),
        StormShape::TypeIa => Ok(2),
        StormShape::TypeII => Ok(3),
        StormShape::TypeIII => Ok(4),
        _ => Err(GenevaError::ContractViolation(
            "legacy column index requested for non-legacy storm shape".to_string(),
        )),
    }
}

fn validate_legacy_source_ordinates(ordinates: &[(f64, f64)]) -> Result<(), GenevaError> {
    if ordinates.len() < 2 {
        return Err(GenevaError::InvalidInput(
            "legacy distribution table must contain at least two points".to_string(),
        ));
    }

    let (first_time_hours, first_fraction) = ordinates[0];
    if first_time_hours.abs() > HYETOGRAPH_FLOAT_TOLERANCE
        || first_fraction.abs() > HYETOGRAPH_FLOAT_TOLERANCE
    {
        return Err(GenevaError::InvalidInput(
            "legacy distribution table must start at (0,0)".to_string(),
        ));
    }
    let (last_time_hours, last_fraction) = ordinates[ordinates.len() - 1];
    if (last_time_hours - LEGACY_SOURCE_CURVE_DURATION_HOURS).abs() > HYETOGRAPH_FLOAT_TOLERANCE
        || (last_fraction - 1.0).abs() > HYETOGRAPH_FLOAT_TOLERANCE
    {
        return Err(GenevaError::InvalidInput(
            "legacy distribution table must end at (24,1)".to_string(),
        ));
    }

    for (idx, (time_hours, fraction)) in ordinates.iter().enumerate() {
        if !time_hours.is_finite() || !fraction.is_finite() {
            return Err(GenevaError::InvalidInput(
                "legacy distribution table contains non-finite values".to_string(),
            ));
        }
        if !(*time_hours >= 0.0
            && *time_hours <= LEGACY_SOURCE_CURVE_DURATION_HOURS
            && *fraction >= 0.0
            && *fraction <= 1.0)
        {
            return Err(GenevaError::InvalidInput(
                "legacy distribution values must be in the expected domain".to_string(),
            ));
        }
        if idx > 0 {
            let (prev_time_hours, prev_fraction) = ordinates[idx - 1];
            if *time_hours <= prev_time_hours + HYETOGRAPH_FLOAT_TOLERANCE {
                return Err(GenevaError::InvalidInput(
                    "legacy distribution time_hours values must be strictly increasing".to_string(),
                ));
            }
            if *fraction + HYETOGRAPH_FLOAT_TOLERANCE < prev_fraction {
                return Err(GenevaError::InvalidInput(
                    "legacy distribution cumulative fractions must be non-decreasing".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn find_embedded_window(
    source_ordinates: &[(f64, f64)],
    duration_hours: f64,
) -> Result<(f64, f64, f64), GenevaError> {
    if duration_hours <= 0.0 || duration_hours > LEGACY_SOURCE_CURVE_DURATION_HOURS {
        return Err(GenevaError::InvalidInput(
            "legacy storm duration must be > 0 and <= 24 hours".to_string(),
        ));
    }

    if (duration_hours - LEGACY_SOURCE_CURVE_DURATION_HOURS).abs() <= HYETOGRAPH_FLOAT_TOLERANCE {
        return Ok((0.0, LEGACY_SOURCE_CURVE_DURATION_HOURS, 1.0));
    }

    let max_start = LEGACY_SOURCE_CURVE_DURATION_HOURS - duration_hours;
    let target_start = (LEGACY_SOURCE_CURVE_DURATION_HOURS - duration_hours) / 2.0;
    let mut candidates = vec![0.0, max_start];
    for (time_hours, _) in source_ordinates {
        candidates.push(*time_hours);
        candidates.push(*time_hours - duration_hours);
    }

    let mut best_start = 0.0;
    let mut best_ratio = -1.0_f64;
    for candidate in candidates {
        if candidate < -HYETOGRAPH_FLOAT_TOLERANCE
            || candidate > max_start + HYETOGRAPH_FLOAT_TOLERANCE
        {
            continue;
        }
        let start_hours = candidate.clamp(0.0, max_start);
        let end_hours = start_hours + duration_hours;
        let start_fraction = interpolate_source_fraction(start_hours, source_ordinates)?;
        let end_fraction = interpolate_source_fraction(end_hours, source_ordinates)?;
        let ratio = end_fraction - start_fraction;
        if ratio > best_ratio + HYETOGRAPH_FLOAT_TOLERANCE
            || ((ratio - best_ratio).abs() <= HYETOGRAPH_FLOAT_TOLERANCE
                && ((start_hours - target_start).abs() < (best_start - target_start).abs()
                    || ((start_hours - target_start).abs() - (best_start - target_start).abs())
                        .abs()
                        <= HYETOGRAPH_FLOAT_TOLERANCE
                        && start_hours < best_start))
        {
            best_start = start_hours;
            best_ratio = ratio;
        }
    }

    if best_ratio <= HYETOGRAPH_FLOAT_TOLERANCE {
        return Err(GenevaError::ContractViolation(
            "failed to find positive embedded rainfall window".to_string(),
        ));
    }

    Ok((best_start, best_start + duration_hours, best_ratio))
}

fn interpolate_source_fraction(
    time_hours: f64,
    source_ordinates: &[(f64, f64)],
) -> Result<f64, GenevaError> {
    if time_hours <= 0.0 {
        return Ok(0.0);
    }
    if time_hours >= LEGACY_SOURCE_CURVE_DURATION_HOURS {
        return Ok(1.0);
    }

    let mut previous = source_ordinates[0];
    for current in source_ordinates.iter().skip(1) {
        let (x0, y0) = previous;
        let (x1, y1) = *current;
        if time_hours <= x1 + HYETOGRAPH_FLOAT_TOLERANCE {
            let ratio = ((time_hours - x0) / (x1 - x0)).clamp(0.0, 1.0);
            return Ok(y0 + ((y1 - y0) * ratio));
        }
        previous = *current;
    }

    Ok(1.0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storm_shape::{DISTRIBUTION_TYPE_II, SUPPORTED_DISTRIBUTION_TYPES};

    fn approx_eq(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} but got {actual} with tolerance {tolerance}"
        );
    }

    fn request_with_distribution(distribution_type: &str) -> Neh4TypeBHyetographRequest {
        Neh4TypeBHyetographRequest {
            kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
            duration_minutes: 60.0,
            depth_mm: 25.0,
            time_step_minutes: 5.0,
            distribution_type: distribution_type.to_string(),
            ordinate_csv_path: None,
        }
    }

    fn assert_hyetograph_contract(response: &Neh4TypeBHyetographResponse) {
        assert_eq!(response.time_minutes.first().copied(), Some(0.0));
        approx_eq(
            response.time_minutes.last().copied().unwrap_or_default(),
            response.duration_minutes,
            HYETOGRAPH_FLOAT_TOLERANCE,
        );
        approx_eq(
            response
                .cumulative_rainfall_mm
                .first()
                .copied()
                .unwrap_or_default(),
            0.0,
            HYETOGRAPH_FLOAT_TOLERANCE,
        );
        approx_eq(
            response
                .cumulative_rainfall_mm
                .last()
                .copied()
                .unwrap_or_default(),
            response.depth_mm,
            response.diagnostics.closure_tolerance_mm,
        );
        assert!(response
            .cumulative_rainfall_mm
            .windows(2)
            .all(|window| window[1] + HYETOGRAPH_FLOAT_TOLERANCE >= window[0]));
        assert!(response
            .incremental_rainfall_mm
            .iter()
            .all(|depth| *depth >= -HYETOGRAPH_FLOAT_TOLERANCE));
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
    fn custom_ordinate_csv_path_is_rejected() {
        let request = Neh4TypeBHyetographRequest {
            kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
            duration_minutes: 60.0,
            depth_mm: 20.0,
            time_step_minutes: 5.0,
            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
            ordinate_csv_path: Some("/tmp/does-not-exist-ordinates.csv".to_string()),
        };
        let error = build_neh4_type_b_hyetograph_from_request(&request)
            .expect_err("custom ordinate path must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn parse_request_rejects_malformed_json() {
        let error = Neh4TypeBHyetographRequest::from_payload_json("{")
            .expect_err("malformed JSON must fail");
        assert_eq!(error.code(), "invalid_json");
    }

    #[test]
    fn missing_distribution_defaults_to_neh4_type_b() {
        let request = Neh4TypeBHyetographRequest::from_payload_json(
            r#"{
                "kernel_schema_version": 1,
                "duration_minutes": 60.0,
                "depth_mm": 25.0,
                "time_step_minutes": 5.0
            }"#,
        )
        .expect("missing distribution_type should default");
        assert_eq!(request.distribution_type, DISTRIBUTION_NEH4_TYPE_B);
    }

    #[test]
    fn rejects_unknown_distribution_type() {
        let mut request = request_with_distribution("type_iv");
        let error = request
            .validate()
            .expect_err("unknown storm shape should fail validation");
        assert_eq!(error.code(), "invalid_input");

        request.distribution_type = DISTRIBUTION_NEH4_TYPE_B.to_string();
        assert!(request.validate().is_ok());
    }

    #[test]
    fn all_supported_distributions_are_monotonic_and_close() {
        for distribution_type in SUPPORTED_DISTRIBUTION_TYPES {
            let request = request_with_distribution(distribution_type);
            let response = build_hyetograph_from_request(&request)
                .expect("supported distribution should build");
            assert_eq!(response.distribution_type, distribution_type);
            assert_hyetograph_contract(&response);

            let metadata_expected = StormShape::parse(distribution_type)
                .expect("known distribution")
                .is_legacy_24h();
            assert_eq!(response.source_metadata.is_some(), metadata_expected);
        }
    }

    #[test]
    fn output_time_grid_includes_exact_endpoints() {
        for step_minutes in [7.0, 200.0] {
            let request = Neh4TypeBHyetographRequest {
                kernel_schema_version: HYETOGRAPH_KERNEL_SCHEMA_VERSION,
                duration_minutes: 95.0,
                depth_mm: 38.0,
                time_step_minutes: step_minutes,
                distribution_type: DISTRIBUTION_TYPE_II.to_string(),
                ordinate_csv_path: None,
            };
            let response = build_hyetograph_from_request(&request)
                .expect("endpoint grid should build for Type II");
            assert_hyetograph_contract(&response);
        }
    }

    #[test]
    fn type_ii_embedded_ratios_match_neh_figure_4_31() {
        let source =
            load_legacy_distribution(StormShape::TypeII).expect("Type II source should parse");
        let targets = [
            (5.0, 0.114),
            (10.0, 0.201),
            (15.0, 0.270),
            (30.0, 0.380),
            (60.0, 0.454),
            (120.0, 0.538),
            (180.0, 0.595),
            (360.0, 0.707),
            (720.0, 0.841),
            (1440.0, 1.000),
        ];

        for (duration_minutes, expected_ratio) in targets {
            let (_, _, ratio) = find_embedded_window(&source, duration_minutes / 60.0)
                .expect("embedded Type II window should resolve");
            approx_eq(ratio, expected_ratio, 0.003);
        }
    }

    #[test]
    fn legacy_short_durations_use_embedded_windows_not_full_curve_compression() {
        for storm_shape in [
            StormShape::TypeI,
            StormShape::TypeIa,
            StormShape::TypeII,
            StormShape::TypeIII,
        ] {
            let request = request_with_distribution(storm_shape.id());
            let response =
                build_hyetograph_from_request(&request).expect("legacy distribution should build");
            let metadata = response
                .source_metadata
                .as_ref()
                .expect("legacy distribution should report extraction metadata");
            assert!(metadata.extraction_start_hours > HYETOGRAPH_FLOAT_TOLERANCE);
            assert!(metadata.extraction_ratio_to_24h < 1.0 - HYETOGRAPH_FLOAT_TOLERANCE);

            let source = load_legacy_distribution(storm_shape).expect("legacy source should parse");
            let max_compression_difference = response
                .time_minutes
                .iter()
                .enumerate()
                .filter(|(_, time)| {
                    **time > HYETOGRAPH_FLOAT_TOLERANCE
                        && **time < request.duration_minutes - HYETOGRAPH_FLOAT_TOLERANCE
                })
                .map(|(idx, time)| {
                    let compressed_source_time =
                        LEGACY_SOURCE_CURVE_DURATION_HOURS * (*time / request.duration_minutes);
                    let compressed_fraction =
                        interpolate_source_fraction(compressed_source_time, &source)
                            .expect("source checkpoint should parse");
                    let embedded_fraction =
                        response.cumulative_rainfall_mm[idx] / response.depth_mm;
                    (embedded_fraction - compressed_fraction).abs()
                })
                .fold(0.0_f64, f64::max);
            assert!(
                max_compression_difference > 0.01,
                "legacy {:?} output looked like full-curve compression",
                storm_shape
            );
        }
    }
}

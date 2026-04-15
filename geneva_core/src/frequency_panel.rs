use crate::error::GenevaError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const FREQUENCY_PANEL_KERNEL_SCHEMA_VERSION: u32 = 1;
const MAX_DURATIONS: usize = 256;
const MAX_ARI_VALUES: usize = 256;
const MAX_REQUESTED_CELLS: usize = 65_536;
const MAX_SOURCE_PATH_LEN: usize = 1024;

const DATASOURCE_CLIGEN: &str = "cligen_freq";
const DATASOURCE_NOAA: &str = "noaa14_pds";
const DISTRIBUTION_NEH4_TYPE_B: &str = "neh4_type_b";
const DEFAULT_CLIGEN_SOURCE: &str = "climate/wepp_cli_pds_mean_metric.csv";
const DEFAULT_NOAA_SOURCE: &str = "climate/atlas14_intensity_pds_mean_metric.csv";

#[derive(Debug, Clone, Deserialize)]
pub struct BuildFrequencyPanelRequest {
    #[serde(alias = "schema_version")]
    pub kernel_schema_version: u32,
    pub durations_minutes: Vec<u32>,
    pub ari_years: Vec<u32>,
    #[serde(default = "default_distribution_type")]
    pub distribution_type: String,
    #[serde(default)]
    pub allow_duration_interpolation: bool,
    #[serde(default)]
    pub source_root: Option<String>,
    #[serde(default)]
    pub sources: Option<FrequencyPanelSources>,
}

impl BuildFrequencyPanelRequest {
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
        if self.kernel_schema_version != FREQUENCY_PANEL_KERNEL_SCHEMA_VERSION {
            return Err(GenevaError::InvalidInput(format!(
                "kernel_schema_version must equal {FREQUENCY_PANEL_KERNEL_SCHEMA_VERSION}"
            )));
        }
        if self.durations_minutes.is_empty() {
            return Err(GenevaError::InvalidInput(
                "durations_minutes must not be empty".to_string(),
            ));
        }
        if self.durations_minutes.len() > MAX_DURATIONS {
            return Err(GenevaError::InvalidInput(format!(
                "durations_minutes length must be <= {MAX_DURATIONS}"
            )));
        }
        if self.ari_years.is_empty() {
            return Err(GenevaError::InvalidInput(
                "ari_years must not be empty".to_string(),
            ));
        }
        if self.ari_years.len() > MAX_ARI_VALUES {
            return Err(GenevaError::InvalidInput(format!(
                "ari_years length must be <= {MAX_ARI_VALUES}"
            )));
        }
        if self.durations_minutes.contains(&0_u32) {
            return Err(GenevaError::InvalidInput(
                "durations_minutes must contain positive values".to_string(),
            ));
        }
        if self.ari_years.contains(&0_u32) {
            return Err(GenevaError::InvalidInput(
                "ari_years must contain positive values".to_string(),
            ));
        }
        if self.distribution_type != DISTRIBUTION_NEH4_TYPE_B {
            return Err(GenevaError::InvalidInput(format!(
                "distribution_type must equal '{DISTRIBUTION_NEH4_TYPE_B}' in v1"
            )));
        }
        if self.allow_duration_interpolation {
            return Err(GenevaError::InvalidInput(
                "allow_duration_interpolation must be false for panel materialization".to_string(),
            ));
        }

        let requested_cells = self
            .durations_minutes
            .len()
            .checked_mul(self.ari_years.len())
            .and_then(|cells| cells.checked_mul(2))
            .ok_or_else(|| {
                GenevaError::InvalidInput(
                    "requested frequency matrix dimensions overflow".to_string(),
                )
            })?;
        if requested_cells > MAX_REQUESTED_CELLS {
            return Err(GenevaError::InvalidInput(format!(
                "durations_minutes * ari_years * datasources must be <= {MAX_REQUESTED_CELLS}"
            )));
        }

        let sources = self.resolved_sources();
        validate_source_path(&sources.cligen_freq, "sources.cligen_freq")?;
        if let Some(noaa_source) = sources.noaa14_pds.as_deref() {
            validate_source_path(noaa_source, "sources.noaa14_pds")?;
        }

        if let Some(source_root) = self.source_root.as_deref() {
            if source_root.trim().is_empty() {
                return Err(GenevaError::InvalidInput(
                    "source_root must not be empty when provided".to_string(),
                ));
            }
        }

        let duration_set: BTreeSet<u32> = self.durations_minutes.iter().copied().collect();
        if duration_set.len() != self.durations_minutes.len() {
            return Err(GenevaError::InvalidInput(
                "durations_minutes must be unique".to_string(),
            ));
        }
        let ari_set: BTreeSet<u32> = self.ari_years.iter().copied().collect();
        if ari_set.len() != self.ari_years.len() {
            return Err(GenevaError::InvalidInput(
                "ari_years must be unique".to_string(),
            ));
        }

        Ok(())
    }

    fn resolved_sources(&self) -> FrequencyPanelSources {
        self.sources
            .clone()
            .unwrap_or_else(FrequencyPanelSources::default_paths)
    }

    fn source_root_path(&self) -> PathBuf {
        match self.source_root.as_deref() {
            Some(path) if !path.trim().is_empty() => PathBuf::from(path),
            _ => PathBuf::from("."),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrequencyPanelSources {
    pub cligen_freq: String,
    #[serde(default)]
    pub noaa14_pds: Option<String>,
}

impl FrequencyPanelSources {
    fn default_paths() -> Self {
        Self {
            cligen_freq: DEFAULT_CLIGEN_SOURCE.to_string(),
            noaa14_pds: Some(DEFAULT_NOAA_SOURCE.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildFrequencyPanelResponse {
    pub status: String,
    pub phase: String,
    pub kernel_schema_version: u32,
    pub datasource_ids: Vec<String>,
    pub distribution_type: String,
    pub durations_minutes: Vec<u32>,
    pub ari_years: Vec<u32>,
    pub cells: Vec<FrequencyPanelCell>,
    pub warnings: Vec<FrequencyPanelWarning>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FrequencyPanelCell {
    pub storm_id: String,
    pub datasource_id: String,
    pub duration_minutes: u32,
    pub ari_years: u32,
    pub depth_mm: Option<f64>,
    pub intensity_mm_per_hr: Option<f64>,
    pub distribution_type: String,
    pub availability: FrequencyCellAvailability,
    pub reason_code: Option<FrequencyUnavailableReasonCode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyCellAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyUnavailableReasonCode {
    DurationUnavailable,
    AriUnavailable,
    SourceMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrequencyPanelWarning {
    pub code: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
struct AvailableFrequencyCell {
    depth_mm: f64,
    intensity_mm_per_hr: f64,
}

#[derive(Debug, Clone, Default)]
struct SourceMatrix {
    cells: BTreeMap<(u32, u32), AvailableFrequencyCell>,
    available_durations: BTreeSet<u32>,
    available_aris: BTreeSet<u32>,
}

impl SourceMatrix {
    fn insert(
        &mut self,
        duration_minutes: u32,
        ari_years: u32,
        depth_mm: f64,
        intensity_mm_per_hr: f64,
    ) -> Result<(), GenevaError> {
        if !depth_mm.is_finite() || depth_mm < 0.0 {
            return Err(GenevaError::InvalidInput(
                "depth_mm must be finite and >= 0".to_string(),
            ));
        }
        if !intensity_mm_per_hr.is_finite() || intensity_mm_per_hr < 0.0 {
            return Err(GenevaError::InvalidInput(
                "intensity_mm_per_hr must be finite and >= 0".to_string(),
            ));
        }

        if self
            .cells
            .insert(
                (duration_minutes, ari_years),
                AvailableFrequencyCell {
                    depth_mm,
                    intensity_mm_per_hr,
                },
            )
            .is_some()
        {
            return Err(GenevaError::InvalidInput(format!(
                "duplicate source cell for duration={duration_minutes} minutes, ari={ari_years}"
            )));
        }
        self.available_durations.insert(duration_minutes);
        self.available_aris.insert(ari_years);
        Ok(())
    }
}

pub fn build_frequency_panel(
    request: &BuildFrequencyPanelRequest,
) -> Result<BuildFrequencyPanelResponse, GenevaError> {
    request.validate()?;
    let durations_minutes = sorted_u32(&request.durations_minutes);
    let ari_years = sorted_u32(&request.ari_years);
    let source_root = request.source_root_path();
    let sources = request.resolved_sources();

    let cligen_csv = read_source_csv(&source_root, &sources.cligen_freq, DATASOURCE_CLIGEN, true)?
        .ok_or_else(|| {
            GenevaError::InvalidInput(
                "required CLIGEN source file could not be resolved".to_string(),
            )
        })?;
    let cligen_matrix = parse_cligen_frequency_csv(&cligen_csv)?;

    let noaa_csv = match sources.noaa14_pds.as_deref() {
        Some(path) => read_source_csv(&source_root, path, DATASOURCE_NOAA, false)?,
        None => None,
    };
    let noaa_matrix = match noaa_csv {
        Some(csv) => Some(parse_noaa_frequency_csv(&csv)?),
        None => None,
    };

    let mut cells = Vec::with_capacity(durations_minutes.len() * ari_years.len() * 2);
    materialize_cells(
        &mut cells,
        DATASOURCE_CLIGEN,
        &durations_minutes,
        &ari_years,
        Some(&cligen_matrix),
    );
    materialize_cells(
        &mut cells,
        DATASOURCE_NOAA,
        &durations_minutes,
        &ari_years,
        noaa_matrix.as_ref(),
    );

    let mut warnings = Vec::new();
    if noaa_matrix.is_none() {
        warnings.push(FrequencyPanelWarning {
            code: "noaa_source_missing".to_string(),
            reason:
                "optional NOAA Atlas 14 source is unavailable; NOAA cells marked source_missing"
                    .to_string(),
        });
    }

    Ok(BuildFrequencyPanelResponse {
        status: "ok".to_string(),
        phase: "build_frequency_panel".to_string(),
        kernel_schema_version: request.kernel_schema_version,
        datasource_ids: vec![DATASOURCE_CLIGEN.to_string(), DATASOURCE_NOAA.to_string()],
        distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
        durations_minutes,
        ari_years,
        cells,
        warnings,
    })
}

pub fn serialize_build_frequency_panel_response(
    response: &BuildFrequencyPanelResponse,
) -> Result<String, GenevaError> {
    serde_json::to_string(response).map_err(|err| GenevaError::Serialization(err.to_string()))
}

fn materialize_cells(
    cells: &mut Vec<FrequencyPanelCell>,
    datasource_id: &str,
    durations_minutes: &[u32],
    ari_years: &[u32],
    source_matrix: Option<&SourceMatrix>,
) {
    for duration_minutes in durations_minutes {
        for ari_years in ari_years {
            let storm_id = build_storm_id(datasource_id, *duration_minutes, *ari_years);
            match source_matrix {
                Some(matrix) => {
                    if let Some(available) = matrix.cells.get(&(*duration_minutes, *ari_years)) {
                        cells.push(FrequencyPanelCell {
                            storm_id,
                            datasource_id: datasource_id.to_string(),
                            duration_minutes: *duration_minutes,
                            ari_years: *ari_years,
                            depth_mm: Some(available.depth_mm),
                            intensity_mm_per_hr: Some(available.intensity_mm_per_hr),
                            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
                            availability: FrequencyCellAvailability::Available,
                            reason_code: None,
                        });
                        continue;
                    }

                    let reason = if !matrix.available_aris.contains(ari_years) {
                        FrequencyUnavailableReasonCode::AriUnavailable
                    } else {
                        FrequencyUnavailableReasonCode::DurationUnavailable
                    };

                    cells.push(FrequencyPanelCell {
                        storm_id,
                        datasource_id: datasource_id.to_string(),
                        duration_minutes: *duration_minutes,
                        ari_years: *ari_years,
                        depth_mm: None,
                        intensity_mm_per_hr: None,
                        distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
                        availability: FrequencyCellAvailability::Unavailable,
                        reason_code: Some(reason),
                    });
                }
                None => cells.push(FrequencyPanelCell {
                    storm_id,
                    datasource_id: datasource_id.to_string(),
                    duration_minutes: *duration_minutes,
                    ari_years: *ari_years,
                    depth_mm: None,
                    intensity_mm_per_hr: None,
                    distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
                    availability: FrequencyCellAvailability::Unavailable,
                    reason_code: Some(FrequencyUnavailableReasonCode::SourceMissing),
                }),
            }
        }
    }
}

fn parse_cligen_frequency_csv(csv_text: &str) -> Result<SourceMatrix, GenevaError> {
    let lines: Vec<&str> = csv_text.lines().collect();
    let header_index = find_header_index(&lines, "by metric for ari")
        .ok_or_else(|| GenevaError::InvalidInput("CLIGEN CSV missing ARI header".to_string()))?;
    let recurrence = parse_recurrence_line(lines[header_index])?;

    let mut depth_values: Option<Vec<f64>> = None;
    let mut duration_values: Option<Vec<f64>> = None;
    let mut parsed_rows = false;

    for line in lines.iter().skip(header_index + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if parsed_rows {
                break;
            }
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("date/time") || lower.starts_with("pyruntime") {
            break;
        }

        if let Some((label, values)) = parse_numeric_row(trimmed, recurrence.len())? {
            parsed_rows = true;
            let label_lower = label.to_ascii_lowercase();
            if label_lower.contains("storm depth") {
                depth_values = Some(values);
            } else if label_lower.contains("storm duration") {
                duration_values = Some(values);
            }
        }
    }

    let depth_values = depth_values.ok_or_else(|| {
        GenevaError::InvalidInput("CLIGEN CSV missing 'storm depth' row".to_string())
    })?;
    let duration_values = duration_values.ok_or_else(|| {
        GenevaError::InvalidInput("CLIGEN CSV missing 'storm duration' row".to_string())
    })?;

    let mut matrix = SourceMatrix::default();
    for (index, ari_years) in recurrence.iter().enumerate() {
        let depth_mm = depth_values[index];
        let duration_hours = duration_values[index];
        if !duration_hours.is_finite() || duration_hours <= 0.0 {
            return Err(GenevaError::InvalidInput(format!(
                "CLIGEN storm duration must be finite and > 0 at ARI {ari_years}"
            )));
        }
        if !depth_mm.is_finite() || depth_mm < 0.0 {
            return Err(GenevaError::InvalidInput(format!(
                "CLIGEN storm depth must be finite and >= 0 at ARI {ari_years}"
            )));
        }

        let duration_minutes = float_to_minutes(duration_hours * 60.0, "CLIGEN duration")?;
        let intensity_mm_per_hr = depth_mm / duration_hours;
        matrix.insert(duration_minutes, *ari_years, depth_mm, intensity_mm_per_hr)?;
    }

    if matrix.cells.is_empty() {
        return Err(GenevaError::InvalidInput(
            "CLIGEN CSV did not yield any available cells".to_string(),
        ));
    }
    Ok(matrix)
}

fn parse_noaa_frequency_csv(csv_text: &str) -> Result<SourceMatrix, GenevaError> {
    let lines: Vec<&str> = csv_text.lines().collect();
    let header_index = find_header_index(&lines, "by duration for ari")
        .ok_or_else(|| GenevaError::InvalidInput("NOAA CSV missing ARI header".to_string()))?;
    let recurrence = parse_recurrence_line(lines[header_index])?;

    let mut matrix = SourceMatrix::default();
    let mut parsed_rows = false;

    for line in lines.iter().skip(header_index + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if parsed_rows {
                break;
            }
            continue;
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("date/time") || lower.starts_with("pyruntime") {
            break;
        }

        if let Some((label, values)) = parse_numeric_row(trimmed, recurrence.len())? {
            let duration_minutes = match parse_duration_label_minutes(&label) {
                Some(duration_minutes) => duration_minutes,
                None => continue,
            };
            parsed_rows = true;
            for (index, ari_years) in recurrence.iter().enumerate() {
                let intensity_mm_per_hr = values[index];
                if !intensity_mm_per_hr.is_finite() || intensity_mm_per_hr < 0.0 {
                    return Err(GenevaError::InvalidInput(format!(
                        "NOAA intensity must be finite and >= 0 at duration={duration_minutes} ari={ari_years}"
                    )));
                }
                let depth_mm = intensity_mm_per_hr * (f64::from(duration_minutes) / 60.0);
                matrix.insert(duration_minutes, *ari_years, depth_mm, intensity_mm_per_hr)?;
            }
        }
    }

    if matrix.cells.is_empty() {
        return Err(GenevaError::InvalidInput(
            "NOAA CSV did not yield any available cells".to_string(),
        ));
    }
    Ok(matrix)
}

fn build_storm_id(datasource_id: &str, duration_minutes: u32, ari_years: u32) -> String {
    let prefix = if datasource_id == DATASOURCE_CLIGEN {
        "cligen"
    } else if datasource_id == DATASOURCE_NOAA {
        "noaa14"
    } else {
        datasource_id
    };
    format!("{prefix}_{duration_minutes}m_{ari_years}y")
}

fn parse_numeric_row(
    line: &str,
    expected_values: usize,
) -> Result<Option<(String, Vec<f64>)>, GenevaError> {
    let (label, values_part) = match line.split_once(':') {
        Some(parts) => parts,
        None => return Ok(None),
    };

    let mut values = Vec::new();
    for token in values_part.split(',') {
        let cleaned = token.trim();
        if cleaned.is_empty() {
            continue;
        }
        let numeric = cleaned.parse::<f64>().map_err(|_| {
            GenevaError::InvalidInput(format!(
                "frequency CSV row '{label}' contains non-numeric value '{cleaned}'"
            ))
        })?;
        if !numeric.is_finite() {
            return Err(GenevaError::InvalidInput(format!(
                "frequency CSV row '{label}' contains non-finite value"
            )));
        }
        values.push(numeric);
    }

    if values.len() != expected_values {
        return Err(GenevaError::InvalidInput(format!(
            "frequency CSV row '{label}' expected {expected_values} values but found {}",
            values.len()
        )));
    }

    Ok(Some((label.trim().to_string(), values)))
}

fn parse_recurrence_line(header_line: &str) -> Result<Vec<u32>, GenevaError> {
    let (_, recurrence_part) = header_line.split_once(':').ok_or_else(|| {
        GenevaError::InvalidInput("frequency CSV header must contain ':'".to_string())
    })?;

    let mut recurrence = Vec::new();
    for token in recurrence_part.split(',') {
        let cleaned = token.trim();
        if cleaned.is_empty() {
            continue;
        }
        recurrence.push(parse_positive_u32(cleaned, "ARI recurrence value")?);
    }

    if recurrence.is_empty() {
        return Err(GenevaError::InvalidInput(
            "frequency CSV header did not include ARI values".to_string(),
        ));
    }

    let recurrence_set: BTreeSet<u32> = recurrence.iter().copied().collect();
    if recurrence_set.len() != recurrence.len() {
        return Err(GenevaError::InvalidInput(
            "frequency CSV header includes duplicate ARI values".to_string(),
        ));
    }

    Ok(recurrence)
}

fn parse_positive_u32(value: &str, label: &str) -> Result<u32, GenevaError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        GenevaError::InvalidInput(format!("{label} value '{value}' is not numeric"))
    })?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(GenevaError::InvalidInput(format!(
            "{label} must be finite and > 0"
        )));
    }
    let rounded = parsed.round();
    if (parsed - rounded).abs() > 1e-9 {
        return Err(GenevaError::InvalidInput(format!(
            "{label} value '{value}' must be an integer"
        )));
    }
    if rounded > f64::from(u32::MAX) {
        return Err(GenevaError::InvalidInput(format!(
            "{label} value '{value}' exceeds supported bounds"
        )));
    }
    Ok(rounded as u32)
}

fn parse_duration_label_minutes(label: &str) -> Option<u32> {
    let compact = label.trim().to_ascii_lowercase().replace(' ', "");
    let (value_part, unit_part) = compact.split_once('-')?;
    let value = value_part.parse::<f64>().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }

    let minute_factor = if unit_part.starts_with("min") {
        1.0
    } else if unit_part.starts_with("hr") || unit_part.starts_with("hour") {
        60.0
    } else if unit_part.starts_with("day") {
        1440.0
    } else {
        return None;
    };

    let minutes = value * minute_factor;
    if !minutes.is_finite() || minutes <= 0.0 {
        return None;
    }
    let rounded = minutes.round();
    if (minutes - rounded).abs() > 1e-6 || rounded > f64::from(u32::MAX) {
        return None;
    }
    Some(rounded as u32)
}

fn float_to_minutes(value: f64, label: &str) -> Result<u32, GenevaError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(GenevaError::InvalidInput(format!(
            "{label} must be finite and > 0"
        )));
    }
    let rounded = value.round();
    if rounded > f64::from(u32::MAX) {
        return Err(GenevaError::InvalidInput(format!(
            "{label} is outside supported range"
        )));
    }
    Ok(rounded as u32)
}

fn sorted_u32(values: &[u32]) -> Vec<u32> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted
}

fn find_header_index(lines: &[&str], prefix: &str) -> Option<usize> {
    lines
        .iter()
        .position(|line| line.trim().to_ascii_lowercase().starts_with(prefix))
}

fn read_source_csv(
    source_root: &Path,
    configured_path: &str,
    datasource_id: &str,
    required: bool,
) -> Result<Option<String>, GenevaError> {
    let resolved_path = resolve_source_path(source_root, configured_path)?;
    match fs::read_to_string(&resolved_path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if !required && err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(GenevaError::InvalidInput(format!(
            "failed reading source CSV for {datasource_id} at '{}': {err}",
            resolved_path.display()
        ))),
    }
}

fn resolve_source_path(source_root: &Path, configured_path: &str) -> Result<PathBuf, GenevaError> {
    let source_path = Path::new(configured_path);
    if source_path.is_absolute() {
        Ok(source_path.to_path_buf())
    } else {
        Ok(source_root.join(source_path))
    }
}

fn validate_source_path(path_value: &str, field_name: &str) -> Result<(), GenevaError> {
    if path_value.trim().is_empty() {
        return Err(GenevaError::InvalidInput(format!(
            "{field_name} must not be empty"
        )));
    }
    if path_value.len() > MAX_SOURCE_PATH_LEN {
        return Err(GenevaError::InvalidInput(format!(
            "{field_name} length must be <= {MAX_SOURCE_PATH_LEN}"
        )));
    }
    if path_value.contains('\0') {
        return Err(GenevaError::InvalidInput(format!(
            "{field_name} contains a NUL byte"
        )));
    }
    if Path::new(path_value)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(GenevaError::InvalidInput(format!(
            "{field_name} must not contain parent-directory traversal segments"
        )));
    }
    Ok(())
}

fn default_distribution_type() -> String {
    DISTRIBUTION_NEH4_TYPE_B.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_file(filename: &str, contents: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be monotonic for test paths")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "geneva_frequency_panel_{}_{}",
            std::process::id(),
            suffix
        ));
        fs::create_dir_all(&dir).expect("temp directory should be created");
        let path = dir.join(filename);
        fs::write(&path, contents).expect("temp CSV should be written");
        path
    }

    fn sample_cligen_csv() -> &'static str {
        r#"
Point precipitation frequency estimates (mm, hours, mm/hour)
PRECIPITATION FREQUENCY ESTIMATES
by metric for ARI (years):, 1,2,5
Storm depth (mm):, 5,12,24
Storm duration (hours):, 0.1666667,0.5,1.0
10-min intensity (mm/hour):, 30,24,24

Date/time (GMT): Tue Apr 14 00:00:00 2026
"#
    }

    fn sample_noaa_csv() -> &'static str {
        r#"
Point precipitation frequency estimates (millimeters/hour)
PRECIPITATION FREQUENCY ESTIMATES
by duration for ARI (years):, 1,2,5
10-min:, 30,40,60
30-min:, 20,30,40
2-hr:, 8,9,12

Date/time (GMT): Tue Apr 14 00:00:00 2026
"#
    }

    fn request_with_sources(
        cligen_path: &Path,
        noaa_path: Option<&Path>,
    ) -> BuildFrequencyPanelRequest {
        BuildFrequencyPanelRequest {
            kernel_schema_version: FREQUENCY_PANEL_KERNEL_SCHEMA_VERSION,
            durations_minutes: vec![10, 30, 60],
            ari_years: vec![1, 2, 5],
            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
            allow_duration_interpolation: false,
            source_root: None,
            sources: Some(FrequencyPanelSources {
                cligen_freq: cligen_path.to_string_lossy().into_owned(),
                noaa14_pds: noaa_path.map(|path| path.to_string_lossy().into_owned()),
            }),
        }
    }

    #[test]
    fn cligen_only_marks_noaa_cells_as_source_missing() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let request = request_with_sources(
            &cligen_path,
            Some(Path::new("/tmp/definitely-missing-noaa.csv")),
        );
        let response = build_frequency_panel(&request).expect("panel build should succeed");

        let cligen_available = response
            .cells
            .iter()
            .filter(|cell| {
                cell.datasource_id == DATASOURCE_CLIGEN
                    && matches!(cell.availability, FrequencyCellAvailability::Available)
            })
            .count();
        assert_eq!(cligen_available, 3);

        let noaa_unavailable = response
            .cells
            .iter()
            .filter(|cell| {
                cell.datasource_id == DATASOURCE_NOAA
                    && cell.reason_code == Some(FrequencyUnavailableReasonCode::SourceMissing)
            })
            .count();
        assert_eq!(noaa_unavailable, 9);
    }

    #[test]
    fn dual_source_materializes_availability_independently() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let noaa_path = write_temp_file("noaa.csv", sample_noaa_csv());
        let request = request_with_sources(&cligen_path, Some(&noaa_path));
        let response = build_frequency_panel(&request).expect("panel build should succeed");

        let noaa_available = response
            .cells
            .iter()
            .filter(|cell| {
                cell.datasource_id == DATASOURCE_NOAA
                    && matches!(cell.availability, FrequencyCellAvailability::Available)
            })
            .count();
        let noaa_duration_unavailable = response
            .cells
            .iter()
            .filter(|cell| {
                cell.datasource_id == DATASOURCE_NOAA
                    && cell.reason_code == Some(FrequencyUnavailableReasonCode::DurationUnavailable)
            })
            .count();
        assert_eq!(noaa_available, 6);
        assert_eq!(noaa_duration_unavailable, 3);
    }

    #[test]
    fn requested_duration_not_in_source_remains_unavailable_without_fill() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let mut request = request_with_sources(&cligen_path, None);
        request.durations_minutes = vec![10, 20];
        request.ari_years = vec![1];

        let response = build_frequency_panel(&request).expect("panel build should succeed");
        let unavailable_cell = response
            .cells
            .iter()
            .find(|cell| {
                cell.datasource_id == DATASOURCE_CLIGEN
                    && cell.duration_minutes == 20
                    && cell.ari_years == 1
            })
            .expect("requested CLIGEN cell should exist");
        assert_eq!(
            unavailable_cell.reason_code,
            Some(FrequencyUnavailableReasonCode::DurationUnavailable)
        );
    }

    #[test]
    fn schema_invariants_hold_for_reason_codes() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let noaa_path = write_temp_file("noaa.csv", sample_noaa_csv());
        let request = request_with_sources(&cligen_path, Some(&noaa_path));
        let response = build_frequency_panel(&request).expect("panel build should succeed");

        for cell in &response.cells {
            match cell.availability {
                FrequencyCellAvailability::Available => {
                    assert!(cell.reason_code.is_none());
                    assert!(cell.depth_mm.is_some());
                    assert!(cell.intensity_mm_per_hr.is_some());
                }
                FrequencyCellAvailability::Unavailable => {
                    let reason = cell
                        .reason_code
                        .expect("unavailable cells need reason code");
                    assert!(matches!(
                        reason,
                        FrequencyUnavailableReasonCode::DurationUnavailable
                            | FrequencyUnavailableReasonCode::AriUnavailable
                            | FrequencyUnavailableReasonCode::SourceMissing
                    ));
                    assert!(cell.depth_mm.is_none());
                    assert!(cell.intensity_mm_per_hr.is_none());
                }
            }
            assert_eq!(cell.distribution_type, DISTRIBUTION_NEH4_TYPE_B);
        }
    }

    #[test]
    fn deterministic_cell_ordering_is_stable() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let noaa_path = write_temp_file("noaa.csv", sample_noaa_csv());
        let request = request_with_sources(&cligen_path, Some(&noaa_path));

        let first = build_frequency_panel(&request).expect("first build should succeed");
        let second = build_frequency_panel(&request).expect("second build should succeed");
        assert_eq!(first, second);

        let mut sorted = first.cells.clone();
        sorted.sort_by(|lhs, rhs| {
            lhs.datasource_id
                .cmp(&rhs.datasource_id)
                .then(lhs.duration_minutes.cmp(&rhs.duration_minutes))
                .then(lhs.ari_years.cmp(&rhs.ari_years))
                .then_with(|| {
                    let lhs_depth = lhs.depth_mm.unwrap_or(-1.0);
                    let rhs_depth = rhs.depth_mm.unwrap_or(-1.0);
                    lhs_depth.partial_cmp(&rhs_depth).unwrap_or(Ordering::Equal)
                })
                .then(lhs.storm_id.cmp(&rhs.storm_id))
        });
        assert_eq!(first.cells, sorted);
    }

    #[test]
    fn rejects_duration_interpolation_flag() {
        let cligen_path = write_temp_file("wepp_cli.csv", sample_cligen_csv());
        let mut request = request_with_sources(&cligen_path, None);
        request.allow_duration_interpolation = true;
        let error = request
            .validate()
            .expect_err("allow_duration_interpolation=true must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn rejects_parent_directory_source_paths() {
        let request = BuildFrequencyPanelRequest {
            kernel_schema_version: FREQUENCY_PANEL_KERNEL_SCHEMA_VERSION,
            durations_minutes: vec![10],
            ari_years: vec![1],
            distribution_type: DISTRIBUTION_NEH4_TYPE_B.to_string(),
            allow_duration_interpolation: false,
            source_root: None,
            sources: Some(FrequencyPanelSources {
                cligen_freq: "../wepp_cli.csv".to_string(),
                noaa14_pds: None,
            }),
        };
        let error = request
            .validate()
            .expect_err("parent-directory traversal must be rejected");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn invalid_cligen_csv_returns_typed_error() {
        let cligen_path = write_temp_file(
            "wepp_cli_bad.csv",
            "by metric for ARI (years):, 1,2\nStorm depth (mm):, 1\n",
        );
        let request = request_with_sources(&cligen_path, None);
        let error = build_frequency_panel(&request).expect_err("malformed CSV should fail");
        assert_eq!(error.code(), "invalid_input");
    }
}

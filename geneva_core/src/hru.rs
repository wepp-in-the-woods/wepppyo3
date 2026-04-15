use crate::error::GenevaError;
use raster::raster::Raster;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MIN_HRU_AREA_HA_FLOOR: f64 = 2.0;
const SQUARE_METERS_PER_HECTARE: f64 = 10_000.0;
const SQUARE_METERS_TO_ACRES: f64 = 0.000_247_105_381_467_165_3;
const FLOAT_TOLERANCE: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedHsgPolicy {
    Error,
    AssumeD,
}

fn default_unresolved_hsg_policy() -> UnresolvedHsgPolicy {
    UnresolvedHsgPolicy::Error
}

fn default_min_hru_area_ha() -> f64 {
    MIN_HRU_AREA_HA_FLOOR
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrepareHrusRequest {
    #[serde(alias = "schema_version")]
    pub kernel_schema_version: u32,
    #[serde(alias = "bound_path")]
    pub bound_tif: String,
    #[serde(alias = "landuse_path")]
    pub landuse_tif: String,
    #[serde(alias = "hydgrpdcd_path")]
    pub hydgrpdcd_tif: String,
    #[serde(default, alias = "burn_severity_path")]
    pub burn_severity_tif: Option<String>,
    #[serde(default)]
    pub default_hsg_code: Option<u8>,
    #[serde(default)]
    pub default_hsg_derivation: Option<String>,
    #[serde(default = "default_unresolved_hsg_policy")]
    pub unresolved_hsg_policy: UnresolvedHsgPolicy,
    #[serde(default)]
    pub strict_burn_nodata: bool,
    #[serde(default)]
    pub allow_cross_hsg_merge: bool,
    #[serde(default = "default_min_hru_area_ha")]
    pub min_hru_area_ha: f64,
    #[serde(default)]
    pub hydrophobic_forest_high: bool,
    #[serde(default)]
    pub hydrophobic_forest_moderate: bool,
    #[serde(default)]
    pub hydrophobic_shrub_high: bool,
    #[serde(default)]
    pub hydrophobic_shrub_moderate: bool,
}

impl PrepareHrusRequest {
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
        if self.kernel_schema_version == 0 {
            return Err(GenevaError::InvalidInput(
                "kernel_schema_version must be >= 1".to_string(),
            ));
        }

        if self.bound_tif.trim().is_empty()
            || self.landuse_tif.trim().is_empty()
            || self.hydgrpdcd_tif.trim().is_empty()
        {
            return Err(GenevaError::InvalidInput(
                "bound_tif, landuse_tif, and hydgrpdcd_tif are required".to_string(),
            ));
        }

        if let Some(code) = self.default_hsg_code {
            if !(1..=4).contains(&code) {
                return Err(GenevaError::InvalidInput(
                    "default_hsg_code must be one of 1,2,3,4 when provided".to_string(),
                ));
            }
        }

        if !self.min_hru_area_ha.is_finite() || self.min_hru_area_ha < MIN_HRU_AREA_HA_FLOOR {
            return Err(GenevaError::InvalidInput(format!(
                "min_hru_area_ha must be >= {MIN_HRU_AREA_HA_FLOOR}"
            )));
        }

        if let Some(derivation) = self.default_hsg_derivation.as_deref() {
            let is_valid = matches!(derivation, "user_override" | "dominant_soil" | "assume_d");
            if !is_valid {
                return Err(GenevaError::InvalidInput(
                    "default_hsg_derivation must be user_override, dominant_soil, or assume_d"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PrepareHrusResponse {
    pub status: String,
    pub phase: String,
    pub kernel_schema_version: u32,
    pub hru_rows: Vec<HruOutputRow>,
    pub diagnostics: HruDiagnostics,
    pub warnings: Vec<KernelWarning>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HruOutputRow {
    pub hru_id: String,
    pub area_m2: f64,
    pub area_ac: f64,
    pub area_fraction: f64,
    pub landuse_class: i32,
    pub hsg_group: String,
    pub burn_severity_class: String,
    pub hydrophobic_class: bool,
    pub is_water: bool,
    pub cn_arc_ii: f64,
    pub cn_lambda_020: f64,
    pub cn_lambda_005: f64,
    pub antecedent_condition_source: String,
    pub cn_source: String,
    pub hsg_source: String,
    pub collapsed_from_hru_ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KernelWarning {
    pub code: String,
    pub reason: String,
    pub cell_count: usize,
    pub area_m2: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HruDiagnostics {
    pub inbound_cell_count: usize,
    pub inbound_area_m2: f64,
    pub hru_area_total_m2: f64,
    pub cell_area_m2: f64,
    pub min_hru_area_m2: f64,
    pub allow_cross_hsg_merge: bool,
    pub default_hsg_code: Option<String>,
    pub default_hsg_derivation: Option<String>,
    pub unresolved_hsg_policy: String,
    pub hsg_provenance_counts: BTreeMap<String, usize>,
    pub hsg_fallback_cell_count: usize,
    pub hsg_fallback_area_m2: f64,
    pub unresolved_hsg_cell_count: usize,
    pub invalid_hydgrpdcd_cell_count: usize,
    pub burn_nodata_cell_count: usize,
    pub collapse_donor_count: usize,
    pub collapse_merge_count: usize,
    pub collapse_unmerged_count: usize,
    pub area_closure_error_m2: f64,
    pub alignment: AlignmentDiagnostics,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AlignmentDiagnostics {
    pub canonical: CanonicalGridDiagnostics,
    pub landuse: RasterAlignmentStatus,
    pub hydgrpdcd: RasterAlignmentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burn_severity: Option<RasterAlignmentStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CanonicalGridDiagnostics {
    pub path: String,
    pub width: usize,
    pub height: usize,
    pub geo_transform: [f64; 6],
    pub projection: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RasterAlignmentStatus {
    pub path: String,
    pub width: usize,
    pub height: usize,
    pub matched_canonical: bool,
    pub resampled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum BurnSeverity {
    Unburned,
    Low,
    Moderate,
    High,
}

impl BurnSeverity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unburned => "unburned",
            Self::Low => "low",
            Self::Moderate => "moderate",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum HsgGroup {
    A,
    B,
    C,
    D,
}

impl HsgGroup {
    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::A),
            2 => Some(Self::B),
            3 => Some(Self::C),
            4 => Some(Self::D),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HsgSource {
    CodedLookup,
    DefaultHsgFallback,
    AssumeDPolicy,
}

impl HsgSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::CodedLookup => "coded_lookup",
            Self::DefaultHsgFallback => "default_hsg_fallback",
            Self::AssumeDPolicy => "assume_d_policy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct HruKey {
    landuse_class: i32,
    hsg_group: HsgGroup,
    burn_severity: BurnSeverity,
    hydrophobic_class: bool,
    is_water: bool,
}

#[derive(Debug, Clone, Copy)]
struct CellAttributes {
    key: HruKey,
    hsg_source: HsgSource,
}

#[derive(Debug, Clone, PartialEq)]
struct HruAggregate {
    hru_id: String,
    key: HruKey,
    area_m2: f64,
    cn_arc_ii: f64,
    hsg_source_counts: BTreeMap<HsgSource, usize>,
    collapsed_from_hru_ids: Vec<String>,
    warnings: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct GridData {
    width: usize,
    height: usize,
    geo_transform: [f64; 6],
    projection: Option<String>,
    nodata: Option<i32>,
    data: Vec<i32>,
    path: String,
}

impl GridData {
    fn cell_area_m2(&self) -> f64 {
        let determinant = self.geo_transform[1] * self.geo_transform[5]
            - self.geo_transform[2] * self.geo_transform[4];
        determinant.abs()
    }

    fn len(&self) -> usize {
        self.width * self.height
    }

    fn xy_to_index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

#[derive(Debug, Clone)]
struct WarningTally {
    reason: String,
    cell_count: usize,
    area_m2: f64,
}

#[derive(Debug, Default, Clone)]
struct WarningAccumulator {
    tallies: BTreeMap<String, WarningTally>,
}

impl WarningAccumulator {
    fn add(&mut self, code: &str, reason: &str, cell_count: usize, area_m2: f64) {
        let entry = self
            .tallies
            .entry(code.to_string())
            .or_insert(WarningTally {
                reason: reason.to_string(),
                cell_count: 0,
                area_m2: 0.0,
            });
        entry.cell_count += cell_count;
        entry.area_m2 += area_m2;
    }

    fn into_sorted_vec(self) -> Vec<KernelWarning> {
        self.tallies
            .into_iter()
            .map(|(code, tally)| KernelWarning {
                code,
                reason: tally.reason,
                cell_count: tally.cell_count,
                area_m2: tally.area_m2,
            })
            .collect()
    }
}

pub fn prepare_hrus(request: &PrepareHrusRequest) -> Result<PrepareHrusResponse, GenevaError> {
    request.validate()?;

    let bound = load_grid(&request.bound_tif, "bound")?;
    let landuse = load_grid(&request.landuse_tif, "landuse")?;
    let hydgrpdcd = load_grid(&request.hydgrpdcd_tif, "hydgrpdcd")?;
    let burn = match request.burn_severity_tif.as_deref() {
        Some(path) if !path.trim().is_empty() => Some(load_grid(path, "burn_severity")?),
        _ => None,
    };

    let canonical = CanonicalGridDiagnostics {
        path: bound.path.clone(),
        width: bound.width,
        height: bound.height,
        geo_transform: bound.geo_transform,
        projection: bound.projection.clone(),
    };

    let (landuse_aligned, landuse_alignment) = align_to_canonical_grid(&bound, &landuse)?;
    let (hydgrpdcd_aligned, hydgrpdcd_alignment) = align_to_canonical_grid(&bound, &hydgrpdcd)?;
    let (burn_aligned, burn_alignment) = if let Some(burn_grid) = burn {
        let (aligned, status) = align_to_canonical_grid(&bound, &burn_grid)?;
        (Some(aligned), Some(status))
    } else {
        (None, None)
    };

    let alignment = AlignmentDiagnostics {
        canonical,
        landuse: landuse_alignment,
        hydgrpdcd: hydgrpdcd_alignment,
        burn_severity: burn_alignment,
    };

    prepare_hrus_from_grids(
        request,
        &bound,
        &landuse_aligned,
        &hydgrpdcd_aligned,
        burn_aligned.as_ref(),
        alignment,
    )
}

pub fn serialize_prepare_hrus_response(
    response: &PrepareHrusResponse,
) -> Result<String, GenevaError> {
    serde_json::to_string(response).map_err(|err| GenevaError::Serialization(err.to_string()))
}

fn prepare_hrus_from_grids(
    request: &PrepareHrusRequest,
    bound: &GridData,
    landuse: &GridData,
    hydgrpdcd: &GridData,
    burn_severity: Option<&GridData>,
    alignment: AlignmentDiagnostics,
) -> Result<PrepareHrusResponse, GenevaError> {
    let cell_area_m2 = bound.cell_area_m2();
    if !cell_area_m2.is_finite() || cell_area_m2 <= 0.0 {
        return Err(GenevaError::ContractViolation(
            "cell area must be finite and positive".to_string(),
        ));
    }

    if bound.len() != landuse.len() || bound.len() != hydgrpdcd.len() {
        return Err(GenevaError::Alignment(
            "aligned raster length mismatch against canonical bound grid".to_string(),
        ));
    }
    if let Some(burn) = burn_severity {
        if bound.len() != burn.len() {
            return Err(GenevaError::Alignment(
                "aligned burn raster length mismatch against canonical bound grid".to_string(),
            ));
        }
    }

    let min_hru_area_m2 = request.min_hru_area_ha * SQUARE_METERS_PER_HECTARE;
    let unresolved_policy_text = match request.unresolved_hsg_policy {
        UnresolvedHsgPolicy::Error => "error",
        UnresolvedHsgPolicy::AssumeD => "assume_d",
    }
    .to_string();

    let mut diagnostics = HruDiagnostics {
        inbound_cell_count: 0,
        inbound_area_m2: 0.0,
        hru_area_total_m2: 0.0,
        cell_area_m2,
        min_hru_area_m2,
        allow_cross_hsg_merge: request.allow_cross_hsg_merge,
        default_hsg_code: request
            .default_hsg_code
            .and_then(|code| HsgGroup::from_code(code).map(|group| group.as_str().to_string())),
        default_hsg_derivation: request.default_hsg_derivation.clone(),
        unresolved_hsg_policy: unresolved_policy_text,
        hsg_provenance_counts: BTreeMap::new(),
        hsg_fallback_cell_count: 0,
        hsg_fallback_area_m2: 0.0,
        unresolved_hsg_cell_count: 0,
        invalid_hydgrpdcd_cell_count: 0,
        burn_nodata_cell_count: 0,
        collapse_donor_count: 0,
        collapse_merge_count: 0,
        collapse_unmerged_count: 0,
        area_closure_error_m2: 0.0,
        alignment,
    };

    if diagnostics.default_hsg_derivation.is_none() && request.default_hsg_code.is_some() {
        diagnostics.default_hsg_derivation = Some("user_override".to_string());
    }

    let mut warning_accumulator = WarningAccumulator::default();
    if diagnostics.alignment.landuse.resampled {
        warning_accumulator.add(
            "landuse_resampled_to_bound",
            "Landuse raster was nearest-neighbor resampled onto canonical bound grid.",
            0,
            0.0,
        );
    }
    if diagnostics.alignment.hydgrpdcd.resampled {
        warning_accumulator.add(
            "hydgrpdcd_resampled_to_bound",
            "hydgrpdcd raster was nearest-neighbor resampled onto canonical bound grid.",
            0,
            0.0,
        );
    }
    if diagnostics
        .alignment
        .burn_severity
        .as_ref()
        .is_some_and(|status| status.resampled)
    {
        warning_accumulator.add(
            "burn_resampled_to_bound",
            "Burn-severity raster was nearest-neighbor resampled onto canonical bound grid.",
            0,
            0.0,
        );
    }

    let mut cell_attributes: Vec<Option<CellAttributes>> = vec![None; bound.len()];

    for (idx, cell_slot) in cell_attributes.iter_mut().enumerate().take(bound.len()) {
        if bound.data[idx] != 1 {
            continue;
        }

        diagnostics.inbound_cell_count += 1;

        let landuse_value = landuse.data[idx];
        if is_nodata(landuse_value, landuse.nodata) {
            return Err(GenevaError::ContractViolation(
                "landuse nodata encountered inside bound==1 extent".to_string(),
            ));
        }

        let burn_class = resolve_burn_severity(
            burn_severity,
            idx,
            request.strict_burn_nodata,
            cell_area_m2,
            &mut diagnostics,
            &mut warning_accumulator,
        )?;

        let hsg_code = hydgrpdcd.data[idx];
        let (hsg_group, hsg_source) = resolve_hsg(
            request,
            hsg_code,
            hydgrpdcd.nodata,
            cell_area_m2,
            &mut diagnostics,
            &mut warning_accumulator,
        )?;

        let hydrophobic_class = derive_hydrophobic(landuse_value, burn_class, request);
        let is_water = landuse_value == 11;

        let key = HruKey {
            landuse_class: landuse_value,
            hsg_group,
            burn_severity: burn_class,
            hydrophobic_class,
            is_water,
        };

        *cell_slot = Some(CellAttributes { key, hsg_source });

        let source_key = hsg_source.as_str().to_string();
        *diagnostics
            .hsg_provenance_counts
            .entry(source_key)
            .or_insert(0) += 1;
    }

    diagnostics.inbound_area_m2 = diagnostics.inbound_cell_count as f64 * cell_area_m2;

    if diagnostics.inbound_cell_count == 0 {
        return Err(GenevaError::ContractViolation(
            "bound raster contains zero in-bound cells (bound==1)".to_string(),
        ));
    }

    let mut rows = build_initial_hrus(&cell_attributes, bound.width, bound.height, cell_area_m2);
    collapse_small_hrus(
        &mut rows,
        min_hru_area_m2,
        request.allow_cross_hsg_merge,
        &mut warning_accumulator,
        &mut diagnostics,
    );

    rows.sort_by(|lhs, rhs| lhs.hru_id.cmp(&rhs.hru_id));

    diagnostics.hru_area_total_m2 = rows.iter().map(|row| row.area_m2).sum::<f64>();
    diagnostics.area_closure_error_m2 =
        (diagnostics.hru_area_total_m2 - diagnostics.inbound_area_m2).abs();

    if diagnostics.area_closure_error_m2 > diagnostics.cell_area_m2 + FLOAT_TOLERANCE {
        return Err(GenevaError::ContractViolation(format!(
            "area closure check failed: |{} - {}| = {} > {}",
            diagnostics.hru_area_total_m2,
            diagnostics.inbound_area_m2,
            diagnostics.area_closure_error_m2,
            diagnostics.cell_area_m2
        )));
    }

    let hru_rows: Vec<HruOutputRow> = rows
        .into_iter()
        .map(|row| {
            let mut row_warnings: Vec<String> = row.warnings.into_iter().collect();
            row_warnings.sort();

            let cn_lambda_020 = row.cn_arc_ii;
            let cn_lambda_005 = derive_cn_lambda_005(row.cn_arc_ii);

            HruOutputRow {
                hru_id: row.hru_id,
                area_m2: row.area_m2,
                area_ac: row.area_m2 * SQUARE_METERS_TO_ACRES,
                area_fraction: row.area_m2 / diagnostics.inbound_area_m2,
                landuse_class: row.key.landuse_class,
                hsg_group: row.key.hsg_group.as_str().to_string(),
                burn_severity_class: row.key.burn_severity.as_str().to_string(),
                hydrophobic_class: row.key.hydrophobic_class,
                is_water: row.key.is_water,
                cn_arc_ii: row.cn_arc_ii,
                cn_lambda_020,
                cn_lambda_005,
                antecedent_condition_source: "arc_ii_seed".to_string(),
                cn_source: "geneva_proxy_cn_v1".to_string(),
                hsg_source: classify_row_hsg_source(&row.hsg_source_counts),
                collapsed_from_hru_ids: row.collapsed_from_hru_ids,
                warnings: row_warnings,
            }
        })
        .collect();

    Ok(PrepareHrusResponse {
        status: "ok".to_string(),
        phase: "prepare_hrus".to_string(),
        kernel_schema_version: request.kernel_schema_version,
        hru_rows,
        diagnostics,
        warnings: warning_accumulator.into_sorted_vec(),
    })
}

fn resolve_burn_severity(
    burn_grid: Option<&GridData>,
    idx: usize,
    strict_burn_nodata: bool,
    cell_area_m2: f64,
    diagnostics: &mut HruDiagnostics,
    warning_accumulator: &mut WarningAccumulator,
) -> Result<BurnSeverity, GenevaError> {
    let Some(grid) = burn_grid else {
        return Ok(BurnSeverity::Unburned);
    };

    let burn_value = grid.data[idx];
    let is_burn_nodata = is_nodata(burn_value, grid.nodata) || burn_value == 255;

    if is_burn_nodata {
        diagnostics.burn_nodata_cell_count += 1;
        if strict_burn_nodata {
            return Err(GenevaError::ContractViolation(
                "burn nodata encountered inside bound==1 while strict_burn_nodata=true".to_string(),
            ));
        }
        warning_accumulator.add(
            "burn_nodata_fallback",
            "Burn nodata inside bound was mapped to unburned (strict_burn_nodata=false).",
            1,
            cell_area_m2,
        );
        return Ok(BurnSeverity::Unburned);
    }

    match burn_value {
        0 => Ok(BurnSeverity::Unburned),
        1 => Ok(BurnSeverity::Low),
        2 => Ok(BurnSeverity::Moderate),
        3 => Ok(BurnSeverity::High),
        _ => Err(GenevaError::ContractViolation(format!(
            "unexpected burn severity code inside bound: {burn_value}"
        ))),
    }
}

fn resolve_hsg(
    request: &PrepareHrusRequest,
    raw_code: i32,
    hydgrpdcd_nodata: Option<i32>,
    cell_area_m2: f64,
    diagnostics: &mut HruDiagnostics,
    warning_accumulator: &mut WarningAccumulator,
) -> Result<(HsgGroup, HsgSource), GenevaError> {
    let normalized_code = if is_nodata(raw_code, hydgrpdcd_nodata) {
        0
    } else {
        raw_code
    };

    if (1..=4).contains(&normalized_code) {
        if let Some(group) = HsgGroup::from_code(normalized_code as u8) {
            return Ok((group, HsgSource::CodedLookup));
        }
    }

    diagnostics.unresolved_hsg_cell_count += 1;

    if normalized_code != 0 {
        diagnostics.invalid_hydgrpdcd_cell_count += 1;
        warning_accumulator.add(
            "hydgrpdcd_invalid_code",
            "Unexpected hydgrpdcd code was treated as unresolved and sent through fallback chain.",
            1,
            cell_area_m2,
        );
    }

    if let Some(default_code) = request.default_hsg_code {
        if let Some(group) = HsgGroup::from_code(default_code) {
            diagnostics.hsg_fallback_cell_count += 1;
            diagnostics.hsg_fallback_area_m2 += cell_area_m2;

            warning_accumulator.add(
                "hsg_default_fallback",
                "default_hsg_code fallback applied to unresolved hydgrpdcd cell.",
                1,
                cell_area_m2,
            );
            return Ok((group, HsgSource::DefaultHsgFallback));
        }
    }

    match request.unresolved_hsg_policy {
        UnresolvedHsgPolicy::AssumeD => {
            diagnostics.hsg_fallback_cell_count += 1;
            diagnostics.hsg_fallback_area_m2 += cell_area_m2;

            warning_accumulator.add(
                "hsg_assume_d_policy",
                "unresolved_hsg_policy=assume_d coerced unresolved hydgrpdcd cell to HSG D.",
                1,
                cell_area_m2,
            );
            Ok((HsgGroup::D, HsgSource::AssumeDPolicy))
        }
        UnresolvedHsgPolicy::Error => Err(GenevaError::ContractViolation(
            "unresolved HSG cell remained after coded_lookup/default fallback and unresolved_hsg_policy=error"
                .to_string(),
        )),
    }
}

fn derive_hydrophobic(
    landuse_class: i32,
    burn_severity: BurnSeverity,
    request: &PrepareHrusRequest,
) -> bool {
    let is_forest = matches!(landuse_class, 41..=43);
    let is_shrub = landuse_class == 52;

    if is_forest {
        return match burn_severity {
            BurnSeverity::High => request.hydrophobic_forest_high,
            BurnSeverity::Moderate => request.hydrophobic_forest_moderate,
            _ => false,
        };
    }

    if is_shrub {
        return match burn_severity {
            BurnSeverity::High => request.hydrophobic_shrub_high,
            BurnSeverity::Moderate => request.hydrophobic_shrub_moderate,
            _ => false,
        };
    }

    false
}

fn build_initial_hrus(
    cell_attributes: &[Option<CellAttributes>],
    width: usize,
    height: usize,
    cell_area_m2: f64,
) -> Vec<HruAggregate> {
    let mut visited = vec![false; cell_attributes.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut component_seq_by_key: BTreeMap<HruKey, usize> = BTreeMap::new();
    let mut rows: Vec<HruAggregate> = Vec::new();

    for idx in 0..cell_attributes.len() {
        if visited[idx] || cell_attributes[idx].is_none() {
            continue;
        }

        let cell = cell_attributes[idx].expect("checked none before component walk");
        let key = cell.key;

        visited[idx] = true;
        queue.push_back(idx);

        let mut component_cell_count: usize = 0;
        let mut hsg_source_counts: BTreeMap<HsgSource, usize> = BTreeMap::new();

        while let Some(current_idx) = queue.pop_front() {
            let current_cell = match cell_attributes[current_idx] {
                Some(value) => value,
                None => continue,
            };

            if current_cell.key != key {
                continue;
            }

            component_cell_count += 1;
            *hsg_source_counts
                .entry(current_cell.hsg_source)
                .or_insert(0) += 1;

            let x = current_idx % width;
            let y = current_idx / width;

            if x > 0 {
                maybe_enqueue_neighbor(
                    current_idx - 1,
                    key,
                    cell_attributes,
                    &mut visited,
                    &mut queue,
                );
            }
            if x + 1 < width {
                maybe_enqueue_neighbor(
                    current_idx + 1,
                    key,
                    cell_attributes,
                    &mut visited,
                    &mut queue,
                );
            }
            if y > 0 {
                maybe_enqueue_neighbor(
                    current_idx - width,
                    key,
                    cell_attributes,
                    &mut visited,
                    &mut queue,
                );
            }
            if y + 1 < height {
                maybe_enqueue_neighbor(
                    current_idx + width,
                    key,
                    cell_attributes,
                    &mut visited,
                    &mut queue,
                );
            }
        }

        let sequence = component_seq_by_key.entry(key).or_insert(0);
        *sequence += 1;

        let hru_id = format!(
            "lu{}_hsg{}_burn{}_hyd{}_c{:03}",
            key.landuse_class,
            key.hsg_group.as_str(),
            key.burn_severity.as_str(),
            if key.hydrophobic_class { 1 } else { 0 },
            *sequence
        );

        rows.push(HruAggregate {
            hru_id,
            key,
            area_m2: component_cell_count as f64 * cell_area_m2,
            cn_arc_ii: estimate_cn_arc_ii(key),
            hsg_source_counts,
            collapsed_from_hru_ids: Vec::new(),
            warnings: BTreeSet::new(),
        });
    }

    rows
}

fn maybe_enqueue_neighbor(
    neighbor_idx: usize,
    key: HruKey,
    cell_attributes: &[Option<CellAttributes>],
    visited: &mut [bool],
    queue: &mut VecDeque<usize>,
) {
    if visited[neighbor_idx] {
        return;
    }

    if let Some(neighbor_cell) = cell_attributes[neighbor_idx] {
        if neighbor_cell.key == key {
            visited[neighbor_idx] = true;
            queue.push_back(neighbor_idx);
        }
    }
}

fn collapse_small_hrus(
    rows: &mut Vec<HruAggregate>,
    min_hru_area_m2: f64,
    allow_cross_hsg_merge: bool,
    warning_accumulator: &mut WarningAccumulator,
    diagnostics: &mut HruDiagnostics,
) {
    diagnostics.collapse_donor_count = rows
        .iter()
        .filter(|row| row.area_m2 < min_hru_area_m2)
        .count();

    loop {
        let mut donor_order: Vec<(f64, String)> = rows
            .iter()
            .filter(|row| row.area_m2 < min_hru_area_m2)
            .map(|row| (row.area_m2, row.hru_id.clone()))
            .collect();

        if donor_order.is_empty() {
            break;
        }

        donor_order.sort_by(|lhs, rhs| compare_f64(lhs.0, rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));

        let mut merged_any = false;

        for (_, donor_id) in donor_order {
            let Some(donor_idx) = rows
                .iter()
                .position(|row| row.hru_id == donor_id && row.area_m2 < min_hru_area_m2)
            else {
                continue;
            };

            if let Some(recipient_idx) =
                find_recipient_index(rows, donor_idx, allow_cross_hsg_merge)
            {
                merge_donor_into_recipient(rows, donor_idx, recipient_idx, warning_accumulator);
                diagnostics.collapse_merge_count += 1;
                merged_any = true;
            } else {
                let donor = &mut rows[donor_idx];
                if donor
                    .warnings
                    .insert("collapse_no_compatible_recipient".to_string())
                {
                    warning_accumulator.add(
                        "collapse_no_compatible_recipient",
                        "No compatible recipient found; donor HRU was retained to preserve water safety or area closure.",
                        0,
                        donor.area_m2,
                    );
                }
            }
        }

        if !merged_any {
            break;
        }
    }

    diagnostics.collapse_unmerged_count = rows
        .iter()
        .filter(|row| row.area_m2 < min_hru_area_m2)
        .count();
}

fn find_recipient_index(
    rows: &[HruAggregate],
    donor_idx: usize,
    allow_cross_hsg_merge: bool,
) -> Option<usize> {
    let donor = &rows[donor_idx];
    let mut candidates: Vec<(usize, f64, f64, String)> = Vec::new();

    for (idx, candidate) in rows.iter().enumerate() {
        if idx == donor_idx {
            continue;
        }
        if !is_compatible_recipient(donor, candidate, allow_cross_hsg_merge) {
            continue;
        }

        candidates.push((
            idx,
            (candidate.cn_arc_ii - donor.cn_arc_ii).abs(),
            candidate.area_m2,
            candidate.hru_id.clone(),
        ));
    }

    candidates.sort_by(|lhs, rhs| {
        compare_f64(lhs.1, rhs.1)
            .then_with(|| compare_f64(rhs.2, lhs.2))
            .then_with(|| lhs.3.cmp(&rhs.3))
    });

    candidates.first().map(|candidate| candidate.0)
}

fn is_compatible_recipient(
    donor: &HruAggregate,
    candidate: &HruAggregate,
    allow_cross_hsg_merge: bool,
) -> bool {
    donor.key.is_water == candidate.key.is_water
        && donor.key.landuse_class == candidate.key.landuse_class
        && donor.key.burn_severity == candidate.key.burn_severity
        && donor.key.hydrophobic_class == candidate.key.hydrophobic_class
        && (allow_cross_hsg_merge || donor.key.hsg_group == candidate.key.hsg_group)
}

fn merge_donor_into_recipient(
    rows: &mut Vec<HruAggregate>,
    donor_idx: usize,
    recipient_idx: usize,
    warning_accumulator: &mut WarningAccumulator,
) {
    let donor = rows.remove(donor_idx);
    let adjusted_recipient_idx = if recipient_idx > donor_idx {
        recipient_idx - 1
    } else {
        recipient_idx
    };

    let recipient = &mut rows[adjusted_recipient_idx];
    recipient.area_m2 += donor.area_m2;

    recipient.collapsed_from_hru_ids.push(donor.hru_id.clone());
    recipient
        .collapsed_from_hru_ids
        .extend(donor.collapsed_from_hru_ids);
    recipient.collapsed_from_hru_ids.sort();
    recipient.collapsed_from_hru_ids.dedup();

    for (source, count) in donor.hsg_source_counts {
        *recipient.hsg_source_counts.entry(source).or_insert(0) += count;
    }

    for warning in donor.warnings {
        recipient.warnings.insert(warning);
    }

    if donor.key.hsg_group != recipient.key.hsg_group {
        recipient.warnings.insert("cross_hsg_merge".to_string());
        warning_accumulator.add(
            "cross_hsg_merge",
            "Cross-HSG merge applied because allow_cross_hsg_merge=true.",
            0,
            donor.area_m2,
        );
    }
}

fn classify_row_hsg_source(source_counts: &BTreeMap<HsgSource, usize>) -> String {
    if source_counts.is_empty() {
        return "unknown".to_string();
    }
    if source_counts.len() == 1 {
        if let Some((source, _)) = source_counts.iter().next() {
            return source.as_str().to_string();
        }
    }
    "mixed".to_string()
}

fn estimate_cn_arc_ii(key: HruKey) -> f64 {
    if key.is_water {
        return 100.0;
    }

    let base_cn = match key.landuse_class {
        11 => 100.0,
        41..=43 => 55.0,
        52 => 65.0,
        71 => 68.0,
        81 => 74.0,
        82 => 78.0,
        _ => 75.0,
    };

    let hsg_adjustment = match key.hsg_group {
        HsgGroup::A => 0.0,
        HsgGroup::B => 7.0,
        HsgGroup::C => 14.0,
        HsgGroup::D => 21.0,
    };

    let burn_adjustment = match key.burn_severity {
        BurnSeverity::Unburned => 0.0,
        BurnSeverity::Low => 2.0,
        BurnSeverity::Moderate => 5.0,
        BurnSeverity::High => 8.0,
    };

    let hydrophobic_adjustment = if key.hydrophobic_class { 6.0 } else { 0.0 };

    let cn_arc_ii: f64 = base_cn + hsg_adjustment + burn_adjustment + hydrophobic_adjustment;
    cn_arc_ii.clamp(30.0_f64, 100.0_f64)
}

fn derive_cn_lambda_005(cn_arc_ii: f64) -> f64 {
    if cn_arc_ii >= (100.0 - FLOAT_TOLERANCE) {
        return 100.0;
    }
    if cn_arc_ii > 98.5 {
        return cn_arc_ii.clamp(0.0, 100.0);
    }

    let term = (100.0 / cn_arc_ii) - 1.0;
    let denominator = (1.879 * term.powf(1.15)) + 1.0;
    (100.0 / denominator).clamp(0.0, 100.0)
}

fn load_grid(path: &str, role: &str) -> Result<GridData, GenevaError> {
    let raster = Raster::<i32>::read(path).map_err(|err| {
        GenevaError::RasterIo(format!("failed to read {role} raster at '{path}': {err}"))
    })?;

    Ok(GridData {
        width: raster.width,
        height: raster.height,
        geo_transform: raster.geo_transform,
        projection: raster.proj4,
        nodata: raster.no_data,
        data: raster.data,
        path: path.to_string(),
    })
}

fn align_to_canonical_grid(
    canonical: &GridData,
    source: &GridData,
) -> Result<(GridData, RasterAlignmentStatus), GenevaError> {
    if !projection_matches(canonical, source) {
        return Err(GenevaError::Alignment(format!(
            "projection mismatch between canonical '{}' and '{}'",
            canonical.path, source.path
        )));
    }

    if grids_match(canonical, source) {
        return Ok((
            source.clone(),
            RasterAlignmentStatus {
                path: source.path.clone(),
                width: source.width,
                height: source.height,
                matched_canonical: true,
                resampled: false,
            },
        ));
    }

    let resampled = nearest_neighbor_resample(source, canonical)?;

    Ok((
        resampled,
        RasterAlignmentStatus {
            path: source.path.clone(),
            width: canonical.width,
            height: canonical.height,
            matched_canonical: false,
            resampled: true,
        },
    ))
}

fn grids_match(canonical: &GridData, source: &GridData) -> bool {
    canonical.width == source.width
        && canonical.height == source.height
        && geo_transform_matches(canonical.geo_transform, source.geo_transform)
}

fn projection_matches(canonical: &GridData, source: &GridData) -> bool {
    match (&canonical.projection, &source.projection) {
        (Some(lhs), Some(rhs)) => normalize_projection(lhs) == normalize_projection(rhs),
        (None, None) => true,
        _ => false,
    }
}

fn normalize_projection(proj: &str) -> String {
    proj.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn geo_transform_matches(lhs: [f64; 6], rhs: [f64; 6]) -> bool {
    lhs.iter()
        .zip(rhs.iter())
        .all(|(a, b)| (a - b).abs() <= FLOAT_TOLERANCE)
}

fn nearest_neighbor_resample(
    source: &GridData,
    canonical: &GridData,
) -> Result<GridData, GenevaError> {
    let inverse = invert_affine(source.geo_transform)?;

    let mut data = vec![source.nodata.unwrap_or(0); canonical.len()];

    for y in 0..canonical.height {
        for x in 0..canonical.width {
            let canonical_idx = canonical.xy_to_index(x, y);
            let (x_center, y_center) = pixel_center(canonical.geo_transform, x, y);

            let (src_x, src_y) = world_to_pixel(inverse, source.geo_transform, x_center, y_center);
            if src_x >= 0
                && src_x < source.width as isize
                && src_y >= 0
                && src_y < source.height as isize
            {
                let source_idx = source.xy_to_index(src_x as usize, src_y as usize);
                data[canonical_idx] = source.data[source_idx];
            } else if let Some(nodata) = source.nodata {
                data[canonical_idx] = nodata;
            } else {
                return Err(GenevaError::Alignment(format!(
                    "nearest-neighbor resample from '{}' to canonical grid sampled outside source extent without nodata",
                    source.path
                )));
            }
        }
    }

    Ok(GridData {
        width: canonical.width,
        height: canonical.height,
        geo_transform: canonical.geo_transform,
        projection: canonical.projection.clone(),
        nodata: source.nodata,
        data,
        path: source.path.clone(),
    })
}

fn invert_affine(geo_transform: [f64; 6]) -> Result<[f64; 4], GenevaError> {
    let a = geo_transform[1];
    let b = geo_transform[2];
    let d = geo_transform[4];
    let e = geo_transform[5];

    let determinant = (a * e) - (b * d);
    if determinant.abs() <= FLOAT_TOLERANCE {
        return Err(GenevaError::Alignment(
            "cannot invert geotransform; determinant is near zero".to_string(),
        ));
    }

    let inv_a = e / determinant;
    let inv_b = -b / determinant;
    let inv_d = -d / determinant;
    let inv_e = a / determinant;

    Ok([inv_a, inv_b, inv_d, inv_e])
}

fn pixel_center(geo_transform: [f64; 6], x: usize, y: usize) -> (f64, f64) {
    let xf = x as f64 + 0.5;
    let yf = y as f64 + 0.5;

    let world_x = geo_transform[0] + (xf * geo_transform[1]) + (yf * geo_transform[2]);
    let world_y = geo_transform[3] + (xf * geo_transform[4]) + (yf * geo_transform[5]);
    (world_x, world_y)
}

fn world_to_pixel(
    inverse: [f64; 4],
    geo_transform: [f64; 6],
    world_x: f64,
    world_y: f64,
) -> (isize, isize) {
    let dx = world_x - geo_transform[0];
    let dy = world_y - geo_transform[3];

    let px_center = inverse[0] * dx + inverse[1] * dy;
    let py_center = inverse[2] * dx + inverse[3] * dy;

    let px = (px_center - 0.5).round() as isize;
    let py = (py_center - 0.5).round() as isize;
    (px, py)
}

fn is_nodata(value: i32, nodata: Option<i32>) -> bool {
    nodata.is_some_and(|nd| value == nd)
}

fn compare_f64(lhs: f64, rhs: f64) -> Ordering {
    lhs.partial_cmp(&rhs).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_request() -> PrepareHrusRequest {
        PrepareHrusRequest {
            kernel_schema_version: 1,
            bound_tif: "bound.tif".to_string(),
            landuse_tif: "landuse.tif".to_string(),
            hydgrpdcd_tif: "hydgrpdcd.tif".to_string(),
            burn_severity_tif: None,
            default_hsg_code: Some(2),
            default_hsg_derivation: Some("user_override".to_string()),
            unresolved_hsg_policy: UnresolvedHsgPolicy::Error,
            strict_burn_nodata: false,
            allow_cross_hsg_merge: false,
            min_hru_area_ha: 2.0,
            hydrophobic_forest_high: true,
            hydrophobic_forest_moderate: false,
            hydrophobic_shrub_high: true,
            hydrophobic_shrub_moderate: false,
        }
    }

    fn test_grid(
        path: &str,
        width: usize,
        height: usize,
        nodata: Option<i32>,
        data: Vec<i32>,
    ) -> GridData {
        GridData {
            width,
            height,
            geo_transform: [500000.0, 30.0, 0.0, 4700000.0, 0.0, -30.0],
            projection: Some("+proj=aea +lat_1=29.5 +lat_2=45.5 +lat_0=23 +lon_0=-96".to_string()),
            nodata,
            data,
            path: path.to_string(),
        }
    }

    fn run_prepare_from_arrays(
        request: &PrepareHrusRequest,
        width: usize,
        height: usize,
        bound_data: Vec<i32>,
        landuse_data: Vec<i32>,
        hydgrpdcd_data: Vec<i32>,
        burn_data: Option<Vec<i32>>,
    ) -> Result<PrepareHrusResponse, GenevaError> {
        let bound = test_grid("bound", width, height, None, bound_data);
        let landuse = test_grid("landuse", width, height, Some(-9999), landuse_data);
        let hydgrpdcd = test_grid("hydgrpdcd", width, height, Some(-9999), hydgrpdcd_data);
        let burn = burn_data.map(|values| test_grid("burn", width, height, Some(255), values));

        let alignment = AlignmentDiagnostics {
            canonical: CanonicalGridDiagnostics {
                path: "bound".to_string(),
                width,
                height,
                geo_transform: bound.geo_transform,
                projection: bound.projection.clone(),
            },
            landuse: RasterAlignmentStatus {
                path: "landuse".to_string(),
                width,
                height,
                matched_canonical: true,
                resampled: false,
            },
            hydgrpdcd: RasterAlignmentStatus {
                path: "hydgrpdcd".to_string(),
                width,
                height,
                matched_canonical: true,
                resampled: false,
            },
            burn_severity: burn.as_ref().map(|_| RasterAlignmentStatus {
                path: "burn".to_string(),
                width,
                height,
                matched_canonical: true,
                resampled: false,
            }),
        };

        prepare_hrus_from_grids(
            request,
            &bound,
            &landuse,
            &hydgrpdcd,
            burn.as_ref(),
            alignment,
        )
    }

    fn synthetic_row(
        hru_id: &str,
        landuse_class: i32,
        hsg_group: HsgGroup,
        burn_severity: BurnSeverity,
        hydrophobic_class: bool,
        is_water: bool,
        area_m2: f64,
    ) -> HruAggregate {
        let key = HruKey {
            landuse_class,
            hsg_group,
            burn_severity,
            hydrophobic_class,
            is_water,
        };
        let mut hsg_source_counts = BTreeMap::new();
        hsg_source_counts.insert(HsgSource::CodedLookup, 1);

        HruAggregate {
            hru_id: hru_id.to_string(),
            key,
            area_m2,
            cn_arc_ii: estimate_cn_arc_ii(key),
            hsg_source_counts,
            collapsed_from_hru_ids: Vec::new(),
            warnings: BTreeSet::new(),
        }
    }

    fn runoff_metrics(
        rows: &[HruAggregate],
        rainfall_mm: f64,
        storm_duration_hours: f64,
    ) -> (f64, f64, f64) {
        let total_area_m2 = rows.iter().map(|row| row.area_m2).sum::<f64>();
        let mut total_volume_m3 = 0.0;
        let mut weighted_peak_numerator = 0.0;

        for row in rows {
            let cn = row.cn_arc_ii.min(99.9);
            let s_mm = (25_400.0 / cn) - 254.0;
            let ia_mm = 0.2 * s_mm;
            let q_mm = if rainfall_mm > ia_mm {
                let numerator = (rainfall_mm - ia_mm).powi(2);
                let denominator = rainfall_mm + (0.8 * s_mm);
                if denominator > 0.0 {
                    numerator / denominator
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let runoff_volume_m3 = (q_mm / 1000.0) * row.area_m2;
            total_volume_m3 += runoff_volume_m3;

            let response_factor = (0.5 + (cn / 200.0)).clamp(0.5, 1.0);
            weighted_peak_numerator += runoff_volume_m3 * response_factor;
        }

        let runoff_depth_mm = if total_area_m2 > 0.0 {
            (total_volume_m3 / total_area_m2) * 1000.0
        } else {
            0.0
        };

        let duration_seconds = storm_duration_hours * 3600.0;
        let peak_discharge_cms = if duration_seconds > 0.0 {
            weighted_peak_numerator / duration_seconds
        } else {
            0.0
        };

        (runoff_depth_mm, total_volume_m3, peak_discharge_cms)
    }

    fn relative_delta(actual: f64, reference: f64) -> f64 {
        if reference.abs() <= FLOAT_TOLERANCE {
            0.0
        } else {
            (actual - reference).abs() / reference.abs()
        }
    }

    #[test]
    fn parse_prepare_request_requires_minimum_threshold() {
        let payload = r#"{
            "kernel_schema_version": 1,
            "bound_tif": "bound.tif",
            "landuse_tif": "landuse.tif",
            "hydgrpdcd_tif": "hyd.tif",
            "min_hru_area_ha": 1.5
        }"#;

        let error = PrepareHrusRequest::from_payload_json(payload)
            .expect_err("payload below hard 2 ha floor must fail");
        assert_eq!(error.code(), "invalid_input");
    }

    #[test]
    fn cn_lambda_005_cap_preserves_cn_above_98_5() {
        let converted = derive_cn_lambda_005(99.0);
        assert!((converted - 99.0).abs() <= 1e-12);
    }

    #[test]
    fn deterministic_hru_keying_is_stable() {
        let request = default_request();
        let width = 4;
        let height = 4;

        let bound = vec![1; width * height];
        let landuse = vec![
            41, 41, 41, 41, 41, 71, 71, 71, 41, 71, 71, 71, 41, 41, 41, 41,
        ];
        let hydgrpdcd = vec![1, 1, 1, 1, 1, 2, 2, 2, 1, 2, 2, 2, 1, 1, 1, 1];
        let burn = Some(vec![0, 0, 0, 0, 0, 2, 2, 2, 0, 2, 2, 2, 0, 0, 0, 0]);

        let first = run_prepare_from_arrays(
            &request,
            width,
            height,
            bound.clone(),
            landuse.clone(),
            hydgrpdcd.clone(),
            burn.clone(),
        )
        .expect("first pass must succeed");
        let second =
            run_prepare_from_arrays(&request, width, height, bound, landuse, hydgrpdcd, burn)
                .expect("second pass must succeed");

        let first_ids: Vec<String> = first
            .hru_rows
            .iter()
            .map(|row| row.hru_id.clone())
            .collect();
        let second_ids: Vec<String> = second
            .hru_rows
            .iter()
            .map(|row| row.hru_id.clone())
            .collect();

        assert_eq!(first_ids, second_ids);
        assert_eq!(first.hru_rows, second.hru_rows);
    }

    #[test]
    fn fallback_precedence_prefers_coded_then_default_then_assume_d() {
        let mut request = default_request();
        let width = 3;
        let height = 1;

        let response = run_prepare_from_arrays(
            &request,
            width,
            height,
            vec![1, 1, 1],
            vec![41, 41, 41],
            vec![1, 0, 7],
            None,
        )
        .expect("coded + default fallback path must succeed");

        assert_eq!(
            response
                .diagnostics
                .hsg_provenance_counts
                .get("coded_lookup")
                .copied()
                .unwrap_or(0),
            1
        );
        assert_eq!(
            response
                .diagnostics
                .hsg_provenance_counts
                .get("default_hsg_fallback")
                .copied()
                .unwrap_or(0),
            2
        );
        assert_eq!(response.diagnostics.invalid_hydgrpdcd_cell_count, 1);

        let warning_codes: BTreeSet<String> = response
            .warnings
            .iter()
            .map(|warning| warning.code.clone())
            .collect();
        assert!(warning_codes.contains("hsg_default_fallback"));
        assert!(warning_codes.contains("hydgrpdcd_invalid_code"));

        request.default_hsg_code = None;
        request.default_hsg_derivation = None;
        request.unresolved_hsg_policy = UnresolvedHsgPolicy::AssumeD;

        let assume_d = run_prepare_from_arrays(&request, 1, 1, vec![1], vec![41], vec![0], None)
            .expect("assume_d fallback path must succeed");

        assert_eq!(
            assume_d
                .diagnostics
                .hsg_provenance_counts
                .get("assume_d_policy")
                .copied()
                .unwrap_or(0),
            1
        );

        request.unresolved_hsg_policy = UnresolvedHsgPolicy::Error;
        let error = run_prepare_from_arrays(&request, 1, 1, vec![1], vec![41], vec![0], None)
            .expect_err("unresolved policy=error must fail when no fallback exists");
        assert_eq!(error.code(), "contract_violation");
    }

    #[test]
    fn collapse_is_deterministic_and_conserves_area() {
        let mut first = vec![
            synthetic_row(
                "row_a",
                41,
                HsgGroup::B,
                BurnSeverity::Unburned,
                false,
                false,
                9000.0,
            ),
            synthetic_row(
                "row_b",
                41,
                HsgGroup::B,
                BurnSeverity::Unburned,
                false,
                false,
                12_000.0,
            ),
            synthetic_row(
                "row_c",
                41,
                HsgGroup::B,
                BurnSeverity::Unburned,
                false,
                false,
                30_000.0,
            ),
        ];
        let mut second = first.clone();

        let mut diagnostics_a = HruDiagnostics {
            inbound_cell_count: 0,
            inbound_area_m2: 0.0,
            hru_area_total_m2: 0.0,
            cell_area_m2: 900.0,
            min_hru_area_m2: 20_000.0,
            allow_cross_hsg_merge: false,
            default_hsg_code: None,
            default_hsg_derivation: None,
            unresolved_hsg_policy: "error".to_string(),
            hsg_provenance_counts: BTreeMap::new(),
            hsg_fallback_cell_count: 0,
            hsg_fallback_area_m2: 0.0,
            unresolved_hsg_cell_count: 0,
            invalid_hydgrpdcd_cell_count: 0,
            burn_nodata_cell_count: 0,
            collapse_donor_count: 0,
            collapse_merge_count: 0,
            collapse_unmerged_count: 0,
            area_closure_error_m2: 0.0,
            alignment: AlignmentDiagnostics {
                canonical: CanonicalGridDiagnostics {
                    path: "bound".to_string(),
                    width: 0,
                    height: 0,
                    geo_transform: [0.0; 6],
                    projection: None,
                },
                landuse: RasterAlignmentStatus {
                    path: "landuse".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                hydgrpdcd: RasterAlignmentStatus {
                    path: "hyd".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                burn_severity: None,
            },
        };
        let mut diagnostics_b = diagnostics_a.clone();
        let mut warnings_a = WarningAccumulator::default();
        let mut warnings_b = WarningAccumulator::default();

        let area_before = first.iter().map(|row| row.area_m2).sum::<f64>();

        collapse_small_hrus(
            &mut first,
            20_000.0,
            false,
            &mut warnings_a,
            &mut diagnostics_a,
        );
        collapse_small_hrus(
            &mut second,
            20_000.0,
            false,
            &mut warnings_b,
            &mut diagnostics_b,
        );

        let area_after = first.iter().map(|row| row.area_m2).sum::<f64>();
        assert!((area_before - area_after).abs() <= FLOAT_TOLERANCE);

        first.sort_by(|lhs, rhs| lhs.hru_id.cmp(&rhs.hru_id));
        second.sort_by(|lhs, rhs| lhs.hru_id.cmp(&rhs.hru_id));

        assert_eq!(first, second);
        assert_eq!(diagnostics_a.collapse_merge_count, 2);
        assert!(diagnostics_a.collapse_unmerged_count == 0);
    }

    #[test]
    fn water_hru_protection_blocks_non_water_merge_targets() {
        let mut rows = vec![
            synthetic_row(
                "water_donor",
                11,
                HsgGroup::D,
                BurnSeverity::Unburned,
                false,
                true,
                10_000.0,
            ),
            synthetic_row(
                "water_recipient",
                11,
                HsgGroup::D,
                BurnSeverity::Unburned,
                false,
                true,
                35_000.0,
            ),
            synthetic_row(
                "non_water_recipient",
                41,
                HsgGroup::D,
                BurnSeverity::Unburned,
                false,
                false,
                80_000.0,
            ),
        ];

        let mut diagnostics = HruDiagnostics {
            inbound_cell_count: 0,
            inbound_area_m2: 0.0,
            hru_area_total_m2: 0.0,
            cell_area_m2: 900.0,
            min_hru_area_m2: 20_000.0,
            allow_cross_hsg_merge: true,
            default_hsg_code: None,
            default_hsg_derivation: None,
            unresolved_hsg_policy: "error".to_string(),
            hsg_provenance_counts: BTreeMap::new(),
            hsg_fallback_cell_count: 0,
            hsg_fallback_area_m2: 0.0,
            unresolved_hsg_cell_count: 0,
            invalid_hydgrpdcd_cell_count: 0,
            burn_nodata_cell_count: 0,
            collapse_donor_count: 0,
            collapse_merge_count: 0,
            collapse_unmerged_count: 0,
            area_closure_error_m2: 0.0,
            alignment: AlignmentDiagnostics {
                canonical: CanonicalGridDiagnostics {
                    path: "bound".to_string(),
                    width: 0,
                    height: 0,
                    geo_transform: [0.0; 6],
                    projection: None,
                },
                landuse: RasterAlignmentStatus {
                    path: "landuse".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                hydgrpdcd: RasterAlignmentStatus {
                    path: "hyd".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                burn_severity: None,
            },
        };
        let mut warnings = WarningAccumulator::default();

        collapse_small_hrus(&mut rows, 20_000.0, true, &mut warnings, &mut diagnostics);

        let water_row = rows
            .iter()
            .find(|row| row.hru_id == "water_recipient")
            .expect("water recipient must remain");
        assert!(water_row
            .collapsed_from_hru_ids
            .iter()
            .any(|id| id == "water_donor"));

        let non_water_row = rows
            .iter()
            .find(|row| row.hru_id == "non_water_recipient")
            .expect("non-water recipient must remain");
        assert!(non_water_row
            .collapsed_from_hru_ids
            .iter()
            .all(|id| id != "water_donor"));
    }

    #[test]
    fn default_collapse_sensitivity_stays_within_thresholds() {
        let mut rows = vec![
            synthetic_row(
                "comp_1",
                41,
                HsgGroup::B,
                BurnSeverity::High,
                true,
                false,
                12_000.0,
            ),
            synthetic_row(
                "comp_2",
                41,
                HsgGroup::B,
                BurnSeverity::High,
                true,
                false,
                14_000.0,
            ),
            synthetic_row(
                "comp_3",
                52,
                HsgGroup::C,
                BurnSeverity::Moderate,
                true,
                false,
                45_000.0,
            ),
        ];
        let reference_rows = rows.clone();

        let (reference_depth, reference_volume, reference_peak) =
            runoff_metrics(&reference_rows, 65.0, 1.5);

        let mut diagnostics = HruDiagnostics {
            inbound_cell_count: 0,
            inbound_area_m2: 0.0,
            hru_area_total_m2: 0.0,
            cell_area_m2: 900.0,
            min_hru_area_m2: 20_000.0,
            allow_cross_hsg_merge: false,
            default_hsg_code: None,
            default_hsg_derivation: None,
            unresolved_hsg_policy: "error".to_string(),
            hsg_provenance_counts: BTreeMap::new(),
            hsg_fallback_cell_count: 0,
            hsg_fallback_area_m2: 0.0,
            unresolved_hsg_cell_count: 0,
            invalid_hydgrpdcd_cell_count: 0,
            burn_nodata_cell_count: 0,
            collapse_donor_count: 0,
            collapse_merge_count: 0,
            collapse_unmerged_count: 0,
            area_closure_error_m2: 0.0,
            alignment: AlignmentDiagnostics {
                canonical: CanonicalGridDiagnostics {
                    path: "bound".to_string(),
                    width: 0,
                    height: 0,
                    geo_transform: [0.0; 6],
                    projection: None,
                },
                landuse: RasterAlignmentStatus {
                    path: "landuse".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                hydgrpdcd: RasterAlignmentStatus {
                    path: "hyd".to_string(),
                    width: 0,
                    height: 0,
                    matched_canonical: true,
                    resampled: false,
                },
                burn_severity: None,
            },
        };
        let mut warnings = WarningAccumulator::default();

        collapse_small_hrus(&mut rows, 20_000.0, false, &mut warnings, &mut diagnostics);
        assert!(diagnostics.collapse_merge_count >= 1);

        let (collapsed_depth, collapsed_volume, collapsed_peak) = runoff_metrics(&rows, 65.0, 1.5);

        assert!(relative_delta(collapsed_depth, reference_depth) <= 0.02);
        assert!(relative_delta(collapsed_volume, reference_volume) <= 0.02);
        assert!(relative_delta(collapsed_peak, reference_peak) <= 0.05);
    }
}

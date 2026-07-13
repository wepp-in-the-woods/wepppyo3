use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::hill_pass::{hillslope_pass_to_columns, PassColumns};
use crate::schema::VersionInfo;

const AG_FIELDS_STRATEGY: &str = "ag_fields_v1";
const AG_FIELDS_SEMANTICS: &str = "ag_fields_pass_semantics_v1";
const CLOSURE_NAMES: [&str; 12] = [
    "runvol_m3",
    "sbrunv_m3",
    "drrunv_m3",
    "gwbfv_m3",
    "gwdsv_m3",
    "tdet_kg",
    "tdep_kg",
    "sediment_class_1_kg",
    "sediment_class_2_kg",
    "sediment_class_3_kg",
    "sediment_class_4_kg",
    "sediment_class_5_kg",
];
const DIRECT_METRIC_COUNT: usize = 7;

type ClosureMetrics = [f64; CLOSURE_NAMES.len()];

#[derive(Clone, Debug)]
pub struct WeightedPassSource {
    pub source_id: String,
    pub path: PathBuf,
    pub represented_area_m2: f64,
}

#[derive(Clone, Debug)]
struct WeightedPassHeader {
    lines: Vec<String>,
    climate_token: String,
    years: i32,
    start_year: i32,
    modeled_area_m2: f64,
    particle_diameters_m: [f64; 5],
    phosphorus_values: [f64; 4],
}

struct WeightedSourceData {
    source: WeightedPassSource,
    header: WeightedPassHeader,
    scale: f64,
    columns: PassColumns,
}

#[derive(Debug)]
struct WeightedSourceDiagnostic {
    source_id: String,
    climate_token: String,
    modeled_area_m2: f64,
    represented_area_m2: f64,
    scale: f64,
    row_count: usize,
    raw_totals: ClosureMetrics,
    weighted_totals: ClosureMetrics,
}

#[derive(Debug)]
struct WeightedEventDiagnostic {
    year: i16,
    julian: i16,
    event: String,
    weighted_input: ClosureMetrics,
    reparsed_output: ClosureMetrics,
    residuals: ClosureMetrics,
    budgets: ClosureMetrics,
}

#[derive(Debug)]
struct WeightedRunDiagnostic {
    weighted_input: ClosureMetrics,
    reparsed_output: ClosureMetrics,
    residuals: ClosureMetrics,
    budgets: ClosureMetrics,
    max_abs_event_residuals: ClosureMetrics,
    max_event_budget_ratios: ClosureMetrics,
}

#[derive(Debug)]
pub struct WeightedPassDiagnostics {
    target_area_m2: f64,
    serialized_target_area_m2: f64,
    target_area_residual_m2: f64,
    target_area_budget_m2: f64,
    sources: Vec<WeightedSourceDiagnostic>,
    events: Vec<WeightedEventDiagnostic>,
    run: WeightedRunDiagnostic,
}

impl WeightedPassDiagnostics {
    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("algorithm", AG_FIELDS_STRATEGY).unwrap();
        dict.set_item("semantic_contract", AG_FIELDS_SEMANTICS)
            .unwrap();
        dict.set_item("target_area_m2", self.target_area_m2)
            .unwrap();
        dict.set_item("serialized_target_area_m2", self.serialized_target_area_m2)
            .unwrap();
        dict.set_item("target_area_residual_m2", self.target_area_residual_m2)
            .unwrap();
        dict.set_item("target_area_budget_m2", self.target_area_budget_m2)
            .unwrap();
        dict.set_item("source_count", self.sources.len()).unwrap();
        dict.set_item("row_count", self.events.len()).unwrap();

        let source_dicts = self
            .sources
            .into_iter()
            .map(|source| {
                let item = PyDict::new_bound(py);
                item.set_item("source_id", source.source_id).unwrap();
                item.set_item("climate_token", source.climate_token)
                    .unwrap();
                item.set_item("modeled_area_m2", source.modeled_area_m2)
                    .unwrap();
                item.set_item("represented_area_m2", source.represented_area_m2)
                    .unwrap();
                item.set_item("scale", source.scale).unwrap();
                item.set_item("row_count", source.row_count).unwrap();
                item.set_item("raw_totals", metrics_to_pydict(py, &source.raw_totals))
                    .unwrap();
                item.set_item(
                    "weighted_totals",
                    metrics_to_pydict(py, &source.weighted_totals),
                )
                .unwrap();
                item.into_py(py)
            })
            .collect::<Vec<PyObject>>();
        dict.set_item("sources", source_dicts).unwrap();

        let event_dicts = self
            .events
            .into_iter()
            .map(|event| {
                let item = PyDict::new_bound(py);
                item.set_item("year", event.year).unwrap();
                item.set_item("julian", event.julian).unwrap();
                item.set_item("event", event.event).unwrap();
                item.set_item(
                    "weighted_input",
                    metrics_to_pydict(py, &event.weighted_input),
                )
                .unwrap();
                item.set_item(
                    "reparsed_output",
                    metrics_to_pydict(py, &event.reparsed_output),
                )
                .unwrap();
                item.set_item("residuals", metrics_to_pydict(py, &event.residuals))
                    .unwrap();
                item.set_item("budgets", metrics_to_pydict(py, &event.budgets))
                    .unwrap();
                item.into_py(py)
            })
            .collect::<Vec<PyObject>>();
        dict.set_item("events", event_dicts).unwrap();

        let run = PyDict::new_bound(py);
        run.set_item(
            "weighted_input",
            metrics_to_pydict(py, &self.run.weighted_input),
        )
        .unwrap();
        run.set_item(
            "reparsed_output",
            metrics_to_pydict(py, &self.run.reparsed_output),
        )
        .unwrap();
        run.set_item("residuals", metrics_to_pydict(py, &self.run.residuals))
            .unwrap();
        run.set_item("budgets", metrics_to_pydict(py, &self.run.budgets))
            .unwrap();
        run.set_item(
            "max_abs_event_residuals",
            metrics_to_pydict(py, &self.run.max_abs_event_residuals),
        )
        .unwrap();
        run.set_item(
            "max_event_budget_ratios",
            metrics_to_pydict(py, &self.run.max_event_budget_ratios),
        )
        .unwrap();
        dict.set_item("run_closure", run).unwrap();
        dict.into_py(py)
    }
}

fn metrics_to_pydict(py: Python<'_>, metrics: &ClosureMetrics) -> PyObject {
    let dict = PyDict::new_bound(py);
    for (name, value) in CLOSURE_NAMES.iter().zip(metrics.iter()) {
        dict.set_item(name, value).unwrap();
    }
    dict.into_py(py)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum EventKind {
    NoEvent = 0,
    SubEvent = 1,
    Event = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct DayKey {
    year: i16,
    julian: i16,
    sim_day_index: i32,
}

#[derive(Clone, Copy, Debug)]
struct RowRef {
    source_idx: usize,
    row_idx: usize,
}

struct SourceData {
    path: PathBuf,
    columns: PassColumns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CombineStrategy {
    Phase1,
    Phase4,
}

fn parse_combine_strategy(
    strategy: &str,
    path: &Path,
) -> Result<CombineStrategy, InterchangeError> {
    if strategy.eq_ignore_ascii_case("phase1") {
        return Ok(CombineStrategy::Phase1);
    }
    if strategy.eq_ignore_ascii_case("phase4") {
        return Ok(CombineStrategy::Phase4);
    }

    Err(InterchangeError::parse(
        path,
        None,
        format!("Unsupported pass combine strategy '{strategy}'"),
        None,
    ))
}

#[derive(Clone, Copy, Debug)]
struct HydroTriangle {
    peak: f64,
    t_peak: f64,
    t_end: f64,
}

#[derive(Clone, Debug)]
struct CombinedRow {
    event: &'static str,
    year: i16,
    julian: i16,
    dur: f64,
    tcs: f64,
    oalpha: f64,
    runoff: f64,
    runvol: f64,
    sbrunf: f64,
    sbrunv: f64,
    drainq: f64,
    drrunv: f64,
    peakro: f64,
    tdet: f64,
    tdep: f64,
    sedcon_1: f64,
    sedcon_2: f64,
    sedcon_3: f64,
    sedcon_4: f64,
    sedcon_5: f64,
    clot: f64,
    slot: f64,
    saot: f64,
    laot: f64,
    sdot: f64,
    gwbfv: f64,
    gwdsv: f64,
}

pub fn combine_hillslope_pass_files(
    base_pass: &Path,
    road_passes: &[PathBuf],
    out_pass: &Path,
    strategy: &str,
) -> Result<(), InterchangeError> {
    let strategy_kind = parse_combine_strategy(strategy, base_pass)?;

    let header_lines = read_pass_header(base_pass)?;
    let version = VersionInfo::new(1, 0);

    let mut sources = Vec::with_capacity(1 + road_passes.len());
    sources.push(SourceData {
        path: base_pass.to_path_buf(),
        columns: hillslope_pass_to_columns(base_pass, None, &version)?,
    });
    for road_pass in road_passes {
        let road_header_lines = read_pass_header(road_pass)?;
        validate_header_compatibility(base_pass, &header_lines, road_pass, &road_header_lines)?;
        sources.push(SourceData {
            path: road_pass.clone(),
            columns: hillslope_pass_to_columns(road_pass, None, &version)?,
        });
    }
    validate_calendar_alignment(&sources)?;

    let mut by_day: BTreeMap<DayKey, Vec<RowRef>> = BTreeMap::new();
    for (source_idx, source) in sources.iter().enumerate() {
        for row_idx in 0..source.columns.len() {
            let key = make_day_key(&source.columns, row_idx);
            by_day.entry(key).or_default().push(RowRef {
                source_idx,
                row_idx,
            });
        }
    }

    let mut combined_rows = Vec::with_capacity(by_day.len());
    for (key, row_refs) in by_day {
        let resolved_kind = resolve_day_kind(&row_refs, &sources)?;
        combined_rows.push(combine_row_for_kind(
            key,
            &row_refs,
            &sources,
            resolved_kind,
            strategy_kind,
        ));
    }

    write_combined_pass(out_pass, &header_lines, &combined_rows, false)?;
    Ok(())
}

pub fn combine_weighted_hillslope_pass_files(
    source_specs: &[WeightedPassSource],
    out_pass: &Path,
    target_area_m2: f64,
    output_climate_token: &str,
    strategy: &str,
) -> Result<WeightedPassDiagnostics, InterchangeError> {
    validate_weighted_call(
        source_specs,
        out_pass,
        target_area_m2,
        output_climate_token,
        strategy,
    )?;

    let version = VersionInfo::new(1, 0);
    let mut sources = Vec::with_capacity(source_specs.len());
    for source in source_specs {
        let header = read_weighted_pass_header(&source.path)?;
        let columns = hillslope_pass_to_columns(&source.path, None, &version)?;
        validate_weighted_source_rows(&source.path, &columns)?;
        let scale = source.represented_area_m2 / header.modeled_area_m2;
        if !scale.is_finite() || scale < 0.0 {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                "Weighted PASS source produced a non-finite or negative scale",
                Some(format!(
                    "source_id={} represented_area_m2={} modeled_area_m2={}",
                    source.source_id, source.represented_area_m2, header.modeled_area_m2
                )),
            ));
        }
        sources.push(WeightedSourceData {
            source: source.clone(),
            header,
            scale,
            columns,
        });
    }

    validate_weighted_headers(&sources)?;
    validate_weighted_calendars(&sources)?;

    let combined_rows = (0..sources[0].columns.len())
        .map(|row_idx| combine_weighted_row(row_idx, &sources, target_area_m2))
        .collect::<Result<Vec<_>, _>>()?;
    let output_header =
        weighted_output_header(&sources[0].header, target_area_m2, output_climate_token)?;
    let temp_path = weighted_temp_path(out_pass);

    let result = (|| {
        write_combined_pass(&temp_path, &output_header, &combined_rows, true)?;
        File::open(&temp_path)
            .and_then(|file| file.sync_all())
            .map_err(|err| InterchangeError::io(&temp_path, err))?;

        let reparsed = hillslope_pass_to_columns(&temp_path, None, &version)?;
        let reparsed_header = read_weighted_pass_header(&temp_path)?;
        let diagnostics = build_weighted_diagnostics(
            &sources,
            &reparsed,
            &reparsed_header,
            target_area_m2,
            out_pass,
        )?;

        fs::rename(&temp_path, out_pass).map_err(|err| InterchangeError::io(out_pass, err))?;
        Ok(diagnostics)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn validate_weighted_call(
    source_specs: &[WeightedPassSource],
    out_pass: &Path,
    target_area_m2: f64,
    output_climate_token: &str,
    strategy: &str,
) -> Result<(), InterchangeError> {
    if strategy != AG_FIELDS_STRATEGY {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!(
                "Unsupported weighted PASS strategy '{strategy}'; expected '{AG_FIELDS_STRATEGY}'"
            ),
            None,
        ));
    }
    if source_specs.is_empty() {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            "Weighted PASS combine requires at least one source",
            None,
        ));
    }
    if !target_area_m2.is_finite() || target_area_m2 <= 0.0 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            "Weighted PASS target area must be finite and positive",
            Some(format!("target_area_m2={target_area_m2}")),
        ));
    }
    if output_climate_token.trim().is_empty()
        || output_climate_token.contains('\n')
        || output_climate_token.contains('\r')
    {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            "Weighted PASS output climate token must be non-empty and single-line",
            None,
        ));
    }

    let mut source_ids = HashSet::with_capacity(source_specs.len());
    let mut represented_sum = 0.0_f64;
    for source in source_specs {
        if source.source_id.trim().is_empty() {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                "Weighted PASS source id must be non-empty",
                None,
            ));
        }
        if !source_ids.insert(source.source_id.as_str()) {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                format!("Duplicate weighted PASS source id '{}'", source.source_id),
                None,
            ));
        }
        if !source.represented_area_m2.is_finite() || source.represented_area_m2 < 0.0 {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                "Weighted PASS represented area must be finite and nonnegative",
                Some(format!(
                    "source_id={} represented_area_m2={}",
                    source.source_id, source.represented_area_m2
                )),
            ));
        }
        represented_sum += source.represented_area_m2;
    }
    let area_budget = area_sum_budget(represented_sum, target_area_m2);
    if !represented_sum.is_finite() || (represented_sum - target_area_m2).abs() > area_budget {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            "Weighted PASS represented areas do not close to target area",
            Some(format!(
                "represented_sum_m2={represented_sum} target_area_m2={target_area_m2} residual_m2={} budget_m2={area_budget}",
                represented_sum - target_area_m2
            )),
        ));
    }
    Ok(())
}

fn read_weighted_pass_header(path: &Path) -> Result<WeightedPassHeader, InterchangeError> {
    let lines = read_pass_header(path)?;
    let (years, start_year) = parse_simulation_header_line(&lines[1], path)?;
    if years <= 0 {
        return Err(InterchangeError::parse(
            path,
            Some(2),
            "PASS simulation years must be positive",
            Some(lines[1].clone()),
        ));
    }

    let modeled_area_m2 = parse_header_float(&lines[2], path, 3, "modeled area")?;
    if !modeled_area_m2.is_finite() || modeled_area_m2 <= 0.0 {
        return Err(InterchangeError::parse(
            path,
            Some(3),
            "PASS modeled area must be finite and positive",
            Some(lines[2].clone()),
        ));
    }

    let particle_tokens = lines[3].split_whitespace().collect::<Vec<_>>();
    if particle_tokens.len() != 6 || particle_tokens[0] != "5" {
        return Err(InterchangeError::parse(
            path,
            Some(4),
            "Weighted PASS v1 requires exactly five particle classes",
            Some(lines[3].clone()),
        ));
    }
    let mut particle_diameters_m = [0.0_f64; 5];
    for (idx, token) in particle_tokens.iter().skip(1).enumerate() {
        particle_diameters_m[idx] = parse_required_float(token).map_err(|message| {
            InterchangeError::parse(path, Some(4), message, Some(lines[3].clone()))
        })?;
        if !particle_diameters_m[idx].is_finite() || particle_diameters_m[idx] < 0.0 {
            return Err(InterchangeError::parse(
                path,
                Some(4),
                "PASS particle diameter must be finite and nonnegative",
                Some(lines[3].clone()),
            ));
        }
    }

    let phosphorus_tokens = lines[4].split_whitespace().collect::<Vec<_>>();
    if phosphorus_tokens.len() != 4 {
        return Err(InterchangeError::parse(
            path,
            Some(5),
            "PASS header must contain four phosphorus values",
            Some(lines[4].clone()),
        ));
    }
    let mut phosphorus_values = [0.0_f64; 4];
    for (idx, token) in phosphorus_tokens.iter().enumerate() {
        phosphorus_values[idx] = parse_required_float(token).map_err(|message| {
            InterchangeError::parse(path, Some(5), message, Some(lines[4].clone()))
        })?;
        if !phosphorus_values[idx].is_finite() || phosphorus_values[idx] < 0.0 {
            return Err(InterchangeError::parse(
                path,
                Some(5),
                "PASS phosphorus value must be finite and nonnegative",
                Some(lines[4].clone()),
            ));
        }
    }

    Ok(WeightedPassHeader {
        climate_token: lines[0].trim().to_string(),
        lines,
        years,
        start_year,
        modeled_area_m2,
        particle_diameters_m,
        phosphorus_values,
    })
}

fn parse_header_float(
    line: &str,
    path: &Path,
    line_number: usize,
    field_name: &str,
) -> Result<f64, InterchangeError> {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    if tokens.len() != 1 {
        return Err(InterchangeError::parse(
            path,
            Some(line_number),
            format!("PASS {field_name} header must contain exactly one value"),
            Some(line.to_string()),
        ));
    }
    parse_required_float(tokens[0]).map_err(|message| {
        InterchangeError::parse(path, Some(line_number), message, Some(line.to_string()))
    })
}

fn validate_weighted_headers(sources: &[WeightedSourceData]) -> Result<(), InterchangeError> {
    let base = &sources[0];
    for source in sources.iter().skip(1) {
        if (source.header.years, source.header.start_year)
            != (base.header.years, base.header.start_year)
        {
            return Err(InterchangeError::parse(
                &source.source.path,
                Some(2),
                "Weighted PASS simulation header does not match first source",
                Some(format!(
                    "source_id={} expected=({}, {}) actual=({}, {})",
                    source.source.source_id,
                    base.header.years,
                    base.header.start_year,
                    source.header.years,
                    source.header.start_year
                )),
            ));
        }
        if source.header.particle_diameters_m != base.header.particle_diameters_m {
            return Err(InterchangeError::parse(
                &source.source.path,
                Some(4),
                "Weighted PASS particle diameters do not match first source",
                Some(format!("source_id={}", source.source.source_id)),
            ));
        }
        if source.header.phosphorus_values != base.header.phosphorus_values {
            return Err(InterchangeError::parse(
                &source.source.path,
                Some(5),
                "Weighted PASS phosphorus header does not match first source",
                Some(format!("source_id={}", source.source.source_id)),
            ));
        }
    }
    Ok(())
}

fn validate_weighted_calendars(sources: &[WeightedSourceData]) -> Result<(), InterchangeError> {
    if sources[0].columns.len() == 0 {
        return Err(InterchangeError::parse(
            &sources[0].source.path,
            None,
            "Weighted PASS source contains no daily rows",
            None,
        ));
    }
    let base = &sources[0];
    for source in sources.iter().skip(1) {
        if source.columns.len() != base.columns.len() {
            return Err(InterchangeError::parse(
                &source.source.path,
                None,
                "Weighted PASS calendar row count does not match first source",
                Some(format!(
                    "source_id={} expected_rows={} actual_rows={}",
                    source.source.source_id,
                    base.columns.len(),
                    source.columns.len()
                )),
            ));
        }
        for row_idx in 0..base.columns.len() {
            let expected = make_day_key(&base.columns, row_idx);
            let actual = make_day_key(&source.columns, row_idx);
            if actual != expected {
                return Err(InterchangeError::parse(
                    &source.source.path,
                    None,
                    "Weighted PASS calendar day key does not align with first source",
                    Some(format!(
                        "source_id={} row={} expected=({},{},{}) actual=({},{},{})",
                        source.source.source_id,
                        row_idx,
                        expected.year,
                        expected.julian,
                        expected.sim_day_index,
                        actual.year,
                        actual.julian,
                        actual.sim_day_index
                    )),
                ));
            }
        }
    }
    Ok(())
}

fn validate_weighted_source_rows(
    path: &Path,
    columns: &PassColumns,
) -> Result<(), InterchangeError> {
    let numeric_columns: [(&str, &[f64]); 24] = [
        ("dur", &columns.dur),
        ("tcs", &columns.tcs),
        ("oalpha", &columns.oalpha),
        ("runoff", &columns.runoff),
        ("runvol", &columns.runvol),
        ("sbrunf", &columns.sbrunf),
        ("sbrunv", &columns.sbrunv),
        ("drainq", &columns.drainq),
        ("drrunv", &columns.drrunv),
        ("peakro", &columns.peakro),
        ("tdet", &columns.tdet),
        ("tdep", &columns.tdep),
        ("sedcon_1", &columns.sedcon_1),
        ("sedcon_2", &columns.sedcon_2),
        ("sedcon_3", &columns.sedcon_3),
        ("sedcon_4", &columns.sedcon_4),
        ("sedcon_5", &columns.sedcon_5),
        ("clot", &columns.clot),
        ("slot", &columns.slot),
        ("saot", &columns.saot),
        ("laot", &columns.laot),
        ("sdot", &columns.sdot),
        ("gwbfv", &columns.gwbfv),
        ("gwdsv", &columns.gwdsv),
    ];
    for (name, values) in numeric_columns {
        for (row_idx, value) in values.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    format!("Weighted PASS field '{name}' must be finite and nonnegative"),
                    Some(format!("row={row_idx} value={value}")),
                ));
            }
        }
    }
    for (row_idx, label) in columns.event.iter().enumerate() {
        parse_event_kind(label, path, row_idx)?;
        let raw = row_metrics(columns, row_idx);
        let sediment_mass = raw[7..].iter().sum::<f64>();
        if sediment_mass > 0.0 {
            let fractions = [
                columns.clot[row_idx],
                columns.slot[row_idx],
                columns.saot[row_idx],
                columns.laot[row_idx],
                columns.sdot[row_idx],
            ];
            let fraction_sum = fractions.iter().sum::<f64>();
            let fraction_budget = fractions
                .iter()
                .map(|value| direct_serialization_budget(*value))
                .sum::<f64>()
                + floating_point_budget(1.0);
            if (fraction_sum - 1.0).abs() > fraction_budget {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    "Weighted PASS particle fractions do not sum to one",
                    Some(format!(
                        "row={row_idx} sum={fraction_sum} residual={} budget={fraction_budget}",
                        fraction_sum - 1.0
                    )),
                ));
            }
        }
    }
    Ok(())
}

fn weighted_output_header(
    base: &WeightedPassHeader,
    target_area_m2: f64,
    output_climate_token: &str,
) -> Result<Vec<String>, InterchangeError> {
    let mut lines = base.lines.clone();
    lines[0] = output_climate_token.to_string();
    lines[2] = format_fortran_e10_5(target_area_m2, Path::new(output_climate_token))?;
    Ok(lines)
}

fn weighted_temp_path(out_pass: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let file_name = out_pass
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("H.weighted.pass.dat");
    out_pass.with_file_name(format!("{file_name}.tmp.{}.{nonce}", std::process::id()))
}

fn combine_weighted_row(
    row_idx: usize,
    sources: &[WeightedSourceData],
    target_area_m2: f64,
) -> Result<CombinedRow, InterchangeError> {
    let positive_sources = sources
        .iter()
        .filter(|source| source.source.represented_area_m2 > 0.0)
        .collect::<Vec<_>>();
    let first = positive_sources[0];
    let year = first.columns.year[row_idx];
    let julian = first.columns.julian[row_idx];

    let mut kind = EventKind::NoEvent;
    for source in &positive_sources {
        let source_kind =
            parse_event_kind(&source.columns.event[row_idx], &source.source.path, row_idx)?;
        kind = kind.max(source_kind);
    }

    let metrics = weighted_input_metrics(row_idx, sources);
    let base = CombinedRow {
        event: match kind {
            EventKind::NoEvent => "NO EVENT",
            EventKind::SubEvent => "SUBEVENT",
            EventKind::Event => "EVENT",
        },
        year,
        julian,
        dur: 0.0,
        tcs: 0.0,
        oalpha: 0.0,
        runoff: 0.0,
        runvol: 0.0,
        sbrunf: 0.0,
        sbrunv: 0.0,
        drainq: 0.0,
        drrunv: 0.0,
        peakro: 0.0,
        tdet: 0.0,
        tdep: 0.0,
        sedcon_1: 0.0,
        sedcon_2: 0.0,
        sedcon_3: 0.0,
        sedcon_4: 0.0,
        sedcon_5: 0.0,
        clot: 0.0,
        slot: 0.0,
        saot: 0.0,
        laot: 0.0,
        sdot: 0.0,
        gwbfv: metrics[3],
        gwdsv: metrics[4],
    };

    if kind == EventKind::NoEvent {
        return Ok(base);
    }

    let mut combined = CombinedRow {
        sbrunf: metrics[1] / target_area_m2,
        sbrunv: metrics[1],
        drainq: metrics[2] / target_area_m2,
        drrunv: metrics[2],
        ..base
    };
    if kind == EventKind::SubEvent {
        return Ok(combined);
    }

    let (peakro, combined_peak_time) = combine_weighted_peak(row_idx, &positive_sources);
    let fallback_tcs = positive_sources
        .iter()
        .filter(|source| {
            parse_event_kind(&source.columns.event[row_idx], &source.source.path, row_idx)
                .map(|kind| kind == EventKind::Event)
                .unwrap_or(false)
        })
        .map(|source| source.columns.tcs[row_idx])
        .fold(0.0_f64, f64::max);
    let tcs = combined_peak_time.unwrap_or(fallback_tcs);
    let runvol = metrics[0];
    let oalpha = if runvol > 0.0 {
        (tcs / 24.0).max((3600.0 * tcs * peakro) / runvol)
    } else {
        tcs / 24.0
    };

    let mut fractions = [0.0_f64; 5];
    let total_sediment_mass = metrics[7..].iter().sum::<f64>();
    if total_sediment_mass > 0.0 {
        for source in &positive_sources {
            let raw = row_metrics(&source.columns, row_idx);
            let source_mass = raw[7..].iter().sum::<f64>() * source.scale;
            if source_mass <= 0.0 {
                continue;
            }
            let source_fractions = [
                source.columns.clot[row_idx],
                source.columns.slot[row_idx],
                source.columns.saot[row_idx],
                source.columns.laot[row_idx],
                source.columns.sdot[row_idx],
            ];
            for idx in 0..5 {
                fractions[idx] += source_mass * source_fractions[idx];
            }
        }
        for value in &mut fractions {
            *value /= total_sediment_mass;
        }
    }

    let concentration = |class_idx: usize| {
        if runvol > 0.0 {
            metrics[7 + class_idx] / runvol
        } else {
            0.0
        }
    };
    combined.dur = positive_sources
        .iter()
        .filter(|source| {
            parse_event_kind(&source.columns.event[row_idx], &source.source.path, row_idx)
                .map(|kind| kind == EventKind::Event)
                .unwrap_or(false)
        })
        .map(|source| source.columns.dur[row_idx])
        .fold(0.0_f64, f64::max);
    combined.tcs = tcs;
    combined.oalpha = oalpha;
    combined.runoff = runvol / target_area_m2;
    combined.runvol = runvol;
    combined.peakro = peakro;
    combined.tdet = metrics[5];
    combined.tdep = metrics[6];
    combined.sedcon_1 = concentration(0);
    combined.sedcon_2 = concentration(1);
    combined.sedcon_3 = concentration(2);
    combined.sedcon_4 = concentration(3);
    combined.sedcon_5 = concentration(4);
    combined.clot = fractions[0];
    combined.slot = fractions[1];
    combined.saot = fractions[2];
    combined.laot = fractions[3];
    combined.sdot = fractions[4];
    Ok(combined)
}

fn combine_weighted_peak(row_idx: usize, sources: &[&WeightedSourceData]) -> (f64, Option<f64>) {
    let mut components = Vec::new();
    for source in sources {
        let columns = &source.columns;
        if let Some(component) = build_hydro_triangle(
            columns.runvol[row_idx] * source.scale,
            columns.peakro[row_idx] * source.scale,
            columns.tcs[row_idx],
        ) {
            components.push(component);
        }
    }
    if components.is_empty() {
        return (0.0, None);
    }

    let mut breakpoints = vec![0.0_f64];
    for component in &components {
        breakpoints.push(component.t_peak);
        breakpoints.push(component.t_end);
    }
    breakpoints.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    breakpoints.dedup_by(|left, right| (*left - *right).abs() < 1.0e-9);

    let mut best_peak = 0.0_f64;
    let mut best_time = 0.0_f64;
    for time in breakpoints {
        let flow = components
            .iter()
            .map(|component| triangle_flow_at(component, time))
            .sum::<f64>();
        if flow > best_peak {
            best_peak = flow;
            best_time = time;
        }
    }
    (best_peak, Some(best_time))
}

fn row_metrics(columns: &PassColumns, row_idx: usize) -> ClosureMetrics {
    let runvol = columns.runvol[row_idx];
    [
        runvol,
        columns.sbrunv[row_idx],
        columns.drrunv[row_idx],
        columns.gwbfv[row_idx],
        columns.gwdsv[row_idx],
        columns.tdet[row_idx],
        columns.tdep[row_idx],
        columns.sedcon_1[row_idx] * runvol,
        columns.sedcon_2[row_idx] * runvol,
        columns.sedcon_3[row_idx] * runvol,
        columns.sedcon_4[row_idx] * runvol,
        columns.sedcon_5[row_idx] * runvol,
    ]
}

fn weighted_input_metrics(row_idx: usize, sources: &[WeightedSourceData]) -> ClosureMetrics {
    let mut metrics = [0.0_f64; CLOSURE_NAMES.len()];
    for source in sources {
        let raw = row_metrics(&source.columns, row_idx);
        for idx in 0..metrics.len() {
            metrics[idx] += raw[idx] * source.scale;
        }
    }
    metrics
}

fn sum_source_metrics(columns: &PassColumns) -> ClosureMetrics {
    let mut totals = [0.0_f64; CLOSURE_NAMES.len()];
    for row_idx in 0..columns.len() {
        let row = row_metrics(columns, row_idx);
        for idx in 0..totals.len() {
            totals[idx] += row[idx];
        }
    }
    totals
}

fn build_weighted_diagnostics(
    sources: &[WeightedSourceData],
    reparsed: &PassColumns,
    reparsed_header: &WeightedPassHeader,
    target_area_m2: f64,
    out_pass: &Path,
) -> Result<WeightedPassDiagnostics, InterchangeError> {
    if reparsed.len() != sources[0].columns.len() {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            "Reparsed weighted PASS row count changed after serialization",
            Some(format!(
                "expected_rows={} reparsed_rows={}",
                sources[0].columns.len(),
                reparsed.len()
            )),
        ));
    }

    let target_area_residual_m2 = reparsed_header.modeled_area_m2 - target_area_m2;
    let target_area_budget_m2 = direct_serialization_budget(target_area_m2);
    if target_area_residual_m2.abs() > target_area_budget_m2 {
        return Err(InterchangeError::parse(
            out_pass,
            Some(3),
            "Serialized weighted PASS target area exceeded its closure budget",
            Some(format!(
                "target_area_m2={target_area_m2} reparsed_area_m2={} residual_m2={target_area_residual_m2} budget_m2={target_area_budget_m2}",
                reparsed_header.modeled_area_m2
            )),
        ));
    }

    let source_diagnostics = sources
        .iter()
        .map(|source| {
            let raw_totals = sum_source_metrics(&source.columns);
            let mut weighted_totals = raw_totals;
            for value in &mut weighted_totals {
                *value *= source.scale;
            }
            WeightedSourceDiagnostic {
                source_id: source.source.source_id.clone(),
                climate_token: source.header.climate_token.clone(),
                modeled_area_m2: source.header.modeled_area_m2,
                represented_area_m2: source.source.represented_area_m2,
                scale: source.scale,
                row_count: source.columns.len(),
                raw_totals,
                weighted_totals,
            }
        })
        .collect::<Vec<_>>();

    let mut events = Vec::with_capacity(reparsed.len());
    let mut run_input = [0.0_f64; CLOSURE_NAMES.len()];
    let mut run_output = [0.0_f64; CLOSURE_NAMES.len()];
    let mut run_budgets = [0.0_f64; CLOSURE_NAMES.len()];
    let mut max_abs_event_residuals = [0.0_f64; CLOSURE_NAMES.len()];
    let mut max_event_budget_ratios = [0.0_f64; CLOSURE_NAMES.len()];

    for row_idx in 0..reparsed.len() {
        let input = weighted_input_metrics(row_idx, sources);
        let output = row_metrics(reparsed, row_idx);
        let mut residuals = [0.0_f64; CLOSURE_NAMES.len()];
        let mut budgets = [0.0_f64; CLOSURE_NAMES.len()];
        for metric_idx in 0..CLOSURE_NAMES.len() {
            residuals[metric_idx] = output[metric_idx] - input[metric_idx];
            budgets[metric_idx] = if metric_idx < DIRECT_METRIC_COUNT {
                direct_serialization_budget(input[metric_idx])
            } else {
                sediment_mass_serialization_budget(input[metric_idx], input[0])
            };
            if residuals[metric_idx].abs() > budgets[metric_idx] {
                return Err(InterchangeError::parse(
                    out_pass,
                    None,
                    format!(
                        "Serialized weighted PASS closure exceeded budget for {}",
                        CLOSURE_NAMES[metric_idx]
                    ),
                    Some(format!(
                        "row={row_idx} year={} julian={} expected={} reparsed={} residual={} budget={}",
                        reparsed.year[row_idx],
                        reparsed.julian[row_idx],
                        input[metric_idx],
                        output[metric_idx],
                        residuals[metric_idx],
                        budgets[metric_idx]
                    )),
                ));
            }
            run_input[metric_idx] += input[metric_idx];
            run_output[metric_idx] += output[metric_idx];
            run_budgets[metric_idx] += budgets[metric_idx];
            max_abs_event_residuals[metric_idx] =
                max_abs_event_residuals[metric_idx].max(residuals[metric_idx].abs());
            if budgets[metric_idx] > 0.0 {
                max_event_budget_ratios[metric_idx] = max_event_budget_ratios[metric_idx]
                    .max(residuals[metric_idx].abs() / budgets[metric_idx]);
            }
        }
        events.push(WeightedEventDiagnostic {
            year: reparsed.year[row_idx],
            julian: reparsed.julian[row_idx],
            event: reparsed.event[row_idx].clone(),
            weighted_input: input,
            reparsed_output: output,
            residuals,
            budgets,
        });
    }

    let mut run_residuals = [0.0_f64; CLOSURE_NAMES.len()];
    for metric_idx in 0..CLOSURE_NAMES.len() {
        run_residuals[metric_idx] = run_output[metric_idx] - run_input[metric_idx];
        run_budgets[metric_idx] += floating_point_budget(run_input[metric_idx]);
        if run_residuals[metric_idx].abs() > run_budgets[metric_idx] {
            return Err(InterchangeError::parse(
                out_pass,
                None,
                format!(
                    "Full-run weighted PASS closure exceeded budget for {}",
                    CLOSURE_NAMES[metric_idx]
                ),
                Some(format!(
                    "expected={} reparsed={} residual={} budget={}",
                    run_input[metric_idx],
                    run_output[metric_idx],
                    run_residuals[metric_idx],
                    run_budgets[metric_idx]
                )),
            ));
        }
    }

    Ok(WeightedPassDiagnostics {
        target_area_m2,
        serialized_target_area_m2: reparsed_header.modeled_area_m2,
        target_area_residual_m2,
        target_area_budget_m2,
        sources: source_diagnostics,
        events,
        run: WeightedRunDiagnostic {
            weighted_input: run_input,
            reparsed_output: run_output,
            residuals: run_residuals,
            budgets: run_budgets,
            max_abs_event_residuals,
            max_event_budget_ratios,
        },
    })
}

fn area_sum_budget(left: f64, right: f64) -> f64 {
    64.0 * f64::EPSILON * left.abs().max(right.abs()).max(1.0)
}

fn floating_point_budget(value: f64) -> f64 {
    16.0 * f64::EPSILON * value.abs().max(1.0)
}

fn serialization_half_ulp(value: f64) -> f64 {
    if value == 0.0 {
        return 0.0;
    }
    0.5 * 10.0_f64.powi(value.abs().log10().floor() as i32 - 4)
}

fn direct_serialization_budget(value: f64) -> f64 {
    serialization_half_ulp(value) + floating_point_budget(value)
}

fn sediment_mass_serialization_budget(mass: f64, volume: f64) -> f64 {
    if mass == 0.0 || volume == 0.0 {
        return floating_point_budget(mass);
    }
    let concentration = mass / volume;
    let concentration_budget = direct_serialization_budget(concentration);
    let volume_budget = direct_serialization_budget(volume);
    volume.abs() * concentration_budget
        + concentration.abs() * volume_budget
        + concentration_budget * volume_budget
        + floating_point_budget(mass)
}

fn read_pass_header(path: &Path) -> Result<Vec<String>, InterchangeError> {
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = Vec::with_capacity(5);
    for line in reader.lines().take(5) {
        let line = line.map_err(|err| InterchangeError::io(path, err))?;
        out.push(line);
    }
    if out.len() < 5 {
        return Err(InterchangeError::parse(
            path,
            None,
            "PASS file missing full 5-line metadata header",
            None,
        ));
    }
    parse_simulation_header_line(&out[1], path)?;
    Ok(out)
}

fn parse_simulation_header_line(line: &str, path: &Path) -> Result<(i32, i32), InterchangeError> {
    let mut tokens = line.split_whitespace();
    let years = tokens
        .next()
        .ok_or_else(|| {
            InterchangeError::parse(
                path,
                None,
                "PASS simulation header missing years token",
                Some("line 2".to_string()),
            )
        })?
        .parse::<i32>()
        .map_err(|_| {
            InterchangeError::parse(
                path,
                None,
                "PASS simulation header has invalid years token",
                Some("line 2".to_string()),
            )
        })?;
    let start_year = tokens
        .next()
        .ok_or_else(|| {
            InterchangeError::parse(
                path,
                None,
                "PASS simulation header missing start-year token",
                Some("line 2".to_string()),
            )
        })?
        .parse::<i32>()
        .map_err(|_| {
            InterchangeError::parse(
                path,
                None,
                "PASS simulation header has invalid start-year token",
                Some("line 2".to_string()),
            )
        })?;
    Ok((years, start_year))
}

fn validate_header_compatibility(
    base_path: &Path,
    base_header: &[String],
    source_path: &Path,
    source_header: &[String],
) -> Result<(), InterchangeError> {
    if source_header.len() < 5 {
        return Err(InterchangeError::parse(
            source_path,
            None,
            "PASS file missing full 5-line metadata header",
            None,
        ));
    }

    let base_cli = base_header[0].trim();
    let source_cli = source_header[0].trim();
    if source_cli != base_cli {
        return Err(InterchangeError::parse(
            source_path,
            None,
            "PASS header climate-file token does not match base pass",
            Some(format!(
                "base={} source={} base_path={}",
                base_cli,
                source_cli,
                base_path.display()
            )),
        ));
    }

    let base_meta = parse_simulation_header_line(&base_header[1], base_path)?;
    let source_meta = parse_simulation_header_line(&source_header[1], source_path)?;
    if source_meta != base_meta {
        return Err(InterchangeError::parse(
            source_path,
            None,
            "PASS simulation header does not match base pass",
            Some(format!(
                "base(years={},start_year={}) source(years={},start_year={}) base_path={}",
                base_meta.0,
                base_meta.1,
                source_meta.0,
                source_meta.1,
                base_path.display()
            )),
        ));
    }

    Ok(())
}

fn make_day_key(columns: &PassColumns, row_idx: usize) -> DayKey {
    let year = columns.year[row_idx];
    let julian = columns.julian[row_idx];
    if year > 0 && julian > 0 {
        DayKey {
            year,
            julian,
            sim_day_index: 0,
        }
    } else {
        DayKey {
            year: 0,
            julian: 0,
            sim_day_index: columns.sim_day_index[row_idx],
        }
    }
}

fn validate_calendar_alignment(sources: &[SourceData]) -> Result<(), InterchangeError> {
    if sources.is_empty() {
        return Ok(());
    }

    let base_source = &sources[0];
    let base_row_count = base_source.columns.len();
    let mut base_keys = Vec::with_capacity(base_row_count);
    for row_idx in 0..base_row_count {
        base_keys.push(make_day_key(&base_source.columns, row_idx));
    }

    for source in sources.iter().skip(1) {
        let source_row_count = source.columns.len();
        if source_row_count != base_row_count {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                "PASS calendar row count does not match base pass",
                Some(format!(
                    "base_rows={} source_rows={} base_path={}",
                    base_row_count,
                    source_row_count,
                    base_source.path.display()
                )),
            ));
        }

        for row_idx in 0..source_row_count {
            let source_key = make_day_key(&source.columns, row_idx);
            let base_key = base_keys[row_idx];
            if source_key != base_key {
                return Err(InterchangeError::parse(
                    &source.path,
                    None,
                    "PASS calendar day key does not align with base pass",
                    Some(format!(
                        "row={} base=({},{},{}) source=({},{},{}) base_path={}",
                        row_idx,
                        base_key.year,
                        base_key.julian,
                        base_key.sim_day_index,
                        source_key.year,
                        source_key.julian,
                        source_key.sim_day_index,
                        base_source.path.display()
                    )),
                ));
            }
        }
    }

    Ok(())
}

fn resolve_day_kind(
    rows: &[RowRef],
    sources: &[SourceData],
) -> Result<EventKind, InterchangeError> {
    let mut kind = EventKind::NoEvent;
    for row in rows {
        let source = &sources[row.source_idx];
        let row_kind = parse_event_kind(
            &source.columns.event[row.row_idx],
            &source.path,
            row.row_idx,
        )?;
        if row_kind > kind {
            kind = row_kind;
        }
    }
    Ok(kind)
}

fn parse_event_kind(
    label: &str,
    path: &Path,
    row_idx: usize,
) -> Result<EventKind, InterchangeError> {
    match label.trim().to_ascii_uppercase().as_str() {
        "EVENT" => Ok(EventKind::Event),
        "SUBEVENT" => Ok(EventKind::SubEvent),
        "NO EVENT" => Ok(EventKind::NoEvent),
        other => Err(InterchangeError::parse(
            path,
            None,
            format!("Unsupported PASS event label '{other}'"),
            Some(format!("row index {row_idx}")),
        )),
    }
}

fn sum_field<F>(rows: &[RowRef], sources: &[SourceData], getter: F) -> f64
where
    F: Fn(&PassColumns, usize) -> f64,
{
    rows.iter()
        .map(|row| getter(&sources[row.source_idx].columns, row.row_idx))
        .sum()
}

fn max_field<F>(rows: &[RowRef], sources: &[SourceData], getter: F) -> f64
where
    F: Fn(&PassColumns, usize) -> f64,
{
    rows.iter()
        .map(|row| getter(&sources[row.source_idx].columns, row.row_idx))
        .fold(0.0_f64, f64::max)
}

fn weighted_average_by_runvol<F>(rows: &[RowRef], sources: &[SourceData], getter: F) -> f64
where
    F: Fn(&PassColumns, usize) -> f64,
{
    let mut weighted_sum = 0.0_f64;
    let mut total_runvol = 0.0_f64;
    for row in rows {
        let cols = &sources[row.source_idx].columns;
        let runvol = cols.runvol[row.row_idx];
        if runvol > 0.0 {
            total_runvol += runvol;
            weighted_sum += getter(cols, row.row_idx) * runvol;
        }
    }
    if total_runvol > 0.0 {
        weighted_sum / total_runvol
    } else {
        0.0
    }
}

fn combine_peakro_phase1(rows: &[RowRef], sources: &[SourceData]) -> f64 {
    let mut hydro_components = Vec::new();
    for row in rows {
        let cols = &sources[row.source_idx].columns;
        let idx = row.row_idx;
        if let Some(component) =
            build_hydro_triangle(cols.runvol[idx], cols.peakro[idx], cols.tcs[idx])
        {
            hydro_components.push(component);
        }
    }

    if hydro_components.is_empty() {
        return 0.0;
    }

    let mut breakpoints = vec![0.0_f64];
    for component in &hydro_components {
        breakpoints.push(component.t_peak);
        breakpoints.push(component.t_end);
    }
    breakpoints.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    breakpoints.dedup_by(|left, right| (*left - *right).abs() < 1.0e-9);

    breakpoints
        .into_iter()
        .map(|t| {
            hydro_components
                .iter()
                .map(|component| triangle_flow_at(component, t))
                .sum::<f64>()
        })
        .fold(0.0_f64, f64::max)
}

fn combine_peakro_phase4(rows: &[RowRef], sources: &[SourceData]) -> f64 {
    // Phase-4 replacement combines currently use the same hydrograph superposition
    // reconstruction as phase-1 while retaining an explicit strategy hook for future
    // divergence without changing Python-facing contracts.
    combine_peakro_phase1(rows, sources)
}

fn combine_peakro(strategy: CombineStrategy, rows: &[RowRef], sources: &[SourceData]) -> f64 {
    match strategy {
        CombineStrategy::Phase1 => combine_peakro_phase1(rows, sources),
        CombineStrategy::Phase4 => combine_peakro_phase4(rows, sources),
    }
}

fn build_hydro_triangle(runvol: f64, peakro: f64, tcs: f64) -> Option<HydroTriangle> {
    if runvol <= 0.0 || peakro <= 0.0 {
        return None;
    }

    let t_peak = tcs.max(1.0e-6);
    let mut t_end = (2.0 * runvol) / (peakro * 3600.0);
    if !t_end.is_finite() || t_end <= 0.0 {
        return None;
    }
    if t_end < t_peak {
        t_end = t_peak;
    }

    Some(HydroTriangle {
        peak: peakro,
        t_peak,
        t_end,
    })
}

fn triangle_flow_at(component: &HydroTriangle, t: f64) -> f64 {
    if t < 0.0 || t > component.t_end {
        return 0.0;
    }

    if t <= component.t_peak {
        return component.peak * (t / component.t_peak);
    }

    let falling_duration = component.t_end - component.t_peak;
    if falling_duration <= 0.0 {
        return component.peak;
    }

    let fraction = 1.0 - ((t - component.t_peak) / falling_duration);
    (component.peak * fraction).max(0.0)
}

fn combine_row_for_kind(
    key: DayKey,
    rows: &[RowRef],
    sources: &[SourceData],
    kind: EventKind,
    strategy: CombineStrategy,
) -> CombinedRow {
    let (year, julian) = if key.year > 0 && key.julian > 0 {
        (key.year, key.julian)
    } else {
        let first = rows[0];
        let columns = &sources[first.source_idx].columns;
        (columns.year[first.row_idx], columns.julian[first.row_idx])
    };

    match kind {
        EventKind::NoEvent => CombinedRow {
            event: "NO EVENT",
            year,
            julian,
            dur: 0.0,
            tcs: 0.0,
            oalpha: 0.0,
            runoff: 0.0,
            runvol: 0.0,
            sbrunf: 0.0,
            sbrunv: 0.0,
            drainq: 0.0,
            drrunv: 0.0,
            peakro: 0.0,
            tdet: 0.0,
            tdep: 0.0,
            sedcon_1: 0.0,
            sedcon_2: 0.0,
            sedcon_3: 0.0,
            sedcon_4: 0.0,
            sedcon_5: 0.0,
            clot: 0.0,
            slot: 0.0,
            saot: 0.0,
            laot: 0.0,
            sdot: 0.0,
            gwbfv: sum_field(rows, sources, |cols, idx| cols.gwbfv[idx]),
            gwdsv: sum_field(rows, sources, |cols, idx| cols.gwdsv[idx]),
        },
        EventKind::SubEvent => CombinedRow {
            event: "SUBEVENT",
            year,
            julian,
            dur: 0.0,
            tcs: 0.0,
            oalpha: 0.0,
            runoff: 0.0,
            runvol: 0.0,
            sbrunf: sum_field(rows, sources, |cols, idx| cols.sbrunf[idx]),
            sbrunv: sum_field(rows, sources, |cols, idx| cols.sbrunv[idx]),
            drainq: sum_field(rows, sources, |cols, idx| cols.drainq[idx]),
            drrunv: sum_field(rows, sources, |cols, idx| cols.drrunv[idx]),
            peakro: 0.0,
            tdet: 0.0,
            tdep: 0.0,
            sedcon_1: 0.0,
            sedcon_2: 0.0,
            sedcon_3: 0.0,
            sedcon_4: 0.0,
            sedcon_5: 0.0,
            clot: 0.0,
            slot: 0.0,
            saot: 0.0,
            laot: 0.0,
            sdot: 0.0,
            gwbfv: sum_field(rows, sources, |cols, idx| cols.gwbfv[idx]),
            gwdsv: sum_field(rows, sources, |cols, idx| cols.gwdsv[idx]),
        },
        EventKind::Event => {
            let runvol = sum_field(rows, sources, |cols, idx| cols.runvol[idx]);
            let tcs = max_field(rows, sources, |cols, idx| cols.tcs[idx]);
            let peakro = combine_peakro(strategy, rows, sources);
            let oalpha = if runvol > 0.0 {
                let tc_floor = tcs / 24.0;
                let hydro_term = (peakro * 3600.0 * tcs) / runvol;
                tc_floor.max(hydro_term)
            } else {
                0.0
            };

            let sediment_mass = |getter: fn(&PassColumns, usize) -> f64| -> f64 {
                rows.iter()
                    .map(|row| {
                        let cols = &sources[row.source_idx].columns;
                        getter(cols, row.row_idx) * cols.runvol[row.row_idx]
                    })
                    .sum()
            };
            let sedcon_from_mass = |mass: f64| -> f64 {
                if runvol > 0.0 {
                    mass / runvol
                } else {
                    0.0
                }
            };

            CombinedRow {
                event: "EVENT",
                year,
                julian,
                dur: max_field(rows, sources, |cols, idx| cols.dur[idx]),
                tcs,
                oalpha,
                runoff: sum_field(rows, sources, |cols, idx| cols.runoff[idx]),
                runvol,
                sbrunf: sum_field(rows, sources, |cols, idx| cols.sbrunf[idx]),
                sbrunv: sum_field(rows, sources, |cols, idx| cols.sbrunv[idx]),
                drainq: sum_field(rows, sources, |cols, idx| cols.drainq[idx]),
                drrunv: sum_field(rows, sources, |cols, idx| cols.drrunv[idx]),
                peakro,
                tdet: sum_field(rows, sources, |cols, idx| cols.tdet[idx]),
                tdep: sum_field(rows, sources, |cols, idx| cols.tdep[idx]),
                sedcon_1: sedcon_from_mass(sediment_mass(|cols, idx| cols.sedcon_1[idx])),
                sedcon_2: sedcon_from_mass(sediment_mass(|cols, idx| cols.sedcon_2[idx])),
                sedcon_3: sedcon_from_mass(sediment_mass(|cols, idx| cols.sedcon_3[idx])),
                sedcon_4: sedcon_from_mass(sediment_mass(|cols, idx| cols.sedcon_4[idx])),
                sedcon_5: sedcon_from_mass(sediment_mass(|cols, idx| cols.sedcon_5[idx])),
                clot: weighted_average_by_runvol(rows, sources, |cols, idx| cols.clot[idx]),
                slot: weighted_average_by_runvol(rows, sources, |cols, idx| cols.slot[idx]),
                saot: weighted_average_by_runvol(rows, sources, |cols, idx| cols.saot[idx]),
                laot: weighted_average_by_runvol(rows, sources, |cols, idx| cols.laot[idx]),
                sdot: weighted_average_by_runvol(rows, sources, |cols, idx| cols.sdot[idx]),
                gwbfv: sum_field(rows, sources, |cols, idx| cols.gwbfv[idx]),
                gwdsv: sum_field(rows, sources, |cols, idx| cols.gwdsv[idx]),
            }
        }
    }
}

fn write_combined_pass(
    out_pass: &Path,
    header_lines: &[String],
    combined_rows: &[CombinedRow],
    legacy_five_significant: bool,
) -> Result<(), InterchangeError> {
    if let Some(parent) = out_pass.parent() {
        fs::create_dir_all(parent).map_err(|err| InterchangeError::io(parent, err))?;
    }

    let file = File::create(out_pass).map_err(|err| InterchangeError::io(out_pass, err))?;
    let mut writer = BufWriter::new(file);

    for line in header_lines {
        writeln!(writer, "{line}").map_err(|err| InterchangeError::io(out_pass, err))?;
    }

    for row in combined_rows {
        if row.event == "EVENT" {
            let values = [
                row.dur,
                row.tcs,
                row.oalpha,
                row.runoff,
                row.runvol,
                row.sbrunf,
                row.sbrunv,
                row.drainq,
                row.drrunv,
                row.peakro,
                row.tdet,
                row.tdep,
                row.sedcon_1,
                row.sedcon_2,
                row.sedcon_3,
                row.sedcon_4,
                row.sedcon_5,
                row.clot,
                row.slot,
                row.saot,
                row.laot,
                row.sdot,
                row.gwbfv,
                row.gwdsv,
            ];
            write_event_line(
                &mut writer,
                row.event,
                row.year,
                row.julian,
                &values,
                out_pass,
                legacy_five_significant,
            )?;
        } else if row.event == "SUBEVENT" {
            let values = [
                row.sbrunf, row.sbrunv, row.drainq, row.drrunv, row.gwbfv, row.gwdsv,
            ];
            write_scalar_row_line(
                &mut writer,
                row.event,
                row.year,
                row.julian,
                &values,
                out_pass,
                legacy_five_significant,
            )?;
        } else {
            let values = [row.gwbfv, row.gwdsv];
            write_scalar_row_line(
                &mut writer,
                row.event,
                row.year,
                row.julian,
                &values,
                out_pass,
                legacy_five_significant,
            )?;
        }
    }

    writer
        .flush()
        .map_err(|err| InterchangeError::io(out_pass, err))?;
    Ok(())
}

fn format_fortran_e11_5(value: f64, out_pass: &Path) -> Result<String, InterchangeError> {
    if !value.is_finite() {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!("PASS writer encountered non-finite value: {value}"),
            None,
        ));
    }

    let scientific = format!("{value:.5E}");
    let (mantissa, exponent_token) = scientific.split_once('E').ok_or_else(|| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to format PASS value '{scientific}'"),
            None,
        )
    })?;
    let exponent = exponent_token.parse::<i32>().map_err(|_| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to parse exponent in PASS value '{scientific}'"),
            None,
        )
    })?;
    let exponent_sign = if exponent >= 0 { '+' } else { '-' };
    let exponent_abs = exponent.abs();
    if exponent_abs >= 100 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!(
                "PASS value exponent magnitude {exponent_abs} cannot be represented in E11.5 format"
            ),
            Some(format!("value={value}")),
        ));
    }
    let exponent_digits = format!("{exponent_abs:02}");

    let formatted = format!("{mantissa}E{exponent_sign}{exponent_digits}");
    Ok(format!("{formatted:>11}"))
}

fn format_legacy_fortran_e11_5(value: f64, out_pass: &Path) -> Result<String, InterchangeError> {
    if !value.is_finite() || value < 0.0 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!("PASS writer encountered non-finite or negative value: {value}"),
            None,
        ));
    }
    if value == 0.0 {
        return Ok("0.00000E+00".to_string());
    }

    let scientific = format!("{value:.4E}");
    let (mantissa, exponent_token) = scientific.split_once('E').ok_or_else(|| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to format PASS value '{scientific}'"),
            None,
        )
    })?;
    let exponent = exponent_token.parse::<i32>().map_err(|_| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to parse exponent in PASS value '{scientific}'"),
            None,
        )
    })? + 1;
    let exponent_sign = if exponent >= 0 { '+' } else { '-' };
    let exponent_abs = exponent.abs();
    if exponent_abs >= 100 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!(
                "PASS value exponent magnitude {exponent_abs} cannot be represented in E11.5 format"
            ),
            Some(format!("value={value}")),
        ));
    }
    let digits = mantissa.replace('.', "");
    Ok(format!(
        "0.{}E{exponent_sign}{exponent_abs:02}",
        &digits[..5]
    ))
}

fn format_pass_value(
    value: f64,
    out_pass: &Path,
    legacy_five_significant: bool,
) -> Result<String, InterchangeError> {
    if legacy_five_significant {
        format_legacy_fortran_e11_5(value, out_pass)
    } else {
        format_fortran_e11_5(value, out_pass)
    }
}

fn format_fortran_e10_5(value: f64, out_pass: &Path) -> Result<String, InterchangeError> {
    if !value.is_finite() || value < 0.0 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!("PASS header area must be finite and nonnegative: {value}"),
            None,
        ));
    }
    if value == 0.0 {
        return Ok(".00000E+00".to_string());
    }

    let scientific = format!("{value:.4E}");
    let (mantissa, exponent_token) = scientific.split_once('E').ok_or_else(|| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to format PASS header area '{scientific}'"),
            None,
        )
    })?;
    let exponent = exponent_token.parse::<i32>().map_err(|_| {
        InterchangeError::parse(
            out_pass,
            None,
            format!("Unable to parse exponent in PASS header area '{scientific}'"),
            None,
        )
    })? + 1;
    let digits = mantissa.replace('.', "");
    let significant = &digits[..5];
    let exponent_sign = if exponent >= 0 { '+' } else { '-' };
    let exponent_abs = exponent.abs();
    if exponent_abs >= 100 {
        return Err(InterchangeError::parse(
            out_pass,
            None,
            format!(
                "PASS header area exponent magnitude {exponent_abs} cannot be represented in E10.5 format"
            ),
            Some(format!("value={value}")),
        ));
    }
    Ok(format!(".{}E{exponent_sign}{exponent_abs:02}", significant))
}

fn write_label_and_day(
    writer: &mut BufWriter<File>,
    label: &str,
    year: i16,
    julian: i16,
    out_pass: &Path,
) -> Result<(), InterchangeError> {
    write!(writer, "{label:<8}  {year:>5} {julian:>5}     ")
        .map_err(|err| InterchangeError::io(out_pass, err))
}

fn write_scalar_row_line(
    writer: &mut BufWriter<File>,
    label: &str,
    year: i16,
    julian: i16,
    values: &[f64],
    out_pass: &Path,
    legacy_five_significant: bool,
) -> Result<(), InterchangeError> {
    write_label_and_day(writer, label, year, julian, out_pass)?;
    for value in values {
        write!(
            writer,
            "{} ",
            format_pass_value(*value, out_pass, legacy_five_significant)?
        )
        .map_err(|err| InterchangeError::io(out_pass, err))?;
    }
    writeln!(writer).map_err(|err| InterchangeError::io(out_pass, err))
}

fn write_event_line(
    writer: &mut BufWriter<File>,
    label: &str,
    year: i16,
    julian: i16,
    values: &[f64],
    out_pass: &Path,
    legacy_five_significant: bool,
) -> Result<(), InterchangeError> {
    write_label_and_day(writer, label, year, julian, out_pass)?;
    let first_group_end = values.len().min(10);
    for value in &values[..first_group_end] {
        write!(
            writer,
            "{} ",
            format_pass_value(*value, out_pass, legacy_five_significant)?
        )
        .map_err(|err| InterchangeError::io(out_pass, err))?;
    }
    if values.len() > 10 {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
        let second_group_end = values.len().min(15);
        for value in &values[10..second_group_end] {
            write!(
                writer,
                "{} ",
                format_pass_value(*value, out_pass, legacy_five_significant)?
            )
            .map_err(|err| InterchangeError::io(out_pass, err))?;
        }
    }
    if values.len() > 15 {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
    }
    let third_group_end = values.len().min(20);
    if values.len() > 15 {
        for value in &values[15..third_group_end] {
            write!(
                writer,
                "{} ",
                format_pass_value(*value, out_pass, legacy_five_significant)?
            )
            .map_err(|err| InterchangeError::io(out_pass, err))?;
        }
    }
    writeln!(writer).map_err(|err| InterchangeError::io(out_pass, err))?;

    let mut idx = third_group_end;
    while idx < values.len() {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
        let next_idx = (idx + 5).min(values.len());
        for value in &values[idx..next_idx] {
            write!(
                writer,
                "{} ",
                format_pass_value(*value, out_pass, legacy_five_significant)?
            )
            .map_err(|err| InterchangeError::io(out_pass, err))?;
        }
        writeln!(writer).map_err(|err| InterchangeError::io(out_pass, err))?;
        idx = next_idx;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn make_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "wepp_interchange_hill_pass_combine_{label}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&dir).expect("failed to create temp directory");
        dir
    }

    fn write_pass(path: &Path, data_lines: &[String]) {
        let mut lines = vec![
            "p1.cli".to_string(),
            "   16      2000".to_string(),
            ".44409E+04".to_string(),
            "  5    0.20000E-05 0.10000E-04 0.30000E-04 0.30600E-03 0.20000E-03".to_string(),
            "    0.00     0.00     0.00     0.00".to_string(),
        ];
        lines.extend(data_lines.iter().cloned());
        let mut payload = lines.join("\n");
        payload.push('\n');
        fs::write(path, payload).expect("failed to write pass file");
    }

    fn format_line(label: &str, year: i32, julian: i32, values: &[f64]) -> String {
        let mut line = format!("{label:<8}{year:>6}{julian:>6}");
        for value in values {
            line.push_str(&format!(" {:>14.5E}", value));
        }
        line
    }

    fn event_line(year: i32, julian: i32, values: [f64; 24]) -> String {
        format_line("EVENT", year, julian, &values)
    }

    fn subevent_line(year: i32, julian: i32, values: [f64; 6]) -> String {
        format_line("SUBEVENT", year, julian, &values)
    }

    fn noevent_line(year: i32, julian: i32, values: [f64; 2]) -> String {
        format_line("NO EVENT", year, julian, &values)
    }

    #[test]
    fn combines_event_and_subevent_rows_with_event_precedence() {
        let tmp_dir = make_temp_dir("event_precedence");
        let base_path = tmp_dir.join("H1.pass.dat");
        let road_path = tmp_dir.join("H900.pass.dat");
        let out_path = tmp_dir.join("H1.combined.pass.dat");

        write_pass(
            &base_path,
            &[event_line(
                2000,
                1,
                [
                    10.0, 2.0, 1.0, 1.5, 100.0, 0.2, 10.0, 0.3, 5.0, 2.0, 1.0, 0.5, 1.0, 2.0, 3.0,
                    4.0, 5.0, 0.1, 0.2, 0.3, 0.4, 0.5, 1.0, 2.0,
                ],
            )],
        );
        write_pass(
            &road_path,
            &[subevent_line(2000, 1, [0.3, 30.0, 0.7, 8.0, 4.0, 5.0])],
        );

        combine_hillslope_pass_files(&base_path, &[road_path.clone()], &out_path, "phase1")
            .expect("combine should succeed");

        let version = VersionInfo::new(1, 0);
        let columns = hillslope_pass_to_columns(&out_path, None, &version)
            .expect("combined pass parse failed");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns.event[0], "EVENT");
        assert!((columns.runvol[0] - 100.0).abs() < 1.0e-8);
        assert!((columns.sbrunv[0] - 40.0).abs() < 1.0e-8);
        assert!((columns.drrunv[0] - 13.0).abs() < 1.0e-8);
        assert!((columns.gwbfv[0] - 5.0).abs() < 1.0e-8);
        assert!((columns.gwdsv[0] - 7.0).abs() < 1.0e-8);

        let header = fs::read_to_string(&base_path).expect("missing base output");
        let combined = fs::read_to_string(&out_path).expect("missing combined output");
        let header_lines: Vec<&str> = header.lines().take(5).collect();
        let combined_header_lines: Vec<&str> = combined.lines().take(5).collect();
        assert_eq!(header_lines, combined_header_lines);
    }

    #[test]
    fn writes_event_rows_with_fortran_style_exponents_and_continuation_line() {
        let tmp_dir = make_temp_dir("fortran_format");
        let base_path = tmp_dir.join("H11.pass.dat");
        let road_path = tmp_dir.join("H911.pass.dat");
        let out_path = tmp_dir.join("H11.combined.pass.dat");

        write_pass(
            &base_path,
            &[event_line(
                2000,
                9,
                [
                    4.0, 1.0, 0.5, 2.0, 120.0, 0.1, 2.0, 0.2, 3.0, 1.2, 0.4, 0.2, 1.0, 0.5, 0.3,
                    0.2, 0.1, 0.7, 0.6, 0.5, 0.4, 0.3, 9.0, 8.0,
                ],
            )],
        );
        write_pass(
            &road_path,
            &[subevent_line(2000, 9, [0.3, 4.0, 0.7, 8.0, 1.0, 2.0])],
        );

        combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase1")
            .expect("combine should succeed");

        let combined = fs::read_to_string(&out_path).expect("missing combined output");
        let data_lines: Vec<&str> = combined.lines().skip(5).collect();
        let event_index = data_lines
            .iter()
            .position(|line| line.starts_with("EVENT"))
            .expect("missing EVENT line");
        let event_line = data_lines[event_index];
        let continuation_line = data_lines
            .get(event_index + 1)
            .expect("missing EVENT continuation line");

        assert!(continuation_line.starts_with("     "));
        assert!(!continuation_line.trim_start().starts_with("EVENT"));

        let check_exponent_tokens = |line: &str, skip_tokens: usize| {
            for token in line.split_whitespace().skip(skip_tokens) {
                let (_, exponent) = token
                    .split_once('E')
                    .expect("numeric token should contain exponent");
                assert!(exponent.starts_with('+') || exponent.starts_with('-'));
                assert!(exponent[1..].len() >= 2);
            }
        };
        check_exponent_tokens(event_line, 3);
        check_exponent_tokens(continuation_line, 0);
    }

    #[test]
    fn enforces_subevent_and_no_event_zeroing_rules() {
        let tmp_dir = make_temp_dir("subevent_noevent");
        let base_path = tmp_dir.join("H2.pass.dat");
        let road_path = tmp_dir.join("H901.pass.dat");
        let out_path = tmp_dir.join("H2.combined.pass.dat");

        write_pass(
            &base_path,
            &[
                subevent_line(2000, 2, [0.2, 5.0, 0.4, 6.0, 1.0, 2.0]),
                noevent_line(2000, 3, [9.0, 8.0]),
            ],
        );
        write_pass(
            &road_path,
            &[
                noevent_line(2000, 2, [3.0, 4.0]),
                noevent_line(2000, 3, [1.0, 1.0]),
            ],
        );

        combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase1")
            .expect("combine should succeed");

        let version = VersionInfo::new(1, 0);
        let columns = hillslope_pass_to_columns(&out_path, None, &version)
            .expect("combined pass parse failed");
        assert_eq!(columns.len(), 2);

        assert_eq!(columns.event[0], "SUBEVENT");
        assert!((columns.sbrunv[0] - 5.0).abs() < 1.0e-8);
        assert!((columns.drrunv[0] - 6.0).abs() < 1.0e-8);
        assert!((columns.gwbfv[0] - 4.0).abs() < 1.0e-8);
        assert!((columns.gwdsv[0] - 6.0).abs() < 1.0e-8);
        assert!(columns.runvol[0].abs() < 1.0e-8);
        assert!(columns.peakro[0].abs() < 1.0e-8);

        assert_eq!(columns.event[1], "NO EVENT");
        assert!((columns.gwbfv[1] - 10.0).abs() < 1.0e-8);
        assert!((columns.gwdsv[1] - 9.0).abs() < 1.0e-8);
    }

    #[test]
    fn recomputes_oalpha_from_combined_peak_and_volume() {
        let tmp_dir = make_temp_dir("oalpha");
        let base_path = tmp_dir.join("H3.pass.dat");
        let road_path = tmp_dir.join("H902.pass.dat");
        let out_path = tmp_dir.join("H3.combined.pass.dat");

        write_pass(
            &base_path,
            &[event_line(
                2000,
                10,
                [
                    5.0, 1.0, 0.2, 0.6, 100.0, 0.0, 0.0, 0.0, 0.0, 2.0, 1.0, 0.0, 1.0, 0.0, 0.0,
                    0.0, 0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0,
                ],
            )],
        );
        write_pass(
            &road_path,
            &[event_line(
                2000,
                10,
                [
                    3.0, 2.0, 0.1, 0.3, 40.0, 0.0, 0.0, 0.0, 0.0, 1.5, 2.0, 0.0, 2.0, 0.0, 0.0,
                    0.0, 0.0, 0.8, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0,
                ],
            )],
        );

        combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase1")
            .expect("combine should succeed");

        let version = VersionInfo::new(1, 0);
        let columns = hillslope_pass_to_columns(&out_path, None, &version)
            .expect("combined pass parse failed");
        assert_eq!(columns.len(), 1);
        assert!((columns.runvol[0] - 140.0).abs() < 1.0e-8);
        assert!((columns.tcs[0] - 2.0).abs() < 1.0e-8);
        assert!((columns.dur[0] - 5.0).abs() < 1.0e-8);

        assert!(columns.oalpha[0] >= (columns.tcs[0] / 24.0));
        assert!(columns.oalpha[0] >= 0.0);

        let expected_sedcon_1 = ((1.0 * 100.0) + (2.0 * 40.0)) / 140.0;
        assert!((columns.sedcon_1[0] - expected_sedcon_1).abs() < 1.0e-5);
    }

    #[test]
    fn accepts_phase4_strategy() {
        let tmp_dir = make_temp_dir("phase4_strategy");
        let base_path = tmp_dir.join("H40.pass.dat");
        let road_path = tmp_dir.join("H940.pass.dat");
        let out_path = tmp_dir.join("H40.combined.pass.dat");

        write_pass(
            &base_path,
            &[event_line(
                2000,
                12,
                [
                    4.0, 1.0, 0.2, 0.5, 100.0, 0.1, 10.0, 0.2, 5.0, 2.0, 0.3, 0.1, 1.0, 0.5, 0.3,
                    0.2, 0.1, 0.7, 0.6, 0.5, 0.4, 0.3, 2.0, 3.0,
                ],
            )],
        );
        write_pass(&road_path, &[noevent_line(2000, 12, [1.0, 2.0])]);

        combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase4")
            .expect("phase4 combine should succeed");

        let version = VersionInfo::new(1, 0);
        let columns = hillslope_pass_to_columns(&out_path, None, &version)
            .expect("combined pass parse failed");
        assert_eq!(columns.len(), 1);
        assert_eq!(columns.event[0], "EVENT");
        assert!((columns.runvol[0] - 100.0).abs() < 1.0e-8);
        assert!((columns.gwbfv[0] - 3.0).abs() < 1.0e-8);
        assert!((columns.gwdsv[0] - 5.0).abs() < 1.0e-8);
    }
    #[test]
    fn rejects_unknown_strategy() {
        let tmp_dir = make_temp_dir("bad_strategy");
        let base_path = tmp_dir.join("H4.pass.dat");
        let out_path = tmp_dir.join("H4.combined.pass.dat");
        write_pass(&base_path, &[noevent_line(2000, 1, [0.0, 0.0])]);

        let err = combine_hillslope_pass_files(&base_path, &[], &out_path, "phase2")
            .expect_err("combine should reject unsupported strategy");
        assert!(err
            .display_message()
            .contains("Unsupported pass combine strategy"));
    }

    #[test]
    fn rejects_calendar_mismatch_between_base_and_road_pass() {
        let tmp_dir = make_temp_dir("calendar_mismatch");
        let base_path = tmp_dir.join("H5.pass.dat");
        let road_path = tmp_dir.join("H905.pass.dat");
        let out_path = tmp_dir.join("H5.combined.pass.dat");

        write_pass(&base_path, &[noevent_line(2000, 1, [0.0, 0.0])]);
        write_pass(&road_path, &[noevent_line(2000, 2, [0.0, 0.0])]);

        let err = combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase1")
            .expect_err("calendar mismatch should fail");
        assert!(err
            .display_message()
            .contains("calendar day key does not align with base pass"));
    }

    #[test]
    fn rejects_truncated_pass_header() {
        let tmp_dir = make_temp_dir("truncated_header");
        let base_path = tmp_dir.join("H6.pass.dat");
        let out_path = tmp_dir.join("H6.combined.pass.dat");

        fs::write(&base_path, "p1.cli\n").expect("failed to write truncated pass");

        let err = combine_hillslope_pass_files(&base_path, &[], &out_path, "phase1")
            .expect_err("truncated header should fail");
        assert!(err
            .display_message()
            .contains("missing full 5-line metadata header"));
    }

    #[test]
    fn rejects_simulation_header_mismatch() {
        let tmp_dir = make_temp_dir("header_mismatch");
        let base_path = tmp_dir.join("H7.pass.dat");
        let road_path = tmp_dir.join("H907.pass.dat");
        let out_path = tmp_dir.join("H7.combined.pass.dat");

        write_pass(&base_path, &[noevent_line(2000, 1, [0.0, 0.0])]);
        write_pass(&road_path, &[noevent_line(2000, 1, [0.0, 0.0])]);

        let road_text = fs::read_to_string(&road_path).expect("failed to read road pass");
        let mut lines: Vec<String> = road_text.lines().map(|line| line.to_string()).collect();
        lines[1] = "   16      2001".to_string();
        fs::write(&road_path, lines.join("\n") + "\n").expect("failed to rewrite road pass");

        let err = combine_hillslope_pass_files(&base_path, &[road_path], &out_path, "phase1")
            .expect_err("simulation header mismatch should fail");
        assert!(err
            .display_message()
            .contains("simulation header does not match base pass"));
    }

    #[test]
    fn rejects_e11_5_exponent_overflow_values() {
        let out_path = PathBuf::from("synthetic.pass.dat");
        let err =
            format_fortran_e11_5(1.0e120, &out_path).expect_err("overflow exponent should fail");
        assert!(err
            .display_message()
            .contains("cannot be represented in E11.5 format"));
    }

    fn weighted_source(
        source_id: &str,
        path: &Path,
        represented_area_m2: f64,
    ) -> WeightedPassSource {
        WeightedPassSource {
            source_id: source_id.to_string(),
            path: path.to_path_buf(),
            represented_area_m2,
        }
    }

    fn rewrite_header(path: &Path, climate_token: &str, modeled_area_m2: f64) {
        let text = fs::read_to_string(path).expect("failed to read synthetic PASS");
        let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
        lines[0] = climate_token.to_string();
        lines[2] = format!("{modeled_area_m2:.5E}");
        fs::write(path, lines.join("\n") + "\n").expect("failed to rewrite synthetic PASS");
    }

    fn representative_event_values() -> [f64; 24] {
        [
            3600.0,
            1.0,
            0.72,
            0.02,
            100.0,
            0.001,
            5.0,
            0.0004,
            2.0,
            2.0,
            3.0,
            1.0,
            0.01,
            0.02,
            0.03,
            0.04,
            0.05,
            1.0 / 15.0,
            2.0 / 15.0,
            3.0 / 15.0,
            4.0 / 15.0,
            5.0 / 15.0,
            4.0,
            6.0,
        ]
    }

    #[test]
    fn weighted_single_source_round_trips_header_and_closure() {
        let tmp_dir = make_temp_dir("weighted_single");
        let source_path = tmp_dir.join("H50.pass.dat");
        let out_path = tmp_dir.join("H50.weighted.pass.dat");
        write_pass(
            &source_path,
            &[
                event_line(2000, 1, representative_event_values()),
                noevent_line(2000, 2, [7.0, 8.0]),
            ],
        );

        let diagnostics = combine_weighted_hillslope_pass_files(
            &[weighted_source("background", &source_path, 4440.9)],
            &out_path,
            4440.9,
            "../runs/p50.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect("weighted combine should succeed");

        assert_eq!(diagnostics.sources.len(), 1);
        assert_eq!(diagnostics.events.len(), 2);
        assert!(diagnostics
            .run
            .residuals
            .iter()
            .zip(diagnostics.run.budgets.iter())
            .all(|(residual, budget)| residual.abs() <= *budget));
        let output = fs::read_to_string(&out_path).expect("missing weighted PASS");
        let header = output.lines().take(5).collect::<Vec<_>>();
        assert_eq!(header[0], "../runs/p50.cli");
        assert_eq!(header[2], ".44409E+04");
        assert!(output.lines().skip(5).any(|line| line.starts_with("EVENT")));
        assert!(output.contains("0.10000E+03"));
    }

    #[test]
    fn weighted_zero_area_background_does_not_change_full_coverage_result() {
        let tmp_dir = make_temp_dir("weighted_full_coverage");
        let background_path = tmp_dir.join("H51.pass.dat");
        let field_path = tmp_dir.join("H951.pass.dat");
        let out_path = tmp_dir.join("H51.weighted.pass.dat");
        let mut background_values = representative_event_values();
        background_values[4] = 999.0;
        let mut field_values = representative_event_values();
        field_values[4] = 120.0;
        write_pass(&background_path, &[event_line(2000, 1, background_values)]);
        write_pass(&field_path, &[event_line(2000, 1, field_values)]);
        rewrite_header(&field_path, "different-token.cli", 4440.9);

        combine_weighted_hillslope_pass_files(
            &[
                weighted_source("background", &background_path, 0.0),
                weighted_source("field:951", &field_path, 4440.9),
            ],
            &out_path,
            4440.9,
            "../runs/p51.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect("full coverage combine should succeed");

        let columns = hillslope_pass_to_columns(&out_path, None, &VersionInfo::new(1, 0))
            .expect("weighted output should parse");
        assert!((columns.runvol[0] - 120.0).abs() < 1.0e-8);
    }

    #[test]
    fn weighted_half_background_plus_identical_half_field_is_identity() {
        let tmp_dir = make_temp_dir("weighted_half_identity");
        let background_path = tmp_dir.join("H52.pass.dat");
        let field_path = tmp_dir.join("H952.pass.dat");
        let out_path = tmp_dir.join("H52.weighted.pass.dat");
        let values = representative_event_values();
        write_pass(&background_path, &[event_line(2000, 1, values)]);
        write_pass(&field_path, &[event_line(2000, 1, values)]);

        combine_weighted_hillslope_pass_files(
            &[
                weighted_source("background", &background_path, 2220.45),
                weighted_source("field:952", &field_path, 2220.45),
            ],
            &out_path,
            4440.9,
            "../runs/p52.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect("half identity combine should succeed");

        let columns = hillslope_pass_to_columns(&out_path, None, &VersionInfo::new(1, 0))
            .expect("weighted output should parse");
        assert!((columns.runvol[0] - 100.0).abs() < 1.0e-8);
        assert!((columns.tdet[0] - 3.0).abs() < 1.0e-8);
        assert!((columns.gwbfv[0] - 4.0).abs() < 1.0e-8);
    }

    #[test]
    fn weighted_combines_all_event_labels_and_extensive_terms() {
        let tmp_dir = make_temp_dir("weighted_labels");
        let background_path = tmp_dir.join("H53.pass.dat");
        let field_path = tmp_dir.join("H953.pass.dat");
        let out_path = tmp_dir.join("H53.weighted.pass.dat");
        write_pass(
            &background_path,
            &[
                event_line(2000, 1, representative_event_values()),
                subevent_line(2000, 2, [0.001, 5.0, 0.002, 8.0, 2.0, 3.0]),
                noevent_line(2000, 3, [4.0, 6.0]),
            ],
        );
        write_pass(
            &field_path,
            &[
                subevent_line(2000, 1, [0.001, 10.0, 0.002, 12.0, 3.0, 5.0]),
                noevent_line(2000, 2, [7.0, 11.0]),
                noevent_line(2000, 3, [13.0, 17.0]),
            ],
        );

        combine_weighted_hillslope_pass_files(
            &[
                weighted_source("background", &background_path, 2220.45),
                weighted_source("field:953", &field_path, 2220.45),
            ],
            &out_path,
            4440.9,
            "../runs/p53.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect("label combine should succeed");

        let columns = hillslope_pass_to_columns(&out_path, None, &VersionInfo::new(1, 0))
            .expect("weighted output should parse");
        assert_eq!(columns.event, vec!["EVENT", "SUBEVENT", "NO EVENT"]);
        assert!((columns.runvol[0] - 50.0).abs() < 1.0e-8);
        assert!((columns.sbrunv[0] - 7.5).abs() < 1.0e-8);
        assert!((columns.gwbfv[1] - 4.5).abs() < 1.0e-8);
        assert!((columns.gwdsv[2] - 11.5).abs() < 1.0e-8);
    }

    #[test]
    fn weighted_rejects_duplicate_ids_area_mismatch_and_calendar_mismatch() {
        let tmp_dir = make_temp_dir("weighted_invalid_contracts");
        let first_path = tmp_dir.join("H54.pass.dat");
        let second_path = tmp_dir.join("H954.pass.dat");
        let out_path = tmp_dir.join("H54.weighted.pass.dat");
        write_pass(&first_path, &[noevent_line(2000, 1, [0.0, 0.0])]);
        write_pass(&second_path, &[noevent_line(2000, 2, [0.0, 0.0])]);

        let duplicate_error = combine_weighted_hillslope_pass_files(
            &[
                weighted_source("same", &first_path, 2220.45),
                weighted_source("same", &second_path, 2220.45),
            ],
            &out_path,
            4440.9,
            "p54.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect_err("duplicate ids should fail");
        assert!(duplicate_error.display_message().contains("Duplicate"));

        let area_error = combine_weighted_hillslope_pass_files(
            &[weighted_source("first", &first_path, 100.0)],
            &out_path,
            4440.9,
            "p54.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect_err("area mismatch should fail");
        assert!(area_error.display_message().contains("do not close"));

        let calendar_error = combine_weighted_hillslope_pass_files(
            &[
                weighted_source("first", &first_path, 2220.45),
                weighted_source("second", &second_path, 2220.45),
            ],
            &out_path,
            4440.9,
            "p54.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect_err("calendar mismatch should fail");
        assert!(calendar_error
            .display_message()
            .contains("calendar day key"));
    }

    #[test]
    fn weighted_failure_preserves_existing_output_and_removes_temporary_file() {
        let tmp_dir = make_temp_dir("weighted_atomic");
        let source_path = tmp_dir.join("H55.pass.dat");
        let out_path = tmp_dir.join("H55.weighted.pass.dat");
        let mut values = representative_event_values();
        values[4] = 1.0e120;
        write_pass(&source_path, &[event_line(2000, 1, values)]);
        fs::write(&out_path, "existing-output\n").expect("failed to seed output");

        let error = combine_weighted_hillslope_pass_files(
            &[weighted_source("background", &source_path, 4440.9)],
            &out_path,
            4440.9,
            "p55.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect_err("serialization overflow should fail");
        assert!(error.display_message().contains("cannot be represented"));
        assert_eq!(
            fs::read_to_string(&out_path).expect("missing seeded output"),
            "existing-output\n"
        );
        let temp_count = fs::read_dir(&tmp_dir)
            .expect("failed to list temp dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(temp_count, 0);
    }

    #[test]
    fn weighted_rejects_nonfinite_source_values() {
        let tmp_dir = make_temp_dir("weighted_nonfinite");
        let source_path = tmp_dir.join("H56.pass.dat");
        let out_path = tmp_dir.join("H56.weighted.pass.dat");
        let mut values = representative_event_values();
        values[4] = f64::NAN;
        write_pass(&source_path, &[event_line(2000, 1, values)]);

        let error = combine_weighted_hillslope_pass_files(
            &[weighted_source("background", &source_path, 4440.9)],
            &out_path,
            4440.9,
            "p56.cli",
            AG_FIELDS_STRATEGY,
        )
        .expect_err("non-finite input should fail");
        assert!(error.display_message().contains("finite and nonnegative"));
        assert!(!out_path.exists());
    }
}

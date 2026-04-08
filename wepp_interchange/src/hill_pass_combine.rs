use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::errors::InterchangeError;
use crate::hill_pass::{hillslope_pass_to_columns, PassColumns};
use crate::schema::VersionInfo;

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

    write_combined_pass(out_pass, &header_lines, &combined_rows)?;
    Ok(())
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
) -> Result<(), InterchangeError> {
    write_label_and_day(writer, label, year, julian, out_pass)?;
    for value in values {
        write!(writer, "{} ", format_fortran_e11_5(*value, out_pass)?)
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
) -> Result<(), InterchangeError> {
    write_label_and_day(writer, label, year, julian, out_pass)?;
    let first_group_end = values.len().min(10);
    for value in &values[..first_group_end] {
        write!(writer, "{} ", format_fortran_e11_5(*value, out_pass)?)
            .map_err(|err| InterchangeError::io(out_pass, err))?;
    }
    if values.len() > 10 {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
        let second_group_end = values.len().min(15);
        for value in &values[10..second_group_end] {
            write!(writer, "{} ", format_fortran_e11_5(*value, out_pass)?)
                .map_err(|err| InterchangeError::io(out_pass, err))?;
        }
    }
    if values.len() > 15 {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
    }
    let third_group_end = values.len().min(20);
    if values.len() > 15 {
        for value in &values[15..third_group_end] {
            write!(writer, "{} ", format_fortran_e11_5(*value, out_pass)?)
                .map_err(|err| InterchangeError::io(out_pass, err))?;
        }
    }
    writeln!(writer).map_err(|err| InterchangeError::io(out_pass, err))?;

    let mut idx = third_group_end;
    while idx < values.len() {
        write!(writer, "     ").map_err(|err| InterchangeError::io(out_pass, err))?;
        let next_idx = (idx + 5).min(values.len());
        for value in &values[idx..next_idx] {
            write!(writer, "{} ", format_fortran_e11_5(*value, out_pass)?)
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
}

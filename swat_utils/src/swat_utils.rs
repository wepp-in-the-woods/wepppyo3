use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::load_cli_calendar;
use crate::errors::InterchangeError;
use crate::hill_pass;
use crate::schema::VersionInfo;

const RECALL_DAY_HEADER: &str =
    "jday mo day_mo iyr ob_typ ob_name flo sed orgn sedp no3 solp chla nh3 no2 cbod dox san sil cla sag lag grv temp";
const RECALL_REC_HEADER: &str = "NUMB NAME TYP FILENAME";
const RECALL_CON_HEADER: &str =
    "      NUMB    NAME    GIS_ID    AREA_HA      LAT     LONG       ELEV    RECALL     WST   CONSTIT  OVERFLOW  RULESET OUT_TOT OBTYP_OUT1 HTYPNO_OUT1  HTYP_OUT1  FRAC_OUT1";

#[derive(Debug, Clone)]
pub struct RecallRow {
    pub year: i16,
    pub julian: i16,
    pub month: i16,
    pub day_mo: i16,
    pub flo: f64,
    pub sed: f64,
    pub cla: f64,
    pub sil: f64,
    pub sag: f64,
    pub lag: f64,
    pub san: f64,
    pub grv: f64,
}

#[derive(Debug, Clone)]
pub struct RecallManifestEntry {
    pub wepp_id: i32,
    pub pass_file: PathBuf,
    pub recall_file: PathBuf,
    pub days_written: usize,
    pub start_year: i16,
    pub end_year: i16,
    pub status: String,
    pub skip_reason: Option<String>,
}

impl RecallManifestEntry {
    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("pass_file", self.pass_file.display().to_string())
            .unwrap();
        dict.set_item("recall_file", self.recall_file.display().to_string())
            .unwrap();
        dict.set_item("days_written", self.days_written).unwrap();
        dict.set_item("start_year", self.start_year).unwrap();
        dict.set_item("end_year", self.end_year).unwrap();
        dict.set_item("status", self.status).unwrap();
        dict.set_item("skip_reason", self.skip_reason).unwrap();
        dict.into_py(py)
    }
}

#[derive(Debug, Clone, Default)]
struct DailyAggregate {
    flo: f64,
    cla: f64,
    sil: f64,
    sag: f64,
    lag: f64,
    san: f64,
}

impl DailyAggregate {
    fn add(&mut self, flo: f64, cla: f64, sil: f64, sag: f64, lag: f64, san: f64) {
        self.flo += flo;
        self.cla += cla;
        self.sil += sil;
        self.sag += sag;
        self.lag += lag;
        self.san += san;
    }

    fn sed(&self) -> f64 {
        self.cla + self.sil + self.sag + self.lag + self.san
    }
}

#[derive(Debug, Clone)]
struct PassTask {
    wepp_id: i32,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct RecallConfig {
    swat_txtinout_dir: PathBuf,
    swat_recall_dir: PathBuf,
    recall_subdir: String,
    recall_wst: String,
    recall_object_type: String,
    cli_calendar_path: Option<PathBuf>,
    version: VersionInfo,
    filename_template: String,
    include_subsurface: bool,
    include_tile: bool,
    include_baseflow: bool,
}

pub fn wepp_hillslope_pass_to_swat_recall(
    wepp_output_dir: &Path,
    swat_txtinout_dir: &Path,
    recall_subdir: &str,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    filename_template: &str,
    include_subsurface: bool,
    include_tile: bool,
    include_baseflow: bool,
    recall_connections: Option<&[(i32, i32)]>,
    recall_wst: &str,
    recall_object_type: &str,
    ncpu: Option<usize>,
    write_manifest: bool,
) -> Result<Option<Vec<RecallManifestEntry>>, InterchangeError> {
    let mut tasks: Vec<PassTask> = Vec::new();
    for entry in
        fs::read_dir(wepp_output_dir).map_err(|err| InterchangeError::io(wepp_output_dir, err))?
    {
        let entry = entry.map_err(|err| InterchangeError::io(wepp_output_dir, err))?;
        let file_type = entry
            .file_type()
            .map_err(|err| InterchangeError::io(entry.path(), err))?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(wepp_id) = parse_pass_filename(&name) {
            tasks.push(PassTask {
                wepp_id,
                path: entry.path(),
            });
        }
    }

    tasks.sort_by_key(|task| task.wepp_id);

    if tasks.is_empty() {
        return if write_manifest {
            Ok(Some(Vec::new()))
        } else {
            Ok(None)
        };
    }

    let recall_dir = swat_txtinout_dir.join(recall_subdir);
    let config = RecallConfig {
        swat_txtinout_dir: swat_txtinout_dir.to_path_buf(),
        swat_recall_dir: recall_dir,
        recall_subdir: recall_subdir.to_string(),
        recall_wst: recall_wst.to_string(),
        recall_object_type: recall_object_type.to_string(),
        cli_calendar_path: cli_calendar_path.map(PathBuf::from),
        version: version.clone(),
        filename_template: filename_template.to_string(),
        include_subsurface,
        include_tile,
        include_baseflow,
    };

    fs::create_dir_all(&config.swat_txtinout_dir)
        .map_err(|err| InterchangeError::io(&config.swat_txtinout_dir, err))?;
    fs::create_dir_all(&config.swat_recall_dir)
        .map_err(|err| InterchangeError::io(&config.swat_recall_dir, err))?;

    let worker_count = match ncpu {
        Some(value) if value > 1 => value,
        Some(_) => 1,
        None => thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
    };

    let results = if worker_count <= 1 || tasks.len() <= 1 {
        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            results.push(process_pass_task(&task, &config)?);
        }
        results
    } else {
        run_parallel(tasks, config.clone(), worker_count)?
    };

    write_recall_master_files(&results, &config, recall_connections)?;

    if write_manifest {
        let mut entries = results;
        entries.sort_by_key(|entry| entry.wepp_id);
        Ok(Some(entries))
    } else {
        Ok(None)
    }
}

fn run_parallel(
    tasks: Vec<PassTask>,
    config: RecallConfig,
    worker_count: usize,
) -> Result<Vec<RecallManifestEntry>, InterchangeError> {
    let task_count = tasks.len();
    let (task_tx, task_rx) = std::sync::mpsc::channel::<PassTask>();
    let (result_tx, result_rx) =
        std::sync::mpsc::channel::<Result<RecallManifestEntry, InterchangeError>>();
    let shared_rx = Arc::new(Mutex::new(task_rx));
    let shared_config = Arc::new(config);

    for _ in 0..worker_count {
        let task_rx = Arc::clone(&shared_rx);
        let result_tx = result_tx.clone();
        let config = Arc::clone(&shared_config);
        thread::spawn(move || loop {
            let task = {
                let rx = task_rx.lock().unwrap();
                rx.recv()
            };
            let task = match task {
                Ok(task) => task,
                Err(_) => break,
            };
            let result = process_pass_task(&task, &config);
            if result_tx.send(result).is_err() {
                break;
            }
        });
    }

    drop(result_tx);
    for task in tasks {
        if task_tx.send(task).is_err() {
            break;
        }
    }
    drop(task_tx);

    let mut results = Vec::with_capacity(task_count);
    for _ in 0..task_count {
        match result_rx.recv() {
            Ok(Ok(entry)) => results.push(entry),
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                return Err(InterchangeError::parse(
                    &shared_config.swat_recall_dir,
                    None,
                    "Failed to collect SWAT recall results",
                    None,
                ))
            }
        }
    }

    Ok(results)
}

fn process_pass_task(
    task: &PassTask,
    config: &RecallConfig,
) -> Result<RecallManifestEntry, InterchangeError> {
    let recall_name = format_recall_filename(&config.filename_template, task.wepp_id);
    let recall_path = config.swat_recall_dir.join(recall_name);

    let columns = hill_pass::hillslope_pass_to_columns(
        &task.path,
        config.cli_calendar_path.as_deref(),
        &config.version,
    )?;

    if columns.len() == 0 {
        eprintln!(
            "SWAT recall: skipping {} (no PASS rows)",
            task.path.display()
        );
        return Ok(RecallManifestEntry {
            wepp_id: task.wepp_id,
            pass_file: task.path.clone(),
            recall_file: recall_path,
            days_written: 0,
            start_year: 0,
            end_year: 0,
            status: "skipped".to_string(),
            skip_reason: Some("no_rows".to_string()),
        });
    }

    let lookup = match config.cli_calendar_path.as_deref() {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };

    let mut aggregated = HashMap::new();
    let mut min_year: i32 = i32::MAX;
    let mut max_year: i32 = i32::MIN;
    let mut min_julian: i32 = i32::MAX;
    let mut max_julian: i32 = i32::MIN;
    let mut min_sim_index: i32 = i32::MAX;
    let mut max_sim_index: i32 = i32::MIN;
    let mut min_row: Option<(i32, i32, i32)> = None;

    for idx in 0..columns.len() {
        let year = columns.year()[idx] as i32;
        let julian = columns.julian()[idx] as i32;
        let sim_day_index = columns.sim_day_index()[idx];
        let runvol = columns.runvol()[idx];
        let sbrunv = columns.sbrunv()[idx];
        let drrunv = columns.drrunv()[idx];
        let gwbfv = columns.gwbfv()[idx];

        let flo = runvol
            + if config.include_subsurface {
                sbrunv
            } else {
                0.0
            }
            + if config.include_tile { drrunv } else { 0.0 }
            + if config.include_baseflow { gwbfv } else { 0.0 };
        let cla = (columns.sedcon_1()[idx] * runvol) / 1000.0;
        let sil = (columns.sedcon_2()[idx] * runvol) / 1000.0;
        let sag = (columns.sedcon_3()[idx] * runvol) / 1000.0;
        let lag = (columns.sedcon_4()[idx] * runvol) / 1000.0;
        let san = (columns.sedcon_5()[idx] * runvol) / 1000.0;

        let entry = if lookup.is_some() {
            aggregated
                .entry(Key::Sim(sim_day_index))
                .or_insert_with(DailyAggregate::default)
        } else {
            aggregated
                .entry(Key::Date(year, julian))
                .or_insert_with(DailyAggregate::default)
        };
        entry.add(flo, cla, sil, sag, lag, san);

        if lookup.is_some() {
            if sim_day_index < min_sim_index {
                min_sim_index = sim_day_index;
                min_row = Some((year, julian, sim_day_index));
            }
            if sim_day_index > max_sim_index {
                max_sim_index = sim_day_index;
            }
        } else {
            if year < min_year || (year == min_year && julian < min_julian) {
                min_year = year;
                min_julian = julian;
            }
            if year > max_year || (year == max_year && julian > max_julian) {
                max_year = year;
                max_julian = julian;
            }
        }
    }

    if aggregated.is_empty() {
        eprintln!(
            "SWAT recall: skipping {} (no usable PASS rows)",
            task.path.display()
        );
        return Ok(RecallManifestEntry {
            wepp_id: task.wepp_id,
            pass_file: task.path.clone(),
            recall_file: recall_path,
            days_written: 0,
            start_year: 0,
            end_year: 0,
            status: "skipped".to_string(),
            skip_reason: Some("no_rows".to_string()),
        });
    }

    let rows = if lookup.is_some() {
        build_rows_from_sim_index(
            &aggregated,
            min_row,
            min_sim_index,
            max_sim_index,
            lookup.as_ref(),
        )
    } else {
        build_rows_from_dates(
            &aggregated,
            (min_year, min_julian),
            (max_year, max_julian),
            lookup.as_ref(),
        )
    };

    let rows = rows?;
    if rows.is_empty() {
        eprintln!(
            "SWAT recall: skipping {} (empty recall series)",
            task.path.display()
        );
        return Ok(RecallManifestEntry {
            wepp_id: task.wepp_id,
            pass_file: task.path.clone(),
            recall_file: recall_path,
            days_written: 0,
            start_year: 0,
            end_year: 0,
            status: "skipped".to_string(),
            skip_reason: Some("no_rows".to_string()),
        });
    }

    if let Some(parent) = recall_path.parent() {
        fs::create_dir_all(parent).map_err(|err| InterchangeError::io(parent, err))?;
    }
    let nbyr = {
        let mut min = i16::MAX;
        let mut max = i16::MIN;
        for row in &rows {
            if row.year < min {
                min = row.year;
            }
            if row.year > max {
                max = row.year;
            }
        }
        (max as i32 - min as i32 + 1).max(1)
    };

    let mut writer = BufWriter::new(
        fs::File::create(&recall_path).map_err(|err| InterchangeError::io(&recall_path, err))?,
    );
    writeln!(writer, "WEPP hillslope {} recall (daily)", task.wepp_id)
        .map_err(|err| InterchangeError::io(&recall_path, err))?;
    writeln!(writer, "{nbyr}").map_err(|err| InterchangeError::io(&recall_path, err))?;
    writeln!(writer, "{RECALL_DAY_HEADER}")
        .map_err(|err| InterchangeError::io(&recall_path, err))?;

    let days_written = rows.len();
    let mut start_year = rows.first().map(|row| row.year).unwrap_or(0);
    let mut end_year = rows.last().map(|row| row.year).unwrap_or(0);
    let ob_name = format_recall_label(&config.filename_template, task.wepp_id);
    let ob_typ = config.recall_object_type.as_str();
    for row in rows {
        start_year = start_year.min(row.year);
        end_year = end_year.max(row.year);
        write_recall_row(&mut writer, &row, ob_typ, &ob_name)
            .map_err(|err| InterchangeError::io(&recall_path, err))?;
    }

    Ok(RecallManifestEntry {
        wepp_id: task.wepp_id,
        pass_file: task.path.clone(),
        recall_file: recall_path,
        days_written,
        start_year,
        end_year,
        status: "written".to_string(),
        skip_reason: None,
    })
}

fn write_recall_master_files(
    entries: &[RecallManifestEntry],
    config: &RecallConfig,
    recall_connections: Option<&[(i32, i32)]>,
) -> Result<(), InterchangeError> {
    let mut written: Vec<&RecallManifestEntry> = entries
        .iter()
        .filter(|entry| entry.status == "written")
        .collect();
    if written.is_empty() {
        return Ok(());
    }
    written.sort_by_key(|entry| entry.wepp_id);

    write_recall_rec(&written, config)?;

    if let Some(connections) = recall_connections {
        let mut connection_map = HashMap::new();
        for (wepp_id, chn_enum) in connections {
            connection_map.insert(*wepp_id, *chn_enum);
        }

        for entry in &written {
            if !connection_map.contains_key(&entry.wepp_id) {
                return Err(InterchangeError::parse(
                    &config.swat_txtinout_dir,
                    None,
                    format!("Missing recall connection for wepp_id {}", entry.wepp_id),
                    None,
                ));
            }
        }

        write_recall_con(&written, config, &connection_map)?;
    }
    Ok(())
}

fn write_recall_rec(
    entries: &[&RecallManifestEntry],
    config: &RecallConfig,
) -> Result<(), InterchangeError> {
    let recall_rec_path = config.swat_txtinout_dir.join("recall.rec");
    let mut writer = BufWriter::new(
        fs::File::create(&recall_rec_path)
            .map_err(|err| InterchangeError::io(&recall_rec_path, err))?,
    );
    writeln!(writer, "recall.rec").map_err(|err| InterchangeError::io(&recall_rec_path, err))?;
    writeln!(writer, "{RECALL_REC_HEADER}")
        .map_err(|err| InterchangeError::io(&recall_rec_path, err))?;

    for (idx, entry) in entries.iter().enumerate() {
        let numb = (idx + 1) as i32;
        let name = format_recall_label(&config.filename_template, entry.wepp_id);
        let filename = format_recall_filename(&config.filename_template, entry.wepp_id);
        let recall_path = filename;
        writeln!(writer, "{:>6} {:>12} {:>4} {}", numb, name, 1, recall_path)
            .map_err(|err| InterchangeError::io(&recall_rec_path, err))?;
    }
    Ok(())
}

fn write_recall_con(
    entries: &[&RecallManifestEntry],
    config: &RecallConfig,
    recall_connections: &HashMap<i32, i32>,
) -> Result<(), InterchangeError> {
    let recall_con_path = config.swat_txtinout_dir.join("recall.con");
    let mut writer = BufWriter::new(
        fs::File::create(&recall_con_path)
            .map_err(|err| InterchangeError::io(&recall_con_path, err))?,
    );
    writeln!(writer, "recall.con (2-stage)")
        .map_err(|err| InterchangeError::io(&recall_con_path, err))?;
    writeln!(writer, "{RECALL_CON_HEADER}")
        .map_err(|err| InterchangeError::io(&recall_con_path, err))?;

    for (idx, entry) in entries.iter().enumerate() {
        let numb = (idx + 1) as i32;
        let name = format_recall_label(&config.filename_template, entry.wepp_id);
        let chn_enum = *recall_connections.get(&entry.wepp_id).ok_or_else(|| {
            InterchangeError::parse(
                &config.swat_txtinout_dir,
                None,
                format!("Missing recall connection for wepp_id {}", entry.wepp_id),
                None,
            )
        })?;
        writeln!(
            writer,
            "{:>10} {:>12} {:>8} {:>10.3} {:>8.3} {:>8.3} {:>9.3} {:>8} {:>6} {:>9} {:>9} {:>8} {:>7} {:>9} {:>11} {:>9} {:>9.4}",
            numb,
            name,
            entry.wepp_id,
            0.0,
            0.0,
            0.0,
            0.0,
            numb,
            config.recall_wst.as_str(),
            0,
            0,
            0,
            1,
            config.recall_object_type.as_str(),
            chn_enum,
            "tot",
            1.0
        )
        .map_err(|err| InterchangeError::io(&recall_con_path, err))?;
    }
    Ok(())
}

fn build_rows_from_dates(
    aggregated: &HashMap<Key, DailyAggregate>,
    min_date: (i32, i32),
    max_date: (i32, i32),
    lookup: Option<&crate::calendar::CalendarLookup>,
) -> Result<Vec<RecallRow>, InterchangeError> {
    let mut rows: Vec<RecallRow> = Vec::new();
    let mut year = min_date.0;
    let mut julian = min_date.1;
    loop {
        let agg = aggregated
            .get(&Key::Date(year, julian))
            .cloned()
            .unwrap_or_default();
        let sed = agg.sed();
        let (month, day_mo) = crate::calendar::julian_to_calendar(year, julian, lookup);
        rows.push(RecallRow {
            year: year as i16,
            julian: julian as i16,
            month: month as i16,
            day_mo: day_mo as i16,
            flo: clean_float(agg.flo),
            sed: clean_float(sed),
            cla: clean_float(agg.cla),
            sil: clean_float(agg.sil),
            sag: clean_float(agg.sag),
            lag: clean_float(agg.lag),
            san: clean_float(agg.san),
            grv: 0.0,
        });
        if (year, julian) == max_date {
            break;
        }
        let (next_year, next_julian) = next_day(year, julian, None);
        year = next_year;
        julian = next_julian;
    }
    Ok(rows)
}

fn build_rows_from_sim_index(
    aggregated: &HashMap<Key, DailyAggregate>,
    min_row: Option<(i32, i32, i32)>,
    min_index: i32,
    max_index: i32,
    lookup: Option<&crate::calendar::CalendarLookup>,
) -> Result<Vec<RecallRow>, InterchangeError> {
    let (min_year, min_julian, min_sim_index) = min_row.ok_or_else(|| {
        InterchangeError::parse(
            "<swat_utils>",
            None,
            "Unable to determine simulation start for recall export",
            None,
        )
    })?;
    let start_year = infer_start_year(min_year, min_julian, min_sim_index, lookup);

    let mut rows: Vec<RecallRow> = Vec::new();
    for sim_index in min_index..=max_index {
        let (year, julian) = sim_day_index_to_date(sim_index, start_year, lookup);
        let (month, day_mo) = crate::calendar::julian_to_calendar(year, julian, lookup);
        let agg = aggregated
            .get(&Key::Sim(sim_index))
            .cloned()
            .unwrap_or_default();
        let sed = agg.sed();
        rows.push(RecallRow {
            year: year as i16,
            julian: julian as i16,
            month: month as i16,
            day_mo: day_mo as i16,
            flo: clean_float(agg.flo),
            sed: clean_float(sed),
            cla: clean_float(agg.cla),
            sil: clean_float(agg.sil),
            sag: clean_float(agg.sag),
            lag: clean_float(agg.lag),
            san: clean_float(agg.san),
            grv: 0.0,
        });
    }

    Ok(rows)
}

fn sim_day_index_to_date(
    sim_day_index: i32,
    start_year: i32,
    lookup: Option<&crate::calendar::CalendarLookup>,
) -> (i32, i32) {
    let mut year = start_year;
    let mut remaining = sim_day_index;
    loop {
        let year_len = year_length(year, lookup);
        if remaining <= year_len {
            return (year, remaining);
        }
        remaining -= year_len;
        year += 1;
    }
}

fn infer_start_year(
    year: i32,
    julian: i32,
    sim_day_index: i32,
    lookup: Option<&crate::calendar::CalendarLookup>,
) -> i32 {
    let mut remaining = sim_day_index - julian;
    if remaining <= 0 {
        return year;
    }
    let mut start_year = year;
    while remaining > 0 {
        start_year -= 1;
        let year_len = year_length(start_year, lookup);
        remaining -= year_len;
        if remaining == 0 {
            return start_year;
        }
        if remaining < 0 {
            break;
        }
    }
    year
}

fn year_length(year: i32, lookup: Option<&crate::calendar::CalendarLookup>) -> i32 {
    if let Some(lookup) = lookup {
        if let Some(len) = lookup.year_len(year) {
            return len as i32;
        }
    }
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

fn next_day(
    year: i32,
    julian: i32,
    lookup: Option<&crate::calendar::CalendarLookup>,
) -> (i32, i32) {
    let year_len = year_length(year, lookup);
    if julian < year_len {
        (year, julian + 1)
    } else {
        (year + 1, 1)
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn clean_float(value: f64) -> f64 {
    if value.abs() < 1e-12 {
        0.0
    } else {
        value
    }
}

fn write_recall_row(
    writer: &mut BufWriter<fs::File>,
    row: &RecallRow,
    ob_typ: &str,
    ob_name: &str,
) -> std::io::Result<()> {
    writeln!(
        writer,
        "{} {} {} {} {} {} {:.6} {:.6} 0 0 0 0 0 0 0 0 0 {:.6} {:.6} {:.6} {:.6} {:.6} {:.6} 0",
        row.julian,
        row.month,
        row.day_mo,
        row.year,
        ob_typ,
        ob_name,
        row.flo,
        row.sed,
        row.san,
        row.sil,
        row.cla,
        row.sag,
        row.lag,
        row.grv
    )
}

fn format_recall_filename(template: &str, wepp_id: i32) -> String {
    if template.contains("{wepp_id:05d}") {
        template.replace("{wepp_id:05d}", &format!("{:05}", wepp_id))
    } else if template.contains("{wepp_id}") {
        template.replace("{wepp_id}", &wepp_id.to_string())
    } else {
        template.to_string()
    }
}

fn format_recall_label(template: &str, wepp_id: i32) -> String {
    let filename = format_recall_filename(template, wepp_id);
    Path::new(&filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&filename)
        .to_string()
}

fn parse_pass_filename(name: &str) -> Option<i32> {
    if !name.starts_with('H') || !name.ends_with(".pass.dat") {
        return None;
    }
    let suffix_len = ".pass.dat".len();
    if name.len() <= 1 + suffix_len {
        return None;
    }
    let middle = &name[1..name.len() - suffix_len];
    if middle.is_empty() || !middle.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    middle.parse::<i32>().ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    Date(i32, i32),
    Sim(i32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_support::Chunk;
    use crate::parquet::write_single_chunk;
    use arrow_schema::{DataType, Field, Schema};
    use std::fs::File;
    use std::io::Write;

    fn make_temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let uniq = format!("swat_utils_test_{}_{}", name, std::process::id());
        path.push(uniq);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_pass_file(path: &Path, start_year: i32, data_lines: &[String]) {
        let mut file = File::create(path).expect("create pass file");
        writeln!(file, "p1.cli").unwrap();
        writeln!(file, "   1      {start_year}").unwrap();
        writeln!(file, ".10000E+00").unwrap();
        writeln!(
            file,
            "  5    0.20000E-05 0.10000E-04 0.30000E-04 0.35000E-03 0.20000E-03"
        )
        .unwrap();
        writeln!(file, "    0.00     0.00     0.00     0.00").unwrap();
        for line in data_lines {
            writeln!(file, "{line}").unwrap();
        }
    }

    fn event_line(
        year: i32,
        julian: i32,
        runvol: f64,
        sbrunv: f64,
        drrunv: f64,
        sedcon: [f64; 5],
    ) -> String {
        let values = [
            1.0, // dur
            1.0, // tcs
            1.0, // oalpha
            0.0, // runoff
            runvol, 0.0, // sbrunf
            sbrunv, 0.0, // drainq
            drrunv, 0.0, // peakro
            0.0, // tdet
            0.0, // tdep
            sedcon[0], sedcon[1], sedcon[2], sedcon[3], sedcon[4], 0.0, // clot
            0.0, // slot
            0.0, // saot
            0.0, // laot
            0.0, // sdot
            0.0, // gwbfv
            0.0, // gwdsv
        ];
        let mut line = format!("EVENT   {year:4} {julian:4}");
        for value in values {
            line.push_str(&format!(" {value:.6}"));
        }
        line
    }

    fn event_line_with_gw(
        year: i32,
        julian: i32,
        runvol: f64,
        sbrunv: f64,
        drrunv: f64,
        sedcon: [f64; 5],
        gwbfv: f64,
        gwdsv: f64,
    ) -> String {
        let values = [
            1.0, // dur
            1.0, // tcs
            1.0, // oalpha
            0.0, // runoff
            runvol, 0.0, // sbrunf
            sbrunv, 0.0, // drainq
            drrunv, 0.0, // peakro
            0.0, // tdet
            0.0, // tdep
            sedcon[0], sedcon[1], sedcon[2], sedcon[3], sedcon[4], 0.0, // clot
            0.0, // slot
            0.0, // saot
            0.0, // laot
            0.0, // sdot
            gwbfv, gwdsv,
        ];
        let mut line = format!("EVENT   {year:4} {julian:4}");
        for value in values {
            line.push_str(&format!(" {value:.6}"));
        }
        line
    }

    fn subevent_line(year: i32, julian: i32, sbrunv: f64, drrunv: f64) -> String {
        let values = [0.0, sbrunv, 0.0, drrunv, 0.0, 0.0];
        let mut line = format!("SUBEVENT {year:4} {julian:4}");
        for value in values {
            line.push_str(&format!(" {value:.6}"));
        }
        line
    }

    fn read_recall_rows(path: &Path) -> Vec<Vec<String>> {
        let content = fs::read_to_string(path).expect("read recall file");
        content
            .lines()
            .skip(3)
            .map(|line| line.split_whitespace().map(|s| s.to_string()).collect())
            .collect()
    }

    fn read_recall_header(path: &Path) -> Vec<String> {
        let content = fs::read_to_string(path).expect("read recall file");
        content.lines().take(3).map(|s| s.to_string()).collect()
    }

    fn write_calendar_parquet(path: &Path, rows: &[(i32, i32, i32)]) {
        let years = arrow_array::Int32Array::from(rows.iter().map(|row| row.0).collect::<Vec<_>>());
        let months =
            arrow_array::Int32Array::from(rows.iter().map(|row| row.1).collect::<Vec<_>>());
        let days = arrow_array::Int32Array::from(rows.iter().map(|row| row.2).collect::<Vec<_>>());

        let schema = Schema::new(vec![
            Field::new("year", DataType::Int32, false),
            Field::new("month", DataType::Int32, false),
            Field::new("day_of_month", DataType::Int32, false),
        ]);
        let chunk = Chunk::new(vec![
            Box::new(years) as Box<dyn arrow_array::Array>,
            Box::new(months) as Box<dyn arrow_array::Array>,
            Box::new(days) as Box<dyn arrow_array::Array>,
        ]);
        write_single_chunk(path, schema, chunk).expect("write calendar parquet");
    }

    #[test]
    fn swat_recall_basic_event() {
        let base = make_temp_dir("basic_event");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H1.pass.dat");
        let line = event_line(2000, 1, 10.0, 2.0, 3.0, [1.0, 2.0, 3.0, 4.0, 5.0]);
        write_pass_file(&pass_path, 2000, &[line]);

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0].status, "written");

        let recall_path = swat_recall.join("hill_00001.rec");
        let rows = read_recall_rows(&recall_path);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        let flo: f64 = row[6].parse().unwrap();
        let sed: f64 = row[7].parse().unwrap();
        let san: f64 = row[17].parse().unwrap();
        let sil: f64 = row[18].parse().unwrap();
        let cla: f64 = row[19].parse().unwrap();
        let sag: f64 = row[20].parse().unwrap();
        let lag: f64 = row[21].parse().unwrap();
        assert!((flo - 15.0).abs() < 1e-6);
        assert!((sed - 0.15).abs() < 1e-6);
        assert!((cla - 0.01).abs() < 1e-6);
        assert!((sil - 0.02).abs() < 1e-6);
        assert!((sag - 0.03).abs() < 1e-6);
        assert!((lag - 0.04).abs() < 1e-6);
        assert!((san - 0.05).abs() < 1e-6);
    }

    #[test]
    fn swat_recall_missing_days_fill() {
        let base = make_temp_dir("missing_days");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H2.pass.dat");
        let line1 = event_line(2001, 1, 1.0, 0.0, 0.0, [1.0, 0.0, 0.0, 0.0, 0.0]);
        let line3 = event_line(2001, 3, 2.0, 0.0, 0.0, [1.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 2001, &[line1, line3]);

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        assert_eq!(manifest[0].days_written, 3);

        let recall_path = swat_recall.join("hill_00002.rec");
        let rows = read_recall_rows(&recall_path);
        assert_eq!(rows.len(), 3);
        let middle = &rows[1];
        let flo: f64 = middle[6].parse().unwrap();
        let sed: f64 = middle[7].parse().unwrap();
        assert_eq!(flo, 0.0);
        assert_eq!(sed, 0.0);
    }

    #[test]
    fn swat_recall_duplicate_day_sums_mass() {
        let base = make_temp_dir("duplicate_day");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H3.pass.dat");
        let line1 = event_line(2002, 5, 10.0, 0.0, 0.0, [1.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = event_line(2002, 5, 20.0, 0.0, 0.0, [2.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 2002, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            false,
            false,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00003.rec");
        let rows = read_recall_rows(&recall_path);
        assert_eq!(rows.len(), 1);
        let cla: f64 = rows[0][19].parse().unwrap();
        let sed: f64 = rows[0][7].parse().unwrap();
        assert!((cla - 0.05).abs() < 1e-6);
        assert!((sed - 0.05).abs() < 1e-6);
    }

    #[test]
    fn swat_recall_subevent_flow_flags() {
        let base = make_temp_dir("subevent_flow");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H4.pass.dat");
        let line = subevent_line(2003, 2, 2.0, 3.0);
        write_pass_file(&pass_path, 2003, &[line]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            false,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00004.rec");
        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        assert!((flo - 2.0).abs() < 1e-6);
    }

    #[test]
    fn swat_recall_empty_pass_skips() {
        let base = make_temp_dir("empty_pass");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H5.pass.dat");
        write_pass_file(&pass_path, 2004, &[]);

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0].status, "skipped");
        assert_eq!(manifest[0].skip_reason.as_deref(), Some("no_rows"));
        assert!(!swat_recall.join("hill_00005.rec").exists());
    }

    #[test]
    fn swat_recall_header_and_nbyr() {
        let base = make_temp_dir("header_nbyr");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H6.pass.dat");
        let line1 = event_line(2000, 365, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = event_line(2001, 1, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 2000, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00006.rec");
        let header = read_recall_header(&recall_path);
        assert_eq!(header.len(), 3);
        assert!(header[0].contains("WEPP hillslope 6 recall (daily)"));
        assert_eq!(header[1].trim(), "2");
        assert_eq!(header[2].trim(), RECALL_DAY_HEADER);
    }

    #[test]
    fn swat_recall_leap_year_fill() {
        let base = make_temp_dir("leap_year");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H7.pass.dat");
        let line1 = event_line(2000, 365, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = event_line(2001, 1, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 2000, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00007.rec");
        let rows = read_recall_rows(&recall_path);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1][0], "366");
        assert_eq!(rows[1][3], "2000");
    }

    #[test]
    fn swat_recall_calendar_lookup_path() {
        let base = make_temp_dir("calendar_lookup");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let calendar_path = base.join("cli_calendar.parquet");
        let rows = vec![(1, 1, 1), (1, 1, 2), (1, 1, 3), (2, 1, 1), (2, 1, 2)];
        write_calendar_parquet(&calendar_path, &rows);

        let pass_path = wepp_output.join("H8.pass.dat");
        let line1 = event_line(1, 1, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = event_line(2, 1, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 1, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            Some(&calendar_path),
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00008.rec");
        let rows = read_recall_rows(&recall_path);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0][0], "1");
        assert_eq!(rows[0][3], "1");
        assert_eq!(rows[2][0], "3");
        assert_eq!(rows[2][3], "1");
        assert_eq!(rows[3][0], "1");
        assert_eq!(rows[3][3], "2");
    }

    #[test]
    fn swat_recall_include_flags_matrix() {
        let base = make_temp_dir("include_flags");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H9.pass.dat");
        let line = event_line(2005, 1, 10.0, 2.0, 3.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 2005, &[line]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            false,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        let recall_path = swat_recall.join("hill_00009.rec");
        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        assert!((flo - 13.0).abs() < 1e-6);

        fs::remove_file(&recall_path).unwrap();
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            false,
            false,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        assert!((flo - 10.0).abs() < 1e-6);
    }

    #[test]
    fn swat_recall_event_and_subevent_same_day() {
        let base = make_temp_dir("event_subevent_mix");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H10.pass.dat");
        let line1 = event_line(2006, 10, 5.0, 1.0, 1.0, [1.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = subevent_line(2006, 10, 2.0, 3.0);
        write_pass_file(&pass_path, 2006, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00010.rec");
        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        let sed: f64 = rows[0][7].parse().unwrap();
        assert!((flo - 12.0).abs() < 1e-6);
        assert!((sed - 0.005).abs() < 1e-6);
    }

    #[test]
    fn swat_recall_manifest_fields_multi_year() {
        let base = make_temp_dir("manifest_fields");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H11.pass.dat");
        let line1 = event_line(1999, 365, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        let line2 = event_line(2000, 2, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
        write_pass_file(&pass_path, 1999, &[line1, line2]);

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        assert_eq!(manifest[0].start_year, 1999);
        assert_eq!(manifest[0].end_year, 2000);
        assert_eq!(manifest[0].days_written, 3);
    }

    #[test]
    fn swat_recall_filename_helpers() {
        assert_eq!(parse_pass_filename("H123.pass.dat"), Some(123));
        assert_eq!(parse_pass_filename("H12a.pass.dat"), None);
        assert_eq!(parse_pass_filename("A12.pass.dat"), None);
        assert_eq!(parse_pass_filename("H12.pass"), None);

        assert_eq!(
            format_recall_filename("hill_{wepp_id:05d}.rec", 7),
            "hill_00007.rec"
        );
        assert_eq!(
            format_recall_filename("hill_{wepp_id}.rec", 7),
            "hill_7.rec"
        );
        assert_eq!(format_recall_filename("custom.rec", 7), "custom.rec");
    }

    #[test]
    fn swat_recall_error_and_empty_dir() {
        let base = make_temp_dir("error_and_empty");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("empty dir");
        assert_eq!(manifest.unwrap().len(), 0);

        let none = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            false,
        )
        .expect("empty dir");
        assert!(none.is_none());

        let bad_path = wepp_output.join("H12.pass.dat");
        let mut file = File::create(&bad_path).unwrap();
        writeln!(file, "bad header").unwrap();
        let err = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect_err("should fail");
        let msg = err.display_message();
        assert!(msg.contains("PASS file missing simulation metadata header"));
    }

    #[test]
    fn swat_recall_parallel_path() {
        let base = make_temp_dir("parallel_path");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        for idx in 1..=4 {
            let pass_path = wepp_output.join(format!("H{idx}.pass.dat"));
            let line = event_line(2007, idx, 1.0, 0.0, 0.0, [0.0, 0.0, 0.0, 0.0, 0.0]);
            write_pass_file(&pass_path, 2007, &[line]);
        }

        let version = VersionInfo::new(3, 0);
        let manifest = wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            true,
            true,
            true,
            None,
            "wea1",
            "sdc",
            Some(2),
            true,
        )
        .expect("run recall")
        .expect("manifest");
        assert_eq!(manifest.len(), 4);
        assert_eq!(manifest[0].wepp_id, 1);
        assert_eq!(manifest[3].wepp_id, 4);
    }

    #[test]
    fn swat_recall_include_baseflow_flag() {
        let base = make_temp_dir("include_baseflow");
        let wepp_output = base.join("wepp_output");
        let swat_txtinout = base.join("swat_txtinout");
        let swat_recall = swat_txtinout.join("recall");
        fs::create_dir_all(&wepp_output).unwrap();

        let pass_path = wepp_output.join("H20.pass.dat");
        let line = event_line_with_gw(
            2008,
            1,
            0.0,
            0.0,
            0.0,
            [0.0, 0.0, 0.0, 0.0, 0.0],
            7.25,
            99.0,
        );
        write_pass_file(&pass_path, 2008, &[line]);

        let version = VersionInfo::new(3, 0);
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            false,
            false,
            true,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let recall_path = swat_recall.join("hill_00020.rec");
        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        assert!((flo - 7.25).abs() < 1e-6);

        fs::remove_file(&recall_path).unwrap();
        wepp_hillslope_pass_to_swat_recall(
            &wepp_output,
            &swat_txtinout,
            "recall",
            None,
            &version,
            "hill_{wepp_id:05d}.rec",
            false,
            false,
            false,
            None,
            "wea1",
            "sdc",
            Some(1),
            true,
        )
        .expect("run recall")
        .expect("manifest");

        let rows = read_recall_rows(&recall_path);
        let flo: f64 = rows[0][6].parse().unwrap();
        assert_eq!(flo, 0.0);
    }
}

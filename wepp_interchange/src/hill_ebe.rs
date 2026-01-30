use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::{compute_sim_day_index, determine_wateryear, load_cli_calendar, CalendarLookup};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const UNIT_SKIP_TOKENS: [&str; 3] = ["---", "--", "----"];

const RAW_HEADER_STANDARD: [&str; 14] = [
    "day", "mo", "year", "Precp", "Runoff", "IR-det", "Av-det", "Mx-det", "Point", "Av-dep",
    "Max-dep", "Point", "Sed.Del", "ER",
];

const RAW_UNITS_STANDARD: [&str; 14] = [
    "---", "--", "----", "(mm)", "(mm)", "kg/m^2", "kg/m^2", "kg/m^2", "(m)", "kg/m^2",
    "kg/m^2", "(m)", "(kg/m)", "----",
];

const RAW_HEADER_REVEG: [&str; 16] = [
    "day", "mo", "year", "Precp", "Runoff", "IR-det", "Av-det", "Mx-det", "Point", "Av-dep",
    "Max-dep", "Point", "Sed.Del", "ER", "Det-Len", "Dep-Len",
];

const RAW_UNITS_REVEG: [&str; 16] = [
    "---", "--", "----", "(mm)", "(mm)", "kg/m^2", "kg/m^2", "kg/m^2", "(m)", "kg/m^2",
    "kg/m^2", "(m)", "(kg/m)", "----", "(m)", "(m)",
];

const MEASUREMENT_COLUMNS_STANDARD: [&str; 11] = [
    "Precp (mm)",
    "Runoff (mm)",
    "IR-det (kg/m^2)",
    "Av-det (kg/m^2)",
    "Mx-det (kg/m^2)",
    "Point (m)",
    "Av-dep (kg/m^2)",
    "Max-dep (kg/m^2)",
    "Point (m)_2",
    "Sed.Del (kg/m)",
    "ER",
];

const MEASUREMENT_COLUMNS_REVEG: [&str; 13] = [
    "Precp (mm)",
    "Runoff (mm)",
    "IR-det (kg/m^2)",
    "Av-det (kg/m^2)",
    "Mx-det (kg/m^2)",
    "Point (m)",
    "Av-dep (kg/m^2)",
    "Max-dep (kg/m^2)",
    "Point (m)_2",
    "Sed.Del (kg/m)",
    "ER",
    "Det-Len (m)",
    "Dep-Len (m)",
];

const MEASUREMENT_FIELD_NAMES: [&str; 13] = [
    "Precip",
    "Runoff",
    "IR-det",
    "Av-det",
    "Mx-det",
    "Det-point",
    "Av-dep",
    "Max-dep",
    "Dep-point",
    "Sed.Del",
    "ER",
    "Det-Len",
    "Dep-Len",
];

fn column_aliases(name: &str) -> &str {
    match name {
        "Precp (mm)" => "Precip",
        "Runoff (mm)" => "Runoff",
        "IR-det (kg/m^2)" => "IR-det",
        "Av-det (kg/m^2)" => "Av-det",
        "Mx-det (kg/m^2)" => "Mx-det",
        "Point (m)" => "Det-point",
        "Point (m)_2" => "Dep-point",
        "Sed.Del (kg/m)" => "Sed.Del",
        "Av-dep (kg/m^2)" => "Av-dep",
        "Max-dep (kg/m^2)" => "Max-dep",
        "Det-Len (m)" => "Det-Len",
        "Dep-Len (m)" => "Dep-Len",
        other => other,
    }
}

pub struct EbeColumns {
    wepp_id: Vec<i32>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    julian: Vec<i16>,
    water_year: Vec<i16>,
    precip: Vec<Option<f64>>,
    runoff: Vec<Option<f64>>,
    ir_det: Vec<Option<f64>>,
    av_det: Vec<Option<f64>>,
    mx_det: Vec<Option<f64>>,
    det_point: Vec<Option<f64>>,
    av_dep: Vec<Option<f64>>,
    max_dep: Vec<Option<f64>>,
    dep_point: Vec<Option<f64>>,
    sed_del: Vec<Option<f64>>,
    er: Vec<Option<f64>>,
    det_len: Vec<Option<f64>>,
    dep_len: Vec<Option<f64>>,
}

impl EbeColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            julian: Vec::new(),
            water_year: Vec::new(),
            precip: Vec::new(),
            runoff: Vec::new(),
            ir_det: Vec::new(),
            av_det: Vec::new(),
            mx_det: Vec::new(),
            det_point: Vec::new(),
            av_dep: Vec::new(),
            max_dep: Vec::new(),
            dep_point: Vec::new(),
            sed_del: Vec::new(),
            er: Vec::new(),
            det_len: Vec::new(),
            dep_len: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("sim_day_index", self.sim_day_index).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("Precip", self.precip).unwrap();
        dict.set_item("Runoff", self.runoff).unwrap();
        dict.set_item("IR-det", self.ir_det).unwrap();
        dict.set_item("Av-det", self.av_det).unwrap();
        dict.set_item("Mx-det", self.mx_det).unwrap();
        dict.set_item("Det-point", self.det_point).unwrap();
        dict.set_item("Av-dep", self.av_dep).unwrap();
        dict.set_item("Max-dep", self.max_dep).unwrap();
        dict.set_item("Dep-point", self.dep_point).unwrap();
        dict.set_item("Sed.Del", self.sed_del).unwrap();
        dict.set_item("ER", self.er).unwrap();
        dict.set_item("Det-Len", self.det_len).unwrap();
        dict.set_item("Dep-Len", self.dep_len).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_ebe_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<EbeColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let wepp_id = extract_wepp_id(path)?;

    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .collect::<Result<_, _>>()
        .map_err(|err| InterchangeError::io(path, err))?;

    let stripped: Vec<String> = lines.into_iter().filter(|line| !line.trim().is_empty()).collect();
    if stripped.len() < 3 {
        return Ok(EbeColumns::new());
    }

    let header_tokens: Vec<&str> = stripped[1].split_whitespace().collect();
    let unit_tokens: Vec<&str> = stripped[2].split_whitespace().collect();

    let (column_names, layout) = normalize_column_names(&header_tokens, &unit_tokens, path)?;
    let measurement_columns: Vec<String> = column_names[3..].to_vec();
    let expected: Vec<&str> = if layout == "standard" {
        MEASUREMENT_COLUMNS_STANDARD.to_vec()
    } else {
        MEASUREMENT_COLUMNS_REVEG.to_vec()
    };
    if measurement_columns.iter().map(|s| s.as_str()).collect::<Vec<_>>() != expected {
        return Err(InterchangeError::parse(
            path,
            None,
            format!("Unexpected EBE measurement columns for layout '{layout}'"),
            Some(measurement_columns.join(" ")),
        ));
    }

    let mut mapping: HashMap<String, usize> = HashMap::new();
    for (idx, name) in MEASUREMENT_FIELD_NAMES.iter().enumerate() {
        mapping.insert(name.to_string(), idx);
    }

    let mut out = EbeColumns::new();

    let calendar_start_year = lookup
        .as_ref()
        .and_then(|cal| cal.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    for raw_line in stripped.iter().skip(3) {
        let tokens: Vec<&str> = raw_line.split_whitespace().collect();
        if tokens.len() != column_names.len() {
            continue;
        }
        let day_of_month: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid day token", Some(raw_line.clone()))
        })?;
        let month: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid month token", Some(raw_line.clone()))
        })?;
        let raw_year: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
        })?;

        let year_val = if normalize_sim_years && raw_year < 1000 {
            resolved_start_year.unwrap_or(raw_year) + raw_year - 1
        } else {
            raw_year
        };
        if sim_start_year.is_none() {
            sim_start_year = Some(year_val);
        }

        let julian = calendar_day_to_julian(year_val, month, day_of_month, lookup.as_ref())?;
        let sim_day_index = compute_sim_day_index(
            year_val,
            julian,
            sim_start_year.unwrap_or(year_val),
            lookup.as_ref(),
        );
        let water_year = determine_wateryear(year_val, julian);

        let mut row_measurements: Vec<Option<f64>> = vec![None; MEASUREMENT_FIELD_NAMES.len()];
        for (column_name, token) in measurement_columns.iter().zip(tokens.iter().skip(3)) {
            let target_name = column_aliases(column_name);
            if let Some(index) = mapping.get(target_name) {
                let value = parse_required_float(token).map_err(|msg| {
                    InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                })?;
                row_measurements[*index] = Some(value);
            }
        }

        out.wepp_id.push(wepp_id);
        out.year.push(year_val as i16);
        out.sim_day_index.push(sim_day_index);
        out.month.push(month as i8);
        out.day_of_month.push(day_of_month as i8);
        out.julian.push(julian as i16);
        out.water_year.push(water_year as i16);
        out.precip.push(row_measurements[0]);
        out.runoff.push(row_measurements[1]);
        out.ir_det.push(row_measurements[2]);
        out.av_det.push(row_measurements[3]);
        out.mx_det.push(row_measurements[4]);
        out.det_point.push(row_measurements[5]);
        out.av_dep.push(row_measurements[6]);
        out.max_dep.push(row_measurements[7]);
        out.dep_point.push(row_measurements[8]);
        out.sed_del.push(row_measurements[9]);
        out.er.push(row_measurements[10]);
        out.det_len.push(row_measurements[11]);
        out.dep_len.push(row_measurements[12]);
    }

    Ok(out)
}

fn normalize_column_names(
    headers: &[&str],
    units: &[&str],
    path: &Path,
) -> Result<(Vec<String>, String), InterchangeError> {
    let layout = if headers == RAW_HEADER_STANDARD && units == RAW_UNITS_STANDARD {
        "standard"
    } else if headers == RAW_HEADER_REVEG && units == RAW_UNITS_REVEG {
        "reveg"
    } else {
        return Err(InterchangeError::parse(
            path,
            None,
            format!("Unexpected EBE header layout: {headers:?} / {units:?}"),
            None,
        ));
    };

    let mut out: Vec<String> = Vec::new();
    for (name, unit) in headers.iter().zip(units.iter()) {
        let mut cleaned = unit.trim().to_string();
        let column = if !cleaned.is_empty() && !UNIT_SKIP_TOKENS.contains(&cleaned.as_str()) {
            if cleaned.starts_with('(') && cleaned.ends_with(')') {
                cleaned = cleaned[1..cleaned.len() - 1].to_string();
            }
            format!("{name} ({cleaned})")
        } else {
            (*name).to_string()
        };
        out.push(column);
    }

    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut deduped: Vec<String> = Vec::new();
    for column in out {
        let count = seen.entry(column.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            deduped.push(format!("{column}_{}", *count));
        } else {
            deduped.push(column);
        }
    }

    Ok((deduped, layout.to_string()))
}

fn calendar_day_to_julian(
    year: i32,
    month: i32,
    day: i32,
    lookup: Option<&CalendarLookup>,
) -> Result<i32, InterchangeError> {
    if let Some(lookup) = lookup {
        if let Some(days) = lookup.by_year.get(&year) {
            for (idx, (m, d)) in days.iter().enumerate() {
                if *m == month && *d == day {
                    return Ok((idx + 1) as i32);
                }
            }
        }
        return Err(InterchangeError::Calendar {
            message: format!(
                "Date {year}-{month}-{day} not found in CLI calendar lookup (years available: {:?})",
                lookup.by_year.keys().collect::<Vec<_>>()
            ),
        });
    }

    let julian = julian_from_ymd(year, month, day).map_err(|message| InterchangeError::Calendar { message })?;
    Ok(julian)
}

fn julian_from_ymd(year: i32, month: i32, day: i32) -> Result<i32, String> {
    if month < 1 || month > 12 {
        return Err(format!("Invalid month {month}"));
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(format!("Invalid day {day} for {year}-{month}"));
    }
    let mut julian = 0i32;
    for m in 1..month {
        julian += days_in_month(year, m);
    }
    julian += day;
    Ok(julian)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    let leap = is_leap_year(year);
    match month {
        1 => 31,
        2 => if leap { 29 } else { 28 },
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn extract_wepp_id(path: &Path) -> Result<i32, InterchangeError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| InterchangeError::parse(path, None, "Missing filename", None))?;
    let mut chars = name.chars();
    if chars.next().map(|c| c.to_ascii_uppercase()) != Some('H') {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unrecognized EBE filename pattern",
            Some(name.to_string()),
        ));
    }
    let mut digits = String::new();
    for c in chars {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unrecognized EBE filename pattern",
            Some(name.to_string()),
        ));
    }
    digits
        .parse::<i32>()
        .map_err(|_| InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string())))
}

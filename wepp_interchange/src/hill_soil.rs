use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::{
    compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar,
};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const RAW_HEADER: [&str; 14] = [
    "OFE",
    "Day",
    "Y",
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
];

const LEGACY_HEADER: [&str; 12] = [
    "OFE", "Day", "Y", "Poros", "Keff", "Suct", "FC", "WP", "Rough", "Ki", "Kr", "Tauc",
];

const RAW_UNITS: [&str; 14] = [
    "", "", "", "%", "mm/hr", "mm", "mm/mm", "mm/mm", "mm", "adjsmt", "adjsmt", "adjsmt", "frac",
    "mm",
];

const MEASUREMENT_COLUMNS: [&str; 11] = [
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
];

pub struct SoilColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    poros: Vec<Option<f64>>,
    keff: Vec<Option<f64>>,
    suct: Vec<Option<f64>>,
    fc: Vec<Option<f64>>,
    wp: Vec<Option<f64>>,
    rough: Vec<Option<f64>>,
    ki: Vec<Option<f64>>,
    kr: Vec<Option<f64>>,
    tauc: Vec<Option<f64>>,
    saturation: Vec<Option<f64>>,
    tsw: Vec<Option<f64>>,
}

impl SoilColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            ofe_id: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            ofe: Vec::new(),
            poros: Vec::new(),
            keff: Vec::new(),
            suct: Vec::new(),
            fc: Vec::new(),
            wp: Vec::new(),
            rough: Vec::new(),
            ki: Vec::new(),
            kr: Vec::new(),
            tauc: Vec::new(),
            saturation: Vec::new(),
            tsw: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("ofe_id", self.ofe_id).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("sim_day_index", self.sim_day_index).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("OFE", self.ofe).unwrap();
        dict.set_item("Poros", self.poros).unwrap();
        dict.set_item("Keff", self.keff).unwrap();
        dict.set_item("Suct", self.suct).unwrap();
        dict.set_item("FC", self.fc).unwrap();
        dict.set_item("WP", self.wp).unwrap();
        dict.set_item("Rough", self.rough).unwrap();
        dict.set_item("Ki", self.ki).unwrap();
        dict.set_item("Kr", self.kr).unwrap();
        dict.set_item("Tauc", self.tauc).unwrap();
        dict.set_item("Saturation", self.saturation).unwrap();
        dict.set_item("TSW", self.tsw).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_soil_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<SoilColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let wepp_id = extract_wepp_id(path)?;

    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = SoilColumns::new();
    let mut header_tokens: Option<Vec<String>> = None;
    let mut measurement_columns: Vec<String> = Vec::new();
    let mut expected_units: Vec<String> = Vec::new();
    let mut unit_tokens_found = false;

    enum ParseState {
        SearchingHeader,
        LookingForUnits,
        SkipAfterUnits,
        Data,
    }

    let mut state = ParseState::SearchingHeader;

    let calendar_start_year = lookup
        .as_ref()
        .and_then(|cal| cal.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        let stripped = raw_line.trim();

        match state {
            ParseState::SearchingHeader => {
                if stripped.is_empty() {
                    continue;
                }
                let tokens: Vec<&str> = stripped.split_whitespace().collect();
                if tokens.len() >= 3 && tokens[0] == "OFE" && tokens[1] == "Day" && tokens[2] == "Y"
                {
                    let tokens_vec = tokens.iter().map(|t| t.to_string()).collect::<Vec<_>>();
                    let header_as_str: Vec<&str> = tokens_vec.iter().map(|s| s.as_str()).collect();
                    let compact_units: Vec<String> = RAW_UNITS
                        .iter()
                        .filter(|token| !token.is_empty())
                        .map(|t| t.to_string())
                        .collect();
                    if header_as_str == RAW_HEADER {
                        expected_units = compact_units.clone();
                        measurement_columns =
                            MEASUREMENT_COLUMNS.iter().map(|s| s.to_string()).collect();
                    } else if header_as_str == LEGACY_HEADER {
                        expected_units = compact_units[..(LEGACY_HEADER.len() - 3)].to_vec();
                        measurement_columns = MEASUREMENT_COLUMNS[..(LEGACY_HEADER.len() - 3)]
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                    } else {
                        return Err(InterchangeError::parse(
                            path,
                            None,
                            format!("Unexpected SOIL header layout: {tokens_vec:?}"),
                            None,
                        ));
                    }
                    header_tokens = Some(tokens_vec);
                    state = ParseState::LookingForUnits;
                }
            }
            ParseState::LookingForUnits => {
                if stripped.is_empty() {
                    continue;
                }
                let tokens: Vec<&str> = stripped.split_whitespace().collect();
                if tokens
                    .iter()
                    .any(|t| ["mm/hr", "frac", "adjsmt"].contains(t))
                {
                    let unit_tokens = tokens.iter().map(|t| t.to_string()).collect::<Vec<_>>();
                    if unit_tokens != expected_units {
                        return Err(InterchangeError::parse(
                            path,
                            None,
                            format!("Unexpected SOIL units: {unit_tokens:?}"),
                            None,
                        ));
                    }
                    unit_tokens_found = true;
                    state = ParseState::SkipAfterUnits;
                }
            }
            ParseState::SkipAfterUnits => {
                if stripped.is_empty() {
                    continue;
                }
                if stripped.chars().all(|c| c == '-') {
                    continue;
                }
                state = ParseState::Data;
            }
            ParseState::Data => {}
        }

        if let ParseState::Data = state {
            if stripped.is_empty() {
                continue;
            }
            let tokens: Vec<&str> = stripped.split_whitespace().collect();
            let expected_columns = header_tokens.as_ref().map(|t| t.len()).unwrap_or(0);
            if tokens.len() != expected_columns {
                continue;
            }

            let ofe_val: i32 = tokens[0].parse().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid OFE token", Some(raw_line.clone()))
            })?;
            let julian_val: i32 = tokens[1].parse().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid julian token", Some(raw_line.clone()))
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

            let (month, day_of_month) = julian_to_calendar(year_val, julian_val, lookup.as_ref());
            let water_year = determine_wateryear(year_val, julian_val);
            let sim_day_index = compute_sim_day_index(
                year_val,
                julian_val,
                sim_start_year.unwrap_or(year_val),
                lookup.as_ref(),
            );

            let mut values: Vec<Option<f64>> = vec![None; MEASUREMENT_COLUMNS.len()];
            for (idx, token) in measurement_columns.iter().zip(tokens.iter().skip(3)) {
                if let Some(pos) = MEASUREMENT_COLUMNS
                    .iter()
                    .position(|name| *name == idx.as_str())
                {
                    let value = parse_required_float(token).map_err(|msg| {
                        InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                    })?;
                    values[pos] = Some(value);
                }
            }

            out.wepp_id.push(wepp_id);
            out.ofe_id.push(ofe_val as i16);
            out.year.push(year_val as i16);
            out.sim_day_index.push(sim_day_index);
            out.julian.push(julian_val as i16);
            out.month.push(month as i8);
            out.day_of_month.push(day_of_month as i8);
            out.water_year.push(water_year as i16);
            out.ofe.push(ofe_val as i16);
            out.poros.push(values[0]);
            out.keff.push(values[1]);
            out.suct.push(values[2]);
            out.fc.push(values[3]);
            out.wp.push(values[4]);
            out.rough.push(values[5]);
            out.ki.push(values[6]);
            out.kr.push(values[7]);
            out.tauc.push(values[8]);
            out.saturation.push(values[9]);
            out.tsw.push(values[10]);
        }
    }

    if header_tokens.is_none() || !unit_tokens_found {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unable to locate SOIL header layout",
            None,
        ));
    }

    Ok(out)
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
            "Unrecognized soil filename pattern",
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
            "Unrecognized soil filename pattern",
            Some(name.to_string()),
        ));
    }
    digits.parse::<i32>().map_err(|_| {
        InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string()))
    })
}

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::determine_wateryear;
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const ELEMENT_FIELD_WIDTHS: [usize; 24] = [
    3, 3, 3, 5, 9, 9, 8, 8, 8, 6, 8, 8, 8, 7, 9, 9, 9, 9, 7, 7, 7, 7, 7, 9,
];

const ELEMENT_COLUMN_NAMES: [&str; 24] = [
    "OFE", "DD", "MM", "YYYY", "Precip", "Runoff", "EffInt", "PeakRO", "EffDur", "Enrich", "Keff",
    "Sm", "LeafArea", "CanHgt", "Cancov", "IntCov", "RilCov", "LivBio", "DeadBio", "Ki", "Kr",
    "Tcrit", "RilWid", "SedLeave",
];

pub struct ElementColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    precip: Vec<f64>,
    runoff: Vec<f64>,
    effint: Vec<f64>,
    peakro: Vec<f64>,
    effdur: Vec<f64>,
    enrich: Vec<f64>,
    keff: Vec<f64>,
    sm: Vec<f64>,
    leaf_area: Vec<f64>,
    can_hgt: Vec<f64>,
    cancov: Vec<f64>,
    intcov: Vec<f64>,
    rilcov: Vec<f64>,
    livbio: Vec<f64>,
    deadbio: Vec<f64>,
    ki: Vec<f64>,
    kr: Vec<f64>,
    tcrit: Vec<f64>,
    rilwid: Vec<f64>,
    sedleave: Vec<f64>,
}

impl ElementColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            ofe_id: Vec::new(),
            year: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            ofe: Vec::new(),
            precip: Vec::new(),
            runoff: Vec::new(),
            effint: Vec::new(),
            peakro: Vec::new(),
            effdur: Vec::new(),
            enrich: Vec::new(),
            keff: Vec::new(),
            sm: Vec::new(),
            leaf_area: Vec::new(),
            can_hgt: Vec::new(),
            cancov: Vec::new(),
            intcov: Vec::new(),
            rilcov: Vec::new(),
            livbio: Vec::new(),
            deadbio: Vec::new(),
            ki: Vec::new(),
            kr: Vec::new(),
            tcrit: Vec::new(),
            rilwid: Vec::new(),
            sedleave: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("ofe_id", self.ofe_id).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("OFE", self.ofe).unwrap();
        dict.set_item("Precip", self.precip).unwrap();
        dict.set_item("Runoff", self.runoff).unwrap();
        dict.set_item("EffInt", self.effint).unwrap();
        dict.set_item("PeakRO", self.peakro).unwrap();
        dict.set_item("EffDur", self.effdur).unwrap();
        dict.set_item("Enrich", self.enrich).unwrap();
        dict.set_item("Keff", self.keff).unwrap();
        dict.set_item("Sm", self.sm).unwrap();
        dict.set_item("LeafArea", self.leaf_area).unwrap();
        dict.set_item("CanHgt", self.can_hgt).unwrap();
        dict.set_item("Cancov", self.cancov).unwrap();
        dict.set_item("IntCov", self.intcov).unwrap();
        dict.set_item("RilCov", self.rilcov).unwrap();
        dict.set_item("LivBio", self.livbio).unwrap();
        dict.set_item("DeadBio", self.deadbio).unwrap();
        dict.set_item("Ki", self.ki).unwrap();
        dict.set_item("Kr", self.kr).unwrap();
        dict.set_item("Tcrit", self.tcrit).unwrap();
        dict.set_item("RilWid", self.rilwid).unwrap();
        dict.set_item("SedLeave", self.sedleave).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_element_to_columns(
    path: &Path,
    _version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<ElementColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = ElementColumns::new();
    let mut previous: Vec<f64> = vec![0.0; ELEMENT_COLUMN_NAMES.len() - 4];
    let mut non_empty_count = 0usize;
    let mut data_index = 0usize;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        if raw_line.trim().is_empty() {
            continue;
        }
        non_empty_count += 1;
        if non_empty_count <= 2 {
            continue;
        }
        let idx = data_index;
        data_index += 1;
        let tokens = split_fixed_width_line(&raw_line, path)?;
        let ofe: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid OFE token", Some(raw_line.clone()))
        })?;
        let day: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid day token", Some(raw_line.clone()))
        })?;
        let month: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid month token", Some(raw_line.clone()))
        })?;
        let year_token: i32 = tokens[3].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
        })?;

        let (year, month, day, julian, water_year) =
            normalize_date_tokens(year_token, month, day, start_year);

        let mut row_values: Vec<f64> = Vec::with_capacity(ELEMENT_COLUMN_NAMES.len() - 4);
        for (col_idx, token) in tokens.iter().skip(4).enumerate() {
            let value = if is_missing_token(token) {
                if idx == 0 {
                    0.0
                } else {
                    previous[col_idx]
                }
            } else {
                parse_required_float(token).map_err(|msg| {
                    InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                })?
            };
            row_values.push(value);
        }

        out.wepp_id.push(wepp_id);
        out.ofe_id.push(ofe as i16);
        out.year.push(year as i16);
        out.julian.push(julian as i16);
        out.month.push(month as i8);
        out.day_of_month.push(day as i8);
        out.water_year.push(water_year as i16);
        out.ofe.push(ofe as i16);
        out.precip.push(row_values[0]);
        out.runoff.push(row_values[1]);
        out.effint.push(row_values[2]);
        out.peakro.push(row_values[3]);
        out.effdur.push(row_values[4]);
        out.enrich.push(row_values[5]);
        out.keff.push(row_values[6]);
        out.sm.push(row_values[7]);
        out.leaf_area.push(row_values[8]);
        out.can_hgt.push(row_values[9]);
        out.cancov.push(row_values[10]);
        out.intcov.push(row_values[11]);
        out.rilcov.push(row_values[12]);
        out.livbio.push(row_values[13]);
        out.deadbio.push(row_values[14]);
        out.ki.push(row_values[15]);
        out.kr.push(row_values[16]);
        out.tcrit.push(row_values[17]);
        out.rilwid.push(row_values[18]);
        out.sedleave.push(row_values[19]);

        previous = row_values;
    }

    Ok(out)
}

fn split_fixed_width_line(raw_line: &str, path: &Path) -> Result<Vec<String>, InterchangeError> {
    let width: usize = ELEMENT_FIELD_WIDTHS.iter().sum();
    let mut line = raw_line.to_string();
    if line.len() < width {
        line.push_str(&" ".repeat(width - line.len()));
    }
    let mut tokens = Vec::new();
    let mut idx = 0usize;
    for width in ELEMENT_FIELD_WIDTHS {
        let end = idx + width;
        let segment = line.get(idx..end).unwrap_or("");
        tokens.push(segment.trim().to_string());
        idx = end;
    }
    if idx < line.len() && line[idx..].trim().len() > 0 {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unexpected trailing characters past fixed width payload",
            Some(raw_line.to_string()),
        ));
    }
    Ok(tokens)
}

fn is_missing_token(token: &str) -> bool {
    let stripped = token.trim();
    !stripped.is_empty() && stripped.chars().all(|c| c == '*')
}

fn normalize_date_tokens(
    raw_year: i32,
    raw_month: i32,
    raw_day: i32,
    start_year: Option<i32>,
) -> (i32, i32, i32, i32, i32) {
    let mut year = raw_year;
    if let Some(start) = start_year {
        if year < 1000 {
            year = start + year - 1;
        }
    }

    let mut month = raw_month.max(1);
    let mut day = raw_day.max(1);

    let extra_years = (month - 1) / 12;
    month = (month - 1) % 12 + 1;
    year += extra_years;

    let max_day = days_in_month(year, month);
    if day > max_day {
        day = max_day;
    }

    let julian = julian_from_ymd(year, month, day);
    let water_year = determine_wateryear(year, julian);
    (year, month, day, julian, water_year)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    let leap = is_leap_year(year);
    match month {
        1 => 31,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
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

fn julian_from_ymd(year: i32, month: i32, day: i32) -> i32 {
    let mut julian = 0;
    for m in 1..month {
        julian += days_in_month(year, m);
    }
    julian + day
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
            "Unrecognized element filename pattern",
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
            "Unrecognized element filename pattern",
            Some(name.to_string()),
        ));
    }
    digits.parse::<i32>().map_err(|_| {
        InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string()))
    })
}

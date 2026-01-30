use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const MEASUREMENT_COLUMNS: [&str; 9] = [
    "Class",
    "Diameter (mm)",
    "Specific Gravity",
    "% Sand",
    "% Silt",
    "% Clay",
    "% O.M.",
    "Sediment Fraction",
    "In Flow Exiting",
];

fn column_alias(name: &str) -> &str {
    match name {
        "Diameter (mm)" => "Diameter",
        other => other,
    }
}

pub struct LossColumns {
    wepp_id: Vec<i32>,
    class_id: Vec<i8>,
    class_val: Vec<i8>,
    diameter: Vec<f64>,
    specific_gravity: Vec<f64>,
    sand: Vec<f64>,
    silt: Vec<f64>,
    clay: Vec<f64>,
    om: Vec<f64>,
    sediment_fraction: Vec<f64>,
    inflow_exiting: Vec<f64>,
}

impl LossColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            class_id: Vec::new(),
            class_val: Vec::new(),
            diameter: Vec::new(),
            specific_gravity: Vec::new(),
            sand: Vec::new(),
            silt: Vec::new(),
            clay: Vec::new(),
            om: Vec::new(),
            sediment_fraction: Vec::new(),
            inflow_exiting: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("class_id", self.class_id).unwrap();
        dict.set_item("Class", self.class_val).unwrap();
        dict.set_item("Diameter", self.diameter).unwrap();
        dict.set_item("Specific Gravity", self.specific_gravity).unwrap();
        dict.set_item("% Sand", self.sand).unwrap();
        dict.set_item("% Silt", self.silt).unwrap();
        dict.set_item("% Clay", self.clay).unwrap();
        dict.set_item("% O.M.", self.om).unwrap();
        dict.set_item("Sediment Fraction", self.sediment_fraction).unwrap();
        dict.set_item("In Flow Exiting", self.inflow_exiting).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_loss_to_columns(
    path: &Path,
    _version: &VersionInfo,
) -> Result<LossColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .collect::<Result<_, _>>()
        .map_err(|err| InterchangeError::io(path, err))?;

    let table_anchor = locate_class_table(&lines);
    if table_anchor.is_none() {
        return Ok(LossColumns::new());
    }

    let data_start = table_anchor.unwrap() + 6;
    let mut out = LossColumns::new();

    for raw_line in lines.iter().skip(data_start) {
        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.chars().all(|c| c == '-') {
            break;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() < MEASUREMENT_COLUMNS.len() {
            continue;
        }

        let class_val: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid class token", Some(raw_line.clone()))
        })?;
        let measurements = &tokens[1..MEASUREMENT_COLUMNS.len()];

        let mut values: HashMap<&str, f64> = HashMap::new();
        for (column_name, token) in MEASUREMENT_COLUMNS[1..].iter().zip(measurements.iter()) {
            let target = column_alias(column_name);
            let value = parse_required_float(token).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
            })?;
            values.insert(target, value);
        }

        out.wepp_id.push(wepp_id);
        out.class_id.push(class_val as i8);
        out.class_val.push(class_val as i8);
        out.diameter.push(*values.get("Diameter").unwrap_or(&0.0));
        out.specific_gravity.push(*values.get("Specific Gravity").unwrap_or(&0.0));
        out.sand.push(*values.get("% Sand").unwrap_or(&0.0));
        out.silt.push(*values.get("% Silt").unwrap_or(&0.0));
        out.clay.push(*values.get("% Clay").unwrap_or(&0.0));
        out.om.push(*values.get("% O.M.").unwrap_or(&0.0));
        out.sediment_fraction.push(*values.get("Sediment Fraction").unwrap_or(&0.0));
        out.inflow_exiting.push(*values.get("In Flow Exiting").unwrap_or(&0.0));
    }

    Ok(out)
}

fn locate_class_table(lines: &[String]) -> Option<usize> {
    let target = "sediment particle information leaving profile";
    let mut start_idx: Option<usize> = None;
    for (idx, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(target) {
            start_idx = Some(idx);
        }
    }
    start_idx
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
            "Unrecognized loss filename pattern",
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
            "Unrecognized loss filename pattern",
            Some(name.to_string()),
        ));
    }
    digits
        .parse::<i32>()
        .map_err(|_| InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string())))
}

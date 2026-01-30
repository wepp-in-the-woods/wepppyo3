use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::{determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const RAW_HEADER_SUBSTITUTIONS: [(&str, &str); 5] = [
    (" -", ""),
    ("#", "(#)"),
    (" mm", ""),
    ("Water(mm)", "Water"),
    ("m^2", "(m^2)"),
];

const WAT_COLUMN_NAMES: [&str; 20] = [
    "OFE",
    "J",
    "Y",
    "P",
    "RM",
    "Q",
    "Ep",
    "Es",
    "Er",
    "Dp",
    "UpStrmQ",
    "SubRIn",
    "latqcc",
    "Total-Soil Water",
    "frozwt",
    "Snow-Water",
    "QOFE",
    "Tile",
    "Irr",
    "Area",
];

fn header_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("OFE (#)", "OFE"),
        ("OFE", "OFE"),
        ("P (mm)", "P"),
        ("RM (mm)", "RM"),
        ("Q (mm)", "Q"),
        ("Ep (mm)", "Ep"),
        ("Es (mm)", "Es"),
        ("Er (mm)", "Er"),
        ("Dp (mm)", "Dp"),
        ("UpStrmQ (mm)", "UpStrmQ"),
        ("SubRIn (mm)", "SubRIn"),
        ("latqcc (mm)", "latqcc"),
        ("Total-Soil Water (mm)", "Total-Soil Water"),
        ("frozwt (mm)", "frozwt"),
        ("Snow-Water (mm)", "Snow-Water"),
        ("QOFE (mm)", "QOFE"),
        ("Tile (mm)", "Tile"),
        ("Irr (mm)", "Irr"),
        ("Area (m^2)", "Area"),
    ])
}

pub struct WatColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    p: Vec<f64>,
    rm: Vec<f64>,
    q: Vec<f64>,
    ep: Vec<f64>,
    es: Vec<f64>,
    er: Vec<f64>,
    dp: Vec<f64>,
    upstrmq: Vec<f64>,
    subrin: Vec<f64>,
    latqcc: Vec<f64>,
    total_soil_water: Vec<f64>,
    frozwt: Vec<f64>,
    snow_water: Vec<f64>,
    qofe: Vec<f64>,
    tile: Vec<f64>,
    irr: Vec<f64>,
    area: Vec<f64>,
}

impl WatColumns {
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
            p: Vec::new(),
            rm: Vec::new(),
            q: Vec::new(),
            ep: Vec::new(),
            es: Vec::new(),
            er: Vec::new(),
            dp: Vec::new(),
            upstrmq: Vec::new(),
            subrin: Vec::new(),
            latqcc: Vec::new(),
            total_soil_water: Vec::new(),
            frozwt: Vec::new(),
            snow_water: Vec::new(),
            qofe: Vec::new(),
            tile: Vec::new(),
            irr: Vec::new(),
            area: Vec::new(),
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
        dict.set_item("P", self.p).unwrap();
        dict.set_item("RM", self.rm).unwrap();
        dict.set_item("Q", self.q).unwrap();
        dict.set_item("Ep", self.ep).unwrap();
        dict.set_item("Es", self.es).unwrap();
        dict.set_item("Er", self.er).unwrap();
        dict.set_item("Dp", self.dp).unwrap();
        dict.set_item("UpStrmQ", self.upstrmq).unwrap();
        dict.set_item("SubRIn", self.subrin).unwrap();
        dict.set_item("latqcc", self.latqcc).unwrap();
        dict.set_item("Total-Soil Water", self.total_soil_water).unwrap();
        dict.set_item("frozwt", self.frozwt).unwrap();
        dict.set_item("Snow-Water", self.snow_water).unwrap();
        dict.set_item("QOFE", self.qofe).unwrap();
        dict.set_item("Tile", self.tile).unwrap();
        dict.set_item("Irr", self.irr).unwrap();
        dict.set_item("Area", self.area).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_wat_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
) -> Result<WatColumns, InterchangeError> {
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

    let (header, data_start) = extract_header(&lines, path)?;
    let column_positions: HashMap<&str, usize> = header
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.as_str(), idx))
        .collect();

    let mut out = WatColumns::new();

    for (idx, raw_line) in lines.iter().skip(data_start).enumerate() {
        if raw_line.trim().is_empty() {
            continue;
        }
        let tokens: Vec<&str> = raw_line.split_whitespace().collect();
        if tokens.len() != header.len() {
            continue;
        }

        let julian_val: i32 = tokens[*column_positions.get("J").unwrap()].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid julian token", Some(raw_line.clone()))
        })?;
        let year_val: i32 = tokens[*column_positions.get("Y").unwrap()].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
        })?;
        let (month, day_of_month) = julian_to_calendar(year_val, julian_val, lookup.as_ref());
        let water_year = determine_wateryear(year_val, julian_val);
        let ofe_val: i32 = tokens[*column_positions.get("OFE").unwrap()].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid OFE token", Some(raw_line.clone()))
        })?;

        out.wepp_id.push(wepp_id);
        out.ofe_id.push(ofe_val as i16);
        out.year.push(year_val as i16);
        out.sim_day_index.push((idx + 1) as i32);
        out.julian.push(julian_val as i16);
        out.month.push(month as i8);
        out.day_of_month.push(day_of_month as i8);
        out.water_year.push(water_year as i16);
        out.ofe.push(ofe_val as i16);

        for name in WAT_COLUMN_NAMES.iter().skip(3) {
            let token = tokens[*column_positions.get(*name).unwrap()];
            let value = parse_required_float(token).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
            })?;
            match *name {
                "P" => out.p.push(value),
                "RM" => out.rm.push(value),
                "Q" => out.q.push(value),
                "Ep" => out.ep.push(value),
                "Es" => out.es.push(value),
                "Er" => out.er.push(value),
                "Dp" => out.dp.push(value),
                "UpStrmQ" => out.upstrmq.push(value),
                "SubRIn" => out.subrin.push(value),
                "latqcc" => out.latqcc.push(value),
                "Total-Soil Water" => out.total_soil_water.push(value),
                "frozwt" => out.frozwt.push(value),
                "Snow-Water" => out.snow_water.push(value),
                "QOFE" => out.qofe.push(value),
                "Tile" => out.tile.push(value),
                "Irr" => out.irr.push(value),
                "Area" => out.area.push(value),
                _ => {}
            }
        }
    }

    Ok(out)
}

fn extract_header(lines: &[String], path: &Path) -> Result<(Vec<String>, usize), InterchangeError> {
    let mut header_start: Option<usize> = None;
    let mut header_end: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.starts_with('-') {
            if header_start.is_none() {
                header_start = Some(idx);
            } else if header_end.is_none() {
                header_end = Some(idx);
                break;
            }
        }
    }

    if header_start.is_none() || header_end.is_none() {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unable to locate WAT header delimiters",
            None,
        ));
    }

    let raw_header_rows: Vec<Vec<String>> = lines[(header_start.unwrap() + 1)..header_end.unwrap()]
        .iter()
        .map(|line| line.split_whitespace().map(|s| s.to_string()).collect())
        .collect();

    let mut header: Vec<String> = Vec::new();
    let min_len = raw_header_rows.iter().map(|row| row.len()).min().unwrap_or(0);
    for col_idx in 0..min_len {
        let mut merged = raw_header_rows
            .iter()
            .map(|row| row[col_idx].clone())
            .collect::<Vec<_>>()
            .join(" ");
        for (old, new) in RAW_HEADER_SUBSTITUTIONS.iter() {
            merged = merged.replace(old, new);
        }
        header.push(merged.trim().to_string());
    }

    let aliases = header_aliases();
    let canonical_header: Vec<String> = header
        .iter()
        .map(|value| aliases.get(value.as_str()).unwrap_or(&value.as_str()).to_string())
        .collect();

    if canonical_header.iter().map(|s| s.as_str()).collect::<Vec<_>>() != WAT_COLUMN_NAMES {
        return Err(InterchangeError::parse(
            path,
            None,
            format!("Unexpected WAT column layout: {header:?}"),
            None,
        ));
    }

    Ok((canonical_header, header_end.unwrap() + 2))
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
            "Unrecognized WAT filename pattern",
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
            "Unrecognized WAT filename pattern",
            Some(name.to_string()),
        ));
    }
    digits
        .parse::<i32>()
        .map_err(|_| InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string())))
}

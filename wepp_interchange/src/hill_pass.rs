use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::calendar::{compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::schema::VersionInfo;

const EVENT_LABELS: [&str; 3] = ["EVENT", "SUBEVENT", "NO EVENT"];
const SEDCLASS_COUNT: usize = 5;
const EVENT_FLOAT_COUNT: usize = 12 + (2 * SEDCLASS_COUNT) + 2;
const SUBEVENT_FLOAT_COUNT: usize = 6;
const NOEVENT_FLOAT_COUNT: usize = 2;

pub struct PassColumns {
    wepp_id: Vec<i32>,
    event: Vec<String>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    dur: Vec<f64>,
    tcs: Vec<f64>,
    oalpha: Vec<f64>,
    runoff: Vec<f64>,
    runvol: Vec<f64>,
    sbrunf: Vec<f64>,
    sbrunv: Vec<f64>,
    drainq: Vec<f64>,
    drrunv: Vec<f64>,
    peakro: Vec<f64>,
    tdet: Vec<f64>,
    tdep: Vec<f64>,
    sedcon_1: Vec<f64>,
    sedcon_2: Vec<f64>,
    sedcon_3: Vec<f64>,
    sedcon_4: Vec<f64>,
    sedcon_5: Vec<f64>,
    clot: Vec<f64>,
    slot: Vec<f64>,
    saot: Vec<f64>,
    laot: Vec<f64>,
    sdot: Vec<f64>,
    gwbfv: Vec<f64>,
    gwdsv: Vec<f64>,
}

impl PassColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            event: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            dur: Vec::new(),
            tcs: Vec::new(),
            oalpha: Vec::new(),
            runoff: Vec::new(),
            runvol: Vec::new(),
            sbrunf: Vec::new(),
            sbrunv: Vec::new(),
            drainq: Vec::new(),
            drrunv: Vec::new(),
            peakro: Vec::new(),
            tdet: Vec::new(),
            tdep: Vec::new(),
            sedcon_1: Vec::new(),
            sedcon_2: Vec::new(),
            sedcon_3: Vec::new(),
            sedcon_4: Vec::new(),
            sedcon_5: Vec::new(),
            clot: Vec::new(),
            slot: Vec::new(),
            saot: Vec::new(),
            laot: Vec::new(),
            sdot: Vec::new(),
            gwbfv: Vec::new(),
            gwdsv: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("event", self.event).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("sim_day_index", self.sim_day_index).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("dur", self.dur).unwrap();
        dict.set_item("tcs", self.tcs).unwrap();
        dict.set_item("oalpha", self.oalpha).unwrap();
        dict.set_item("runoff", self.runoff).unwrap();
        dict.set_item("runvol", self.runvol).unwrap();
        dict.set_item("sbrunf", self.sbrunf).unwrap();
        dict.set_item("sbrunv", self.sbrunv).unwrap();
        dict.set_item("drainq", self.drainq).unwrap();
        dict.set_item("drrunv", self.drrunv).unwrap();
        dict.set_item("peakro", self.peakro).unwrap();
        dict.set_item("tdet", self.tdet).unwrap();
        dict.set_item("tdep", self.tdep).unwrap();
        dict.set_item("sedcon_1", self.sedcon_1).unwrap();
        dict.set_item("sedcon_2", self.sedcon_2).unwrap();
        dict.set_item("sedcon_3", self.sedcon_3).unwrap();
        dict.set_item("sedcon_4", self.sedcon_4).unwrap();
        dict.set_item("sedcon_5", self.sedcon_5).unwrap();
        dict.set_item("clot", self.clot).unwrap();
        dict.set_item("slot", self.slot).unwrap();
        dict.set_item("saot", self.saot).unwrap();
        dict.set_item("laot", self.laot).unwrap();
        dict.set_item("sdot", self.sdot).unwrap();
        dict.set_item("gwbfv", self.gwbfv).unwrap();
        dict.set_item("gwdsv", self.gwdsv).unwrap();
        dict.into_py(py)
    }
}

pub fn hillslope_pass_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
) -> Result<PassColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let wepp_id = extract_wepp_id(path)?;

    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut header_lines: Vec<String> = Vec::new();
    let mut line_iter = reader.lines();
    while header_lines.len() < 5 {
        match line_iter.next() {
            Some(Ok(line)) => header_lines.push(line),
            Some(Err(err)) => return Err(InterchangeError::io(path, err)),
            None => break,
        }
    }

    if header_lines.len() < 2 {
        return Err(InterchangeError::parse(
            path,
            None,
            "PASS file missing simulation metadata header",
            None,
        ));
    }

    let header_tokens: Vec<&str> = header_lines[1].split_whitespace().collect();
    if header_tokens.is_empty() {
        return Err(InterchangeError::parse(
            path,
            Some(2),
            "Unable to determine simulation start year from PASS header",
            Some(header_lines[1].clone()),
        ));
    }
    let begin_year = header_tokens
        .last()
        .and_then(|token| token.parse::<i32>().ok())
        .ok_or_else(|| {
            InterchangeError::parse(
                path,
                Some(2),
                "PASS header does not contain a valid start year",
                Some(header_lines[1].clone()),
            )
        })?;
    let mut out = PassColumns::new();

    if header_lines.len() < 5 {
        return Ok(out);
    }

    let mut pending_line: Option<String> = None;
    loop {
        let raw_line = if let Some(line) = pending_line.take() {
            line
        } else {
            match line_iter.next() {
                Some(Ok(line)) => line,
                Some(Err(err)) => return Err(InterchangeError::io(path, err)),
                None => break,
            }
        };
        let label = raw_line
            .get(0..8)
            .unwrap_or(raw_line.as_str())
            .trim()
            .to_ascii_uppercase();
        if label.is_empty() || !EVENT_LABELS.contains(&label.as_str()) {
            continue;
        }

        let (tokens, buffered_line) = if label == "EVENT" {
            let mut tokens: Vec<String> = line_tokens(&raw_line);
            let expected = 2 + EVENT_FLOAT_COUNT;
            let mut buffer: Option<String> = None;
            while tokens.len() < expected {
                let next_line = match line_iter.next() {
                    Some(Ok(line)) => line,
                    Some(Err(err)) => return Err(InterchangeError::io(path, err)),
                    None => break,
                };
                let candidate_label = next_line.get(0..8).unwrap_or(&next_line).trim();
                if !candidate_label.is_empty() {
                    let upper = candidate_label.to_ascii_uppercase();
                    if EVENT_LABELS.contains(&upper.as_str()) {
                        buffer = Some(next_line);
                        break;
                    }
                }
                tokens.extend(next_line.split_whitespace().map(|token| token.to_string()));
            }
            (tokens, buffer)
        } else {
            (line_tokens(&raw_line), None)
        };
        if let Some(buffered) = buffered_line {
            pending_line = Some(buffered);
        }

        if tokens.len() < 2 {
            continue;
        }

        let year: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
        })?;
        let julian: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid julian token", Some(raw_line.clone()))
        })?;

        let (month, day_of_month) = julian_to_calendar(year, julian, lookup.as_ref());
        let water_year = determine_wateryear(year, julian);
        let sim_day_index = compute_sim_day_index(year, julian, begin_year, lookup.as_ref());
        if sim_day_index < 1 {
            return Err(InterchangeError::parse(
                path,
                None,
                format!("Computed negative simulation day index ({sim_day_index})"),
                Some(raw_line.clone()),
            ));
        }

        let mut row = PassRow::new();
        row.event = label.clone();
        row.year = year as i16;
        row.sim_day_index = sim_day_index;
        row.julian = julian as i16;
        row.month = month as i8;
        row.day_of_month = day_of_month as i8;
        row.water_year = water_year as i16;

        if label == "EVENT" {
            let values = &tokens[2..];
            if values.len() != EVENT_FLOAT_COUNT {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    format!("Unexpected EVENT token count in {path:?}: {}", values.len()),
                    Some(raw_line.clone()),
                ));
            }
            row.fill_event(values, path, &raw_line)?;
        } else if label == "SUBEVENT" {
            let values = &tokens[2..];
            if values.len() != SUBEVENT_FLOAT_COUNT {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    format!("Unexpected SUBEVENT token count in {path:?}: {}", values.len()),
                    Some(raw_line.clone()),
                ));
            }
            row.fill_subevent(values, path, &raw_line)?;
        } else {
            let values = &tokens[2..];
            if values.len() != NOEVENT_FLOAT_COUNT {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    format!("Unexpected NO EVENT token count in {path:?}: {}", values.len()),
                    Some(raw_line.clone()),
                ));
            }
            row.fill_noevent(values, path, &raw_line)?;
        }

        row.push_into(&mut out, wepp_id);
    }

    Ok(out)
}

struct PassRow {
    event: String,
    year: i16,
    sim_day_index: i32,
    julian: i16,
    month: i8,
    day_of_month: i8,
    water_year: i16,
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

impl PassRow {
    fn new() -> Self {
        Self {
            event: String::new(),
            year: 0,
            sim_day_index: 0,
            julian: 0,
            month: 0,
            day_of_month: 0,
            water_year: 0,
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
            gwbfv: 0.0,
            gwdsv: 0.0,
        }
    }

    fn fill_event(&mut self, values: &[String], path: &Path, raw_line: &str) -> Result<(), InterchangeError> {
        let mut iter = values.iter();
        self.dur = parse_token(iter.next(), path, raw_line)?;
        self.tcs = parse_token(iter.next(), path, raw_line)?;
        self.oalpha = parse_token(iter.next(), path, raw_line)?;
        self.runoff = parse_token(iter.next(), path, raw_line)?;
        self.runvol = parse_token(iter.next(), path, raw_line)?;
        self.sbrunf = parse_token(iter.next(), path, raw_line)?;
        self.sbrunv = parse_token(iter.next(), path, raw_line)?;
        self.drainq = parse_token(iter.next(), path, raw_line)?;
        self.drrunv = parse_token(iter.next(), path, raw_line)?;
        self.peakro = parse_token(iter.next(), path, raw_line)?;
        self.tdet = parse_token(iter.next(), path, raw_line)?;
        self.tdep = parse_token(iter.next(), path, raw_line)?;
        self.sedcon_1 = parse_token(iter.next(), path, raw_line)?;
        self.sedcon_2 = parse_token(iter.next(), path, raw_line)?;
        self.sedcon_3 = parse_token(iter.next(), path, raw_line)?;
        self.sedcon_4 = parse_token(iter.next(), path, raw_line)?;
        self.sedcon_5 = parse_token(iter.next(), path, raw_line)?;
        self.clot = parse_token(iter.next(), path, raw_line)?;
        self.slot = parse_token(iter.next(), path, raw_line)?;
        self.saot = parse_token(iter.next(), path, raw_line)?;
        self.laot = parse_token(iter.next(), path, raw_line)?;
        self.sdot = parse_token(iter.next(), path, raw_line)?;
        self.gwbfv = parse_token(iter.next(), path, raw_line)?;
        self.gwdsv = parse_token(iter.next(), path, raw_line)?;
        Ok(())
    }

    fn fill_subevent(&mut self, values: &[String], path: &Path, raw_line: &str) -> Result<(), InterchangeError> {
        let mut iter = values.iter();
        self.sbrunf = parse_token(iter.next(), path, raw_line)?;
        self.sbrunv = parse_token(iter.next(), path, raw_line)?;
        self.drainq = parse_token(iter.next(), path, raw_line)?;
        self.drrunv = parse_token(iter.next(), path, raw_line)?;
        self.gwbfv = parse_token(iter.next(), path, raw_line)?;
        self.gwdsv = parse_token(iter.next(), path, raw_line)?;
        Ok(())
    }

    fn fill_noevent(&mut self, values: &[String], path: &Path, raw_line: &str) -> Result<(), InterchangeError> {
        let mut iter = values.iter();
        self.gwbfv = parse_token(iter.next(), path, raw_line)?;
        self.gwdsv = parse_token(iter.next(), path, raw_line)?;
        Ok(())
    }

    fn push_into(self, out: &mut PassColumns, wepp_id: i32) {
        out.wepp_id.push(wepp_id);
        out.event.push(self.event);
        out.year.push(self.year);
        out.sim_day_index.push(self.sim_day_index);
        out.julian.push(self.julian);
        out.month.push(self.month);
        out.day_of_month.push(self.day_of_month);
        out.water_year.push(self.water_year);
        out.dur.push(self.dur);
        out.tcs.push(self.tcs);
        out.oalpha.push(self.oalpha);
        out.runoff.push(self.runoff);
        out.runvol.push(self.runvol);
        out.sbrunf.push(self.sbrunf);
        out.sbrunv.push(self.sbrunv);
        out.drainq.push(self.drainq);
        out.drrunv.push(self.drrunv);
        out.peakro.push(self.peakro);
        out.tdet.push(self.tdet);
        out.tdep.push(self.tdep);
        out.sedcon_1.push(self.sedcon_1);
        out.sedcon_2.push(self.sedcon_2);
        out.sedcon_3.push(self.sedcon_3);
        out.sedcon_4.push(self.sedcon_4);
        out.sedcon_5.push(self.sedcon_5);
        out.clot.push(self.clot);
        out.slot.push(self.slot);
        out.saot.push(self.saot);
        out.laot.push(self.laot);
        out.sdot.push(self.sdot);
        out.gwbfv.push(self.gwbfv);
        out.gwdsv.push(self.gwdsv);
    }
}

fn parse_token(token: Option<&String>, path: &Path, raw_line: &str) -> Result<f64, InterchangeError> {
    let token = token.ok_or_else(|| {
        InterchangeError::parse(path, None, "Missing numeric token", Some(raw_line.to_string()))
    })?;
    parse_required_float(token).map_err(|msg| InterchangeError::parse(path, None, msg, Some(raw_line.to_string())))
}

fn line_tokens(line: &str) -> Vec<String> {
    let payload = if line.len() > 8 { &line[8..] } else { "" };
    payload.split_whitespace().map(|t| t.to_string()).collect()
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
            "Unrecognized PASS filename pattern",
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
            "Unrecognized PASS filename pattern",
            Some(name.to_string()),
        ));
    }
    digits
        .parse::<i32>()
        .map_err(|_| InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string())))
}

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arrow_array::{Array, Float64Array, Int16Array, Int32Array};

use crate::arrow_support::{BoxedArray, Chunk};
use crate::calendar::{compute_sim_day_index, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink};
use crate::schema::{watershed_tc_out_schema, VersionInfo};

const DEFAULT_CHUNK_ROWS: usize = 250_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcOutWriteSummary {
    pub rows_written: usize,
    pub row_groups: usize,
    pub output_paths: Vec<String>,
    pub outlet_channel: Option<i32>,
}

struct TcOutStore {
    day: Vec<i16>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    time_of_conc: Vec<f64>,
    storm_duration: Vec<f64>,
    storm_peak: Vec<f64>,
}

impl TcOutStore {
    fn new() -> Self {
        Self {
            day: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            julian: Vec::new(),
            time_of_conc: Vec::new(),
            storm_duration: Vec::new(),
            storm_peak: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.day.len()
    }

    fn to_chunk(&mut self) -> Chunk<Box<dyn Array>> {
        Chunk::new(vec![
            Int16Array::from(std::mem::take(&mut self.day)).boxed(),
            Int16Array::from(std::mem::take(&mut self.year)).boxed(),
            Int32Array::from(std::mem::take(&mut self.sim_day_index)).boxed(),
            Int16Array::from(std::mem::take(&mut self.julian)).boxed(),
            Float64Array::from(std::mem::take(&mut self.time_of_conc)).boxed(),
            Float64Array::from(std::mem::take(&mut self.storm_duration)).boxed(),
            Float64Array::from(std::mem::take(&mut self.storm_peak)).boxed(),
        ])
    }
}

pub fn watershed_tc_out_to_parquet(
    tc_out_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
) -> Result<TcOutWriteSummary, InterchangeError> {
    let outlet_channel = find_outlet_channel(tc_out_path)?;
    let Some(outlet_channel) = outlet_channel else {
        return Ok(TcOutWriteSummary {
            rows_written: 0,
            row_groups: 0,
            output_paths: Vec::new(),
            outlet_channel: None,
        });
    };

    let calendar_lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let calendar_start_year = calendar_lookup
        .as_ref()
        .and_then(|calendar| calendar.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    let schema = watershed_tc_out_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    let mut store = TcOutStore::new();
    let chunk_rows = chunk_rows.unwrap_or(DEFAULT_CHUNK_ROWS).max(1);
    let reader = BufReader::new(
        File::open(tc_out_path).map_err(|err| InterchangeError::io(tc_out_path, err))?,
    );
    let mut row_counter = 0usize;

    for (line_idx, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| InterchangeError::io(tc_out_path, err))?;
        let stripped = raw_line.trim();
        if should_skip_line(stripped) {
            continue;
        }

        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() < 9 || tokens[1] != "C" {
            continue;
        }

        let Ok(channel_id) = tokens[2].parse::<i32>() else {
            continue;
        };
        if channel_id != outlet_channel {
            continue;
        }

        let Ok(day) = tokens[3].parse::<i16>() else {
            continue;
        };
        let Ok(raw_year) = tokens[4].parse::<i32>() else {
            continue;
        };
        let year = if normalize_sim_years && raw_year < 1000 {
            resolved_start_year.unwrap_or(raw_year) + raw_year - 1
        } else {
            raw_year
        };
        if sim_start_year.is_none() {
            sim_start_year = Some(year);
        }
        let sim_day_index = compute_sim_day_index(
            year,
            i32::from(day),
            sim_start_year.unwrap_or(year),
            calendar_lookup.as_ref(),
        );

        store.day.push(day);
        store.year.push(year as i16);
        store.sim_day_index.push(sim_day_index);
        store.julian.push(day);
        store.time_of_conc.push(parse_measurement(
            tokens[6],
            tc_out_path,
            line_idx + 1,
            &raw_line,
        )?);
        store.storm_duration.push(parse_measurement(
            tokens[7],
            tc_out_path,
            line_idx + 1,
            &raw_line,
        )?);
        store.storm_peak.push(parse_measurement(
            tokens[8],
            tc_out_path,
            line_idx + 1,
            &raw_line,
        )?);

        row_counter += 1;
        if row_counter % chunk_rows == 0 {
            sink.write_chunk(store.to_chunk())?;
        }
    }

    if store.len() > 0 {
        sink.write_chunk(store.to_chunk())?;
    } else if row_counter == 0 {
        sink.write_chunk(empty_chunk(&schema))?;
    }
    let write_summary = sink.finish()?;
    Ok(TcOutWriteSummary {
        rows_written: write_summary.rows_written,
        row_groups: write_summary.row_groups,
        output_paths: vec![output_path.display().to_string()],
        outlet_channel: Some(outlet_channel),
    })
}

fn find_outlet_channel(tc_out_path: &Path) -> Result<Option<i32>, InterchangeError> {
    let reader = BufReader::new(
        File::open(tc_out_path).map_err(|err| InterchangeError::io(tc_out_path, err))?,
    );
    let mut outlet_channel: Option<i32> = None;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(tc_out_path, err))?;
        let stripped = raw_line.trim();
        if should_skip_line(stripped) {
            continue;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() < 9 || tokens[1] != "C" {
            continue;
        }
        let Ok(channel_id) = tokens[2].parse::<i32>() else {
            continue;
        };
        outlet_channel = Some(outlet_channel.map_or(channel_id, |current| current.max(channel_id)));
    }

    Ok(outlet_channel)
}

fn should_skip_line(line: &str) -> bool {
    line.is_empty() || line.starts_with("Element") || line.starts_with('-')
}

fn parse_measurement(
    token: &str,
    path: &Path,
    line_no: usize,
    raw_line: &str,
) -> Result<f64, InterchangeError> {
    parse_required_float(token).map_err(|message| {
        InterchangeError::parse(path, Some(line_no), message, Some(raw_line.to_string()))
    })
}

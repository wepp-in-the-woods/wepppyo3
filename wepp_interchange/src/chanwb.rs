use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::arrow_support::{BoxedArray, Chunk};

use crate::calendar::{determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{watershed_chanwb_schema, VersionInfo};

const DEFAULT_CHUNK_ROWS: usize = 500_000;

pub fn watershed_chanwb_to_parquet(
    chanwb_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = watershed_chanwb_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    let chunk_size = chunk_rows.unwrap_or(DEFAULT_CHUNK_ROWS);

    let file = File::open(chanwb_path).map_err(|err| InterchangeError::io(chanwb_path, err))?;
    let reader = BufReader::new(file);
    let mut data_section = false;

    let mut years: Vec<i16> = Vec::new();
    let mut simulation_years: Vec<i16> = Vec::new();
    let mut julians: Vec<i16> = Vec::new();
    let mut months: Vec<i8> = Vec::new();
    let mut days: Vec<i8> = Vec::new();
    let mut water_years: Vec<i16> = Vec::new();
    let mut elmt_id: Vec<i32> = Vec::new();
    let mut chan_id: Vec<i32> = Vec::new();
    let mut inflow: Vec<f64> = Vec::new();
    let mut outflow: Vec<f64> = Vec::new();
    let mut storage: Vec<f64> = Vec::new();
    let mut baseflow: Vec<f64> = Vec::new();
    let mut loss: Vec<f64> = Vec::new();
    let mut balance: Vec<f64> = Vec::new();

    let mut row_counter = 0usize;
    for (line_no, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| InterchangeError::io(chanwb_path, err))?;
        let stripped = raw_line.trim();
        if !data_section {
            if stripped.starts_with("Year") && stripped.contains("Elmt_ID") {
                data_section = true;
            }
            continue;
        }
        if stripped.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() != 10 {
            continue;
        }

        let sim_year: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(
                chanwb_path,
                Some(line_no + 1),
                "Invalid simulation year token",
                Some(raw_line.clone()),
            )
        })?;
        let julian: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(
                chanwb_path,
                Some(line_no + 1),
                "Invalid julian token",
                Some(raw_line.clone()),
            )
        })?;
        let elmt: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(
                chanwb_path,
                Some(line_no + 1),
                "Invalid Elmt_ID token",
                Some(raw_line.clone()),
            )
        })?;
        let chan: i32 = tokens[3].parse().map_err(|_| {
            InterchangeError::parse(
                chanwb_path,
                Some(line_no + 1),
                "Invalid Chan_ID token",
                Some(raw_line.clone()),
            )
        })?;

        let year = if start_year.is_some() && sim_year < 1000 {
            start_year.unwrap_or(sim_year) + sim_year - 1
        } else {
            sim_year
        };
        let (month, day_of_month) = julian_to_calendar(year, julian, lookup.as_ref());
        let water_year = determine_wateryear(year, julian);

        years.push(year as i16);
        simulation_years.push(sim_year as i16);
        julians.push(julian as i16);
        months.push(month as i8);
        days.push(day_of_month as i8);
        water_years.push(water_year as i16);
        elmt_id.push(elmt);
        chan_id.push(chan);

        inflow.push(parse_required_float(tokens[4]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        outflow.push(parse_required_float(tokens[5]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        storage.push(parse_required_float(tokens[6]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        baseflow.push(parse_required_float(tokens[7]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        loss.push(parse_required_float(tokens[8]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        balance.push(parse_required_float(tokens[9]).map_err(|msg| {
            InterchangeError::parse(chanwb_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);

        row_counter += 1;
        if row_counter % chunk_size == 0 {
            flush_chunk(
                &mut sink,
                &mut years,
                &mut simulation_years,
                &mut julians,
                &mut months,
                &mut days,
                &mut water_years,
                &mut elmt_id,
                &mut chan_id,
                &mut inflow,
                &mut outflow,
                &mut storage,
                &mut baseflow,
                &mut loss,
                &mut balance,
            )?;
        }
    }

    if row_counter == 0 {
        sink.write_chunk(empty_chunk(&schema))?;
    } else if !years.is_empty() {
        flush_chunk(
            &mut sink,
            &mut years,
            &mut simulation_years,
            &mut julians,
            &mut months,
            &mut days,
            &mut water_years,
            &mut elmt_id,
            &mut chan_id,
            &mut inflow,
            &mut outflow,
            &mut storage,
            &mut baseflow,
            &mut loss,
            &mut balance,
        )?;
    }

    sink.finish()
}

#[allow(clippy::too_many_arguments)]
fn flush_chunk(
    sink: &mut ParquetSink,
    years: &mut Vec<i16>,
    simulation_years: &mut Vec<i16>,
    julians: &mut Vec<i16>,
    months: &mut Vec<i8>,
    days: &mut Vec<i8>,
    water_years: &mut Vec<i16>,
    elmt_id: &mut Vec<i32>,
    chan_id: &mut Vec<i32>,
    inflow: &mut Vec<f64>,
    outflow: &mut Vec<f64>,
    storage: &mut Vec<f64>,
    baseflow: &mut Vec<f64>,
    loss: &mut Vec<f64>,
    balance: &mut Vec<f64>,
) -> Result<(), InterchangeError> {
    if years.is_empty() {
        return Ok(());
    }
    let chunk = Chunk::new(vec![
        arrow_array::Int16Array::from(std::mem::take(years)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(simulation_years)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(julians)).boxed(),
        arrow_array::Int8Array::from(std::mem::take(months)).boxed(),
        arrow_array::Int8Array::from(std::mem::take(days)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(water_years)).boxed(),
        arrow_array::Int32Array::from(std::mem::take(elmt_id)).boxed(),
        arrow_array::Int32Array::from(std::mem::take(chan_id)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(inflow)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(outflow)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(storage)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(baseflow)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(loss)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(balance)).boxed(),
    ]);
    sink.write_chunk(chunk)?;
    Ok(())
}

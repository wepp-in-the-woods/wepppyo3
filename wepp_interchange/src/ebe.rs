use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arrow2::array::PrimitiveArray;
use arrow2::chunk::Chunk;

use crate::calendar::{
    compute_sim_day_index, determine_wateryear, load_cli_calendar, CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{watershed_ebe_schema, VersionInfo};

const DEFAULT_CHUNK_ROWS: usize = 250_000;

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
        } else if !lookup.by_year.is_empty() {
            for days in lookup.by_year.values() {
                for (idx, (m, d)) in days.iter().enumerate() {
                    if *m == month && *d == day {
                        return Ok((idx + 1) as i32);
                    }
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

    let date =
        chrono_date(year, month, day).map_err(|message| InterchangeError::Calendar { message })?;
    Ok(date)
}

fn chrono_date(year: i32, month: i32, day: i32) -> Result<i32, String> {
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

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn watershed_ebe_to_parquet(
    ebe_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
    legacy_element_id: Option<i32>,
    chunk_rows: Option<usize>,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = watershed_ebe_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    let chunk_size = chunk_rows.unwrap_or(DEFAULT_CHUNK_ROWS);

    let calendar_start_year = lookup
        .as_ref()
        .and_then(|cal| cal.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    let file = File::open(ebe_path).map_err(|err| InterchangeError::io(ebe_path, err))?;
    let reader = BufReader::new(file);

    let mut years: Vec<i16> = Vec::new();
    let mut sim_day_index: Vec<i32> = Vec::new();
    let mut simulation_year: Vec<i16> = Vec::new();
    let mut months: Vec<i8> = Vec::new();
    let mut days: Vec<i8> = Vec::new();
    let mut julians: Vec<i16> = Vec::new();
    let mut water_years: Vec<i16> = Vec::new();
    let mut precip: Vec<f64> = Vec::new();
    let mut runoff_volume: Vec<f64> = Vec::new();
    let mut peak_runoff: Vec<f64> = Vec::new();
    let mut sediment_yield: Vec<f64> = Vec::new();
    let mut soluble_pollutant: Vec<f64> = Vec::new();
    let mut particulate_pollutant: Vec<f64> = Vec::new();
    let mut total_pollutant: Vec<f64> = Vec::new();
    let mut element_id: Vec<Option<i32>> = Vec::new();

    let mut row_counter = 0usize;

    for (line_no, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| InterchangeError::io(ebe_path, err))?;
        let stripped = raw_line.trim();
        if stripped.is_empty()
            || stripped.starts_with("WATERSHED")
            || stripped.starts_with('(')
            || stripped.starts_with("Day")
            || stripped.starts_with('-')
            || stripped.starts_with("Month")
            || stripped.starts_with("Year")
        {
            continue;
        }

        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() != 10 && tokens.len() != 11 {
            continue;
        }

        let day_of_month: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid day token",
                Some(raw_line.clone()),
            )
        })?;
        let month: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid month token",
                Some(raw_line.clone()),
            )
        })?;
        let sim_year: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid simulation year token",
                Some(raw_line.clone()),
            )
        })?;

        let year = if normalize_sim_years {
            if sim_year < 1000 {
                resolved_start_year.unwrap_or(sim_year) + sim_year - 1
            } else {
                sim_year
            }
        } else {
            sim_year
        };
        if sim_start_year.is_none() {
            sim_start_year = Some(year);
        }

        let julian = calendar_day_to_julian(year, month, day_of_month, lookup.as_ref())?;
        let sim_day = compute_sim_day_index(
            year,
            julian,
            sim_start_year.unwrap_or(year),
            lookup.as_ref(),
        );
        let water_year = determine_wateryear(year, julian);

        years.push(year as i16);
        sim_day_index.push(sim_day);
        simulation_year.push(sim_year as i16);
        months.push(month as i8);
        days.push(day_of_month as i8);
        julians.push(julian as i16);
        water_years.push(water_year as i16);

        precip.push(parse_required_float(tokens[3]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        runoff_volume.push(parse_required_float(tokens[4]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        peak_runoff.push(parse_required_float(tokens[5]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        sediment_yield.push(parse_required_float(tokens[6]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        soluble_pollutant.push(parse_required_float(tokens[7]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        particulate_pollutant.push(parse_required_float(tokens[8]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        total_pollutant.push(parse_required_float(tokens[9]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);

        let element_value = if tokens.len() == 11 {
            Some(tokens[10].parse::<i32>().map_err(|_| {
                InterchangeError::parse(
                    ebe_path,
                    Some(line_no + 1),
                    "Invalid element id token",
                    Some(raw_line.clone()),
                )
            })?)
        } else {
            legacy_element_id
        };
        element_id.push(element_value);

        row_counter += 1;
        if row_counter % chunk_size == 0 {
            flush_chunk(
                &mut sink,
                &schema,
                &mut years,
                &mut sim_day_index,
                &mut simulation_year,
                &mut months,
                &mut days,
                &mut julians,
                &mut water_years,
                &mut precip,
                &mut runoff_volume,
                &mut peak_runoff,
                &mut sediment_yield,
                &mut soluble_pollutant,
                &mut particulate_pollutant,
                &mut total_pollutant,
                &mut element_id,
            )?;
        }
    }

    if row_counter == 0 {
        sink.write_chunk(empty_chunk(&schema))?;
    } else if !years.is_empty() {
        flush_chunk(
            &mut sink,
            &schema,
            &mut years,
            &mut sim_day_index,
            &mut simulation_year,
            &mut months,
            &mut days,
            &mut julians,
            &mut water_years,
            &mut precip,
            &mut runoff_volume,
            &mut peak_runoff,
            &mut sediment_yield,
            &mut soluble_pollutant,
            &mut particulate_pollutant,
            &mut total_pollutant,
            &mut element_id,
        )?;
    }

    sink.finish()
}

#[allow(clippy::too_many_arguments)]
fn flush_chunk(
    sink: &mut ParquetSink,
    _schema: &arrow2::datatypes::Schema,
    years: &mut Vec<i16>,
    sim_day_index: &mut Vec<i32>,
    simulation_year: &mut Vec<i16>,
    months: &mut Vec<i8>,
    days: &mut Vec<i8>,
    julians: &mut Vec<i16>,
    water_years: &mut Vec<i16>,
    precip: &mut Vec<f64>,
    runoff_volume: &mut Vec<f64>,
    peak_runoff: &mut Vec<f64>,
    sediment_yield: &mut Vec<f64>,
    soluble_pollutant: &mut Vec<f64>,
    particulate_pollutant: &mut Vec<f64>,
    total_pollutant: &mut Vec<f64>,
    element_id: &mut Vec<Option<i32>>,
) -> Result<(), InterchangeError> {
    if years.is_empty() {
        return Ok(());
    }
    let chunk = Chunk::new(vec![
        PrimitiveArray::<i16>::from_vec(std::mem::take(years)).boxed(),
        PrimitiveArray::<i32>::from_vec(std::mem::take(sim_day_index)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(simulation_year)).boxed(),
        PrimitiveArray::<i8>::from_vec(std::mem::take(months)).boxed(),
        PrimitiveArray::<i8>::from_vec(std::mem::take(days)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(julians)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(water_years)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(precip)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(runoff_volume)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(peak_runoff)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(sediment_yield)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(soluble_pollutant)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(particulate_pollutant)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(total_pollutant)).boxed(),
        PrimitiveArray::<i32>::from(std::mem::take(element_id)).boxed(),
    ]);
    sink.write_chunk(chunk)?;
    Ok(())
}

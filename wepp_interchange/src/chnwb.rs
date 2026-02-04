use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arrow2::array::PrimitiveArray;
use arrow2::chunk::Chunk;

use crate::calendar::{determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{watershed_chnwb_schema, VersionInfo};

const DEFAULT_CHUNK_ROWS: usize = 250_000;

pub fn watershed_chnwb_to_parquet(
    chnwb_path: &Path,
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
    let schema = watershed_chnwb_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    let chunk_size = chunk_rows.unwrap_or(DEFAULT_CHUNK_ROWS);

    let file = File::open(chnwb_path).map_err(|err| InterchangeError::io(chnwb_path, err))?;
    let reader = BufReader::new(file);

    let mut header_found = false;
    let mut data_offset = 0usize;

    let mut wepp_id: Vec<i32> = Vec::new();
    let mut julian: Vec<i16> = Vec::new();
    let mut year: Vec<i16> = Vec::new();
    let mut simulation_year: Vec<i16> = Vec::new();
    let mut month: Vec<i8> = Vec::new();
    let mut day_of_month: Vec<i8> = Vec::new();
    let mut water_year: Vec<i16> = Vec::new();
    let mut ofe: Vec<i16> = Vec::new();
    let mut j_val: Vec<i16> = Vec::new();
    let mut y_val: Vec<i16> = Vec::new();

    let mut p: Vec<f64> = Vec::new();
    let mut rm: Vec<f64> = Vec::new();
    let mut q: Vec<f64> = Vec::new();
    let mut ep: Vec<f64> = Vec::new();
    let mut es: Vec<f64> = Vec::new();
    let mut er: Vec<f64> = Vec::new();
    let mut dp: Vec<f64> = Vec::new();
    let mut upstrmq: Vec<f64> = Vec::new();
    let mut subrin: Vec<f64> = Vec::new();
    let mut latqcc: Vec<f64> = Vec::new();
    let mut total_soil_water: Vec<f64> = Vec::new();
    let mut frozwt: Vec<f64> = Vec::new();
    let mut snow_water: Vec<f64> = Vec::new();
    let mut qofe: Vec<f64> = Vec::new();
    let mut tile: Vec<f64> = Vec::new();
    let mut irr: Vec<f64> = Vec::new();
    let mut surf: Vec<f64> = Vec::new();
    let mut base: Vec<f64> = Vec::new();
    let mut area: Vec<f64> = Vec::new();

    let mut row_counter = 0usize;

    for (idx, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| InterchangeError::io(chnwb_path, err))?;
        let stripped = raw_line.trim();
        if !header_found {
            if stripped.starts_with("OFE") {
                header_found = true;
                data_offset = idx + 3;
            }
            continue;
        }
        if idx < data_offset {
            continue;
        }
        if stripped.is_empty() || stripped.starts_with('-') {
            continue;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() != 22 {
            continue;
        }

        let ofe_val: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(
                chnwb_path,
                Some(idx + 1),
                "Invalid OFE token",
                Some(raw_line.clone()),
            )
        })?;
        let julian_val: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(
                chnwb_path,
                Some(idx + 1),
                "Invalid julian token",
                Some(raw_line.clone()),
            )
        })?;
        let sim_year: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(
                chnwb_path,
                Some(idx + 1),
                "Invalid simulation year token",
                Some(raw_line.clone()),
            )
        })?;

        let year_val = if start_year.is_some() && sim_year < 1000 {
            start_year.unwrap_or(sim_year) + sim_year - 1
        } else {
            sim_year
        };
        let (month_val, day_val) = julian_to_calendar(year_val, julian_val, lookup.as_ref());
        let water_year_val = determine_wateryear(year_val, julian_val);

        wepp_id.push(ofe_val);
        julian.push(julian_val as i16);
        year.push(year_val as i16);
        simulation_year.push(sim_year as i16);
        month.push(month_val as i8);
        day_of_month.push(day_val as i8);
        water_year.push(water_year_val as i16);
        ofe.push(ofe_val as i16);
        j_val.push(julian_val as i16);
        y_val.push(sim_year as i16);

        let mut measurements = tokens[3..].iter();
        let parse_value = |token: &str| {
            parse_required_float(token).map_err(|msg| {
                InterchangeError::parse(chnwb_path, Some(idx + 1), msg, Some(raw_line.clone()))
            })
        };
        p.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        rm.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        q.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        ep.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        es.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        er.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        dp.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        upstrmq.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        subrin.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        latqcc.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        total_soil_water.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        frozwt.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        snow_water.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        qofe.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        tile.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        irr.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        surf.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        base.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);
        area.push(parse_value(next_token(
            &mut measurements,
            chnwb_path,
            idx,
            &raw_line,
        )?)?);

        row_counter += 1;
        if row_counter % chunk_size == 0 {
            flush_chunk(
                &mut sink,
                &mut wepp_id,
                &mut julian,
                &mut year,
                &mut simulation_year,
                &mut month,
                &mut day_of_month,
                &mut water_year,
                &mut ofe,
                &mut j_val,
                &mut y_val,
                &mut p,
                &mut rm,
                &mut q,
                &mut ep,
                &mut es,
                &mut er,
                &mut dp,
                &mut upstrmq,
                &mut subrin,
                &mut latqcc,
                &mut total_soil_water,
                &mut frozwt,
                &mut snow_water,
                &mut qofe,
                &mut tile,
                &mut irr,
                &mut surf,
                &mut base,
                &mut area,
            )?;
        }
    }

    if row_counter == 0 {
        sink.write_chunk(empty_chunk(&schema))?;
    } else if !wepp_id.is_empty() {
        flush_chunk(
            &mut sink,
            &mut wepp_id,
            &mut julian,
            &mut year,
            &mut simulation_year,
            &mut month,
            &mut day_of_month,
            &mut water_year,
            &mut ofe,
            &mut j_val,
            &mut y_val,
            &mut p,
            &mut rm,
            &mut q,
            &mut ep,
            &mut es,
            &mut er,
            &mut dp,
            &mut upstrmq,
            &mut subrin,
            &mut latqcc,
            &mut total_soil_water,
            &mut frozwt,
            &mut snow_water,
            &mut qofe,
            &mut tile,
            &mut irr,
            &mut surf,
            &mut base,
            &mut area,
        )?;
    }

    sink.finish()
}

fn next_token<'a>(
    iter: &mut impl Iterator<Item = &'a &'a str>,
    path: &Path,
    line_no: usize,
    raw_line: &str,
) -> Result<&'a str, InterchangeError> {
    iter.next().copied().ok_or_else(|| {
        InterchangeError::parse(
            path,
            Some(line_no + 1),
            "Missing measurement token",
            Some(raw_line.to_string()),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn flush_chunk(
    sink: &mut ParquetSink,
    wepp_id: &mut Vec<i32>,
    julian: &mut Vec<i16>,
    year: &mut Vec<i16>,
    simulation_year: &mut Vec<i16>,
    month: &mut Vec<i8>,
    day_of_month: &mut Vec<i8>,
    water_year: &mut Vec<i16>,
    ofe: &mut Vec<i16>,
    j_val: &mut Vec<i16>,
    y_val: &mut Vec<i16>,
    p: &mut Vec<f64>,
    rm: &mut Vec<f64>,
    q: &mut Vec<f64>,
    ep: &mut Vec<f64>,
    es: &mut Vec<f64>,
    er: &mut Vec<f64>,
    dp: &mut Vec<f64>,
    upstrmq: &mut Vec<f64>,
    subrin: &mut Vec<f64>,
    latqcc: &mut Vec<f64>,
    total_soil_water: &mut Vec<f64>,
    frozwt: &mut Vec<f64>,
    snow_water: &mut Vec<f64>,
    qofe: &mut Vec<f64>,
    tile: &mut Vec<f64>,
    irr: &mut Vec<f64>,
    surf: &mut Vec<f64>,
    base: &mut Vec<f64>,
    area: &mut Vec<f64>,
) -> Result<(), InterchangeError> {
    if wepp_id.is_empty() {
        return Ok(());
    }
    let chunk = Chunk::new(vec![
        PrimitiveArray::<i32>::from_vec(std::mem::take(wepp_id)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(julian)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(year)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(simulation_year)).boxed(),
        PrimitiveArray::<i8>::from_vec(std::mem::take(month)).boxed(),
        PrimitiveArray::<i8>::from_vec(std::mem::take(day_of_month)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(water_year)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(ofe)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(j_val)).boxed(),
        PrimitiveArray::<i16>::from_vec(std::mem::take(y_val)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(p)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(rm)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(q)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(ep)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(es)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(er)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(dp)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(upstrmq)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(subrin)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(latqcc)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(total_soil_water)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(frozwt)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(snow_water)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(qofe)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(tile)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(irr)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(surf)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(base)).boxed(),
        PrimitiveArray::<f64>::from_vec(std::mem::take(area)).boxed(),
    ]);
    sink.write_chunk(chunk)?;
    Ok(())
}

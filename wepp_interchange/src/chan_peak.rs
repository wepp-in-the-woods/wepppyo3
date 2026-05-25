use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::arrow_support::{BoxedArray, Chunk};
use arrow_array::{Array, Float64Array, Int16Array, Int32Array, Int8Array};
use arrow_schema::{DataType, Schema};

use crate::calendar::{determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::parse_float_loose;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{field_with_meta, schema_with_version, VersionInfo};

const CHUNK_SIZE: usize = 500_000;

pub fn chan_peak_schema(version: &VersionInfo) -> Schema {
    let schema = Schema::new(vec![
        field_with_meta("year", DataType::Int16, None, Some("Calendar year")),
        field_with_meta(
            "simulation_year",
            DataType::Int16,
            None,
            Some("Simulation year from chan.out"),
        ),
        field_with_meta(
            "julian",
            DataType::Int16,
            None,
            Some("Julian day reported by WEPP"),
        ),
        field_with_meta(
            "month",
            DataType::Int8,
            None,
            Some("Calendar month derived from Julian day"),
        ),
        field_with_meta(
            "day_of_month",
            DataType::Int8,
            None,
            Some("Calendar day-of-month derived from Julian day"),
        ),
        field_with_meta(
            "water_year",
            DataType::Int16,
            None,
            Some("Water year computed from Julian day"),
        ),
        field_with_meta(
            "Elmt_ID",
            DataType::Int32,
            None,
            Some("Channel element identifier"),
        ),
        field_with_meta(
            "Chan_ID",
            DataType::Int32,
            None,
            Some("Channel ID reported by WEPP"),
        ),
        field_with_meta(
            "Time (s)",
            DataType::Float64,
            Some("s"),
            Some("Time to peak discharge"),
        ),
        field_with_meta(
            "Peak_Discharge (m^3/s)",
            DataType::Float64,
            Some("m^3/s"),
            Some("Peak discharge within the reporting interval"),
        ),
    ]);
    schema_with_version(schema, version)
}

struct ChanPeakStore {
    year: Vec<i16>,
    simulation_year: Vec<i16>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    elmt_id: Vec<i32>,
    chan_id: Vec<i32>,
    time_s: Vec<f64>,
    peak_q: Vec<f64>,
}

impl ChanPeakStore {
    fn new() -> Self {
        Self {
            year: Vec::new(),
            simulation_year: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            elmt_id: Vec::new(),
            chan_id: Vec::new(),
            time_s: Vec::new(),
            peak_q: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.year.len()
    }

    fn to_chunk(&mut self, _schema: &Schema) -> Chunk<Box<dyn Array>> {
        let arrays: Vec<Box<dyn Array>> = vec![
            Int16Array::from(std::mem::take(&mut self.year)).boxed(),
            Int16Array::from(std::mem::take(&mut self.simulation_year)).boxed(),
            Int16Array::from(std::mem::take(&mut self.julian)).boxed(),
            Int8Array::from(std::mem::take(&mut self.month)).boxed(),
            Int8Array::from(std::mem::take(&mut self.day_of_month)).boxed(),
            Int16Array::from(std::mem::take(&mut self.water_year)).boxed(),
            Int32Array::from(std::mem::take(&mut self.elmt_id)).boxed(),
            Int32Array::from(std::mem::take(&mut self.chan_id)).boxed(),
            Float64Array::from(std::mem::take(&mut self.time_s)).boxed(),
            Float64Array::from(std::mem::take(&mut self.peak_q)).boxed(),
        ];
        Chunk::new(arrays)
    }
}

pub fn watershed_chan_peak_to_parquet(
    chan_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
    chunk_rows: Option<usize>,
) -> Result<WriteSummary, InterchangeError> {
    let calendar_lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };

    let schema = chan_peak_schema(version);
    let mut writer = ParquetSink::try_new(output_path, schema.clone())?;
    let mut store = ChanPeakStore::new();
    let chunk_rows = chunk_rows.unwrap_or(CHUNK_SIZE);

    let reader =
        BufReader::new(File::open(chan_path).map_err(|err| InterchangeError::io(chan_path, err))?);
    let mut line_reader = LineReader::new(reader);
    let mut data_section = false;
    let mut row_counter: usize = 0;

    while let Some((line_no, line)) = line_reader.next_line()? {
        let stripped = line.trim();
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
        if tokens.len() != 6 {
            continue;
        }

        let sim_year = tokens[0].parse::<i16>().map_err(|_| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid simulation year",
                Some(line.clone()),
            )
        })?;
        let julian = tokens[1].parse::<i16>().map_err(|_| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid julian day",
                Some(line.clone()),
            )
        })?;
        let elmt_id = tokens[2].parse::<i32>().map_err(|_| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid element id",
                Some(line.clone()),
            )
        })?;
        let chan_id = tokens[3].parse::<i32>().map_err(|_| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid channel id",
                Some(line.clone()),
            )
        })?;
        let time_s = parse_float_loose(tokens[4]).ok_or_else(|| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid time value",
                Some(line.clone()),
            )
        })?;
        let peak_q = parse_float_loose(tokens[5]).ok_or_else(|| {
            InterchangeError::parse(
                chan_path,
                Some(line_no),
                "Invalid peak discharge value",
                Some(line.clone()),
            )
        })?;

        let year = if let Some(start_year) = start_year {
            if sim_year < 1000 {
                start_year + sim_year as i32 - 1
            } else {
                sim_year as i32
            }
        } else {
            sim_year as i32
        };

        let (month, day_of_month) =
            julian_to_calendar(year, julian as i32, calendar_lookup.as_ref());
        let water_year = determine_wateryear(year, julian as i32);

        store.year.push(year as i16);
        store.simulation_year.push(sim_year);
        store.julian.push(julian);
        store.month.push(month as i8);
        store.day_of_month.push(day_of_month as i8);
        store.water_year.push(water_year as i16);
        store.elmt_id.push(elmt_id);
        store.chan_id.push(chan_id);
        store.time_s.push(time_s);
        store.peak_q.push(peak_q);

        row_counter += 1;
        if row_counter % chunk_rows == 0 {
            let chunk = store.to_chunk(&schema);
            writer.write_chunk(chunk)?;
        }
    }

    if store.len() > 0 {
        let chunk = store.to_chunk(&schema);
        writer.write_chunk(chunk)?;
    } else if row_counter == 0 {
        writer.write_chunk(empty_chunk(&schema))?;
    }

    writer.finish()
}

struct LineReader<R: BufRead> {
    reader: R,
    line_number: usize,
}

impl<R: BufRead> LineReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            line_number: 0,
        }
    }

    fn next_line(&mut self) -> Result<Option<(usize, String)>, InterchangeError> {
        let mut buffer = String::new();
        let bytes = self
            .reader
            .read_line(&mut buffer)
            .map_err(|err| InterchangeError::io("chan stream", err))?;
        if bytes == 0 {
            return Ok(None);
        }
        self.line_number += 1;
        if buffer.ends_with('\n') {
            buffer.pop();
            if buffer.ends_with('\r') {
                buffer.pop();
            }
        }
        Ok(Some((self.line_number, buffer)))
    }
}

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow2::array::{Array, PrimitiveArray};
use arrow2::chunk::Chunk;
use arrow2::datatypes::{DataType, Field, Schema};
use flate2::read::GzDecoder;

use crate::arrays::dictionary_array_from_strings;
use crate::calendar::{compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::floats::{parse_float_loose, parse_required_float, tokenize_numeric_line};
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{field_with_meta, schema_with_version, VersionInfo};

const EVENT_CHUNK_SIZE: usize = 250_000;

#[derive(Debug)]
struct PassMetadata {
    version: f64,
    nhill: i32,
    max_years: i32,
    begin_year: i32,
    npart: usize,
    hillslope_ids: Vec<i32>,
    climate_files: Vec<String>,
    particle_diams: Vec<Vec<f64>>,
    areas: Vec<f64>,
    srp: Vec<f64>,
    slfp: Vec<f64>,
    bfp: Vec<f64>,
    scp: Vec<f64>,
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
        let bytes = self.reader.read_line(&mut buffer).map_err(|err| {
            InterchangeError::io("pass stream", err)
        })?;
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

struct PassReader<R: BufRead> {
    line_reader: LineReader<R>,
    buffer: Vec<f64>,
    path: PathBuf,
}

impl<R: BufRead> PassReader<R> {
    fn new(reader: R, path: &Path) -> Self {
        Self {
            line_reader: LineReader::new(reader),
            buffer: Vec::new(),
            path: path.to_path_buf(),
        }
    }

    fn read_header(&mut self) -> Result<Vec<String>, InterchangeError> {
        let mut header_lines: Vec<String> = Vec::new();
        while let Some((_line_no, line)) = self.line_reader.next_line()? {
            if line.trim() == "BEGIN HILLSLOPE HYDROLOGY AND SEDIMENT INFORMATION" {
                return Ok(header_lines);
            }
            header_lines.push(line);
        }
        Err(InterchangeError::parse(
            &self.path,
            None,
            "Unable to locate beginning of hydrology section in pass file.",
            None,
        ))
    }

    fn next_event_header(&mut self) -> Result<Option<(String, i32, i32, usize, String)>, InterchangeError> {
        while let Some((line_no, line)) = self.line_reader.next_line()? {
            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }
            if let Some((label, year, julian)) = parse_event_header(stripped) {
                return Ok(Some((label, year, julian, line_no, line)));
            }
            let numeric = tokenize_numeric_line(stripped);
            if !numeric.is_empty() {
                self.buffer.extend(numeric);
                continue;
            }
            return Err(InterchangeError::parse(
                &self.path,
                Some(line_no),
                "Unrecognized event header line",
                Some(truncate_line(&line)),
            ));
        }
        Ok(None)
    }

    fn read_values(&mut self, count: usize) -> Result<Vec<f64>, InterchangeError> {
        let mut values: Vec<f64> = Vec::with_capacity(count);
        while values.len() < count {
            if !self.buffer.is_empty() {
                let take = std::cmp::min(count - values.len(), self.buffer.len());
                values.extend(self.buffer.drain(0..take));
                continue;
            }

            let next = self.line_reader.next_line()?;
            let (line_no, line) = match next {
                Some(val) => val,
                None => {
                    return Err(InterchangeError::parse(
                        &self.path,
                        None,
                        "Unexpected end of pass file while collecting numeric values.",
                        None,
                    ));
                }
            };

            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }
            let numeric = tokenize_numeric_line(stripped);
            if numeric.is_empty() {
                return Err(InterchangeError::parse(
                    &self.path,
                    Some(line_no),
                    "Expected numeric values for pass event",
                    Some(truncate_line(&line)),
                ));
            }
            self.buffer.extend(numeric);
        }
        Ok(values)
    }
}

fn parse_event_header(line: &str) -> Option<(String, i32, i32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let year = parts[parts.len() - 2].parse::<i32>().ok()?;
    let julian = parts[parts.len() - 1].parse::<i32>().ok()?;
    let label = parts[..parts.len() - 2].join(" ");
    Some((label, year, julian))
}

fn truncate_line(line: &str) -> String {
    const LIMIT: usize = 160;
    if line.len() <= LIMIT {
        line.to_string()
    } else {
        format!("{}...", &line[..LIMIT])
    }
}

fn parse_metadata(header_lines: &[String], path: &Path) -> Result<PassMetadata, InterchangeError> {
    let mut version: Option<f64> = None;
    let mut nhill: Option<i32> = None;
    let mut max_years: Option<i32> = None;
    let mut begin_year: Option<i32> = None;

    let mut hillslope_ids = Vec::new();
    let mut climate_files = Vec::new();
    let mut particle_diams: Vec<Vec<f64>> = Vec::new();
    let mut areas = Vec::new();
    let mut srp = Vec::new();
    let mut slfp = Vec::new();
    let mut bfp = Vec::new();
    let mut scp = Vec::new();

    for line in header_lines {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        if stripped.ends_with("--> VERSION NUMBER") {
            if let Some(token) = stripped.split_whitespace().next() {
                version = parse_float_loose(token);
            }
        } else if stripped.ends_with("NUMBER OF UNIQUE HILLSLOPES IN WATERSHED") {
            nhill = stripped.split_whitespace().next().and_then(|t| t.parse::<i32>().ok());
        } else if stripped.ends_with("WATERSHED MAXIMUM SIMULATION TIME (YEARS)") {
            max_years = stripped.split_whitespace().next().and_then(|t| t.parse::<i32>().ok());
        } else if stripped.ends_with("BEGINNING YEAR OF WATERSHED CLIMATE FILE") {
            begin_year = stripped.split_whitespace().next().and_then(|t| t.parse::<i32>().ok());
        } else if stripped.starts_with("HILLSLOPE") {
            let parts: Vec<&str> = stripped.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            if !parts[1].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            if parts.len() < 10 {
                return Err(InterchangeError::parse(
                    path,
                    None,
                    format!("Unexpected HILLSLOPE metadata line: {stripped}"),
                    None,
                ));
            }
            let wepp_id = parts[1].parse::<i32>().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid hillslope id", Some(truncate_line(stripped)))
            })?;
            let climate_file = parts[2].to_string();

            let srp_val = parse_required_float(parts[parts.len() - 4]).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
            })?;
            let slfp_val = parse_required_float(parts[parts.len() - 3]).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
            })?;
            let bfp_val = parse_required_float(parts[parts.len() - 2]).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
            })?;
            let scp_val = parse_required_float(parts[parts.len() - 1]).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
            })?;
            let area_val = parse_required_float(parts[parts.len() - 5]).map_err(|msg| {
                InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
            })?;
            let dia_tokens = &parts[3..parts.len() - 5];
            let mut dias = Vec::with_capacity(dia_tokens.len());
            for token in dia_tokens {
                dias.push(parse_required_float(token).map_err(|msg| {
                    InterchangeError::parse(path, None, msg, Some(truncate_line(stripped)))
                })?);
            }

            hillslope_ids.push(wepp_id);
            climate_files.push(climate_file);
            particle_diams.push(dias);
            areas.push(area_val);
            srp.push(srp_val);
            slfp.push(slfp_val);
            bfp.push(bfp_val);
            scp.push(scp_val);
        }
    }

    let version = version.ok_or_else(|| {
        InterchangeError::parse(path, None, "Missing version metadata in pass file header.", None)
    })?;
    let nhill = nhill.ok_or_else(|| {
        InterchangeError::parse(path, None, "Missing hillslope count metadata in pass file header.", None)
    })?;
    let max_years = max_years.ok_or_else(|| {
        InterchangeError::parse(path, None, "Missing max years metadata in pass file header.", None)
    })?;
    let begin_year = begin_year.ok_or_else(|| {
        InterchangeError::parse(path, None, "Missing begin year metadata in pass file header.", None)
    })?;

    if hillslope_ids.len() != nhill as usize {
        return Err(InterchangeError::parse(
            path,
            None,
            "Mismatch between declared hillslope count and metadata lines.",
            None,
        ));
    }

    let npart = particle_diams.first().map(|row| row.len()).unwrap_or(0);

    Ok(PassMetadata {
        version,
        nhill,
        max_years,
        begin_year,
        npart,
        hillslope_ids,
        climate_files,
        particle_diams,
        areas,
        srp,
        slfp,
        bfp,
        scp,
    })
}

fn build_event_schema(meta: &PassMetadata, version: &VersionInfo) -> Schema {
    let mut fields: Vec<Field> = vec![
        Field::new("event", DataType::Utf8, true),
        Field::new("year", DataType::Int16, true),
        Field::new("sim_day_index", DataType::Int32, true),
        Field::new("julian", DataType::Int16, true),
        Field::new("month", DataType::Int8, true),
        Field::new("day_of_month", DataType::Int8, true),
        Field::new("water_year", DataType::Int16, true),
        Field::new("wepp_id", DataType::Int32, true),
        Field::new("dur", DataType::Float64, true),
        Field::new("tcs", DataType::Float64, true),
        Field::new("oalpha", DataType::Float64, true),
        Field::new("runoff", DataType::Float64, true),
        Field::new("runvol", DataType::Float64, true),
        Field::new("sbrunf", DataType::Float64, true),
        Field::new("sbrunv", DataType::Float64, true),
        Field::new("drainq", DataType::Float64, true),
        Field::new("drrunv", DataType::Float64, true),
        Field::new("peakro", DataType::Float64, true),
        Field::new("tdet", DataType::Float64, true),
        Field::new("tdep", DataType::Float64, true),
        Field::new("gwbfv", DataType::Float64, true),
        Field::new("gwdsv", DataType::Float64, true),
    ];

    for idx in 0..meta.npart {
        fields.push(Field::new(format!("sedcon_{}", idx + 1), DataType::Float64, true));
    }
    for idx in 0..meta.npart {
        fields.push(Field::new(format!("frcflw_{}", idx + 1), DataType::Float64, true));
    }

    let mut schema = Schema::from(fields);
    let mut metadata = std::mem::take(&mut schema.metadata);
    metadata.insert("version".to_string(), meta.version.to_string());
    metadata.insert("nhill".to_string(), meta.nhill.to_string());
    metadata.insert("max_years".to_string(), meta.max_years.to_string());
    metadata.insert("begin_year".to_string(), meta.begin_year.to_string());
    metadata.insert("npart".to_string(), meta.npart.to_string());
    schema.metadata = metadata;

    schema_with_version(schema, version)
}

fn build_metadata_schema(meta: &PassMetadata, version: &VersionInfo) -> Schema {
    let mut fields: Vec<Field> = vec![
        Field::new("wepp_id", DataType::Int32, true),
        Field::new("climate_file", DataType::Utf8, true),
        field_with_meta("area", DataType::Float64, Some("m^2"), None),
        field_with_meta("srp", DataType::Float64, Some("mg/L"), None),
        field_with_meta("slfp", DataType::Float64, Some("mg/L"), None),
        field_with_meta("bfp", DataType::Float64, Some("mg/L"), None),
        field_with_meta("scp", DataType::Float64, Some("mg/kg"), None),
    ];

    for idx in 0..meta.npart {
        fields.push(field_with_meta(
            &format!("dia_{}", idx + 1),
            DataType::Float64,
            Some("m"),
            None,
        ));
    }

    let mut schema = Schema::from(fields);
    let mut metadata = std::mem::take(&mut schema.metadata);
    metadata.insert("version".to_string(), meta.version.to_string());
    metadata.insert("nhill".to_string(), meta.nhill.to_string());
    metadata.insert("max_years".to_string(), meta.max_years.to_string());
    metadata.insert("begin_year".to_string(), meta.begin_year.to_string());
    metadata.insert("npart".to_string(), meta.npart.to_string());
    schema.metadata = metadata;

    schema_with_version(schema, version)
}

struct EventStore {
    event: Vec<String>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    wepp_id: Vec<i32>,
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
    gwbfv: Vec<f64>,
    gwdsv: Vec<f64>,
    sedcon: Vec<Vec<f64>>,
    frcflw: Vec<Vec<f64>>,
}

impl EventStore {
    fn new(npart: usize) -> Self {
        Self {
            event: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            wepp_id: Vec::new(),
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
            gwbfv: Vec::new(),
            gwdsv: Vec::new(),
            sedcon: vec![Vec::new(); npart],
            frcflw: vec![Vec::new(); npart],
        }
    }

    fn len(&self) -> usize {
        self.event.len()
    }

    fn clear(&mut self) {
        self.event.clear();
        self.year.clear();
        self.sim_day_index.clear();
        self.julian.clear();
        self.month.clear();
        self.day_of_month.clear();
        self.water_year.clear();
        self.wepp_id.clear();
        self.dur.clear();
        self.tcs.clear();
        self.oalpha.clear();
        self.runoff.clear();
        self.runvol.clear();
        self.sbrunf.clear();
        self.sbrunv.clear();
        self.drainq.clear();
        self.drrunv.clear();
        self.peakro.clear();
        self.tdet.clear();
        self.tdep.clear();
        self.gwbfv.clear();
        self.gwdsv.clear();
        for col in &mut self.sedcon {
            col.clear();
        }
        for col in &mut self.frcflw {
            col.clear();
        }
    }

    fn to_chunk(&mut self, schema: &Schema) -> Chunk<Box<dyn Array>> {
        let mut arrays: Vec<Box<dyn Array>> = Vec::with_capacity(schema.fields.len());
        let event_values = std::mem::take(&mut self.event);
        let event_array = dictionary_array_from_strings(event_values)
            .expect("dictionary encoding for events");
        arrays.push(event_array.boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.year)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.sim_day_index)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.julian)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.month)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.day_of_month)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.water_year)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.wepp_id)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.dur)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.tcs)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.oalpha)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.runoff)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.runvol)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.sbrunf)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.sbrunv)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.drainq)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.drrunv)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.peakro)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.tdet)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.tdep)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.gwbfv)).boxed());
        arrays.push(PrimitiveArray::from_vec(std::mem::take(&mut self.gwdsv)).boxed());

        for col in &mut self.sedcon {
            arrays.push(PrimitiveArray::from_vec(std::mem::take(col)).boxed());
        }
        for col in &mut self.frcflw {
            arrays.push(PrimitiveArray::from_vec(std::mem::take(col)).boxed());
        }

        Chunk::new(arrays)
    }
}

pub fn watershed_pass_to_parquet(
    pass_path: &Path,
    events_path: &Path,
    metadata_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    chunk_rows: Option<usize>,
) -> Result<(WriteSummary, WriteSummary), InterchangeError> {
    let reader = open_pass_reader(pass_path)?;
    let mut pass_reader = PassReader::new(reader, pass_path);

    let header_lines = pass_reader.read_header()?;
    let meta = parse_metadata(&header_lines, pass_path)?;

    let calendar_lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };

    let event_schema = build_event_schema(&meta, version);
    let metadata_schema = build_metadata_schema(&meta, version);

    let mut event_writer = ParquetSink::try_new(events_path, event_schema.clone())?;

    let mut store = EventStore::new(meta.npart);
    let chunk_rows = chunk_rows.unwrap_or(EVENT_CHUNK_SIZE);
    let mut row_counter: usize = 0;

    while let Some((label, year, julian, _line_no, _line)) = pass_reader.next_event_header()? {
        let month_day = julian_to_calendar(year, julian, calendar_lookup.as_ref());
        let water_year = determine_wateryear(year, julian);
        let sim_day_index = compute_sim_day_index(year, julian, meta.begin_year, calendar_lookup.as_ref());
        if sim_day_index < 1 {
            return Err(InterchangeError::parse(
                pass_path,
                None,
                format!("Computed simulation day index {sim_day_index} before simulation start"),
                None,
            ));
        }

        match label.as_str() {
            "EVENT" => {
                let dur = pass_reader.read_values(meta.nhill as usize)?;
                let tcs = pass_reader.read_values(meta.nhill as usize)?;
                let oalpha = pass_reader.read_values(meta.nhill as usize)?;
                let runoff = pass_reader.read_values(meta.nhill as usize)?;
                let runvol = pass_reader.read_values(meta.nhill as usize)?;
                let sbrunf = pass_reader.read_values(meta.nhill as usize)?;
                let sbrunv = pass_reader.read_values(meta.nhill as usize)?;
                let drainq = pass_reader.read_values(meta.nhill as usize)?;
                let drrunv = pass_reader.read_values(meta.nhill as usize)?;
                let peakro = pass_reader.read_values(meta.nhill as usize)?;
                let tdet = pass_reader.read_values(meta.nhill as usize)?;
                let tdep = pass_reader.read_values(meta.nhill as usize)?;
                let sedcon = if meta.npart > 0 {
                    pass_reader.read_values(meta.nhill as usize * meta.npart)?
                } else {
                    Vec::new()
                };
                let frcflw = if meta.npart > 0 {
                    pass_reader.read_values(meta.nhill as usize * meta.npart)?
                } else {
                    Vec::new()
                };
                let gwbfv = pass_reader.read_values(meta.nhill as usize)?;
                let gwdsv = pass_reader.read_values(meta.nhill as usize)?;

                for (pos, wepp_id) in meta.hillslope_ids.iter().enumerate() {
                    store.event.push(label.clone());
                    store.year.push(year as i16);
                    store.sim_day_index.push(sim_day_index);
                    store.julian.push(julian as i16);
                    store.month.push(month_day.0 as i8);
                    store.day_of_month.push(month_day.1 as i8);
                    store.water_year.push(water_year as i16);
                    store.wepp_id.push(*wepp_id);
                    store.dur.push(dur[pos]);
                    store.tcs.push(tcs[pos]);
                    store.oalpha.push(oalpha[pos]);
                    store.runoff.push(runoff[pos]);
                    store.runvol.push(runvol[pos]);
                    store.sbrunf.push(sbrunf[pos]);
                    store.sbrunv.push(sbrunv[pos]);
                    store.drainq.push(drainq[pos]);
                    store.drrunv.push(drrunv[pos]);
                    store.peakro.push(peakro[pos]);
                    store.tdet.push(tdet[pos]);
                    store.tdep.push(tdep[pos]);
                    store.gwbfv.push(gwbfv[pos]);
                    store.gwdsv.push(gwdsv[pos]);

                    if meta.npart > 0 {
                        let base = pos * meta.npart;
                        let row_sed = &sedcon[base..base + meta.npart];
                        let row_frc = &frcflw[base..base + meta.npart];
                        for (idx, value) in row_sed.iter().enumerate() {
                            store.sedcon[idx].push(*value);
                        }
                        for (idx, value) in row_frc.iter().enumerate() {
                            store.frcflw[idx].push(*value);
                        }
                    }

                    row_counter += 1;
                    if row_counter % chunk_rows == 0 {
                        let chunk = store.to_chunk(&event_schema);
                        event_writer.write_chunk(chunk)?;
                    }
                }
            }
            "SUBEVENT" => {
                let sbrunf = pass_reader.read_values(meta.nhill as usize)?;
                let sbrunv = pass_reader.read_values(meta.nhill as usize)?;
                let drainq = pass_reader.read_values(meta.nhill as usize)?;
                let drrunv = pass_reader.read_values(meta.nhill as usize)?;
                let gwbfv = pass_reader.read_values(meta.nhill as usize)?;
                let gwdsv = pass_reader.read_values(meta.nhill as usize)?;

                for (pos, wepp_id) in meta.hillslope_ids.iter().enumerate() {
                    store.event.push(label.clone());
                    store.year.push(year as i16);
                    store.sim_day_index.push(sim_day_index);
                    store.julian.push(julian as i16);
                    store.month.push(month_day.0 as i8);
                    store.day_of_month.push(month_day.1 as i8);
                    store.water_year.push(water_year as i16);
                    store.wepp_id.push(*wepp_id);
                    store.dur.push(0.0);
                    store.tcs.push(0.0);
                    store.oalpha.push(0.0);
                    store.runoff.push(0.0);
                    store.runvol.push(0.0);
                    store.sbrunf.push(sbrunf[pos]);
                    store.sbrunv.push(sbrunv[pos]);
                    store.drainq.push(drainq[pos]);
                    store.drrunv.push(drrunv[pos]);
                    store.peakro.push(0.0);
                    store.tdet.push(0.0);
                    store.tdep.push(0.0);
                    store.gwbfv.push(gwbfv[pos]);
                    store.gwdsv.push(gwdsv[pos]);

                    for idx in 0..meta.npart {
                        store.sedcon[idx].push(0.0);
                        store.frcflw[idx].push(0.0);
                    }

                    row_counter += 1;
                    if row_counter % chunk_rows == 0 {
                        let chunk = store.to_chunk(&event_schema);
                        event_writer.write_chunk(chunk)?;
                    }
                }
            }
            "NO EVENT" => {
                let gwbfv = pass_reader.read_values(meta.nhill as usize)?;
                let gwdsv = pass_reader.read_values(meta.nhill as usize)?;

                for (pos, wepp_id) in meta.hillslope_ids.iter().enumerate() {
                    store.event.push(label.clone());
                    store.year.push(year as i16);
                    store.sim_day_index.push(sim_day_index);
                    store.julian.push(julian as i16);
                    store.month.push(month_day.0 as i8);
                    store.day_of_month.push(month_day.1 as i8);
                    store.water_year.push(water_year as i16);
                    store.wepp_id.push(*wepp_id);
                    store.dur.push(0.0);
                    store.tcs.push(0.0);
                    store.oalpha.push(0.0);
                    store.runoff.push(0.0);
                    store.runvol.push(0.0);
                    store.sbrunf.push(0.0);
                    store.sbrunv.push(0.0);
                    store.drainq.push(0.0);
                    store.drrunv.push(0.0);
                    store.peakro.push(0.0);
                    store.tdet.push(0.0);
                    store.tdep.push(0.0);
                    store.gwbfv.push(gwbfv[pos]);
                    store.gwdsv.push(gwdsv[pos]);

                    for idx in 0..meta.npart {
                        store.sedcon[idx].push(0.0);
                        store.frcflw[idx].push(0.0);
                    }

                    row_counter += 1;
                    if row_counter % chunk_rows == 0 {
                        let chunk = store.to_chunk(&event_schema);
                        event_writer.write_chunk(chunk)?;
                    }
                }
            }
            _ => {
                return Err(InterchangeError::parse(
                    pass_path,
                    None,
                    format!("Unsupported pass file event label: {label}"),
                    None,
                ));
            }
        }
    }

    if store.len() > 0 {
        let chunk = store.to_chunk(&event_schema);
        event_writer.write_chunk(chunk)?;
    } else if row_counter == 0 {
        event_writer.write_chunk(empty_chunk(&event_schema))?;
    }

    let event_summary = event_writer.finish()?;

    let metadata_chunk = build_metadata_chunk(&meta, &metadata_schema);
    let metadata_summary = {
        let mut metadata_writer = ParquetSink::try_new(metadata_path, metadata_schema)?;
        metadata_writer.write_chunk(metadata_chunk)?;
        metadata_writer.finish()?
    };

    Ok((event_summary, metadata_summary))
}

fn build_metadata_chunk(meta: &PassMetadata, schema: &Schema) -> Chunk<Box<dyn Array>> {
    let mut arrays: Vec<Box<dyn Array>> = Vec::with_capacity(schema.fields.len());
    arrays.push(PrimitiveArray::from_vec(meta.hillslope_ids.clone()).boxed());
    let climate_array = dictionary_array_from_strings(meta.climate_files.clone()).expect("dictionary encoding for climate files");
    arrays.push(climate_array.boxed());
    arrays.push(PrimitiveArray::from_vec(meta.areas.clone()).boxed());
    arrays.push(PrimitiveArray::from_vec(meta.srp.clone()).boxed());
    arrays.push(PrimitiveArray::from_vec(meta.slfp.clone()).boxed());
    arrays.push(PrimitiveArray::from_vec(meta.bfp.clone()).boxed());
    arrays.push(PrimitiveArray::from_vec(meta.scp.clone()).boxed());

    for idx in 0..meta.npart {
        let column = meta
            .particle_diams
            .iter()
            .map(|row| row[idx])
            .collect::<Vec<f64>>();
        arrays.push(PrimitiveArray::from_vec(column).boxed());
    }

    Chunk::new(arrays)
}

fn open_pass_reader(path: &Path) -> Result<Box<dyn BufRead>, InterchangeError> {
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let is_gzip = path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("gz")).unwrap_or(false);
    if is_gzip {
        let decoder = GzDecoder::new(file);
        Ok(Box::new(BufReader::new(decoder)))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

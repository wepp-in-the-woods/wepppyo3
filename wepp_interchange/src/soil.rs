use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::arrow_support::{BoxedArray, Chunk};
use arrow_array::{Array, Float64Array, Int16Array, Int32Array, Int8Array};
use arrow_schema::{DataType, Schema};
use flate2::read::GzDecoder;

use crate::calendar::{determine_wateryear, julian_to_calendar, load_cli_calendar};
use crate::errors::InterchangeError;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{field_with_meta, schema_with_version, VersionInfo};

const SOIL_CHUNK_SIZE: usize = 250_000;
const RAW_HEADER: [&str; 14] = [
    "OFE",
    "Day",
    "Y",
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
];

const TSMF_HEADER: [&str; 15] = [
    "OFE",
    "Day",
    "Y",
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
    "TSMF",
];

const LEGACY_HEADER: [&str; 12] = [
    "OFE", "Day", "Y", "Poros", "Keff", "Suct", "FC", "WP", "Rough", "Ki", "Kr", "Tauc",
];

const MEASUREMENT_COLUMNS: [&str; 12] = [
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
    "TSMF",
];

const RAW_MEASUREMENT_COLUMNS: [&str; 11] = [
    "Poros",
    "Keff",
    "Suct",
    "FC",
    "WP",
    "Rough",
    "Ki",
    "Kr",
    "Tauc",
    "Saturation",
    "TSW",
];

const LEGACY_MEASUREMENT_COLUMNS: [&str; 9] = [
    "Poros", "Keff", "Suct", "FC", "WP", "Rough", "Ki", "Kr", "Tauc",
];

fn split_soil_row_fixed_width(raw_line: &str, expected_columns: usize) -> Option<Vec<String>> {
    if expected_columns != LEGACY_HEADER.len()
        && expected_columns != RAW_HEADER.len()
        && expected_columns != TSMF_HEADER.len()
    {
        return None;
    }

    let mut idx: usize = 0;
    let mut tokens: Vec<String> = Vec::with_capacity(expected_columns);

    fn take<'a>(line: &'a str, idx: &mut usize, n: usize) -> Option<&'a str> {
        let start = *idx;
        let end = start.saturating_add(n);
        let chunk = line.get(start..end)?;
        *idx = end;
        Some(chunk)
    }

    // Matches `watbal.for` / `watbal_hourly.for` soil output:
    //   1x,i2,2x,i3,2x,i5,1x,9f7.2,[1x,f7.2,1x,f7.2,[1x,f7.4]]
    take(raw_line, &mut idx, 1)?;
    tokens.push(take(raw_line, &mut idx, 2)?.trim().to_string()); // OFE
    take(raw_line, &mut idx, 2)?;
    tokens.push(take(raw_line, &mut idx, 3)?.trim().to_string()); // Day
    take(raw_line, &mut idx, 2)?;
    tokens.push(take(raw_line, &mut idx, 5)?.trim().to_string()); // Y
    take(raw_line, &mut idx, 1)?;

    for _ in 0..9 {
        tokens.push(take(raw_line, &mut idx, 7)?.trim().to_string());
    }

    if expected_columns == LEGACY_HEADER.len() {
        return Some(tokens);
    }

    take(raw_line, &mut idx, 1)?;
    tokens.push(take(raw_line, &mut idx, 7)?.trim().to_string()); // Saturation
    take(raw_line, &mut idx, 1)?;
    tokens.push(take(raw_line, &mut idx, 7)?.trim().to_string()); // TSW

    if expected_columns == RAW_HEADER.len() {
        return Some(tokens);
    }

    take(raw_line, &mut idx, 1)?;
    tokens.push(take(raw_line, &mut idx, 7)?.trim().to_string()); // TSMF
    Some(tokens)
}

pub fn soil_schema(version: &VersionInfo) -> Schema {
    let schema = Schema::new(vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("ofe_id", DataType::Int16, None, None),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta("day", DataType::Int16, None, None),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta("OFE", DataType::Int16, None, None),
        field_with_meta("Poros", DataType::Float64, Some("%"), Some("Soil porosity")),
        field_with_meta(
            "Keff",
            DataType::Float64,
            Some("mm/hr"),
            Some("Effective hydraulic conductivity"),
        ),
        field_with_meta(
            "Suct",
            DataType::Float64,
            Some("mm"),
            Some("Suction across wetting front"),
        ),
        field_with_meta(
            "FC",
            DataType::Float64,
            Some("mm/mm"),
            Some("Field capacity"),
        ),
        field_with_meta(
            "WP",
            DataType::Float64,
            Some("mm/mm"),
            Some("Wilting point"),
        ),
        field_with_meta(
            "Rough",
            DataType::Float64,
            Some("mm"),
            Some("Surface roughness"),
        ),
        field_with_meta(
            "Ki",
            DataType::Float64,
            Some("adjsmt"),
            Some("Interrill erodibility adjustment factor"),
        ),
        field_with_meta(
            "Kr",
            DataType::Float64,
            Some("adjsmt"),
            Some("Rill erodibility adjustment factor"),
        ),
        field_with_meta(
            "Tauc",
            DataType::Float64,
            Some("adjsmt"),
            Some("Critical shear stress adjustment factor"),
        ),
        field_with_meta(
            "Saturation",
            DataType::Float64,
            Some("frac"),
            Some("Saturation as fraction"),
        ),
        field_with_meta(
            "TSW",
            DataType::Float64,
            Some("mm"),
            Some("Total soil water"),
        ),
        field_with_meta(
            "TSMF",
            DataType::Float64,
            Some("frac"),
            Some("True soil moisture fraction (full profile)"),
        ),
    ]);
    schema_with_version(schema, version)
}

struct SoilStore {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    day: Vec<i16>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    measurements: Vec<Vec<Option<f64>>>,
}

impl SoilStore {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            ofe_id: Vec::new(),
            year: Vec::new(),
            day: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            ofe: Vec::new(),
            measurements: vec![Vec::new(); MEASUREMENT_COLUMNS.len()],
        }
    }

    fn len(&self) -> usize {
        self.wepp_id.len()
    }

    fn clear(&mut self) {
        self.wepp_id.clear();
        self.ofe_id.clear();
        self.year.clear();
        self.day.clear();
        self.julian.clear();
        self.month.clear();
        self.day_of_month.clear();
        self.water_year.clear();
        self.ofe.clear();
        for col in &mut self.measurements {
            col.clear();
        }
    }

    fn to_chunk(&mut self, schema: &Schema) -> Chunk<Box<dyn Array>> {
        let mut arrays: Vec<Box<dyn Array>> = Vec::with_capacity(schema.fields().len());
        arrays.push(Int32Array::from(std::mem::take(&mut self.wepp_id)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.ofe_id)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.year)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.day)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.julian)).boxed());
        arrays.push(Int8Array::from(std::mem::take(&mut self.month)).boxed());
        arrays.push(Int8Array::from(std::mem::take(&mut self.day_of_month)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.water_year)).boxed());
        arrays.push(Int16Array::from(std::mem::take(&mut self.ofe)).boxed());

        for col in &mut self.measurements {
            arrays.push(Float64Array::from(std::mem::take(col)).boxed());
        }

        Chunk::new(arrays)
    }
}

pub fn watershed_soil_to_parquet(
    soil_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    chunk_rows: Option<usize>,
) -> Result<WriteSummary, InterchangeError> {
    let calendar_lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };

    let schema = soil_schema(version);
    let mut writer = ParquetSink::try_new(output_path, schema.clone())?;
    let mut store = SoilStore::new();
    let chunk_rows = chunk_rows.unwrap_or(SOIL_CHUNK_SIZE);

    let reader = open_soil_reader(soil_path)?;
    let mut line_reader = LineReader::new(reader);

    let mut header_found = false;
    let mut data_start = 0usize;
    let mut header_tokens: Vec<String> = Vec::new();
    let mut measurement_columns: Vec<String> = Vec::new();
    let mut expected_tokens = RAW_HEADER.len();

    let mut row_counter: usize = 0;

    while let Some((line_no, line)) = line_reader.next_line()? {
        let stripped = line.trim();
        if !header_found {
            if stripped.starts_with("OFE") {
                header_found = true;
                header_tokens = stripped.split_whitespace().map(|s| s.to_string()).collect();
                if header_tokens
                    == TSMF_HEADER
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                {
                    measurement_columns =
                        MEASUREMENT_COLUMNS.iter().map(|s| s.to_string()).collect();
                } else if header_tokens
                    == RAW_HEADER.iter().map(|s| s.to_string()).collect::<Vec<_>>()
                {
                    measurement_columns = RAW_MEASUREMENT_COLUMNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                } else if header_tokens
                    == LEGACY_HEADER
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                {
                    measurement_columns = LEGACY_MEASUREMENT_COLUMNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                } else {
                    return Err(InterchangeError::parse(
                        soil_path,
                        Some(line_no),
                        format!("Unexpected watershed soil header: {header_tokens:?}"),
                        None,
                    ));
                }
                expected_tokens = header_tokens.len();
                data_start = line_no + 1;
            }
            continue;
        }

        if line_no <= data_start {
            continue;
        }
        if stripped.is_empty() || stripped.starts_with('-') {
            continue;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.is_empty() || !tokens[0].chars().all(|c| c.is_ascii_digit()) {
            return Err(InterchangeError::parse(
                soil_path,
                Some(line_no),
                "Expected a watershed SOIL data record after the header",
                Some(line.clone()),
            ));
        }
        let mut tokens: Vec<String> = stripped.split_whitespace().map(|s| s.to_string()).collect();
        if tokens.len() != expected_tokens {
            if let Some(recovered) = split_soil_row_fixed_width(&line, expected_tokens) {
                if recovered.len() == expected_tokens && recovered.iter().all(|t| !t.is_empty()) {
                    tokens = recovered;
                } else {
                    return Err(InterchangeError::parse(
                        soil_path,
                        Some(line_no),
                        format!("Unexpected token count in soil row: {line}"),
                        None,
                    ));
                }
            } else {
                return Err(InterchangeError::parse(
                    soil_path,
                    Some(line_no),
                    format!("Unexpected token count in soil row: {line}"),
                    None,
                ));
            }
        }

        let ofe = tokens[0].parse::<i16>().map_err(|_| {
            InterchangeError::parse(
                soil_path,
                Some(line_no),
                "Invalid OFE id",
                Some(line.clone()),
            )
        })?;
        let julian = tokens[1].parse::<i16>().map_err(|_| {
            InterchangeError::parse(
                soil_path,
                Some(line_no),
                "Invalid julian day",
                Some(line.clone()),
            )
        })?;
        let year = tokens[2].parse::<i16>().map_err(|_| {
            InterchangeError::parse(soil_path, Some(line_no), "Invalid year", Some(line.clone()))
        })?;

        let mut values_map = std::collections::HashMap::new();
        for (column, token) in measurement_columns.iter().zip(tokens[3..].iter()) {
            let value = token.parse::<f64>().map_err(|_| {
                InterchangeError::parse(
                    soil_path,
                    Some(line_no),
                    "Invalid soil measurement",
                    Some(line.clone()),
                )
            })?;
            values_map.insert(column.clone(), value);
        }

        let (month, day_of_month) =
            julian_to_calendar(year as i32, julian as i32, calendar_lookup.as_ref());
        let water_year = determine_wateryear(year as i32, julian as i32);

        store.wepp_id.push(ofe as i32);
        store.ofe_id.push(ofe);
        store.year.push(year);
        store.day.push(julian);
        store.julian.push(julian);
        store.month.push(month as i8);
        store.day_of_month.push(day_of_month as i8);
        store.water_year.push(water_year as i16);
        store.ofe.push(ofe);

        for (idx, column) in MEASUREMENT_COLUMNS.iter().enumerate() {
            store.measurements[idx].push(values_map.get(*column).copied());
        }

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
            .map_err(|err| InterchangeError::io("soil stream", err))?;
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

fn open_soil_reader(path: &Path) -> Result<Box<dyn BufRead>, InterchangeError> {
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let is_gzip = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("gz"))
        .unwrap_or(false);
    if is_gzip {
        let decoder = GzDecoder::new(file);
        Ok(Box::new(BufReader::new(decoder)))
    } else {
        Ok(Box::new(BufReader::new(file)))
    }
}

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow_array::{Float64Array, Int16Array, Int32Array, Int8Array};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::arrow_support::{BoxedArray, Chunk};
use crate::calendar::{
    compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar,
    CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{hill_soil_schema, VersionInfo};

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

const RAW_UNITS: [&str; 14] = [
    "", "", "", "%", "mm/hr", "mm", "mm/mm", "mm/mm", "mm", "adjsmt", "adjsmt", "adjsmt", "frac",
    "mm",
];

const TSMF_UNITS: [&str; 15] = [
    "", "", "", "%", "mm/hr", "mm", "mm/mm", "mm/mm", "mm", "adjsmt", "adjsmt", "adjsmt", "frac",
    "mm", "frac",
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

pub struct SoilColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    poros: Vec<Option<f64>>,
    keff: Vec<Option<f64>>,
    suct: Vec<Option<f64>>,
    fc: Vec<Option<f64>>,
    wp: Vec<Option<f64>>,
    rough: Vec<Option<f64>>,
    ki: Vec<Option<f64>>,
    kr: Vec<Option<f64>>,
    tauc: Vec<Option<f64>>,
    saturation: Vec<Option<f64>>,
    tsw: Vec<Option<f64>>,
    tsmf: Vec<Option<f64>>,
}

impl SoilColumns {
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
            poros: Vec::new(),
            keff: Vec::new(),
            suct: Vec::new(),
            fc: Vec::new(),
            wp: Vec::new(),
            rough: Vec::new(),
            ki: Vec::new(),
            kr: Vec::new(),
            tauc: Vec::new(),
            saturation: Vec::new(),
            tsw: Vec::new(),
            tsmf: Vec::new(),
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
        dict.set_item("Poros", self.poros).unwrap();
        dict.set_item("Keff", self.keff).unwrap();
        dict.set_item("Suct", self.suct).unwrap();
        dict.set_item("FC", self.fc).unwrap();
        dict.set_item("WP", self.wp).unwrap();
        dict.set_item("Rough", self.rough).unwrap();
        dict.set_item("Ki", self.ki).unwrap();
        dict.set_item("Kr", self.kr).unwrap();
        dict.set_item("Tauc", self.tauc).unwrap();
        dict.set_item("Saturation", self.saturation).unwrap();
        dict.set_item("TSW", self.tsw).unwrap();
        dict.set_item("TSMF", self.tsmf).unwrap();
        dict.into_py(py)
    }

    fn into_chunk(self) -> Chunk<Box<dyn arrow_array::Array>> {
        Chunk::new(vec![
            Int32Array::from(self.wepp_id).boxed(),
            Int16Array::from(self.ofe_id).boxed(),
            Int16Array::from(self.year).boxed(),
            Int32Array::from(self.sim_day_index).boxed(),
            Int16Array::from(self.julian).boxed(),
            Int8Array::from(self.month).boxed(),
            Int8Array::from(self.day_of_month).boxed(),
            Int16Array::from(self.water_year).boxed(),
            Int16Array::from(self.ofe).boxed(),
            Float64Array::from(self.poros).boxed(),
            Float64Array::from(self.keff).boxed(),
            Float64Array::from(self.suct).boxed(),
            Float64Array::from(self.fc).boxed(),
            Float64Array::from(self.wp).boxed(),
            Float64Array::from(self.rough).boxed(),
            Float64Array::from(self.ki).boxed(),
            Float64Array::from(self.kr).boxed(),
            Float64Array::from(self.tauc).boxed(),
            Float64Array::from(self.saturation).boxed(),
            Float64Array::from(self.tsw).boxed(),
            Float64Array::from(self.tsmf).boxed(),
        ])
    }
}

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

pub fn hillslope_soil_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<SoilColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    hillslope_soil_to_columns_with_lookup(path, lookup.as_ref(), start_year)
}

fn hillslope_soil_to_columns_with_lookup(
    path: &Path,
    lookup: Option<&CalendarLookup>,
    start_year: Option<i32>,
) -> Result<SoilColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;

    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = SoilColumns::new();
    let mut header_tokens: Option<Vec<String>> = None;
    let mut measurement_columns: Vec<String> = Vec::new();
    let mut expected_units: Vec<String> = Vec::new();
    let mut unit_tokens_found = false;

    enum ParseState {
        SearchingHeader,
        LookingForUnits,
        SkipAfterUnits,
        Data,
    }

    let mut state = ParseState::SearchingHeader;

    let calendar_start_year = lookup.and_then(|cal| cal.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        let stripped = raw_line.trim();

        match state {
            ParseState::SearchingHeader => {
                if stripped.is_empty() {
                    continue;
                }
                let tokens: Vec<&str> = stripped.split_whitespace().collect();
                if tokens.len() >= 3 && tokens[0] == "OFE" && tokens[1] == "Day" && tokens[2] == "Y"
                {
                    let tokens_vec = tokens.iter().map(|t| t.to_string()).collect::<Vec<_>>();
                    let header_as_str: Vec<&str> = tokens_vec.iter().map(|s| s.as_str()).collect();
                    let compact_units: Vec<String> = RAW_UNITS
                        .iter()
                        .filter(|token| !token.is_empty())
                        .map(|t| t.to_string())
                        .collect();
                    let tsmf_compact_units: Vec<String> = TSMF_UNITS
                        .iter()
                        .filter(|token| !token.is_empty())
                        .map(|t| t.to_string())
                        .collect();
                    if header_as_str == RAW_HEADER {
                        expected_units = compact_units.clone();
                        measurement_columns = RAW_MEASUREMENT_COLUMNS
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                    } else if header_as_str == TSMF_HEADER {
                        expected_units = tsmf_compact_units;
                        measurement_columns =
                            MEASUREMENT_COLUMNS.iter().map(|s| s.to_string()).collect();
                    } else if header_as_str == LEGACY_HEADER {
                        expected_units = compact_units[..LEGACY_MEASUREMENT_COLUMNS.len()].to_vec();
                        measurement_columns = LEGACY_MEASUREMENT_COLUMNS
                            .iter()
                            .map(|s| s.to_string())
                            .collect();
                    } else {
                        return Err(InterchangeError::parse(
                            path,
                            None,
                            format!("Unexpected SOIL header layout: {tokens_vec:?}"),
                            None,
                        ));
                    }
                    header_tokens = Some(tokens_vec);
                    state = ParseState::LookingForUnits;
                }
            }
            ParseState::LookingForUnits => {
                if stripped.is_empty() {
                    continue;
                }
                let tokens: Vec<&str> = stripped.split_whitespace().collect();
                if tokens
                    .iter()
                    .any(|t| ["mm/hr", "frac", "adjsmt"].contains(t))
                {
                    let unit_tokens = tokens.iter().map(|t| t.to_string()).collect::<Vec<_>>();
                    if unit_tokens != expected_units {
                        return Err(InterchangeError::parse(
                            path,
                            None,
                            format!("Unexpected SOIL units: {unit_tokens:?}"),
                            None,
                        ));
                    }
                    unit_tokens_found = true;
                    state = ParseState::SkipAfterUnits;
                }
            }
            ParseState::SkipAfterUnits => {
                if stripped.is_empty() {
                    continue;
                }
                if stripped.chars().all(|c| c == '-') {
                    continue;
                }
                state = ParseState::Data;
            }
            ParseState::Data => {}
        }

        if let ParseState::Data = state {
            if stripped.is_empty() {
                continue;
            }
            let mut tokens: Vec<String> = stripped
                .split_whitespace()
                .map(|token| token.to_string())
                .collect();
            let expected_columns = header_tokens.as_ref().map(|t| t.len()).unwrap_or(0);
            if tokens.len() != expected_columns {
                if let Some(recovered) = split_soil_row_fixed_width(&raw_line, expected_columns) {
                    if recovered.len() == expected_columns
                        && recovered.iter().all(|t| !t.is_empty())
                    {
                        tokens = recovered;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let ofe_val: i32 = tokens[0].parse().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid OFE token", Some(raw_line.clone()))
            })?;
            let julian_val: i32 = tokens[1].parse().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid julian token", Some(raw_line.clone()))
            })?;
            let raw_year: i32 = tokens[2].parse().map_err(|_| {
                InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
            })?;
            let year_val = if normalize_sim_years && raw_year < 1000 {
                resolved_start_year.unwrap_or(raw_year) + raw_year - 1
            } else {
                raw_year
            };
            if sim_start_year.is_none() {
                sim_start_year = Some(year_val);
            }

            let (month, day_of_month) = julian_to_calendar(year_val, julian_val, lookup);
            let water_year = determine_wateryear(year_val, julian_val);
            let sim_day_index = compute_sim_day_index(
                year_val,
                julian_val,
                sim_start_year.unwrap_or(year_val),
                lookup,
            );

            let mut values: Vec<Option<f64>> = vec![None; MEASUREMENT_COLUMNS.len()];
            for (idx, token) in measurement_columns.iter().zip(tokens.iter().skip(3)) {
                if let Some(pos) = MEASUREMENT_COLUMNS
                    .iter()
                    .position(|name| *name == idx.as_str())
                {
                    let value = parse_required_float(token.as_str()).map_err(|msg| {
                        InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                    })?;
                    values[pos] = Some(value);
                }
            }

            out.wepp_id.push(wepp_id);
            out.ofe_id.push(ofe_val as i16);
            out.year.push(year_val as i16);
            out.sim_day_index.push(sim_day_index);
            out.julian.push(julian_val as i16);
            out.month.push(month as i8);
            out.day_of_month.push(day_of_month as i8);
            out.water_year.push(water_year as i16);
            out.ofe.push(ofe_val as i16);
            out.poros.push(values[0]);
            out.keff.push(values[1]);
            out.suct.push(values[2]);
            out.fc.push(values[3]);
            out.wp.push(values[4]);
            out.rough.push(values[5]);
            out.ki.push(values[6]);
            out.kr.push(values[7]);
            out.tauc.push(values[8]);
            out.saturation.push(values[9]);
            out.tsw.push(values[10]);
            out.tsmf.push(values[11]);
        }
    }

    if header_tokens.is_none() || !unit_tokens_found {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unable to locate SOIL header layout",
            None,
        ));
    }

    Ok(out)
}

pub fn hillslope_soil_files_to_parquet(
    paths: &[PathBuf],
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = hill_soil_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    if paths.is_empty() {
        sink.write_chunk(empty_chunk(&schema))?;
    } else {
        for path in paths {
            let columns = hillslope_soil_to_columns_with_lookup(path, lookup.as_ref(), start_year)?;
            sink.write_chunk(columns.into_chunk())?;
        }
    }
    sink.finish()
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
            "Unrecognized soil filename pattern",
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
            "Unrecognized soil filename pattern",
            Some(name.to_string()),
        ));
    }
    digits.parse::<i32>().map_err(|_| {
        InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wepp_interchange_hill_soil_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn write_soil(path: &Path) {
        let units = RAW_UNITS
            .iter()
            .filter(|token| !token.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let payload = format!(
            "{}\n{units}\n----------------\n1 1 2000 40 10 100 0.3 0.1 5 1 1 1 0.5 50\n",
            RAW_HEADER.join(" ")
        );
        fs::write(path, payload).expect("write SOIL fixture");
    }

    #[test]
    fn bulk_writer_preserves_path_order_and_row_groups() {
        let dir = temp_dir();
        let first = dir.join("H8.soil.dat");
        let second = dir.join("H5.soil.dat");
        let output = dir.join("H.soil.parquet");
        write_soil(&first);
        write_soil(&second);

        let version = VersionInfo::new(1, 0);
        let summary =
            hillslope_soil_files_to_parquet(&[first, second], &output, None, &version, Some(2000))
                .expect("write SOIL parquet");
        assert_eq!(summary.rows_written, 2);
        assert_eq!(summary.row_groups, 2);

        let builder = ParquetRecordBatchReaderBuilder::try_new(
            File::open(&output).expect("open SOIL parquet"),
        )
        .expect("build SOIL parquet reader");
        assert_eq!(builder.schema().as_ref(), &hill_soil_schema(&version));
        assert_eq!(builder.metadata().num_row_groups(), 2);
        let mut ids = Vec::new();
        for batch in builder.build().expect("build batch reader") {
            let batch = batch.expect("read SOIL batch");
            let values = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("wepp_id Int32");
            ids.extend(values.values().iter().copied());
        }
        assert_eq!(ids, [8, 5]);
    }

    #[test]
    fn bulk_writer_emits_empty_parquet_with_schema_metadata() {
        let dir = temp_dir();
        let output = dir.join("H.soil.parquet");
        let version = VersionInfo::new(2, 7);
        let summary = hillslope_soil_files_to_parquet(&[], &output, None, &version, Some(2000))
            .expect("write empty SOIL parquet");
        assert_eq!(summary.rows_written, 0);
        assert_eq!(summary.row_groups, 0);

        let builder = ParquetRecordBatchReaderBuilder::try_new(
            File::open(&output).expect("open empty SOIL parquet"),
        )
        .expect("build empty SOIL parquet reader");
        assert_eq!(builder.schema().as_ref(), &hill_soil_schema(&version));
    }
}

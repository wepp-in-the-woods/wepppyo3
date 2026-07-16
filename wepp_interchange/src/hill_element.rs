use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow_array::{Float64Array, Int16Array, Int32Array, Int8Array};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::ag_fields::{self, Source as AgFieldsSource};
use crate::arrow_support::{BoxedArray, Chunk};
use crate::calendar::determine_wateryear;
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{hill_element_schema, VersionInfo};

const ELEMENT_FIELD_WIDTHS: [usize; 24] = [
    3, 3, 3, 5, 9, 9, 8, 8, 8, 6, 8, 8, 8, 7, 9, 9, 9, 9, 7, 7, 7, 7, 7, 9,
];

const ELEMENT_OPTIONAL_FIELD_WIDTHS: [usize; 2] = [9, 9];

const ELEMENT_COLUMN_NAMES: [&str; 24] = [
    "OFE", "DD", "MM", "YYYY", "Precip", "Runoff", "EffInt", "PeakRO", "EffDur", "Enrich", "Keff",
    "Sm", "LeafArea", "CanHgt", "Cancov", "IntCov", "RilCov", "LivBio", "DeadBio", "Ki", "Kr",
    "Tcrit", "RilWid", "SedLeave",
];

const ELEMENT_OPTIONAL_COLUMN_NAMES: [&str; 2] = ["QRain", "QSnow"];

pub struct ElementColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    precip: Vec<f64>,
    runoff: Vec<f64>,
    effint: Vec<f64>,
    peakro: Vec<f64>,
    effdur: Vec<f64>,
    enrich: Vec<f64>,
    keff: Vec<f64>,
    sm: Vec<f64>,
    leaf_area: Vec<f64>,
    can_hgt: Vec<f64>,
    cancov: Vec<f64>,
    intcov: Vec<f64>,
    rilcov: Vec<f64>,
    livbio: Vec<f64>,
    deadbio: Vec<f64>,
    ki: Vec<f64>,
    kr: Vec<f64>,
    tcrit: Vec<f64>,
    rilwid: Vec<f64>,
    sedleave: Vec<f64>,
    qrain: Vec<Option<f64>>,
    qsnow: Vec<Option<f64>>,
}

impl ElementColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            ofe_id: Vec::new(),
            year: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            ofe: Vec::new(),
            precip: Vec::new(),
            runoff: Vec::new(),
            effint: Vec::new(),
            peakro: Vec::new(),
            effdur: Vec::new(),
            enrich: Vec::new(),
            keff: Vec::new(),
            sm: Vec::new(),
            leaf_area: Vec::new(),
            can_hgt: Vec::new(),
            cancov: Vec::new(),
            intcov: Vec::new(),
            rilcov: Vec::new(),
            livbio: Vec::new(),
            deadbio: Vec::new(),
            ki: Vec::new(),
            kr: Vec::new(),
            tcrit: Vec::new(),
            rilwid: Vec::new(),
            sedleave: Vec::new(),
            qrain: Vec::new(),
            qsnow: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("ofe_id", self.ofe_id).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("OFE", self.ofe).unwrap();
        dict.set_item("Precip", self.precip).unwrap();
        dict.set_item("Runoff", self.runoff).unwrap();
        dict.set_item("EffInt", self.effint).unwrap();
        dict.set_item("PeakRO", self.peakro).unwrap();
        dict.set_item("EffDur", self.effdur).unwrap();
        dict.set_item("Enrich", self.enrich).unwrap();
        dict.set_item("Keff", self.keff).unwrap();
        dict.set_item("Sm", self.sm).unwrap();
        dict.set_item("LeafArea", self.leaf_area).unwrap();
        dict.set_item("CanHgt", self.can_hgt).unwrap();
        dict.set_item("Cancov", self.cancov).unwrap();
        dict.set_item("IntCov", self.intcov).unwrap();
        dict.set_item("RilCov", self.rilcov).unwrap();
        dict.set_item("LivBio", self.livbio).unwrap();
        dict.set_item("DeadBio", self.deadbio).unwrap();
        dict.set_item("Ki", self.ki).unwrap();
        dict.set_item("Kr", self.kr).unwrap();
        dict.set_item("Tcrit", self.tcrit).unwrap();
        dict.set_item("RilWid", self.rilwid).unwrap();
        dict.set_item("SedLeave", self.sedleave).unwrap();
        dict.set_item("QRain", self.qrain).unwrap();
        dict.set_item("QSnow", self.qsnow).unwrap();
        dict.into_py(py)
    }

    pub(crate) fn into_chunk(self) -> Chunk<Box<dyn arrow_array::Array>> {
        Chunk::new(vec![
            Int32Array::from(self.wepp_id).boxed(),
            Int16Array::from(self.ofe_id).boxed(),
            Int16Array::from(self.year).boxed(),
            Int16Array::from(self.julian).boxed(),
            Int8Array::from(self.month).boxed(),
            Int8Array::from(self.day_of_month).boxed(),
            Int16Array::from(self.water_year).boxed(),
            Int16Array::from(self.ofe).boxed(),
            Float64Array::from(self.precip).boxed(),
            Float64Array::from(self.runoff).boxed(),
            Float64Array::from(self.effint).boxed(),
            Float64Array::from(self.peakro).boxed(),
            Float64Array::from(self.effdur).boxed(),
            Float64Array::from(self.enrich).boxed(),
            Float64Array::from(self.keff).boxed(),
            Float64Array::from(self.sm).boxed(),
            Float64Array::from(self.leaf_area).boxed(),
            Float64Array::from(self.can_hgt).boxed(),
            Float64Array::from(self.cancov).boxed(),
            Float64Array::from(self.intcov).boxed(),
            Float64Array::from(self.rilcov).boxed(),
            Float64Array::from(self.livbio).boxed(),
            Float64Array::from(self.deadbio).boxed(),
            Float64Array::from(self.ki).boxed(),
            Float64Array::from(self.kr).boxed(),
            Float64Array::from(self.tcrit).boxed(),
            Float64Array::from(self.rilwid).boxed(),
            Float64Array::from(self.sedleave).boxed(),
            Float64Array::from(self.qrain).boxed(),
            Float64Array::from(self.qsnow).boxed(),
        ])
    }
}

pub fn hillslope_element_to_columns(
    path: &Path,
    _version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<ElementColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = ElementColumns::new();
    let base_value_count = ELEMENT_COLUMN_NAMES.len() - 4;
    let optional_offset = 4 + base_value_count;
    let mut previous: Vec<f64> = vec![0.0; base_value_count];
    let mut non_empty_count = 0usize;
    let mut data_index = 0usize;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        if raw_line.trim().is_empty() {
            continue;
        }
        non_empty_count += 1;
        if non_empty_count <= 2 {
            continue;
        }
        let idx = data_index;
        data_index += 1;
        let tokens = split_fixed_width_line(&raw_line, path)?;
        let ofe: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid OFE token", Some(raw_line.clone()))
        })?;
        let day: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid day token", Some(raw_line.clone()))
        })?;
        let month: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid month token", Some(raw_line.clone()))
        })?;
        let year_token: i32 = tokens[3].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid year token", Some(raw_line.clone()))
        })?;

        let (year, month, day, julian, water_year) =
            normalize_date_tokens(year_token, month, day, start_year);

        let mut row_values: Vec<f64> = Vec::with_capacity(base_value_count);
        for (col_idx, token) in tokens.iter().skip(4).take(base_value_count).enumerate() {
            let value = if is_missing_token(token) {
                if idx == 0 {
                    0.0
                } else {
                    previous[col_idx]
                }
            } else {
                parse_required_float(token).map_err(|msg| {
                    InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                })?
            };
            row_values.push(value);
        }

        let mut optional_values: Vec<Option<f64>> =
            Vec::with_capacity(ELEMENT_OPTIONAL_COLUMN_NAMES.len());
        for token in tokens.iter().skip(optional_offset) {
            let trimmed = token.trim();
            if trimmed.is_empty() || is_missing_token(trimmed) {
                optional_values.push(None);
                continue;
            }
            let value = parse_required_float(trimmed)
                .map_err(|msg| InterchangeError::parse(path, None, msg, Some(raw_line.clone())))?;
            optional_values.push(Some(value));
        }

        out.wepp_id.push(wepp_id);
        out.ofe_id.push(ofe as i16);
        out.year.push(year as i16);
        out.julian.push(julian as i16);
        out.month.push(month as i8);
        out.day_of_month.push(day as i8);
        out.water_year.push(water_year as i16);
        out.ofe.push(ofe as i16);
        out.precip.push(row_values[0]);
        out.runoff.push(row_values[1]);
        out.effint.push(row_values[2]);
        out.peakro.push(row_values[3]);
        out.effdur.push(row_values[4]);
        out.enrich.push(row_values[5]);
        out.keff.push(row_values[6]);
        out.sm.push(row_values[7]);
        out.leaf_area.push(row_values[8]);
        out.can_hgt.push(row_values[9]);
        out.cancov.push(row_values[10]);
        out.intcov.push(row_values[11]);
        out.rilcov.push(row_values[12]);
        out.livbio.push(row_values[13]);
        out.deadbio.push(row_values[14]);
        out.ki.push(row_values[15]);
        out.kr.push(row_values[16]);
        out.tcrit.push(row_values[17]);
        out.rilwid.push(row_values[18]);
        out.sedleave.push(row_values[19]);
        out.qrain.push(optional_values[0]);
        out.qsnow.push(optional_values[1]);

        previous = row_values;
    }

    Ok(out)
}

pub fn hillslope_element_files_to_parquet(
    paths: &[PathBuf],
    output_path: &Path,
    version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<WriteSummary, InterchangeError> {
    let schema = hill_element_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    if paths.is_empty() {
        sink.write_chunk(empty_chunk(&schema))?;
    } else {
        for path in paths {
            let columns = hillslope_element_to_columns(path, version, start_year)?;
            sink.write_chunk(columns.into_chunk())?;
        }
    }
    sink.finish()
}

pub fn ag_fields_hillslope_element_files_to_parquet(
    sources: &[AgFieldsSource],
    output_path: &Path,
    version: &VersionInfo,
    start_year: Option<i32>,
) -> Result<WriteSummary, InterchangeError> {
    let schema = ag_fields::schema_from_hillslope(hill_element_schema(version));
    ag_fields::write_sources(sources, output_path, schema, |path| {
        hillslope_element_to_columns(path, version, start_year).map(ElementColumns::into_chunk)
    })
}

fn split_fixed_width_payload(raw_line: &str, field_widths: &[usize]) -> (Vec<String>, String) {
    let width: usize = field_widths.iter().sum();
    let mut line = raw_line.to_string();
    if line.len() < width {
        line.push_str(&" ".repeat(width - line.len()));
    }
    let mut tokens = Vec::new();
    let mut idx = 0usize;
    for width in field_widths {
        let end = idx + width;
        let segment = line.get(idx..end).unwrap_or("");
        tokens.push(segment.trim().to_string());
        idx = end;
    }
    let remainder = if idx < line.len() {
        line[idx..].to_string()
    } else {
        String::new()
    };
    (tokens, remainder)
}

fn split_fixed_width_line(raw_line: &str, path: &Path) -> Result<Vec<String>, InterchangeError> {
    let (mut tokens, remainder) = split_fixed_width_payload(raw_line, &ELEMENT_FIELD_WIDTHS);
    if remainder.trim().is_empty() {
        for _ in 0..ELEMENT_OPTIONAL_COLUMN_NAMES.len() {
            tokens.push(String::new());
        }
        return Ok(tokens);
    }

    let (optional_tokens, tail) =
        split_fixed_width_payload(&remainder, &ELEMENT_OPTIONAL_FIELD_WIDTHS);
    if !tail.trim().is_empty() {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unexpected trailing characters past fixed width payload",
            Some(raw_line.to_string()),
        ));
    }
    tokens.extend(optional_tokens);
    Ok(tokens)
}

fn is_missing_token(token: &str) -> bool {
    let stripped = token.trim();
    !stripped.is_empty() && stripped.chars().all(|c| c == '*')
}

fn normalize_date_tokens(
    raw_year: i32,
    raw_month: i32,
    raw_day: i32,
    start_year: Option<i32>,
) -> (i32, i32, i32, i32, i32) {
    let mut year = raw_year;
    if let Some(start) = start_year {
        if year < 1000 {
            year = start + year - 1;
        }
    }

    let mut month = raw_month.max(1);
    let mut day = raw_day.max(1);

    let extra_years = (month - 1) / 12;
    month = (month - 1) % 12 + 1;
    year += extra_years;

    let max_day = days_in_month(year, month);
    if day > max_day {
        day = max_day;
    }

    let julian = julian_from_ymd(year, month, day);
    let water_year = determine_wateryear(year, julian);
    (year, month, day, julian, water_year)
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

fn julian_from_ymd(year: i32, month: i32, day: i32) -> i32 {
    let mut julian = 0;
    for m in 1..month {
        julian += days_in_month(year, m);
    }
    julian + day
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
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
            "Unrecognized element filename pattern",
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
            "Unrecognized element filename pattern",
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
            "wepp_interchange_hill_element_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn write_element(path: &Path) {
        let mut values = vec!["1", "1", "1", "2000"];
        values.extend(std::iter::repeat_n("1", 20));
        let row = values
            .iter()
            .zip(ELEMENT_FIELD_WIDTHS)
            .map(|(value, width)| format!("{value:>width$}"))
            .collect::<String>();
        fs::write(path, format!("header 1\nheader 2\n{row}\n")).expect("write ELEMENT fixture");
    }

    #[test]
    fn bulk_writer_preserves_path_order_and_row_groups() {
        let dir = temp_dir();
        let first = dir.join("H9.element.dat");
        let second = dir.join("H4.element.dat");
        let output = dir.join("H.element.parquet");
        write_element(&first);
        write_element(&second);

        let version = VersionInfo::new(1, 0);
        let summary =
            hillslope_element_files_to_parquet(&[first, second], &output, &version, Some(2000))
                .expect("write ELEMENT parquet");
        assert_eq!(summary.rows_written, 2);
        assert_eq!(summary.row_groups, 2);

        let builder = ParquetRecordBatchReaderBuilder::try_new(
            File::open(&output).expect("open ELEMENT parquet"),
        )
        .expect("build ELEMENT parquet reader");
        assert_eq!(builder.schema().as_ref(), &hill_element_schema(&version));
        assert_eq!(builder.metadata().num_row_groups(), 2);
        let mut ids = Vec::new();
        for batch in builder.build().expect("build batch reader") {
            let batch = batch.expect("read ELEMENT batch");
            let values = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("wepp_id Int32");
            ids.extend(values.values().iter().copied());
        }
        assert_eq!(ids, [9, 4]);
    }

    #[test]
    fn ag_fields_writer_preserves_all_element_values_and_coupled_identity() {
        let dir = temp_dir();
        let paths = [
            dir.join("H9.element.dat"),
            dir.join("H4.element.dat"),
            dir.join("H10.element.dat"),
        ];
        for path in &paths {
            write_element(path);
        }
        let ordinary = dir.join("ordinary.element.parquet");
        let ag_output = dir.join("ag_fields.element.parquet");
        let version = VersionInfo::new(1, 2);
        let ordinary_summary =
            hillslope_element_files_to_parquet(&paths, &ordinary, &version, Some(2000))
                .expect("write ordinary ELEMENT parquet");
        let sources = vec![
            AgFieldsSource::new(paths[0].clone(), 60, 9),
            AgFieldsSource::new(paths[1].clone(), 60, 4),
            AgFieldsSource::new(paths[2].clone(), 61, 10),
        ];
        let ag_summary = ag_fields_hillslope_element_files_to_parquet(
            &sources,
            &ag_output,
            &version,
            Some(2000),
        )
        .expect("write AgFields ELEMENT parquet");

        assert_eq!(ordinary_summary.rows_written, ag_summary.rows_written);
        assert_eq!(ordinary_summary.row_groups, ag_summary.row_groups);
        crate::ag_fields::assert_parquet_parity(&ordinary, &ag_output, &sources);
    }
}

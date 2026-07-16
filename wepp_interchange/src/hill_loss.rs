use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow_array::{Float64Array, Int32Array, Int8Array};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::ag_fields::{self, Source as AgFieldsSource};
use crate::arrow_support::{BoxedArray, Chunk};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{hill_loss_schema, VersionInfo};

const MEASUREMENT_COLUMNS: [&str; 9] = [
    "Class",
    "Diameter (mm)",
    "Specific Gravity",
    "% Sand",
    "% Silt",
    "% Clay",
    "% O.M.",
    "Sediment Fraction",
    "In Flow Exiting",
];

fn column_alias(name: &str) -> &str {
    match name {
        "Diameter (mm)" => "Diameter",
        other => other,
    }
}

pub struct LossColumns {
    wepp_id: Vec<i32>,
    class_id: Vec<i8>,
    class_val: Vec<i8>,
    diameter: Vec<f64>,
    specific_gravity: Vec<f64>,
    sand: Vec<f64>,
    silt: Vec<f64>,
    clay: Vec<f64>,
    om: Vec<f64>,
    sediment_fraction: Vec<f64>,
    inflow_exiting: Vec<f64>,
}

impl LossColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            class_id: Vec::new(),
            class_val: Vec::new(),
            diameter: Vec::new(),
            specific_gravity: Vec::new(),
            sand: Vec::new(),
            silt: Vec::new(),
            clay: Vec::new(),
            om: Vec::new(),
            sediment_fraction: Vec::new(),
            inflow_exiting: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("class_id", self.class_id).unwrap();
        dict.set_item("Class", self.class_val).unwrap();
        dict.set_item("Diameter", self.diameter).unwrap();
        dict.set_item("Specific Gravity", self.specific_gravity)
            .unwrap();
        dict.set_item("% Sand", self.sand).unwrap();
        dict.set_item("% Silt", self.silt).unwrap();
        dict.set_item("% Clay", self.clay).unwrap();
        dict.set_item("% O.M.", self.om).unwrap();
        dict.set_item("Sediment Fraction", self.sediment_fraction)
            .unwrap();
        dict.set_item("In Flow Exiting", self.inflow_exiting)
            .unwrap();
        dict.into_py(py)
    }

    pub(crate) fn into_chunk(self) -> Chunk<Box<dyn arrow_array::Array>> {
        Chunk::new(vec![
            Int32Array::from(self.wepp_id).boxed(),
            Int8Array::from(self.class_id).boxed(),
            Int8Array::from(self.class_val).boxed(),
            Float64Array::from(self.diameter).boxed(),
            Float64Array::from(self.specific_gravity).boxed(),
            Float64Array::from(self.sand).boxed(),
            Float64Array::from(self.silt).boxed(),
            Float64Array::from(self.clay).boxed(),
            Float64Array::from(self.om).boxed(),
            Float64Array::from(self.sediment_fraction).boxed(),
            Float64Array::from(self.inflow_exiting).boxed(),
        ])
    }
}

pub fn hillslope_loss_to_columns(
    path: &Path,
    _version: &VersionInfo,
) -> Result<LossColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut out = LossColumns::new();
    let mut skip_remaining: Option<usize> = None;
    let mut in_table = false;
    let target = "sediment particle information leaving profile";

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        if !in_table {
            let lower = raw_line.to_lowercase();
            if lower.contains(target) {
                skip_remaining = Some(5);
                continue;
            }
            if let Some(remaining) = skip_remaining {
                if remaining > 0 {
                    skip_remaining = Some(remaining - 1);
                    continue;
                }
                skip_remaining = None;
                in_table = true;
            } else {
                continue;
            }
        }

        let stripped = raw_line.trim();
        if stripped.is_empty() || stripped.chars().all(|c| c == '-') {
            break;
        }
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() < MEASUREMENT_COLUMNS.len() {
            continue;
        }

        let class_val: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(path, None, "Invalid class token", Some(raw_line.clone()))
        })?;
        let measurements = &tokens[1..MEASUREMENT_COLUMNS.len()];

        let mut values: HashMap<&str, f64> = HashMap::new();
        for (column_name, token) in MEASUREMENT_COLUMNS[1..].iter().zip(measurements.iter()) {
            let target = column_alias(column_name);
            let value = parse_required_float(token)
                .map_err(|msg| InterchangeError::parse(path, None, msg, Some(raw_line.clone())))?;
            values.insert(target, value);
        }

        out.wepp_id.push(wepp_id);
        out.class_id.push(class_val as i8);
        out.class_val.push(class_val as i8);
        out.diameter.push(*values.get("Diameter").unwrap_or(&0.0));
        out.specific_gravity
            .push(*values.get("Specific Gravity").unwrap_or(&0.0));
        out.sand.push(*values.get("% Sand").unwrap_or(&0.0));
        out.silt.push(*values.get("% Silt").unwrap_or(&0.0));
        out.clay.push(*values.get("% Clay").unwrap_or(&0.0));
        out.om.push(*values.get("% O.M.").unwrap_or(&0.0));
        out.sediment_fraction
            .push(*values.get("Sediment Fraction").unwrap_or(&0.0));
        out.inflow_exiting
            .push(*values.get("In Flow Exiting").unwrap_or(&0.0));
    }

    Ok(out)
}

pub fn hillslope_loss_files_to_parquet(
    paths: &[PathBuf],
    output_path: &Path,
    version: &VersionInfo,
) -> Result<WriteSummary, InterchangeError> {
    let schema = hill_loss_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    if paths.is_empty() {
        sink.write_chunk(empty_chunk(&schema))?;
    } else {
        for path in paths {
            let columns = hillslope_loss_to_columns(path, version)?;
            sink.write_chunk(columns.into_chunk())?;
        }
    }
    sink.finish()
}

pub fn ag_fields_hillslope_loss_files_to_parquet(
    sources: &[AgFieldsSource],
    output_path: &Path,
    version: &VersionInfo,
) -> Result<WriteSummary, InterchangeError> {
    let schema = ag_fields::schema_from_hillslope(hill_loss_schema(version));
    ag_fields::write_sources(sources, output_path, schema, |path| {
        hillslope_loss_to_columns(path, version).map(LossColumns::into_chunk)
    })
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
            "Unrecognized loss filename pattern",
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
            "Unrecognized loss filename pattern",
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
            "wepp_interchange_hill_loss_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn write_loss(path: &Path) {
        fs::write(
            path,
            "Sediment particle information leaving profile\nskip 1\nskip 2\nskip 3\nskip 4\nskip 5\n1 0.1 2.6 10 20 70 1 0.4 0.5\n\n",
        )
        .expect("write LOSS fixture");
    }

    #[test]
    fn bulk_writer_preserves_path_order_and_row_groups() {
        let dir = temp_dir();
        let first = dir.join("H6.loss.dat");
        let second = dir.join("H2.loss.dat");
        let output = dir.join("H.loss.parquet");
        write_loss(&first);
        write_loss(&second);

        let version = VersionInfo::new(1, 0);
        let summary = hillslope_loss_files_to_parquet(&[first, second], &output, &version)
            .expect("write LOSS parquet");
        assert_eq!(summary.rows_written, 2);
        assert_eq!(summary.row_groups, 2);

        let builder = ParquetRecordBatchReaderBuilder::try_new(
            File::open(&output).expect("open LOSS parquet"),
        )
        .expect("build LOSS parquet reader");
        assert_eq!(builder.schema().as_ref(), &hill_loss_schema(&version));
        assert_eq!(builder.metadata().num_row_groups(), 2);
        let mut ids = Vec::new();
        for batch in builder.build().expect("build batch reader") {
            let batch = batch.expect("read LOSS batch");
            let values = batch
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("wepp_id Int32");
            ids.extend(values.values().iter().copied());
        }
        assert_eq!(ids, [6, 2]);
    }

    #[test]
    fn ag_fields_writer_preserves_all_loss_values_and_coupled_identity() {
        let dir = temp_dir();
        let paths = [
            dir.join("H6.loss.dat"),
            dir.join("H2.loss.dat"),
            dir.join("H7.loss.dat"),
        ];
        for path in &paths {
            write_loss(path);
        }
        let ordinary = dir.join("ordinary.loss.parquet");
        let ag_output = dir.join("ag_fields.loss.parquet");
        let version = VersionInfo::new(1, 2);
        let ordinary_summary = hillslope_loss_files_to_parquet(&paths, &ordinary, &version)
            .expect("write ordinary LOSS parquet");
        let sources = vec![
            AgFieldsSource::new(paths[0].clone(), 70, 6),
            AgFieldsSource::new(paths[1].clone(), 70, 2),
            AgFieldsSource::new(paths[2].clone(), 71, 7),
        ];
        let ag_summary = ag_fields_hillslope_loss_files_to_parquet(&sources, &ag_output, &version)
            .expect("write AgFields LOSS parquet");

        assert_eq!(ordinary_summary.rows_written, ag_summary.rows_written);
        assert_eq!(ordinary_summary.row_groups, ag_summary.row_groups);
        crate::ag_fields::assert_parquet_parity(&ordinary, &ag_output, &sources);
    }
}

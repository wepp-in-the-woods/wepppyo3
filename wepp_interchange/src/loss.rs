use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::arrow_support::{BoxedArray, Chunk};
use arrow_array::{Array, Float64Array, Int16Array, Int32Array, Int8Array};
use arrow_schema::{DataType, Schema};

use crate::arrays::string_array_from_optional_strings;
use crate::errors::InterchangeError;
use crate::parquet::{commit_staged, empty_chunk, ParquetSink, StagedParquet, WriteSummary};
use crate::schema::{field_with_meta, schema_with_version, VersionInfo};

const SCHEMA_VERSION: &str = "1";

const HILL_HEADER: [&str; 11] = [
    "Type",
    "wepp_id",
    "Runoff Volume",
    "Subrunoff Volume",
    "Baseflow Volume",
    "Soil Loss",
    "Sediment Deposition",
    "Sediment Yield",
    "Solub. React. Pollutant",
    "Particulate Pollutant",
    "Total Pollutant",
];

const HILL_AVG_HEADER: [&str; 12] = [
    "Type",
    "wepp_id",
    "Runoff Volume",
    "Subrunoff Volume",
    "Baseflow Volume",
    "Soil Loss",
    "Sediment Deposition",
    "Sediment Yield",
    "Hillslope Area",
    "Solub. React. Pollutant",
    "Particulate Pollutant",
    "Total Pollutant",
];

const HILL_UNITS: [Option<&str>; 11] = [
    None,
    None,
    Some("m^3"),
    Some("m^3"),
    Some("m^3"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
];

const HILL_AVG_UNITS: [Option<&str>; 12] = [
    None,
    None,
    Some("m^3"),
    Some("m^3"),
    Some("m^3"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("ha"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
];

const CHN_HEADER: [&str; 10] = [
    "Type",
    "chn_enum",
    "Discharge Volume",
    "Sediment Yield",
    "Soil Loss",
    "Upland Charge",
    "Subsuface Flow Volume",
    "Solub. React. Pollutant",
    "Particulate Pollutant",
    "Total Pollutant",
];

const CHN_AVG_HEADER: [&str; 11] = [
    "Type",
    "chn_enum",
    "Discharge Volume",
    "Sediment Yield",
    "Soil Loss",
    "Upland Charge",
    "Subsuface Flow Volume",
    "Contributing Area",
    "Solub. React. Pollutant",
    "Particulate Pollutant",
    "Total Pollutant",
];

const CHN_UNITS: [Option<&str>; 11] = [
    None,
    None,
    Some("m^3"),
    Some("tonne"),
    Some("kg"),
    Some("m^3"),
    Some("m^3"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
];

const CHN_AVG_UNITS: [Option<&str>; 12] = [
    None,
    None,
    Some("m^3"),
    Some("tonne"),
    Some("kg"),
    Some("m^3"),
    Some("m^3"),
    Some("ha"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
    Some("kg"),
];

const CLASS_HEADER: [&str; 8] = [
    "Class",
    "Diameter",
    "Specific Gravity",
    "Pct Sand",
    "Pct Silt",
    "Pct Clay",
    "Pct OM",
    "Fraction In Flow Exiting",
];

const CLASS_UNITS: [Option<&str>; 8] = [
    None,
    Some("mm"),
    None,
    Some("%"),
    Some("%"),
    Some("%"),
    Some("%"),
    Some(""),
];

const UNIT_CONSISTENCY_MAP: [(&str, &str); 3] = [
    ("T/ha/yr", "tonne/ha/yr"),
    ("tonnes/ha", "tonne/ha"),
    ("tonnes/yr", "tonne/yr"),
];

#[derive(Debug, Clone)]
enum LossValue {
    Int(i32),
    Float(f64),
    Str(String),
}

#[derive(Debug, Clone)]
struct OutRow {
    key: String,
    value: LossValue,
    units: String,
}

#[derive(Debug, Default)]
struct ParsedLossData {
    yearly_hill: Vec<Vec<LossValue>>,
    yearly_hill_years: Vec<i16>,
    yearly_chn: Vec<Vec<LossValue>>,
    yearly_chn_years: Vec<i16>,
    yearly_out: Vec<OutRow>,
    yearly_out_years: Vec<i16>,
    yearly_class: Vec<Vec<LossValue>>,
    yearly_class_years: Vec<i16>,
    average_hill: Vec<Vec<LossValue>>,
    average_chn: Vec<Vec<LossValue>>,
    average_out: Vec<OutRow>,
    average_class: Vec<Vec<LossValue>>,
    average_years: Option<i16>,
}

pub struct LossOutputs {
    pub paths: HashMap<String, PathBuf>,
    pub summaries: HashMap<String, WriteSummary>,
}

struct StagedLossOutputs {
    paths: HashMap<String, PathBuf>,
    keys: Vec<String>,
    staged: Vec<StagedParquet>,
}

pub fn watershed_loss_to_parquet(
    loss_path: &Path,
    output_dir: &Path,
    version: &VersionInfo,
) -> Result<LossOutputs, InterchangeError> {
    let staged_outputs = stage_watershed_loss_outputs(loss_path, output_dir, version)?;
    let write_summaries = commit_staged(staged_outputs.staged)?;
    let summaries = staged_outputs
        .keys
        .into_iter()
        .zip(write_summaries)
        .collect::<HashMap<_, _>>();
    Ok(LossOutputs {
        paths: staged_outputs.paths,
        summaries,
    })
}

fn stage_watershed_loss_outputs(
    loss_path: &Path,
    output_dir: &Path,
    version: &VersionInfo,
) -> Result<StagedLossOutputs, InterchangeError> {
    let parsed = parse_loss_file(loss_path)?;

    let hill_count = max_hill_id(&parsed.average_hill);

    let mut paths: HashMap<String, PathBuf> = HashMap::new();

    std::fs::create_dir_all(output_dir).map_err(|err| InterchangeError::io(output_dir, err))?;

    let (hill_all_schema, hill_avg_schema) = hill_schemas(version, parsed.average_years);
    let (chn_all_schema, chn_avg_schema) =
        chn_schemas(version, parsed.average_years, hill_count.is_some());
    let (out_all_schema, out_avg_schema) = out_schemas(version, parsed.average_years);
    let (class_all_schema, class_avg_schema) = class_schemas(version, parsed.average_years);

    let hill_all_chunk = build_chunk(
        &hill_all_schema,
        &HILL_HEADER,
        &parsed.yearly_hill,
        Some(&parsed.yearly_hill_years),
        None,
    )?;
    let hill_avg_chunk = build_chunk(
        &hill_avg_schema,
        &HILL_AVG_HEADER,
        &parsed.average_hill,
        None,
        None,
    )?;

    let chn_all_extra = hill_count.map(|count| {
        parsed
            .yearly_chn
            .iter()
            .map(|row| coerce_int(row.get(chn_enum_index())).map(|value| value + count))
            .collect::<Vec<_>>()
    });
    let chn_avg_extra = hill_count.map(|count| {
        parsed
            .average_chn
            .iter()
            .map(|row| coerce_int(row.get(chn_enum_index())).map(|value| value + count))
            .collect::<Vec<_>>()
    });

    let chn_all_chunk = build_chunk(
        &chn_all_schema,
        &CHN_HEADER,
        &parsed.yearly_chn,
        Some(&parsed.yearly_chn_years),
        chn_all_extra.as_deref(),
    )?;
    let chn_avg_chunk = build_chunk(
        &chn_avg_schema,
        &CHN_AVG_HEADER,
        &parsed.average_chn,
        None,
        chn_avg_extra.as_deref(),
    )?;

    let out_all_rows = out_rows_to_values(&parsed.yearly_out);
    let out_avg_rows = out_rows_to_values(&parsed.average_out);
    let out_all_chunk = build_chunk(
        &out_all_schema,
        &["key", "value", "units"],
        &out_all_rows,
        Some(&parsed.yearly_out_years),
        None,
    )?;
    let out_avg_chunk = build_chunk(
        &out_avg_schema,
        &["key", "value", "units"],
        &out_avg_rows,
        None,
        None,
    )?;

    let class_all_chunk = build_chunk(
        &class_all_schema,
        &CLASS_HEADER,
        &parsed.yearly_class,
        Some(&parsed.yearly_class_years),
        None,
    )?;
    let class_avg_chunk = build_chunk(
        &class_avg_schema,
        &CLASS_HEADER,
        &parsed.average_class,
        None,
        None,
    )?;

    let mapping = [
        ("average_hill", "loss_pw0.hill.parquet"),
        ("average_chn", "loss_pw0.chn.parquet"),
        ("average_out", "loss_pw0.out.parquet"),
        ("average_class", "loss_pw0.class_data.parquet"),
        ("all_years_hill", "loss_pw0.all_years.hill.parquet"),
        ("all_years_chn", "loss_pw0.all_years.chn.parquet"),
        ("all_years_out", "loss_pw0.all_years.out.parquet"),
        ("all_years_class", "loss_pw0.all_years.class_data.parquet"),
    ];

    let chunks = [
        ("average_hill", hill_avg_chunk, hill_avg_schema),
        ("average_chn", chn_avg_chunk, chn_avg_schema),
        ("average_out", out_avg_chunk, out_avg_schema),
        ("average_class", class_avg_chunk, class_avg_schema),
        ("all_years_hill", hill_all_chunk, hill_all_schema),
        ("all_years_chn", chn_all_chunk, chn_all_schema),
        ("all_years_out", out_all_chunk, out_all_schema),
        ("all_years_class", class_all_chunk, class_all_schema),
    ];

    for (key, filename) in mapping {
        let path = output_dir.join(filename);
        paths.insert(key.to_string(), path);
    }

    let mut keys = Vec::with_capacity(chunks.len());
    let mut staged = Vec::with_capacity(chunks.len());
    for (key, chunk, schema) in chunks {
        let path = paths.get(key).expect("output path").clone();
        let mut sink = ParquetSink::try_new(&path, schema.clone())?;
        if chunk.len() == 0 {
            sink.write_chunk(empty_chunk(&schema))?;
        } else {
            sink.write_chunk(chunk)?;
        }
        keys.push(key.to_string());
        staged.push(sink.finish_staged()?);
    }

    Ok(StagedLossOutputs {
        paths,
        keys,
        staged,
    })
}

fn hill_schemas(version: &VersionInfo, average_years: Option<i16>) -> (Schema, Schema) {
    let mut hill_fields = vec![field_with_meta("year", DataType::Int16, None, None)];
    hill_fields.push(field_with_meta("Type", DataType::Utf8, None, None));
    for (idx, name) in HILL_HEADER.iter().enumerate().skip(1) {
        let dtype = if idx == 1 {
            DataType::Int32
        } else {
            DataType::Float64
        };
        let units = HILL_UNITS[idx];
        hill_fields.push(field_with_meta(name, dtype, units, None));
    }
    let hill_schema = schema_with_version(
        Schema::new(hill_fields).with_metadata(loss_metadata("loss_pw0.all_years.hill")),
        version,
    );

    let mut avg_fields = vec![field_with_meta("Type", DataType::Utf8, None, None)];
    for (idx, name) in HILL_AVG_HEADER.iter().enumerate().skip(1) {
        let dtype = if idx == 1 {
            DataType::Int32
        } else {
            DataType::Float64
        };
        let units = HILL_AVG_UNITS[idx];
        avg_fields.push(field_with_meta(name, dtype, units, None));
    }
    let mut avg_schema = schema_with_version(
        Schema::new(avg_fields).with_metadata(loss_metadata("loss_pw0.hill")),
        version,
    );
    if let Some(avg) = average_years {
        let mut metadata = avg_schema.metadata().clone();
        metadata.insert("average_years".to_string(), avg.to_string());
        avg_schema = avg_schema.with_metadata(metadata);
    }

    (hill_schema, avg_schema)
}

fn chn_schemas(
    version: &VersionInfo,
    average_years: Option<i16>,
    append_wepp: bool,
) -> (Schema, Schema) {
    let mut chn_fields = vec![field_with_meta("year", DataType::Int16, None, None)];
    chn_fields.push(field_with_meta("Type", DataType::Utf8, None, None));
    for (idx, name) in CHN_HEADER.iter().enumerate().skip(1) {
        let dtype = if idx == 1 {
            DataType::Int32
        } else {
            DataType::Float64
        };
        let units = CHN_UNITS[idx];
        chn_fields.push(field_with_meta(name, dtype, units, None));
    }
    if append_wepp {
        chn_fields.push(field_with_meta("wepp_id", DataType::Int32, None, None));
    }
    let chn_schema = schema_with_version(
        Schema::new(chn_fields).with_metadata(loss_metadata("loss_pw0.all_years.chn")),
        version,
    );

    let mut avg_fields = vec![field_with_meta("Type", DataType::Utf8, None, None)];
    for (idx, name) in CHN_AVG_HEADER.iter().enumerate().skip(1) {
        let dtype = if idx == 1 {
            DataType::Int32
        } else {
            DataType::Float64
        };
        let units = CHN_AVG_UNITS[idx];
        avg_fields.push(field_with_meta(name, dtype, units, None));
    }
    if append_wepp {
        avg_fields.push(field_with_meta("wepp_id", DataType::Int32, None, None));
    }
    let mut avg_schema = schema_with_version(
        Schema::new(avg_fields).with_metadata(loss_metadata("loss_pw0.chn")),
        version,
    );
    if let Some(avg) = average_years {
        let mut metadata = avg_schema.metadata().clone();
        metadata.insert("average_years".to_string(), avg.to_string());
        avg_schema = avg_schema.with_metadata(metadata);
    }

    (chn_schema, avg_schema)
}

fn out_schemas(version: &VersionInfo, average_years: Option<i16>) -> (Schema, Schema) {
    let all_schema = Schema::new(vec![
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta("key", DataType::Utf8, None, None),
        field_with_meta("value", DataType::Float64, None, None),
        field_with_meta("units", DataType::Utf8, None, None),
    ])
    .with_metadata(loss_metadata("loss_pw0.all_years.out"));
    let all_schema = schema_with_version(all_schema, version);

    let mut avg_schema = schema_with_version(
        Schema::new(vec![
            field_with_meta("key", DataType::Utf8, None, None),
            field_with_meta("value", DataType::Float64, None, None),
            field_with_meta("units", DataType::Utf8, None, None),
        ])
        .with_metadata(loss_metadata("loss_pw0.out")),
        version,
    );
    if let Some(avg) = average_years {
        let mut metadata = avg_schema.metadata().clone();
        metadata.insert("average_years".to_string(), avg.to_string());
        avg_schema = avg_schema.with_metadata(metadata);
    }

    (all_schema, avg_schema)
}

fn class_schemas(version: &VersionInfo, average_years: Option<i16>) -> (Schema, Schema) {
    let mut all_fields = vec![field_with_meta("year", DataType::Int16, None, None)];
    for (idx, name) in CLASS_HEADER.iter().enumerate() {
        let dtype = if *name == "Class" {
            DataType::Int8
        } else {
            DataType::Float64
        };
        all_fields.push(field_with_meta(name, dtype, CLASS_UNITS[idx], None));
    }
    let all_schema = schema_with_version(
        Schema::new(all_fields).with_metadata(loss_metadata("loss_pw0.all_years.class_data")),
        version,
    );

    let mut avg_fields = Vec::new();
    for (idx, name) in CLASS_HEADER.iter().enumerate() {
        let dtype = if *name == "Class" {
            DataType::Int8
        } else {
            DataType::Float64
        };
        avg_fields.push(field_with_meta(name, dtype, CLASS_UNITS[idx], None));
    }
    let mut avg_schema = schema_with_version(
        Schema::new(avg_fields).with_metadata(loss_metadata("loss_pw0.class_data")),
        version,
    );
    if let Some(avg) = average_years {
        let mut metadata = avg_schema.metadata().clone();
        metadata.insert("average_years".to_string(), avg.to_string());
        avg_schema = avg_schema.with_metadata(metadata);
    }

    (all_schema, avg_schema)
}

fn loss_metadata(table: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    metadata.insert("schema_version".to_string(), SCHEMA_VERSION.to_string());
    metadata.insert("table".to_string(), table.to_string());
    metadata
}

fn max_hill_id(rows: &[Vec<LossValue>]) -> Option<i32> {
    if rows.is_empty() {
        return None;
    }
    let idx = hill_wepp_index();
    rows.iter().filter_map(|row| coerce_int(row.get(idx))).max()
}

fn hill_wepp_index() -> usize {
    HILL_AVG_HEADER
        .iter()
        .position(|name| *name == "wepp_id")
        .unwrap_or(1)
}

fn chn_enum_index() -> usize {
    CHN_HEADER
        .iter()
        .position(|name| *name == "chn_enum")
        .unwrap_or(1)
}

fn out_rows_to_values(rows: &[OutRow]) -> Vec<Vec<LossValue>> {
    rows.iter()
        .map(|row| {
            vec![
                LossValue::Str(row.key.clone()),
                row.value.clone(),
                LossValue::Str(row.units.clone()),
            ]
        })
        .collect()
}

fn build_chunk(
    schema: &Schema,
    header: &[&str],
    rows: &[Vec<LossValue>],
    years: Option<&[i16]>,
    extra_wepp: Option<&[Option<i32>]>,
) -> Result<Chunk<Box<dyn Array>>, InterchangeError> {
    let mut header_index: HashMap<&str, usize> = HashMap::new();
    for (idx, name) in header.iter().enumerate() {
        header_index.insert(*name, idx);
    }

    let mut arrays: Vec<Box<dyn Array>> = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let name = field.name().as_str();
        match field.data_type() {
            DataType::Int8 => {
                let mut values: Vec<Option<i8>> = Vec::with_capacity(rows.len());
                for (row_idx, row) in rows.iter().enumerate() {
                    let value = if name == "year" {
                        years
                            .and_then(|vals| vals.get(row_idx).copied())
                            .map(|v| v as i8)
                    } else if name == "wepp_id"
                        && extra_wepp.is_some()
                        && !header_index.contains_key(name)
                    {
                        extra_wepp
                            .and_then(|vals| vals.get(row_idx).cloned())
                            .flatten()
                            .map(|v| v as i8)
                    } else {
                        header_index
                            .get(name)
                            .and_then(|idx| coerce_int(row.get(*idx)).map(|v| v as i8))
                    };
                    values.push(value);
                }
                arrays.push(Int8Array::from(values).boxed());
            }
            DataType::Int16 => {
                let mut values: Vec<Option<i16>> = Vec::with_capacity(rows.len());
                for (row_idx, row) in rows.iter().enumerate() {
                    let value = if name == "year" {
                        years.and_then(|vals| vals.get(row_idx).copied())
                    } else if name == "wepp_id"
                        && extra_wepp.is_some()
                        && !header_index.contains_key(name)
                    {
                        extra_wepp
                            .and_then(|vals| vals.get(row_idx).cloned())
                            .flatten()
                            .map(|v| v as i16)
                    } else {
                        header_index
                            .get(name)
                            .and_then(|idx| coerce_int(row.get(*idx)).map(|v| v as i16))
                    };
                    values.push(value);
                }
                arrays.push(Int16Array::from(values).boxed());
            }
            DataType::Int32 => {
                let mut values: Vec<Option<i32>> = Vec::with_capacity(rows.len());
                for (row_idx, row) in rows.iter().enumerate() {
                    let value = if name == "year" {
                        years
                            .and_then(|vals| vals.get(row_idx).copied())
                            .map(|v| v as i32)
                    } else if name == "wepp_id"
                        && extra_wepp.is_some()
                        && !header_index.contains_key(name)
                    {
                        extra_wepp
                            .and_then(|vals| vals.get(row_idx).cloned())
                            .flatten()
                    } else {
                        header_index
                            .get(name)
                            .and_then(|idx| coerce_int(row.get(*idx)))
                    };
                    values.push(value);
                }
                arrays.push(Int32Array::from(values).boxed());
            }
            DataType::Float64 => {
                let mut values: Vec<Option<f64>> = Vec::with_capacity(rows.len());
                for (row_idx, row) in rows.iter().enumerate() {
                    let value = if name == "year" {
                        years
                            .and_then(|vals| vals.get(row_idx).copied())
                            .map(|v| v as f64)
                    } else if name == "wepp_id"
                        && extra_wepp.is_some()
                        && !header_index.contains_key(name)
                    {
                        extra_wepp
                            .and_then(|vals| vals.get(row_idx).cloned())
                            .flatten()
                            .map(|v| v as f64)
                    } else {
                        header_index
                            .get(name)
                            .and_then(|idx| coerce_float(row.get(*idx)))
                    };
                    values.push(value);
                }
                arrays.push(Float64Array::from(values).boxed());
            }
            DataType::Utf8 => {
                let mut values: Vec<Option<String>> = Vec::with_capacity(rows.len());
                for (row_idx, row) in rows.iter().enumerate() {
                    let value = if name == "year" {
                        years
                            .and_then(|vals| vals.get(row_idx).copied())
                            .map(|v| v.to_string())
                    } else if name == "wepp_id"
                        && extra_wepp.is_some()
                        && !header_index.contains_key(name)
                    {
                        extra_wepp
                            .and_then(|vals| vals.get(row_idx).cloned())
                            .flatten()
                            .map(|v| v.to_string())
                    } else {
                        header_index
                            .get(name)
                            .and_then(|idx| coerce_string(row.get(*idx)))
                    };
                    values.push(value);
                }
                arrays.push(string_array_from_optional_strings(values).boxed());
            }
            _ => {
                return Err(InterchangeError::Arrow(format!(
                    "Unsupported loss column '{}' type {:?}",
                    field.name(),
                    field.data_type()
                )))
            }
        }
    }

    Ok(Chunk::new(arrays))
}

fn coerce_int(value: Option<&LossValue>) -> Option<i32> {
    match value? {
        LossValue::Int(v) => Some(*v),
        LossValue::Float(v) => {
            if v.is_nan() {
                None
            } else {
                Some(*v as i32)
            }
        }
        LossValue::Str(s) => {
            let stripped = s.trim();
            if stripped.is_empty() {
                None
            } else {
                parse_float_strict(stripped).and_then(|v| {
                    if v.is_nan() {
                        None
                    } else {
                        Some(v as i32)
                    }
                })
            }
        }
    }
}

fn coerce_float(value: Option<&LossValue>) -> Option<f64> {
    match value? {
        LossValue::Int(v) => Some(*v as f64),
        LossValue::Float(v) => Some(*v),
        LossValue::Str(s) => {
            let stripped = s.trim();
            if stripped.is_empty() {
                None
            } else if stripped == "********" {
                Some(f64::NAN)
            } else {
                parse_float_strict(stripped)
            }
        }
    }
}

fn coerce_string(value: Option<&LossValue>) -> Option<String> {
    match value? {
        LossValue::Int(v) => Some(v.to_string()),
        LossValue::Float(v) => Some(v.to_string()),
        LossValue::Str(s) => Some(s.clone()),
    }
}

fn parse_float_strict(token: &str) -> Option<f64> {
    let mut candidate = token.trim();
    if candidate.is_empty() {
        return None;
    }
    let mut owned = String::new();
    if candidate.starts_with('.') {
        owned.push('0');
        owned.push_str(candidate);
        candidate = owned.as_str();
    }
    fast_float::parse::<f64, _>(candidate).ok()
}

fn parse_loss_file(path: &Path) -> Result<ParsedLossData, InterchangeError> {
    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let mut lines: Vec<String> = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|err| InterchangeError::io(path, err))?;
        let cleaned = line.replace("*** total soil loss < 1 kg ***", "");
        lines.push(cleaned.trim().to_string());
    }

    let mut yearly_sections: Vec<(usize, i16)> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.contains("ANNUAL SUMMARY FOR WATERSHED IN YEAR") {
            if let Some(year) = extract_year(line) {
                yearly_sections.push((idx, year));
            } else {
                return Err(InterchangeError::parse(
                    path,
                    Some(idx + 1),
                    "Unable to extract year from line",
                    Some(line.clone()),
                ));
            }
        }
    }

    let mut average_idx: Option<usize> = None;
    let mut average_years: Option<i16> = None;
    for (idx, line) in lines.iter().enumerate() {
        if line.contains("YEAR AVERAGE ANNUAL VALUES FOR WATERSHED") {
            average_idx = Some(idx);
            average_years = extract_first_number(line);
            break;
        }
    }

    let average_idx = average_idx.ok_or_else(|| {
        InterchangeError::parse(
            path,
            None,
            "Average annual section not found in loss file.",
            None,
        )
    })?;

    let mut section_indices: Vec<usize> = yearly_sections.iter().map(|(idx, _)| *idx).collect();
    section_indices.push(average_idx);
    section_indices.sort_unstable();

    let mut parsed = ParsedLossData::default();
    parsed.average_years = average_years;

    for (idx, year) in yearly_sections {
        let (hill_start, chn_start, out_start) = find_tbl_starts(idx, &lines, path)?;
        let next_section = section_indices
            .iter()
            .copied()
            .find(|pos| *pos > idx)
            .unwrap_or(lines.len());

        let hill_rows = parse_tbl(&lines[hill_start..], HILL_HEADER.len(), path, hill_start)?;
        for row in hill_rows {
            parsed.yearly_hill.push(row);
            parsed.yearly_hill_years.push(year);
        }

        let chn_rows = parse_tbl(&lines[chn_start..], CHN_HEADER.len(), path, chn_start)?;
        for row in chn_rows {
            parsed.yearly_chn.push(row);
            parsed.yearly_chn_years.push(year);
        }

        let out_rows = parse_out(&lines[out_start..]);
        for row in out_rows {
            parsed.yearly_out.push(row);
            parsed.yearly_out_years.push(year);
        }

        let class_rows = collect_class_block(&lines, out_start, next_section, path)?;
        for row in class_rows {
            parsed.yearly_class.push(row);
            parsed.yearly_class_years.push(year);
        }
    }

    let (avg_hill_start, avg_chn_start, avg_out_start) =
        find_tbl_starts(average_idx, &lines, path)?;
    parsed.average_hill = parse_tbl(
        &lines[avg_hill_start..],
        HILL_AVG_HEADER.len(),
        path,
        avg_hill_start,
    )?;
    parsed.average_chn = parse_tbl(
        &lines[avg_chn_start..],
        CHN_AVG_HEADER.len(),
        path,
        avg_chn_start,
    )?;
    parsed.average_out = parse_out(&lines[avg_out_start..]);
    parsed.average_class = collect_class_block(&lines, avg_out_start, lines.len(), path)?;

    Ok(parsed)
}

fn extract_year(line: &str) -> Option<i16> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for idx in 0..parts.len().saturating_sub(1) {
        if parts[idx] == "YEAR" {
            if let Ok(val) = parts[idx + 1].parse::<i16>() {
                return Some(val);
            }
        }
    }
    None
}

fn extract_first_number(line: &str) -> Option<i16> {
    for token in line.split_whitespace() {
        if token.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(val) = token.parse::<i16>() {
                return Some(val);
            }
        }
    }
    None
}

fn find_tbl_starts(
    section_index: usize,
    lines: &[String],
    path: &Path,
) -> Result<(usize, usize, usize), InterchangeError> {
    let mut header_idx: Vec<usize> = Vec::new();
    for (offset, line) in lines.iter().enumerate().skip(section_index + 2) {
        if line.starts_with("----") {
            header_idx.push(offset);
        }
        if header_idx.len() == 3 {
            break;
        }
    }
    if header_idx.len() < 3 {
        return Err(InterchangeError::parse(
            path,
            Some(section_index + 1),
            "Unable to locate table separators in loss file.",
            None,
        ));
    }

    let hill0 = header_idx[0] + 1;
    let chn0 = header_idx[1] + 2;
    let out0 = header_idx[2] + 2;
    Ok((hill0, chn0, out0))
}

fn parse_tbl(
    lines: &[String],
    header_len: usize,
    path: &Path,
    start_line: usize,
) -> Result<Vec<Vec<LossValue>>, InterchangeError> {
    let mut data: Vec<Vec<LossValue>> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if line.is_empty() {
            break;
        }
        let mut row: Vec<LossValue> = Vec::new();
        let line_no = start_line + idx + 1;
        for token in line.split_whitespace() {
            extend_row_from_token(&mut row, token, line, path, line_no)?;
        }
        if row.len() != header_len {
            return Err(InterchangeError::parse(
                path,
                Some(line_no),
                format!(
                    "Unexpected column count while parsing loss table: expected {header_len}, got {}, line={line:?}",
                    row.len()
                ),
                None,
            ));
        }
        data.push(row);
    }
    Ok(data)
}

fn extend_row_from_token(
    row: &mut Vec<LossValue>,
    token: &str,
    line: &str,
    path: &Path,
    line_no: usize,
) -> Result<(), InterchangeError> {
    let dot_count = token.as_bytes().iter().filter(|b| **b == b'.').count();

    if dot_count == 2 {
        let idx = token.find('.').unwrap_or(0);
        let cut = std::cmp::min(idx + 3, token.len());
        let part0 = &token[..cut];
        let part1 = &token[cut..];
        row.push(LossValue::Float(parse_required_float(
            part0, path, line_no, line,
        )?));
        row.push(LossValue::Float(parse_required_float(
            part1, path, line_no, line,
        )?));
        return Ok(());
    }

    if dot_count == 3 {
        let part0 = token.get(..3).unwrap_or(token);
        let part1 = token.get(3..11).unwrap_or("");
        let part2 = token.get(11..).unwrap_or("");
        row.push(LossValue::Float(parse_required_float(
            part0, path, line_no, line,
        )?));
        row.push(LossValue::Float(parse_required_float(
            part1, path, line_no, line,
        )?));
        row.push(LossValue::Float(parse_required_float(
            part2, path, line_no, line,
        )?));
        return Ok(());
    }

    if token.contains('.') {
        if token.contains('*') {
            let mut cleaned = token.replace('*', "");
            if cleaned.ends_with('.') {
                cleaned.pop();
            }
            if token.ends_with('*') {
                if cleaned.is_empty() {
                    row.push(LossValue::Float(f64::NAN));
                } else {
                    row.push(LossValue::Float(parse_required_float(
                        &cleaned, path, line_no, line,
                    )?));
                }
                row.push(LossValue::Str("********".to_string()));
            } else {
                row.push(LossValue::Str("********".to_string()));
                row.push(LossValue::Float(parse_required_float(
                    &cleaned, path, line_no, line,
                )?));
            }
        } else if let Some(value) = parse_float_strict(token) {
            row.push(LossValue::Float(value));
        } else {
            row.push(LossValue::Str(token.trim().to_string()));
        }
        return Ok(());
    }

    if let Ok(value) = token.parse::<i32>() {
        row.push(LossValue::Int(value));
    } else if token.contains("NaN") {
        row.push(LossValue::Float(f64::NAN));
    } else {
        row.push(LossValue::Str(token.trim().to_string()));
    }
    Ok(())
}

fn parse_required_float(
    token: &str,
    path: &Path,
    line_no: usize,
    line: &str,
) -> Result<f64, InterchangeError> {
    parse_float_strict(token).ok_or_else(|| {
        InterchangeError::parse(
            path,
            Some(line_no),
            "Unable to parse loss float value",
            Some(truncate_line(line)),
        )
    })
}

fn parse_out(lines: &[String]) -> Vec<OutRow> {
    let mut data: Vec<OutRow> = Vec::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if !line.contains('=') {
            break;
        }
        let mut parts = line.splitn(2, '=');
        let key_part = parts.next().unwrap_or("");
        let value_part = parts.next().unwrap_or("");
        let value_tokens: Vec<&str> = value_part.split_whitespace().collect();

        let (value_str, units) = if value_tokens.len() == 2 {
            let units = normalize_units(value_tokens[1].trim());
            (value_tokens[0], units.to_string())
        } else if value_tokens.len() >= 1 {
            (value_tokens[0], "".to_string())
        } else {
            ("", "".to_string())
        };

        let key = key_part.trim().to_string();
        let value_str = value_str.trim();
        let value = if value_str.contains('.') {
            if let Some(val) = parse_float_strict(value_str) {
                LossValue::Float(val)
            } else {
                LossValue::Str(value_str.to_string())
            }
        } else if let Ok(val) = value_str.parse::<i32>() {
            LossValue::Int(val)
        } else if value_str.contains("NaN") {
            LossValue::Float(f64::NAN)
        } else {
            LossValue::Str(value_str.to_string())
        };

        data.push(OutRow { key, value, units });
    }
    data
}

fn normalize_units(units: &str) -> &str {
    for (src, target) in UNIT_CONSISTENCY_MAP {
        if units == src {
            return target;
        }
    }
    units
}

fn collect_class_block(
    lines: &[String],
    start: usize,
    end: usize,
    path: &Path,
) -> Result<Vec<Vec<LossValue>>, InterchangeError> {
    let mut target_line = None;
    for idx in start..end {
        if lines[idx]
            .to_lowercase()
            .contains("sediment particle information leaving")
        {
            target_line = Some(idx);
            break;
        }
    }
    let Some(target) = target_line else {
        return Ok(Vec::new());
    };
    extract_class_data(&lines[target + 1..end], path, target + 1)
}

fn extract_class_data(
    lines: &[String],
    path: &Path,
    start_line: usize,
) -> Result<Vec<Vec<LossValue>>, InterchangeError> {
    let mut class_lines: Vec<String> = Vec::new();
    for line in lines {
        if line.is_empty() {
            if !class_lines.is_empty() {
                break;
            }
            continue;
        }
        let stripped = line.trim();
        if stripped.chars().all(|c| c == '-') {
            continue;
        }
        if stripped
            .to_lowercase()
            .starts_with("distribution of primary particles")
        {
            break;
        }
        if stripped
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            class_lines.push(line.clone());
        }
    }
    if class_lines.is_empty() {
        return Ok(Vec::new());
    }
    parse_tbl(&class_lines, CLASS_HEADER.len(), path, start_line)
}

fn truncate_line(line: &str) -> String {
    const LIMIT: usize = 160;
    if line.len() <= LIMIT {
        line.to_string()
    } else {
        format!("{}...", &line[..LIMIT])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const OUTPUT_FILENAMES: [&str; 8] = [
        "loss_pw0.hill.parquet",
        "loss_pw0.chn.parquet",
        "loss_pw0.out.parquet",
        "loss_pw0.class_data.parquet",
        "loss_pw0.all_years.hill.parquet",
        "loss_pw0.all_years.chn.parquet",
        "loss_pw0.all_years.out.parquet",
        "loss_pw0.all_years.class_data.parquet",
    ];

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wepp_interchange_loss_atomic_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    fn minimal_loss_fixture() -> &'static str {
        "1 YEAR AVERAGE ANNUAL VALUES FOR WATERSHED\n\
         header\n\
         ----\n\
         Hill 1 1 1 1 1 1 1 1 1 1 1\n\
         \n\
         ----\n\
         channel header\n\
         Channel 1 1 1 1 1 1 1 1 1 1\n\
         \n\
         ----\n\
         outlet header\n\
         Total = 1.0 mm\n\
         \n"
    }

    fn temporary_artifacts(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .expect("read output directory")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().contains(".wepp-"))
                    .unwrap_or(false)
            })
            .collect()
    }

    #[test]
    fn watershed_loss_later_output_failure_restores_all_eight_prior_outputs() {
        let dir = temp_dir();
        let source = dir.join("loss_pw0.txt");
        fs::write(&source, minimal_loss_fixture()).expect("write LOSS fixture");
        for filename in OUTPUT_FILENAMES {
            fs::write(dir.join(filename), format!("old-{filename}")).expect("write prior output");
        }

        let staged = stage_watershed_loss_outputs(&source, &dir, &VersionInfo::new(1, 2))
            .expect("stage LOSS outputs");
        assert_eq!(staged.staged.len(), 8);
        crate::parquet::commit_staged_with_failure(staged.staged, 6)
            .expect_err("later LOSS output publication must fail");

        for filename in OUTPUT_FILENAMES {
            assert_eq!(
                fs::read(dir.join(filename)).expect("read restored output"),
                format!("old-{filename}").as_bytes()
            );
        }
        assert!(temporary_artifacts(&dir).is_empty());
        fs::remove_dir_all(dir).expect("cleanup LOSS fixture");
    }
}

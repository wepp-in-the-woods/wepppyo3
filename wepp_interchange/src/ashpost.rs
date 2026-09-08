//! Streaming AshPost extraction; Python owns model and run lifecycle policy.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use arrow_array::*;
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;
use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;

use crate::arrow_support::Chunk;
use crate::parquet::ParquetSink;

pub type ManifestEntry = (String, i64, f64, i64);
pub type Metadata = HashMap<String, HashMap<String, String>>;
pub type Row = Vec<f64>;
pub const TRANSPORT: [&str; 3] = ["wind_transport", "water_transport", "ash_transport"];
const OPTIONAL: [&str; 3] = [
    "transportable_ash (tonne/ha)",
    "ash_depth (mm)",
    "ash_runoff (mm)",
];
const KEYS: [&str; 4] = ["year0", "year", "julian", "days_from_fire (days)"];
const BATCH: usize = 8192;

pub fn invalid(message: impl ToString) -> PyErr {
    PyValueError::new_err(message.to_string())
}
fn io(error: crate::errors::InterchangeError) -> PyErr {
    match error {
        crate::errors::InterchangeError::Io { source, .. } => source.into(),
        other => PyOSError::new_err(other.to_string()),
    }
}

#[derive(Clone, Copy, Default)]
struct Sum {
    count: usize,
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, x: f64) {
        if x.is_nan() {
            return;
        }
        self.count += 1;
        if self.value.is_infinite() {
            return;
        }
        let y = x - self.correction;
        let t = self.value + y;
        self.correction = (t - self.value) - y;
        self.value = t;
    }
}
#[derive(Clone, Copy, Default)]
struct AreaSum {
    value: f32,
    correction: f32,
}
impl AreaSum {
    fn add(&mut self, x: f32) {
        if self.value.is_infinite() {
            return;
        }
        let y = x - self.correction;
        let t = self.value + y;
        self.correction = (t - self.value) - y;
        self.value = t;
    }
}

fn number(array: &dyn Array, row: usize) -> PyResult<f64> {
    if !matches!(
        array.data_type(),
        DataType::Float64
            | DataType::Float32
            | DataType::Int64
            | DataType::Int32
            | DataType::Int16
            | DataType::Int8
            | DataType::UInt64
            | DataType::UInt32
            | DataType::UInt16
            | DataType::UInt8
            | DataType::Null
    ) {
        return Err(invalid("AshPost requires numeric columns"));
    }
    if array.is_null(row) || array.data_type() == &DataType::Null {
        return Ok(f64::NAN);
    }
    macro_rules! value {
        ($a:ty) => {
            array
                .as_any()
                .downcast_ref::<$a>()
                .ok_or_else(|| invalid("invalid numeric array"))?
                .value(row) as f64
        };
    }
    let value = match array.data_type() {
        DataType::Float64 => value!(Float64Array),
        DataType::Float32 => value!(Float32Array),
        DataType::Int64 => value!(Int64Array),
        DataType::Int32 => value!(Int32Array),
        DataType::Int16 => value!(Int16Array),
        DataType::Int8 => value!(Int8Array),
        DataType::UInt64 => value!(UInt64Array),
        DataType::UInt32 => value!(UInt32Array),
        DataType::UInt16 => value!(UInt16Array),
        DataType::UInt8 => value!(UInt8Array),
        _ => return Err(invalid("AshPost requires numeric columns")),
    };
    if value.is_infinite() {
        return Err(invalid("infinite AshPost numeric value"));
    }
    Ok(value)
}
fn key(value: f64, name: &str) -> PyResult<i64> {
    if !value.is_finite() || value.fract() != 0.0 || !(0.0..=65535.0).contains(&value) {
        return Err(invalid(format!("invalid AshPost {name}: {value}")));
    }
    Ok(value as i64)
}
fn contained(root: &Path, relative: &str) -> PyResult<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("AshPost input path must be relative and contained"));
    }
    let mut current = root.to_path_buf();
    for component in path.components() {
        current.push(component);
        let meta = fs::symlink_metadata(&current)?;
        if meta.file_type().is_symlink() {
            return Err(invalid("AshPost input symlink is not allowed"));
        }
    }
    if !current.is_file() {
        return Err(invalid("AshPost input is not a regular file"));
    }
    Ok(current)
}
fn schema(path: &Path) -> PyResult<Schema> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?).map_err(invalid)?;
    Ok(reader.schema().as_ref().clone())
}
fn scan(
    path: &Path,
    names: &[String],
    mut consume: impl FnMut(&RecordBatch) -> PyResult<()>,
) -> PyResult<usize> {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?).map_err(invalid)?;
    let indices = names
        .iter()
        .map(|n| builder.schema().index_of(n).map_err(invalid))
        .collect::<PyResult<Vec<_>>>()?;
    let mask = ProjectionMask::roots(builder.parquet_schema(), indices);
    let reader = builder
        .with_projection(mask)
        .with_batch_size(BATCH)
        .build()
        .map_err(invalid)?;
    let mut count = 0usize;
    for batch in reader {
        let batch = batch.map_err(invalid)?;
        count = count
            .checked_add(batch.num_rows())
            .ok_or_else(|| invalid("AshPost row count overflow"))?;
        consume(&batch)?;
    }
    Ok(count)
}
fn column<'a>(batch: &'a RecordBatch, name: &str) -> PyResult<&'a dyn Array> {
    batch
        .column_by_name(name)
        .map(|c| c.as_ref())
        .ok_or_else(|| invalid(format!("missing AshPost column {name}")))
}

#[derive(Clone)]
struct HillDay {
    keys: [i64; 4],
    metrics: Vec<Sum>,
}
#[derive(Clone)]
struct Aggregate {
    keys: [i64; 4],
    area: AreaSum,
    metrics: Vec<Sum>,
}
impl Aggregate {
    fn new(keys: [i64; 4], count: usize) -> Self {
        Self {
            keys,
            area: AreaSum::default(),
            metrics: vec![Sum::default(); count],
        }
    }
    fn add(&mut self, area: f32, metrics: &[f64]) {
        self.area.add(area);
        for (sum, value) in self.metrics.iter_mut().zip(metrics) {
            sum.add(*value);
        }
    }
}

pub struct Table {
    pub name: &'static str,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}
impl Table {
    pub fn index(&self, name: &str) -> PyResult<usize> {
        self.columns
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| invalid(format!("missing output {name}")))
    }
    fn write(&self, directory: &Path, metadata: &Metadata) -> PyResult<()> {
        let fields = self
            .columns
            .iter()
            .map(|name| {
                let dtype = if name == "burn_class" {
                    DataType::UInt8
                } else if KEYS.contains(&name.as_str()) || name == "topaz_id" {
                    DataType::UInt16
                } else if name == "area (ha)" {
                    DataType::Float32
                } else {
                    DataType::Float64
                };
                let mut meta = metadata.get(name).cloned().unwrap_or_default();
                if let Some((_, unit)) = name.rsplit_once(" (") {
                    if let Some(unit) = unit.strip_suffix(')') {
                        meta.entry("units".into()).or_insert(unit.into());
                    }
                }
                Field::new(name, dtype, true).with_metadata(meta)
            })
            .collect::<Vec<_>>();
        let schema = Schema::new(fields).with_metadata(HashMap::from([
            ("dataset_name".into(), "ashpost".into()),
            ("dataset_version".into(), "1.0".into()),
            ("dataset_version_major".into(), "1".into()),
            ("dataset_version_minor".into(), "0".into()),
        ]));
        let path = directory.join(self.name);
        let mode = match fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() => Some(m.permissions().mode() & 0o777),
            Ok(_) => return Err(invalid("AshPost output must be a regular file")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let mut sink = ParquetSink::try_new_with_mode(&path, schema.clone(), mode).map_err(io)?;
        for rows in self.rows.chunks(BATCH) {
            let arrays = schema
                .fields()
                .iter()
                .enumerate()
                .map(|(col, field)| -> PyResult<Box<dyn Array>> {
                    let values = rows.iter().map(|r| r[col]).collect::<Vec<_>>();
                    Ok(match field.data_type() {
                        DataType::UInt16 => Box::new(UInt16Array::from(
                            values
                                .iter()
                                .map(|v| key(*v, field.name()).map(|x| x as u16))
                                .collect::<PyResult<Vec<_>>>()?,
                        )),
                        DataType::UInt8 => Box::new(UInt8Array::from(
                            values.iter().map(|v| *v as u8).collect::<Vec<_>>(),
                        )),
                        DataType::Float32 => Box::new(Float32Array::from(
                            values
                                .iter()
                                .map(|v| if v.is_nan() { None } else { Some(*v as f32) })
                                .collect::<Vec<_>>(),
                        )),
                        _ => Box::new(Float64Array::from(
                            values
                                .iter()
                                .map(|v| if v.is_nan() { None } else { Some(*v) })
                                .collect::<Vec<_>>(),
                        )),
                    })
                })
                .collect::<PyResult<Vec<_>>>()?;
            sink.write_chunk(Chunk::new(arrays)).map_err(io)?;
        }
        sink.finish().map_err(io)?;
        Ok(())
    }
}

fn mass_columns(metrics: &[String]) -> Vec<String> {
    metrics
        .iter()
        .map(|m| m.replace("tonne/ha", "tonne").replace("mm", "m^3"))
        .collect()
}
fn summarize_columns(prefix: &[&str], metrics: &[String]) -> Vec<String> {
    let mut columns = prefix.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    for m in mass_columns(metrics) {
        columns.push(m.clone());
        columns.push(m.replace("tonne)", "tonne/ha)").replace("m^3)", "mm)"));
    }
    columns
}
fn summary_row(prefix: Vec<f64>, aggregate: &Aggregate, metrics: &[String]) -> Row {
    let mut row = prefix;
    let area = aggregate.area.value as f64;
    for (name, sum) in metrics.iter().zip(&aggregate.metrics) {
        row.push(sum.value);
        let mut per_area = if area > 0.0 { sum.value / area } else { 0.0 };
        if name.ends_with("(mm)") {
            per_area *= 1000.0 / 10000.0;
        }
        row.push(per_area);
    }
    row
}

pub struct Products {
    input_identities: HashSet<(u64, u64)>,
    pub input_rows: usize,
    pub tables: Vec<Table>,
}

pub fn aggregate(
    root: &Path,
    manifest: &[ManifestEntry],
    hydrology: Option<&str>,
    wat: Option<&str>,
    ash_ids: &[i64],
) -> PyResult<Products> {
    let root = fs::canonicalize(root)?;
    let mut identities = HashSet::new();
    let mut ids = HashSet::new();
    let mut inputs = Vec::new();
    let mut contract: Option<Vec<(String, DataType)>> = None;
    let mut metrics = TRANSPORT
        .iter()
        .map(|m| format!("{m} (tonne/ha)"))
        .collect::<Vec<_>>();
    let mut cumulative = Vec::new();
    for (relative, id, area, class) in manifest {
        if *id <= 0
            || *id > 65535
            || !(0..=4).contains(class)
            || !area.is_finite()
            || *area < 0.0
            || !(*area as f32).is_finite()
        {
            return Err(invalid("invalid AshPost manifest metadata"));
        }
        let path = contained(&root, relative)?;
        let meta = fs::metadata(&path)?;
        if !ids.insert(*id) || !identities.insert((meta.dev(), meta.ino())) {
            return Err(invalid("duplicate AshPost input or Topaz ID"));
        }
        let schema = schema(&path)?;
        let fields = schema
            .fields()
            .iter()
            .filter(|f| f.name() != "__index_level_0__")
            .map(|f| (f.name().clone(), f.data_type().clone()))
            .collect::<Vec<_>>();
        if let Some(previous) = &contract {
            if previous != &fields {
                return Err(invalid("mixed AshPost model schemas"));
            }
        } else {
            for name in OPTIONAL {
                if schema.index_of(name).is_ok() {
                    metrics.push(name.into());
                }
            }
            cumulative = schema
                .fields()
                .iter()
                .filter(|f| {
                    f.name().starts_with("cum_")
                        && (f.name().contains("(mm)") || f.name().contains("(tonne/ha)"))
                })
                .map(|f| f.name().clone())
                .collect();
            contract = Some(fields);
        }
        for name in KEYS
            .iter()
            .map(|s| s.to_string())
            .chain(TRANSPORT.iter().map(|m| format!("{m} (tonne/ha)")))
            .chain(TRANSPORT.iter().map(|m| format!("cum_{m} (tonne/ha)")))
        {
            schema.index_of(&name).map_err(invalid)?;
        }
        for name in metrics.iter().chain(&cumulative) {
            if schema.field_with_name(name).map_err(invalid)?.data_type() != &DataType::Float64 {
                return Err(invalid(format!(
                    "AshPost model metric {name} must be Float64"
                )));
            }
        }
        inputs.push((path, *id, *area as f32, *class));
    }
    let mut daily: BTreeMap<[i64; 3], Aggregate> = BTreeMap::new();
    let mut annual: BTreeMap<i64, Aggregate> = BTreeMap::new();
    let mut classes: BTreeMap<[i64; 4], Aggregate> = BTreeMap::new();
    let mut hills: BTreeMap<(i64, i64), [Sum; 3]> = BTreeMap::new();
    let mut cumulatives: BTreeMap<i64, (i64, Vec<Sum>)> = BTreeMap::new();
    let mut cumulative_names = cumulative.clone();
    cumulative_names.extend(
        cumulative
            .iter()
            .filter(|s| s.contains("tonne/ha"))
            .map(|s| s.replace("tonne/ha", "tonne")),
    );
    cumulative_names.extend(
        cumulative
            .iter()
            .filter(|s| s.contains("mm"))
            .map(|s| s.replace("mm", "m^3")),
    );
    let mut input_rows = 0;
    let names = KEYS
        .iter()
        .map(|s| s.to_string())
        .chain(metrics.iter().cloned())
        .chain(cumulative.iter().cloned())
        .collect::<Vec<_>>();
    for (path, id, area, class) in inputs {
        let mut days: BTreeMap<(i64, i64), HillDay> = BTreeMap::new();
        let count = scan(&path, &names, |batch| {
            let cols = names
                .iter()
                .map(|n| column(batch, n))
                .collect::<PyResult<Vec<_>>>()?;
            for row in 0..batch.num_rows() {
                let mut keys = [0; 4];
                for i in 0..4 {
                    keys[i] = key(number(cols[i], row)?, KEYS[i])?;
                }
                if keys[2] < 1 || keys[2] > 366 {
                    return Err(invalid("invalid AshPost Julian day"));
                }
                let day = days.entry((keys[1], keys[2])).or_insert_with(|| HillDay {
                    keys,
                    metrics: vec![Sum::default(); names.len() - 4],
                });
                for (i, col) in cols[4..].iter().enumerate() {
                    let value = number(*col, row)?;
                    if value.is_infinite() {
                        return Err(invalid("infinite AshPost metric"));
                    }
                    day.metrics[i].add(value);
                    if !day.metrics[i].value.is_finite() {
                        return Err(invalid("AshPost aggregate overflow"));
                    }
                }
            }
            Ok(())
        })?;
        if count == 0 {
            return Err(invalid("empty AshPost input file"));
        }
        input_rows += count;
        let mut last: BTreeMap<i64, &HillDay> = BTreeMap::new();
        for day in days.values() {
            if last
                .get(&day.keys[0])
                .map(|old| old.keys[2] <= day.keys[2])
                .unwrap_or(true)
            {
                last.insert(day.keys[0], day);
            }
            if day.keys[3] > 365 {
                continue;
            }
            let mut converted = Vec::with_capacity(metrics.len());
            for (name, sum) in metrics.iter().zip(&day.metrics) {
                converted.push(if name.contains("tonne/ha") {
                    sum.value * area as f64
                } else {
                    (sum.value * 0.001) * (area as f64 * 10000.0)
                });
            }
            let yearly = hills.entry((id, day.keys[1])).or_default();
            for i in 0..3 {
                yearly[i].add(day.metrics[i].value);
            }
            let dkey = [day.keys[0], day.keys[1], day.keys[2]];
            daily
                .entry(dkey)
                .or_insert_with(|| Aggregate::new(day.keys, metrics.len()))
                .add(area, &converted);
            annual
                .entry(day.keys[1])
                .or_insert_with(|| Aggregate::new(day.keys, metrics.len()))
                .add(area, &converted);
            classes
                .entry([dkey[0], dkey[1], dkey[2], class])
                .or_insert_with(|| Aggregate::new(day.keys, metrics.len()))
                .add(area, &converted);
        }
        for (year, day) in last {
            let entry = cumulatives
                .entry(year)
                .or_insert_with(|| (day.keys[3], vec![Sum::default(); cumulative_names.len()]));
            let values = day.metrics[metrics.len()..]
                .iter()
                .map(|s| s.value)
                .collect::<Vec<_>>();
            let mut converted = values.clone();
            converted.extend(
                cumulative
                    .iter()
                    .zip(&values)
                    .filter(|(n, _)| n.contains("tonne/ha"))
                    .map(|(_, v)| v * area as f64),
            );
            converted.extend(
                cumulative
                    .iter()
                    .zip(&values)
                    .filter(|(n, _)| n.contains("mm"))
                    .map(|(_, v)| (v * 0.001) * (area as f64 * 10000.0)),
            );
            for (sum, value) in entry.1.iter_mut().zip(converted) {
                sum.add(value);
            }
        }
    }
    let mut hill_means: BTreeMap<i64, ([Sum; 3], usize)> = BTreeMap::new();
    for ((id, _), sums) in hills {
        let entry = hill_means.entry(id).or_default();
        entry.1 += 1;
        for i in 0..3 {
            entry.0[i].add(sums[i].value);
        }
    }
    let hill_table = Table {
        name: "hillslope_annuals.parquet",
        columns: std::iter::once("topaz_id".to_string())
            .chain(metrics[..3].iter().cloned())
            .collect(),
        rows: hill_means
            .into_iter()
            .map(|(id, (sums, n))| {
                vec![
                    id as f64,
                    sums[0].value / n as f64,
                    sums[1].value / n as f64,
                    sums[2].value / n as f64,
                ]
            })
            .collect(),
    };
    let annual_table = Table {
        name: "watershed_annuals.parquet",
        columns: summarize_columns(
            &["year", "year0", "days_from_fire (days)", "area (ha)"],
            &metrics,
        ),
        rows: annual
            .iter()
            .map(|(year, a)| {
                summary_row(
                    vec![
                        *year as f64,
                        a.keys[0] as f64,
                        a.keys[3] as f64,
                        a.area.value as f64,
                    ],
                    a,
                    &metrics,
                )
            })
            .collect(),
    };
    let mut daily_table = Table {
        name: "watershed_daily.parquet",
        columns: summarize_columns(
            &[
                "year0",
                "year",
                "julian",
                "days_from_fire (days)",
                "area (ha)",
            ],
            &metrics,
        ),
        rows: daily
            .iter()
            .map(|(key, a)| {
                summary_row(
                    vec![
                        key[0] as f64,
                        key[1] as f64,
                        key[2] as f64,
                        a.keys[3] as f64,
                        a.area.value as f64,
                    ],
                    a,
                    &metrics,
                )
            })
            .collect(),
    };
    add_hydrology(&root, &mut daily_table, hydrology, wat, ash_ids)?;
    for relative in hydrology
        .into_iter()
        .chain(wat.filter(|_| !ash_ids.is_empty()))
    {
        let meta = fs::metadata(contained(&root, relative)?)?;
        identities.insert((meta.dev(), meta.ino()));
    }
    let class_table = Table {
        name: "watershed_daily_by_burn_class.parquet",
        columns: summarize_columns(
            &[
                "burn_class",
                "year0",
                "year",
                "julian",
                "days_from_fire (days)",
                "area (ha)",
            ],
            &metrics,
        ),
        rows: classes
            .iter()
            .map(|(key, a)| {
                summary_row(
                    vec![
                        key[3] as f64,
                        key[0] as f64,
                        key[1] as f64,
                        key[2] as f64,
                        a.keys[3] as f64,
                        a.area.value as f64,
                    ],
                    a,
                    &metrics,
                )
            })
            .collect(),
    };
    let cumulative_table = Table {
        name: "watershed_cumulatives.parquet",
        columns: [
            vec!["year0".into(), "days_from_fire (days)".into()],
            cumulative_names,
        ]
        .concat(),
        rows: cumulatives
            .into_iter()
            .map(|(year, (days, sums))| {
                std::iter::once(year as f64)
                    .chain(std::iter::once(days as f64))
                    .chain(sums.iter().map(|s| s.value))
                    .collect()
            })
            .collect(),
    };
    let tables = vec![
        hill_table,
        annual_table,
        daily_table,
        class_table,
        cumulative_table,
    ];
    if tables
        .iter()
        .flat_map(|t| &t.rows)
        .flatten()
        .any(|v| v.is_infinite())
    {
        return Err(invalid("AshPost aggregate overflow"));
    }
    Ok(Products {
        input_identities: identities,
        input_rows,
        tables,
    })
}

fn add_hydrology(
    root: &Path,
    table: &mut Table,
    hydrology: Option<&str>,
    wat: Option<&str>,
    ids: &[i64],
) -> PyResult<()> {
    let mut hydro: BTreeMap<(i64, i64), Vec<f64>> = BTreeMap::new();
    if let Some(path) = hydrology {
        let path = contained(root, path)?;
        let schema = schema(&path)?;
        let empty = ParquetRecordBatchReaderBuilder::try_new(File::open(&path)?)
            .map_err(invalid)?
            .metadata()
            .file_metadata()
            .num_rows()
            == 0;
        let mut names = [
            "year",
            "julian",
            "Streamflow",
            "Runoff",
            "Lateral Flow",
            "Baseflow",
            "Area",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
        let sediment = (1..=5)
            .map(|i| format!("seddep_{i}"))
            .filter(|s| schema.index_of(s).is_ok())
            .collect::<Vec<_>>();
        names.extend(sediment);
        if !empty {
            scan(&path, &names, |b| {
                let cols = names
                    .iter()
                    .map(|n| column(b, n))
                    .collect::<PyResult<Vec<_>>>()?;
                for r in 0..b.num_rows() {
                    let k = (
                        key(number(cols[0], r)?, "year")?,
                        key(number(cols[1], r)?, "julian")?,
                    );
                    if let std::collections::btree_map::Entry::Vacant(entry) = hydro.entry(k) {
                        let mut v = cols[2..7]
                            .iter()
                            .map(|c| number(*c, r))
                            .collect::<PyResult<Vec<_>>>()?;
                        let mut sed = 0.0;
                        for c in &cols[7..] {
                            sed += number(*c, r)?;
                        }
                        v.push(sed / 1000.0);
                        entry.insert(v);
                    }
                }
                Ok(())
            })?;
        }
    }
    let mut runoff: BTreeMap<(i64, i64), Sum> = BTreeMap::new();
    let mut runoff_nan = HashSet::new();
    if !ids.is_empty() {
        if let Some(path) = wat {
            let path = contained(root, path)?;
            let selected = ids.iter().copied().collect::<HashSet<_>>();
            let names = ["wepp_id", "year", "julian", "QOFE", "Area"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            scan(&path, &names, |b| {
                let cols = names
                    .iter()
                    .map(|n| column(b, n))
                    .collect::<PyResult<Vec<_>>>()?;
                for r in 0..b.num_rows() {
                    if !selected.contains(&key(number(cols[0], r)?, "WEPP ID")?) {
                        continue;
                    }
                    let k = (
                        key(number(cols[1], r)?, "year")?,
                        key(number(cols[2], r)?, "julian")?,
                    );
                    let sum = runoff.entry(k).or_default();
                    // SQL SUM ignores NULL, while a valid IEEE NaN propagates.
                    if cols[3].is_null(r) || cols[4].is_null(r) {
                        continue;
                    }
                    let value = (number(cols[3], r)? * 0.001) * number(cols[4], r)?;
                    if value.is_nan() {
                        runoff_nan.insert(k);
                    }
                    sum.add(value);
                }
                Ok(())
            })?;
        }
    }
    if runoff.values().any(|s| !s.value.is_finite()) {
        return Err(invalid("AshPost runoff aggregate overflow"));
    }
    let mass = table.index("ash_transport (tonne)")?;
    let ash_runoff = table.columns.iter().position(|s| s == "ash_runoff (m^3)");
    // Legacy places the correction columns before the optional volume columns.
    let insertion = table
        .columns
        .iter()
        .position(|s| s.ends_with("(m^3)"))
        .unwrap_or(table.columns.len());
    let names = [
        "Streamflow_orig (mm)",
        "Streamflow_ash_corr (mm)",
        "tot_seddep+ash (tonne)",
        "tot_seddep+ash (tonne/ha)",
    ];
    for row in &mut table.rows {
        let mut values = vec![0.0; 4];
        if !hydro.is_empty() {
            let k = (row[1] as i64, row[2] as i64);
            let h = hydro.get(&k).cloned().unwrap_or_else(|| vec![f64::NAN; 6]);
            let a = h[4];
            let ar = ash_runoff
                .map(|i| row[i])
                .filter(|v| !v.is_nan())
                .unwrap_or(0.0);
            let nonash = (h[1] * 0.001 * a)
                - runoff
                    .get(&k)
                    .filter(|s| s.count > 0 && !runoff_nan.contains(&k))
                    .map(|s| s.value)
                    .unwrap_or(f64::NAN);
            if nonash.is_infinite() {
                return Err(invalid("AshPost hydrology arithmetic overflow"));
            }
            let nonash = if nonash.is_nan() {
                f64::NAN
            } else {
                nonash.max(0.0)
            };
            let flow = if a > 0.0 {
                ((nonash + ar) + (h[2] * 0.001 * a) + (h[3] * 0.001 * a)) / a * 1000.0
            } else {
                0.0
            };
            let solids = h[5] + row[mass];
            let perha = if a > 0.0 { solids / (a / 10000.0) } else { 0.0 };
            values = vec![h[0], flow, solids, perha];
        }
        row.splice(insertion..insertion, values);
    }
    table
        .columns
        .splice(insertion..insertion, names.iter().map(|s| s.to_string()));
    Ok(())
}

pub fn write(products: &Products, output: &Path, metadata: &Metadata) -> PyResult<()> {
    if let Ok(m) = fs::symlink_metadata(output) {
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(invalid("AshPost destination must be an ordinary directory"));
        }
    }
    fs::create_dir_all(output)?;
    for table in &products.tables {
        if let Ok(meta) = fs::metadata(output.join(table.name)) {
            if products
                .input_identities
                .contains(&(meta.dev(), meta.ino()))
            {
                return Err(invalid("AshPost output aliases an input file"));
            }
        }
        table.write(output, metadata)?;
    }
    Ok(())
}

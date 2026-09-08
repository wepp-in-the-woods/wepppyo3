//! Streaming watershed daily aggregation. Joins retain one hillslope at a time.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::*;
use arrow_schema::DataType;
use parquet::arrow::arrow_reader::{
    ArrowReaderMetadata, ArrowReaderOptions, ParquetRecordBatchReaderBuilder,
};
use parquet::arrow::ProjectionMask;
use parquet::basic::{Compression, Encoding};
use parquet::file::metadata::{
    ColumnChunkMetaData, FileMetaData, ParquetMetaData, RowGroupMetaData,
};
use parquet::file::statistics::Statistics;
use parquet::schema::types::{SchemaDescriptor, Type};

use crate::arrow_support::Chunk;
use crate::errors::InterchangeError as Error;
use crate::parquet::{ParquetSink, WriteSummary};
use crate::schema::VersionInfo;
use crate::totalwatsed_schema::{output_schema, NAMES};

const BATCH_ROWS: usize = 8192;
const DATE: [&str; 6] = [
    "year",
    "sim_day_index",
    "julian",
    "month",
    "day_of_month",
    "water_year",
];
const PASS: [&str; 9] = [
    "runvol", "sbrunv", "tdet", "tdep", "sedcon_1", "sedcon_2", "sedcon_3", "sedcon_4", "sedcon_5",
];
const WAT: [&str; 27] = [
    "Area",
    "P",
    "RM",
    "Q",
    "Dp",
    "latqcc",
    "QOFE",
    "Ep",
    "Es",
    "Er",
    "UpStrmQ",
    "SubRIn",
    "Total-Soil Water",
    "frozwt",
    "Snow-Water",
    "Tile",
    "Irr",
    "SoilWaterTotal",
    "ProfileDepth",
    "ProfilePorosityCap",
    "ProfileFCStore",
    "ProfileWPStore",
    "Interception",
    "InterceptionStorage",
    "TSMF",
    "QRain",
    "QSnow",
];
const ASH: [&str; 4] = [
    "wind_transport",
    "water_transport",
    "ash_transport",
    "transportable_ash",
];
type Key = [i64; 6];
type CalendarKey = [i64; 5];
type ElementAreas = HashMap<(i64, CalendarKey), Vec<(i64, Sum)>>;
pub type AshInput = (String, f64, Option<String>, Option<f64>);

fn err(message: impl Into<String>) -> Error {
    Error::Parquet(message.into())
}
fn file(path: &Path) -> Result<File, Error> {
    File::open(path).map_err(|e| Error::io(path, e))
}

#[derive(Clone, Copy, Default)]
struct Sum {
    value: f64,
    seen: bool,
}
impl Sum {
    fn add(&mut self, v: Option<f64>) {
        if let Some(v) = v {
            self.value += v;
            self.seen = true;
        }
    }
    fn get(self) -> f64 {
        if self.seen {
            self.value
        } else {
            f64::NAN
        }
    }
}
// A shared validity bitmap avoids padding every daily float sum to 16 bytes.
#[derive(Clone, Copy)]
struct Sums<const N: usize> {
    values: [f64; N],
    seen: u32,
}
impl<const N: usize> Default for Sums<N> {
    fn default() -> Self {
        Self {
            values: [0.0; N],
            seen: 0,
        }
    }
}
impl<const N: usize> Sums<N> {
    fn add(&mut self, index: usize, value: Option<f64>) {
        if let Some(value) = value {
            self.values[index] += value;
            self.seen |= 1 << index;
        }
    }
    fn get(&self, index: usize) -> f64 {
        if self.seen & (1 << index) != 0 {
            self.values[index]
        } else {
            f64::NAN
        }
    }
    fn values(&self) -> [f64; N] {
        std::array::from_fn(|i| self.get(i))
    }
    fn merge(&mut self, other: Self) {
        for i in 0..N {
            if other.seen & (1 << i) != 0 {
                self.add(i, Some(other.values[i]));
            }
        }
    }
}

#[derive(Default)]
struct Daily {
    pass: Sums<9>,
    wat: Sums<27>,
    weights: Sums<3>,
    has_wat: bool,
}
#[derive(Default)]
struct AshDaily {
    mass: [Sum; 4],
    typed: [[Sum; 3]; 2],
    area: Sum,
    typed_area: [Sum; 2],
    volume: Sum,
    black_volume: Sum,
}

struct Batch {
    cols: Vec<Vec<Option<f64>>>,
    rows: usize,
}
impl Batch {
    fn val(&self, col: usize, row: usize) -> Option<f64> {
        self.cols[col][row]
    }
    fn int(&self, col: usize, row: usize) -> Result<i64, Error> {
        match self.val(col, row) {
            Some(v)
                if v.is_finite()
                    && v.fract() == 0.0
                    && v >= i64::MIN as f64
                    && v < i64::MAX as f64 =>
            {
                Ok(v as i64)
            }
            _ => Err(err("null, nonintegral or out-of-range totalwatsed3 key")),
        }
    }
    fn key(&self, row: usize) -> Result<Key, Error> {
        Ok([
            self.int(1, row)?,
            self.int(2, row)?,
            self.int(3, row)?,
            self.int(4, row)?,
            self.int(5, row)?,
            self.int(6, row)?,
        ])
    }
}

fn numbers(array: &dyn Array) -> Result<Vec<Option<f64>>, Error> {
    macro_rules! vals {
        ($ty:ty) => {{
            let a = array
                .as_any()
                .downcast_ref::<$ty>()
                .ok_or_else(|| err("invalid numeric Arrow array"))?;
            a.iter().map(|v| v.map(|v| v as f64)).collect()
        }};
    }
    Ok(match array.data_type() {
        DataType::Int8 => vals!(Int8Array),
        DataType::Int16 => vals!(Int16Array),
        DataType::Int32 => vals!(Int32Array),
        DataType::Int64 => vals!(Int64Array),
        DataType::UInt8 => vals!(UInt8Array),
        DataType::UInt16 => vals!(UInt16Array),
        DataType::UInt32 => vals!(UInt32Array),
        DataType::UInt64 => vals!(UInt64Array),
        DataType::Float32 => vals!(Float32Array),
        DataType::Float64 => vals!(Float64Array),
        DataType::Decimal128(_, scale) => {
            let a = array
                .as_any()
                .downcast_ref::<Decimal128Array>()
                .ok_or_else(|| err("invalid decimal array"))?;
            let factor = 10_f64.powi(*scale as i32);
            a.iter().map(|v| v.map(|v| v as f64 / factor)).collect()
        }
        DataType::Null => vec![None; array.len()],
        ty => {
            return Err(err(format!(
                "unsupported totalwatsed3 numeric type: {ty:?}"
            )))
        }
    })
}

// Retain only columns consumed by this source. Arrow's batch projection alone
// leaves every column chunk in memory for every row group, which grows with the
// hillslope count. Preserve physical chunk offsets and original Arrow types.
fn project_metadata(
    metadata: ArrowReaderMetadata,
    metrics: &[&str],
) -> Result<ArrowReaderMetadata, Error> {
    let wanted = |name: &str| {
        DATE.contains(&name)
            || ["wepp_id", "day", "ofe_id", "OFE"].contains(&name)
            || metrics.contains(&name)
    };
    let roots: Vec<_> = metadata
        .schema()
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(i, f)| wanted(f.name()).then_some(i))
        .collect();
    let arrow_schema = Arc::new(metadata.schema().project(&roots)?);
    let original = metadata.metadata();
    let fm = original.file_metadata();
    let descriptor = fm.schema_descr();
    let leaves: Vec<_> = (0..descriptor.num_columns())
        .filter(|&i| roots.contains(&descriptor.get_column_root_idx(i)))
        .collect();
    let fields = roots
        .iter()
        .map(|&i| fm.schema().get_fields()[i].clone())
        .collect();
    let schema = Arc::new(SchemaDescriptor::new(Arc::new(
        Type::group_type_builder(fm.schema().name())
            .with_fields(fields)
            .build()?,
    )));
    let groups = original
        .row_groups()
        .iter()
        .map(|group| {
            RowGroupMetaData::builder(schema.clone())
                .set_num_rows(group.num_rows())
                .set_total_byte_size(group.total_byte_size())
                .set_column_metadata(leaves.iter().map(|&i| group.column(i).clone()).collect())
                .build()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let file_metadata = FileMetaData::new(
        fm.version(),
        fm.num_rows(),
        fm.created_by().map(str::to_owned),
        fm.key_value_metadata().cloned(),
        schema,
        fm.column_orders()
            .map(|orders| leaves.iter().map(|&i| orders[i]).collect()),
    );
    ArrowReaderMetadata::try_new(
        Arc::new(ParquetMetaData::new(file_metadata, groups)),
        ArrowReaderOptions::default().with_schema(arrow_schema),
    )
    .map_err(Error::from)
}

// Parquet's byte_range asserts on negative offsets/sizes. Reject malformed
// footer locations before projection or decoding so errors cross PyO3 normally.
fn validate_metadata(metadata: &ParquetMetaData, file_bytes: u64) -> Result<(), Error> {
    if metadata.file_metadata().num_rows() < 0 {
        return Err(err("negative totalwatsed3 Parquet file row count"));
    }
    for group in metadata.row_groups() {
        if group.num_rows() < 0 || group.total_byte_size() < 0 {
            return Err(err("negative totalwatsed3 Parquet row-group count or size"));
        }
        for column in group.columns() {
            if column.num_values() < 0
                || column.compressed_size() < 0
                || column.uncompressed_size() < 0
                || column.data_page_offset() < 0
                || column.dictionary_page_offset().is_some_and(|v| v < 0)
            {
                return Err(err(
                    "negative totalwatsed3 Parquet column count, size or offset",
                ));
            }
            let start = column
                .dictionary_page_offset()
                .unwrap_or(column.data_page_offset()) as u64;
            let end = start
                .checked_add(column.compressed_size() as u64)
                .ok_or_else(|| err("totalwatsed3 Parquet column range overflow"))?;
            let data = column.data_page_offset() as u64;
            // Empty dictionary chunks may use zero as the absent data-page sentinel.
            let absent_data_page = column.num_values() == 0 && data == 0;
            if (!absent_data_page && (start > data || data > end)) || end > file_bytes {
                return Err(err("totalwatsed3 Parquet column range outside input file"));
            }
        }
    }
    Ok(())
}

// Page decoding needs chunk locations, sizes, codec, and the shared schema.
// Full statistics/histograms are not used after the hillslope index is built.
struct ColumnLocation {
    values: i64,
    compressed: i64,
    uncompressed: i64,
    data_offset: i64,
    dictionary_offset: Option<i64>,
    codec: Compression,
    encodings: Box<[Encoding]>,
}
struct GroupLocation {
    rows: i64,
    bytes: i64,
    single_hill: Option<i64>,
    columns: Vec<ColumnLocation>,
}

struct Source {
    path: PathBuf,
    metadata: ArrowReaderMetadata,
    hills: BTreeMap<i64, Vec<usize>>,
    groups: Vec<GroupLocation>,
}
impl Source {
    fn open(path: &Path, metrics: Option<&[&str]>) -> Result<Self, Error> {
        let input = file(path)?;
        let file_bytes = input.metadata().map_err(|e| Error::io(path, e))?.len();
        let metadata = ArrowReaderMetadata::load(&input, ArrowReaderOptions::default())?;
        validate_metadata(metadata.metadata(), file_bytes)?;
        let metadata = match metrics {
            Some(metrics) => project_metadata(metadata, metrics)?,
            None => metadata,
        };
        let hill_col = metadata.schema().index_of("wepp_id").ok();
        let groups = metadata
            .metadata()
            .row_groups()
            .iter()
            .map(|group| {
                let single_hill = hill_col.and_then(|i| match group.column(i).statistics() {
                    Some(Statistics::Int32(s)) if s.min_opt() == s.max_opt() => {
                        s.min_opt().map(|v| *v as i64)
                    }
                    Some(Statistics::Int64(s)) if s.min_opt() == s.max_opt() => {
                        s.min_opt().copied()
                    }
                    _ => None,
                });
                GroupLocation {
                    rows: group.num_rows(),
                    bytes: group.total_byte_size(),
                    single_hill,
                    columns: group
                        .columns()
                        .iter()
                        .map(|c| ColumnLocation {
                            values: c.num_values(),
                            compressed: c.compressed_size(),
                            uncompressed: c.uncompressed_size(),
                            data_offset: c.data_page_offset(),
                            dictionary_offset: c.dictionary_page_offset(),
                            codec: c.compression(),
                            encodings: c.encodings().clone().into_boxed_slice(),
                        })
                        .collect(),
                }
            })
            .collect();
        let metadata = ArrowReaderMetadata::try_new(
            Arc::new(ParquetMetaData::new(
                metadata.metadata().file_metadata().clone(),
                Vec::new(),
            )),
            ArrowReaderOptions::default().with_schema(metadata.schema().clone()),
        )?;
        Ok(Self {
            path: path.into(),
            metadata,
            groups,
            hills: BTreeMap::new(),
        })
    }
    fn has(&self, name: &str) -> bool {
        self.metadata.schema().index_of(name).is_ok()
    }
    fn day(&self) -> Result<&str, Error> {
        if self.has("sim_day_index") {
            Ok("sim_day_index")
        } else if self.has("day") {
            Ok("day")
        } else {
            Err(err(format!(
                "missing simulation day in {}",
                self.path.display()
            )))
        }
    }
    fn ofe(&self) -> Option<&str> {
        if self.has("ofe_id") {
            Some("ofe_id")
        } else if self.has("OFE") {
            Some("OFE")
        } else {
            None
        }
    }
    fn dates(&self) -> Result<Vec<String>, Error> {
        let mut names = vec!["wepp_id".to_string()];
        names.extend(
            DATE.iter()
                .map(|n| {
                    if *n == "sim_day_index" {
                        self.day().map(str::to_string)
                    } else {
                        Ok(n.to_string())
                    }
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        Ok(names)
    }
    fn scan(
        &self,
        groups: &[usize],
        names: &[String],
        mut f: impl FnMut(Batch) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if groups.is_empty() {
            return Ok(());
        }
        // Keep reconstructed metadata bounded even when PASS scans many groups.
        for groups in groups.chunks(16) {
            let builder = ParquetRecordBatchReaderBuilder::new_with_metadata(
                file(&self.path)?,
                self.reader_metadata(groups)?,
            );
            let indices = names
                .iter()
                .map(|n| builder.schema().index_of(n).map_err(Error::from))
                .collect::<Result<Vec<_>, _>>()?;
            let mask = ProjectionMask::roots(builder.parquet_schema(), indices);
            let reader = builder
                .with_row_groups((0..groups.len()).collect())
                .with_projection(mask)
                .with_batch_size(BATCH_ROWS)
                .build()?;
            for batch in reader {
                let batch = batch?;
                let cols = names
                    .iter()
                    .map(|n| {
                        numbers(
                            batch
                                .column_by_name(n)
                                .ok_or_else(|| err(format!("missing projected {n}")))?
                                .as_ref(),
                        )
                    })
                    .collect::<Result<_, _>>()?;
                f(Batch {
                    cols,
                    rows: batch.num_rows(),
                })?;
            }
        }
        Ok(())
    }
    fn reader_metadata(&self, groups: &[usize]) -> Result<ArrowReaderMetadata, Error> {
        let fm = self.metadata.metadata().file_metadata();
        let schema = fm.schema_descr_ptr();
        let row_groups = groups
            .iter()
            .map(|&i| {
                let group = &self.groups[i];
                let columns = group
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        ColumnChunkMetaData::builder(schema.column(i))
                            .set_num_values(c.values)
                            .set_compression(c.codec)
                            .set_encodings(c.encodings.to_vec())
                            .set_total_compressed_size(c.compressed)
                            .set_total_uncompressed_size(c.uncompressed)
                            .set_data_page_offset(c.data_offset)
                            .set_dictionary_page_offset(c.dictionary_offset)
                            .build()
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                RowGroupMetaData::builder(schema.clone())
                    .set_num_rows(group.rows)
                    .set_total_byte_size(group.bytes)
                    .set_column_metadata(columns)
                    .build()
            })
            .collect::<Result<Vec<_>, parquet::errors::ParquetError>>()?;
        ArrowReaderMetadata::try_new(
            Arc::new(ParquetMetaData::new(fm.clone(), row_groups)),
            ArrowReaderOptions::default().with_schema(self.metadata.schema().clone()),
        )
        .map_err(Error::from)
    }
    fn all(&self) -> Vec<usize> {
        (0..self.groups.len()).collect()
    }
    fn index(&mut self, selected: &Option<HashSet<i64>>) -> Result<(), Error> {
        self.metadata.schema().index_of("wepp_id")?;
        for group in self.all() {
            let single = self.groups[group].single_hill;
            let mut ids = BTreeSet::new();
            if let Some(id) = single {
                ids.insert(id);
            } else {
                self.scan(&[group], &["wepp_id".into()], |b| {
                    for r in 0..b.rows {
                        if let Some(v) = b.val(0, r) {
                            ids.insert(v as i64);
                        }
                    }
                    Ok(())
                })?;
            }
            for id in ids {
                if selected.as_ref().is_none_or(|s| s.contains(&id)) {
                    self.hills.entry(id).or_default().push(group);
                }
            }
        }
        Ok(())
    }
}

fn calendar(k: &Key) -> CalendarKey {
    [k[0], k[2], k[3], k[4], k[5]]
}
fn product(a: Option<f64>, b: Option<f64>, factor: f64) -> Option<f64> {
    a.zip(b).map(|(a, b)| a * factor * b)
}
fn depth(v: f64, a: f64) -> f64 {
    if a > 0.0 {
        (v / a) * 1000.0
    } else {
        0.0
    }
}
fn ratio(v: f64, a: f64) -> f64 {
    if a > 0.0 {
        v / a
    } else {
        0.0
    }
}
fn zero_nan(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

fn aggregate_pass(
    source: &Source,
    groups: &[usize],
    selected: &Option<HashSet<i64>>,
    days: &mut HashMap<Key, Box<Daily>>,
) -> Result<(), Error> {
    let mut names = source.dates()?;
    names.extend(PASS.map(str::to_string));
    source.scan(groups, &names, |b| {
        for row in 0..b.rows {
            if selected
                .as_ref()
                .is_some_and(|s| !b.val(0, row).is_some_and(|v| s.contains(&(v as i64))))
            {
                continue;
            }
            let d = days.entry(b.key(row)?).or_default();
            for i in 0..9 {
                d.pass.add(
                    i,
                    if i < 4 {
                        b.val(7 + i, row)
                    } else {
                        product(b.val(7 + i, row), b.val(7, row), 1.0)
                    },
                );
            }
        }
        Ok(())
    })
}

fn aggregate_hill(
    wat: &Source,
    soil: Option<&Source>,
    element: Option<&Source>,
    hill: i64,
    days: &mut HashMap<Key, Box<Daily>>,
) -> Result<(), Error> {
    let mut names = wat.dates()?;
    let metrics: Vec<(usize, usize)> = WAT[..24]
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            if wat.has(n) {
                let col = names.len();
                names.push(n.to_string());
                Some((i, col))
            } else {
                None
            }
        })
        .collect();
    for n in &WAT[..17] {
        if !wat.has(n) {
            return Err(err(format!("missing required WAT column {n}")));
        }
    }
    let area_col = names
        .iter()
        .position(|n| n == "Area")
        .ok_or_else(|| err("missing Area"))?;
    let ofe_col = wat.ofe().map(|n| {
        let col = names.len();
        names.push(n.into());
        col
    });
    let need_soil = soil.is_some_and(|s| s.has("TSMF") && s.ofe().is_some()) && ofe_col.is_some();
    let need_element = element
        .is_some_and(|s| (s.has("QRain") || s.has("QSnow")) && s.ofe().is_some())
        && ofe_col.is_some();
    let mut soil_areas: HashMap<(i64, i64, i64), Sum> = HashMap::new();
    let mut element_areas: ElementAreas = HashMap::new();
    let mut lateral: HashMap<(i64, Key), Sum> = HashMap::new();
    let mut max_ofe = None;
    wat.scan(&wat.hills[&hill], &names, |b| {
        for row in 0..b.rows {
            if b.val(0, row) != Some(hill as f64) {
                continue;
            }
            let key = b.key(row)?;
            let area = b.val(area_col, row);
            let ofe = ofe_col.and_then(|c| b.val(c, row)).map(|v| v as i64);
            let d = days.entry(key).or_default();
            d.has_wat = true;
            for &(metric, col) in &metrics {
                let val = if metric == 0 {
                    b.val(col, row)
                } else {
                    product(b.val(col, row), area, 0.001)
                };
                if metric == 5 && ofe_col.is_some() {
                    if let Some(ofe) = ofe {
                        lateral.entry((ofe, key)).or_default().add(val);
                    } else {
                        d.wat.add(metric, Some(0.0));
                    }
                } else {
                    d.wat.add(metric, val);
                }
            }
            if let Some(ofe) = ofe {
                max_ofe = Some(max_ofe.map_or(ofe, |v: i64| v.max(ofe)));
                if need_soil {
                    soil_areas
                        .entry((ofe, key[0], key[1]))
                        .or_default()
                        .add(area);
                }
                if need_element {
                    let areas = element_areas.entry((ofe, calendar(&key))).or_default();
                    if let Some((_, sum)) = areas.iter_mut().find(|(sim, _)| *sim == key[1]) {
                        sum.add(area);
                    } else {
                        let mut sum = Sum::default();
                        sum.add(area);
                        areas.push((key[1], sum));
                    }
                }
            }
        }
        Ok(())
    })?;
    if let Some(max) = max_ofe {
        for ((ofe, key), sum) in lateral {
            let value = if ofe != max {
                Some(0.0)
            } else if sum.seen {
                Some(sum.value)
            } else {
                None
            };
            days.entry(key).or_default().wat.add(5, value);
        }
    }
    if need_soil {
        let s = soil.ok_or_else(|| err("missing soil source"))?;
        if let Some(groups) = s.hills.get(&hill) {
            let mut names = s.dates()?;
            names.push(s.ofe().ok_or_else(|| err("missing soil OFE"))?.into());
            names.push("TSMF".into());
            s.scan(groups, &names, |b| {
                for row in 0..b.rows {
                    if b.val(0, row) != Some(hill as f64) {
                        continue;
                    }
                    let key = b.key(row)?;
                    if let Some(ofe) = b.val(7, row) {
                        if let Some(area) = soil_areas.get(&(ofe as i64, key[0], key[1])) {
                            let d = days.entry(key).or_default();
                            if let Some(value) = b.val(8, row).filter(|_| area.seen) {
                                d.wat.add(24, Some(value * area.value));
                                d.weights.add(0, Some(area.value));
                            }
                        }
                    }
                }
                Ok(())
            })?;
        }
    }
    if need_element {
        let s = element.ok_or_else(|| err("missing element source"))?;
        if let Some(groups) = s.hills.get(&hill) {
            let mut names = vec![
                "wepp_id".into(),
                s.ofe().ok_or_else(|| err("missing element OFE"))?.into(),
            ];
            names.extend(
                ["year", "julian", "month", "day_of_month", "water_year"].map(str::to_string),
            );
            let metrics: Vec<(usize, usize)> = ["QRain", "QSnow"]
                .iter()
                .enumerate()
                .filter_map(|(i, n)| {
                    if s.has(n) {
                        let c = names.len();
                        names.push(n.to_string());
                        Some((i, c))
                    } else {
                        None
                    }
                })
                .collect();
            s.scan(groups, &names, |b| {
                for row in 0..b.rows {
                    if b.val(0, row) != Some(hill as f64) {
                        continue;
                    }
                    let cal = [
                        b.int(2, row)?,
                        b.int(3, row)?,
                        b.int(4, row)?,
                        b.int(5, row)?,
                        b.int(6, row)?,
                    ];
                    if let Some(ofe) = b.val(1, row) {
                        if let Some(areas) = element_areas.get(&(ofe as i64, cal)) {
                            for (sim, area) in areas {
                                let key = [cal[0], *sim, cal[1], cal[2], cal[3], cal[4]];
                                let d = days.entry(key).or_default();
                                for &(i, col) in &metrics {
                                    if let Some(value) = b.val(col, row).filter(|_| area.seen) {
                                        d.wat.add(25 + i, Some(value * 0.001 * area.value));
                                        d.weights.add(1 + i, Some(area.value));
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn aggregate_ash(inputs: &[AshInput]) -> Result<HashMap<[i64; 4], AshDaily>, Error> {
    let mut result: HashMap<[i64; 4], AshDaily> = HashMap::new();
    for (path, area, kind, density) in inputs {
        if *area <= 0.0 || area.is_nan() {
            continue;
        }
        let s = Source::open(Path::new(path), None)?;
        let metric_names: Vec<String> = ASH.iter().map(|n| format!("{n} (tonne/ha)")).collect();
        if !metric_names.iter().all(|n| s.has(n)) {
            continue;
        }
        let mut names = vec!["year".into(), "julian".into()];
        names.extend(metric_names);
        let year0 = if s.has("year0") {
            let c = names.len();
            names.push("year0".into());
            Some(c)
        } else {
            None
        };
        let fire_days = if s.has("days_from_fire (days)") {
            let c = names.len();
            names.push("days_from_fire (days)".into());
            Some(c)
        } else {
            None
        };
        type HillAsh = ([Sum; 4], Option<f64>, Option<f64>);
        let mut hill: BTreeMap<(i64, i64), HillAsh> = BTreeMap::new();
        s.scan(&s.all(), &names, |b| {
            for r in 0..b.rows {
                let entry = hill.entry((b.int(0, r)?, b.int(1, r)?)).or_default();
                for i in 0..4 {
                    if let Some(v) = b.val(2 + i, r) {
                        if !v.is_nan() {
                            entry.0[i].add(Some(v));
                        }
                    }
                }
                if entry.1.is_none() {
                    entry.1 = year0.and_then(|c| b.val(c, r)).filter(|v| !v.is_nan());
                }
                if entry.2.is_none() {
                    entry.2 = fire_days.and_then(|c| b.val(c, r)).filter(|v| !v.is_nan());
                }
            }
            Ok(())
        })?;
        for ((year, julian), (mass, y0, days)) in hill {
            if year0.is_some() {
                if y0 != Some(year as f64) {
                    continue;
                }
            } else if fire_days.is_some() && !days.is_some_and(|v| v <= 365.0) {
                continue;
            }
            let date = time::Date::from_ordinal_date(
                i32::try_from(year).map_err(|_| err("ash year out of range"))?,
                u16::try_from(julian).map_err(|_| err("ash Julian day out of range"))?,
            )
            .map_err(|e| err(e.to_string()))?;
            let key = [year, julian, date.month() as i64, date.day() as i64];
            let d = result.entry(key).or_default();
            d.area.add(Some(*area));
            let typed = match kind.as_deref() {
                Some("black") => Some(0),
                Some("white") => Some(1),
                _ => None,
            };
            if let Some(t) = typed {
                d.typed_area[t].add(Some(*area));
            }
            for i in 0..4 {
                let m = mass[i].value * area;
                d.mass[i].add(Some(m));
                if i < 3 {
                    if let Some(t) = typed {
                        d.typed[t][i].add(Some(m));
                    }
                }
            }
            if let Some(density) = density.filter(|v| *v > 0.0) {
                let v = mass[2].value * area * 1000.0 / density;
                d.volume.add(Some(v));
                if typed == Some(0) {
                    d.black_volume.add(Some(v));
                }
            }
        }
    }
    Ok(result)
}

fn finish_row(
    key: &Key,
    d: &Daily,
    ash: Option<&AshDaily>,
    reservoir: f64,
    baseflow: f64,
    losses: f64,
) -> Vec<f64> {
    let mut out = vec![f64::NAN; NAMES.len()];
    let mut set = |name: &str, value: f64| {
        if let Some(i) = NAMES.iter().position(|n| *n == name) {
            out[i] = value;
        }
    };
    for i in 0..6 {
        set(DATE[i], key[i] as f64);
    }
    let p = d.pass.values().map(zero_nan);
    for i in 0..4 {
        set(PASS[i], p[i]);
    }
    let densities = [2600.0, 2650.0, 1800.0, 1600.0, 2650.0];
    let mut solids = 0.0;
    let mut sediment = 0.0;
    for i in 0..5 {
        set(&format!("seddep_{}", i + 1), p[i + 4]);
        sediment += p[i + 4];
        // Derive concentration before the historical post-join null fill.
        solids += d.pass.get(i + 4) / densities[i];
    }
    set("sed_del", sediment);
    let sed_conc = zero_nan(ratio(solids, p[0]));
    set("sed_vol_conc", sed_conc);
    let w = d.wat.values();
    let area = w[0];
    for i in 0..24 {
        let v = if i < 10 { w[i] } else { depth(w[i], area) };
        set(
            WAT[i],
            if i == 22 {
                zero_nan(v).clamp(-f64::MAX, f64::MAX)
            } else {
                v
            },
        );
    }
    for i in 0..3 {
        set(
            WAT[24 + i],
            if d.weights.get(i) > 0.0 {
                w[24 + i] / d.weights.get(i) * if i == 0 { 1.0 } else { 1000.0 }
            } else {
                f64::NAN
            },
        );
    }
    for (name, index) in [
        ("Precipitation", 1),
        ("Rain+Melt", 2),
        ("Percolation", 4),
        ("Lateral Flow", 5),
        ("Transpiration", 7),
    ] {
        set(name, depth(w[index], area));
    }
    let runoff = depth(p[0], area);
    set("Runoff", runoff);
    set("Evaporation", depth(w[8] + w[9], area));
    set("ET", depth(w[7] + w[8] + w[9], area));
    set("Reservoir Volume", reservoir);
    set("Baseflow", baseflow);
    set("Aquifer losses", losses);
    set("Streamflow", runoff + depth(w[5], area) + baseflow);
    let empty = AshDaily::default();
    let a = ash.unwrap_or(&empty);
    for (i, name) in ASH.iter().enumerate() {
        set(name, a.mass[i].value);
        set(
            &format!("{name}_per_ha"),
            ratio(a.mass[i].value, a.area.value),
        );
        if i < 3 {
            for (t, kind) in ["black", "white"].iter().enumerate() {
                set(&format!("{name}_{kind}"), a.typed[t][i].value);
                set(
                    &format!("{name}_{kind}_per_ha"),
                    ratio(a.typed[t][i].value, a.typed_area[t].value),
                );
            }
        }
    }
    set("ash_vol_conc", ratio(a.volume.value, p[0]));
    set(
        "ash_black_pct_by_vol",
        ratio(a.black_volume.value, a.volume.value) * 100.0,
    );
    set(
        "sed+ash_vol_conc",
        ratio(sed_conc * p[0] + a.volume.value, p[0]),
    );
    out
}

pub fn produce(
    pass_path: &Path,
    wat_path: &Path,
    output: &Path,
    gwstorage: f64,
    bfcoeff: f64,
    dscoeff: f64,
    version: &VersionInfo,
    soil_path: Option<&Path>,
    element_path: Option<&Path>,
    wepp_ids: Option<Vec<i64>>,
    ash_inputs: &[AshInput],
    pandas_metadata: Option<&str>,
) -> Result<WriteSummary, Error> {
    for input in [Some(pass_path), Some(wat_path), soil_path, element_path]
        .into_iter()
        .flatten()
        .chain(ash_inputs.iter().map(|a| Path::new(&a.0)))
    {
        if output == input
            || (output.exists()
                && std::fs::canonicalize(output).ok() == std::fs::canonicalize(input).ok())
        {
            return Err(err("totalwatsed3 output must not alias an input"));
        }
    }
    let selected = wepp_ids.map(|ids| ids.into_iter().collect::<HashSet<_>>());
    let pass = Source::open(pass_path, Some(&PASS))?;
    let mut wat = Source::open(wat_path, Some(&WAT))?;
    wat.index(&selected)?;
    let mut soil = soil_path
        .map(|path| Source::open(path, Some(&["TSMF"])))
        .transpose()?;
    let mut element = element_path
        .map(|path| Source::open(path, Some(&["QRain", "QSnow"])))
        .transpose()?;
    if let Some(s) = &mut soil {
        if s.has("TSMF") && s.ofe().is_some() {
            s.index(&selected)?;
        }
    }
    if let Some(s) = &mut element {
        if (s.has("QRain") || s.has("QSnow")) && s.ofe().is_some() {
            s.index(&selected)?;
        }
    }
    // Each worker owns date aggregates and one hillslope's joins. The CPU budget
    // is bounded by the package's twelve-CPU worker; no whole-run rows survive.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(12);
    let hills: Vec<_> = wat.hills.keys().copied().collect();
    let groups = pass.all();
    let partials = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let (pass, wat, soil, element, selected, hills, groups) = (
                    &pass,
                    &wat,
                    soil.as_ref(),
                    element.as_ref(),
                    &selected,
                    &hills,
                    &groups,
                );
                scope.spawn(move || {
                    let mut days = HashMap::new();
                    let groups: Vec<_> = groups
                        .iter()
                        .copied()
                        .skip(worker)
                        .step_by(workers)
                        .collect();
                    aggregate_pass(pass, &groups, selected, &mut days)?;
                    for hill in hills.iter().skip(worker).step_by(workers) {
                        aggregate_hill(wat, soil, element, *hill, &mut days)?;
                    }
                    Ok::<_, Error>(days)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .map_err(|_| err("totalwatsed3 scan thread panicked"))?
            })
            .collect::<Result<Vec<_>, Error>>()
    })?;
    let mut days: HashMap<Key, Box<Daily>> = HashMap::new();
    for partial in partials {
        for (key, src) in partial {
            let dest = days.entry(key).or_default();
            dest.has_wat |= src.has_wat;
            dest.pass.merge(src.pass);
            dest.wat.merge(src.wat);
            dest.weights.merge(src.weights);
        }
    }
    let mut days: Vec<_> = days.into_iter().filter(|(_, d)| d.has_wat).collect();
    days.sort_by_key(|(k, _)| (k[0], k[2], k[1]));
    let ash = if days.is_empty() {
        HashMap::new()
    } else {
        aggregate_ash(ash_inputs)?
    };
    let schema = output_schema(
        version,
        if days.is_empty() {
            None
        } else {
            pandas_metadata
        },
    );
    let mut sink = ParquetSink::try_new(output, schema.clone())?;
    let mut columns: Vec<Vec<Option<f64>>> = vec![Vec::with_capacity(BATCH_ROWS); NAMES.len()];
    let optional_present: [bool; 7] =
        std::array::from_fn(|i| days.iter().any(|(_, d)| !d.wat.get(17 + i).is_nan()));
    let mut reservoir = gwstorage;
    let mut baseflow = 0.0;
    for (i, (key, d)) in days.iter().enumerate() {
        if i > 0 {
            reservoir =
                reservoir - baseflow + depth(d.wat.get(4), d.wat.get(0)) - reservoir * dscoeff;
            baseflow = reservoir * bfcoeff;
        }
        let losses = if i + 1 < days.len() {
            reservoir * dscoeff
        } else {
            0.0
        };
        let mut row = finish_row(
            key,
            d,
            ash.get(&[key[0], key[2], key[3], key[4]]),
            reservoir,
            baseflow,
            losses,
        );
        for (i, present) in optional_present.iter().enumerate() {
            if !present {
                let name = WAT[17 + i];
                if let Some(index) = NAMES.iter().position(|n| *n == name) {
                    row[index] = if name == "Interception" {
                        0.0
                    } else {
                        f64::NAN
                    };
                }
            }
        }
        for (col, v) in columns.iter_mut().zip(row) {
            col.push(if v.is_nan() { None } else { Some(v) });
        }
        if columns[0].len() == BATCH_ROWS || i + 1 == days.len() {
            let arrays = columns
                .iter_mut()
                .zip(schema.fields())
                .map(|(col, field)| {
                    let values = std::mem::replace(col, Vec::with_capacity(BATCH_ROWS));
                    match field.data_type() {
                        DataType::Int8 => Box::new(Int8Array::from(
                            values
                                .into_iter()
                                .map(|v| v.map(|v| v as i8))
                                .collect::<Vec<_>>(),
                        )) as Box<dyn Array>,
                        DataType::Int16 => Box::new(Int16Array::from(
                            values
                                .into_iter()
                                .map(|v| v.map(|v| v as i16))
                                .collect::<Vec<_>>(),
                        )) as Box<dyn Array>,
                        DataType::Int32 => Box::new(Int32Array::from(
                            values
                                .into_iter()
                                .map(|v| v.map(|v| v as i32))
                                .collect::<Vec<_>>(),
                        )) as Box<dyn Array>,
                        _ => Box::new(Float64Array::from(values)) as Box<dyn Array>,
                    }
                })
                .collect();
            sink.write_chunk(Chunk::new(arrays))?;
        }
    }
    sink.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pass_runoff_and_sediment_units() {
        let mut day = Daily::default();
        day.wat.add(0, Some(10000.0));
        day.pass.add(0, Some(1.0));
        for i in 4..9 {
            day.pass.add(i, Some(0.05));
        }
        let mut row = finish_row(&[2020, 1, 1, 1, 1, 2020], &day, None, 1.0, 0.0, 0.0);
        let get = |name| row[NAMES.iter().position(|n| *n == name).unwrap()];
        assert_eq!(get("Runoff"), 0.1);
        assert_eq!(get("sed_del"), 0.25);
        assert!((get("sed_vol_conc") - 1.159943960651508e-4).abs() < 1e-15);
    }
    #[test]
    fn null_sum_and_nonpositive_depth_contract() {
        let mut sum = Sum::default();
        sum.add(None);
        assert!(sum.get().is_nan());
        sum.add(Some(2.0));
        assert_eq!(sum.get(), 2.0);
        assert_eq!(depth(10.0, 0.0), 0.0);
        assert_eq!(ratio(10.0, -1.0), 0.0);
    }
}

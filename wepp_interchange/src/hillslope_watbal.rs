//! Bounded hillslope/year water-balance cache producer. WEPPpy owns mapping policy.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use arrow_array::*;
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;
use pyo3::exceptions::{PyKeyError, PyOSError, PyValueError};
use pyo3::PyResult;

use crate::arrow_support::Chunk;
use crate::parquet::ParquetSink;

const INPUT: [&str; 11] = [
    "wepp_id",
    "ofe_id",
    "water_year",
    "P",
    "Dp",
    "QOFE",
    "latqcc",
    "Ep",
    "Es",
    "Er",
    "Area",
];
const OUTPUT: [&str; 8] = [
    "TopazID",
    "WaterYear",
    "Area_m2",
    "Precipitation (mm)",
    "Percolation (mm)",
    "Surface Runoff (mm)",
    "Lateral Flow (mm)",
    "Transpiration + Evaporation (mm)",
];
const BATCH_ROWS: usize = 8192;

fn invalid(message: impl ToString) -> pyo3::PyErr {
    PyValueError::new_err(message.to_string())
}
fn io(message: impl ToString) -> pyo3::PyErr {
    PyOSError::new_err(message.to_string())
}

fn reader(
    path: &Path,
    names: &[&str],
) -> PyResult<parquet::arrow::arrow_reader::ParquetRecordBatchReader> {
    let builder =
        ParquetRecordBatchReaderBuilder::try_new(File::open(path).map_err(io)?).map_err(invalid)?;
    let schema = builder.schema();
    let mut indices = Vec::new();
    for name in names {
        let index = schema.index_of(name).map_err(invalid)?;
        let ty = schema.field(index).data_type();
        if !matches!(
            ty,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Float32
                | DataType::Float64
                | DataType::Null
        ) {
            return Err(invalid(format!(
                "unsupported hillslope watbal column {name}: {ty}"
            )));
        }
        indices.push(index);
    }
    let projection = ProjectionMask::roots(builder.parquet_schema(), indices);
    builder
        .with_projection(projection)
        .with_batch_size(BATCH_ROWS)
        .build()
        .map_err(invalid)
}

fn key(array: &dyn Array, row: usize, optional: bool) -> PyResult<Option<i64>> {
    if array.is_null(row) || array.data_type() == &DataType::Null {
        return if optional {
            Ok(None)
        } else {
            Err(invalid("null hillslope watbal WEPP/year key"))
        };
    }
    macro_rules! integer {
        ($ty:ty) => {{
            let a = array
                .as_any()
                .downcast_ref::<$ty>()
                .ok_or_else(|| invalid("invalid key array"))?;
            i64::try_from(a.value(row)).map(Some).map_err(invalid)
        }};
    }
    match array.data_type() {
        DataType::Int8 => integer!(Int8Array),
        DataType::Int16 => integer!(Int16Array),
        DataType::Int32 => integer!(Int32Array),
        DataType::Int64 => integer!(Int64Array),
        DataType::UInt8 => integer!(UInt8Array),
        DataType::UInt16 => integer!(UInt16Array),
        DataType::UInt32 => integer!(UInt32Array),
        DataType::UInt64 => integer!(UInt64Array),
        _ => {
            let v = number(array, row)?;
            if optional && v.is_nan() {
                return Ok(None);
            }
            if !v.is_finite() || v.fract() != 0.0 || v < i64::MIN as f64 || v >= i64::MAX as f64 {
                return Err(invalid("nonintegral or out-of-range hillslope watbal key"));
            }
            Ok(Some(v as i64))
        }
    }
}

fn number(array: &dyn Array, row: usize) -> PyResult<f64> {
    if array.is_null(row) || array.data_type() == &DataType::Null {
        return Ok(f64::NAN);
    }
    macro_rules! value {
        ($ty:ty) => {
            array
                .as_any()
                .downcast_ref::<$ty>()
                .ok_or_else(|| invalid("invalid numeric array"))?
                .value(row) as f64
        };
    }
    Ok(match array.data_type() {
        DataType::Int8 => value!(Int8Array),
        DataType::Int16 => value!(Int16Array),
        DataType::Int32 => value!(Int32Array),
        DataType::Int64 => value!(Int64Array),
        DataType::UInt8 => value!(UInt8Array),
        DataType::UInt16 => value!(UInt16Array),
        DataType::UInt32 => value!(UInt32Array),
        DataType::UInt64 => value!(UInt64Array),
        DataType::Float32 => value!(Float32Array),
        DataType::Float64 => value!(Float64Array),
        _ => return Err(invalid("unsupported hillslope watbal numeric array")),
    })
}

fn metrics(array: &dyn Array) -> PyResult<Vec<f64>> {
    macro_rules! values {
        ($ty:ty) => {{
            let a = array
                .as_any()
                .downcast_ref::<$ty>()
                .ok_or_else(|| invalid("invalid metric array"))?;
            a.iter()
                .map(|v| v.map(|x| x as f64).unwrap_or(0.0))
                .collect::<Vec<_>>()
        }};
    }
    let mut values = match array.data_type() {
        DataType::Int8 => values!(Int8Array),
        DataType::Int16 => values!(Int16Array),
        DataType::Int32 => values!(Int32Array),
        DataType::Int64 => values!(Int64Array),
        DataType::UInt8 => values!(UInt8Array),
        DataType::UInt16 => values!(UInt16Array),
        DataType::UInt32 => values!(UInt32Array),
        DataType::UInt64 => values!(UInt64Array),
        DataType::Float32 => values!(Float32Array),
        DataType::Float64 => values!(Float64Array),
        DataType::Null => vec![0.0; array.len()],
        _ => return Err(invalid("unsupported metric array")),
    };
    for v in &mut values {
        if v.is_infinite() {
            return Err(invalid("infinite hillslope watbal metric"));
        }
        if v.is_nan() {
            *v = 0.0;
        }
    }
    Ok(values)
}

#[derive(Clone, Copy, Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) -> PyResult<()> {
        let y = value - self.correction;
        let t = self.value + y;
        self.correction = (t - self.value) - y;
        self.value = t;
        if !t.is_finite() {
            return Err(invalid("hillslope watbal aggregate overflow"));
        }
        Ok(())
    }
}

pub fn wepp_ids(path: &Path) -> PyResult<Vec<i64>> {
    let mut ids = BTreeSet::new();
    for batch in reader(path, &["wepp_id"])? {
        let batch = batch.map_err(invalid)?;
        for row in 0..batch.num_rows() {
            ids.insert(
                key(batch.column(0).as_ref(), row, false)?
                    .ok_or_else(|| invalid("missing WEPP key"))?,
            );
        }
    }
    Ok(ids.into_iter().collect())
}

pub fn produce(
    path: &Path,
    output: &Path,
    mapping: &HashMap<i64, i64>,
    pandas_metadata: Option<&str>,
) -> PyResult<(usize, usize, usize)> {
    let input_identity = std::fs::canonicalize(path).map_err(io)?;
    if output == path
        || (output.exists() && std::fs::canonicalize(output).map_err(io)? == input_identity)
    {
        return Err(invalid("hillslope watbal output must not alias its input"));
    }
    let output_mode = match std::fs::symlink_metadata(output) {
        Ok(meta) if !meta.is_file() => {
            return Err(io("hillslope watbal output must be a regular file"))
        }
        Ok(meta) => Some(meta.permissions().mode() & 0o777),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io(e)),
    };
    let mut flux: HashMap<(i64, i64), [Sum; 7]> = HashMap::new();
    let mut areas = BTreeMap::new();
    let mut input_rows = 0;
    for batch in reader(path, &INPUT)? {
        let batch = batch.map_err(invalid)?;
        let columns = INPUT
            .iter()
            .map(|name| {
                batch
                    .column_by_name(name)
                    .ok_or_else(|| invalid(format!("missing {name}")))
            })
            .collect::<PyResult<Vec<_>>>()?;
        let numeric = columns[3..]
            .iter()
            .map(|a| metrics(a.as_ref()))
            .collect::<PyResult<Vec<_>>>()?;
        for row in 0..batch.num_rows() {
            let wepp =
                key(columns[0].as_ref(), row, false)?.ok_or_else(|| invalid("missing WEPP key"))?;
            let ofe = key(columns[1].as_ref(), row, true)?;
            let year =
                key(columns[2].as_ref(), row, false)?.ok_or_else(|| invalid("missing year key"))?;
            let topaz = *mapping
                .get(&wepp)
                .ok_or_else(|| PyKeyError::new_err(wepp))?;
            if let Some(ofe) = ofe {
                areas.entry((wepp, ofe)).or_insert(numeric[7][row]);
            }
            let sums = flux.entry((topaz, year)).or_default();
            for i in 0..7 {
                sums[i].add(numeric[i][row])?;
            }
        }
        input_rows += batch.num_rows();
    }
    let ofe_keys = areas.len();
    let mut hill_areas: BTreeMap<i64, Sum> = BTreeMap::new();
    for ((wepp, _), area) in areas {
        hill_areas.entry(wepp).or_default().add(area)?;
    }
    let mut topaz_areas: HashMap<i64, f64> = HashMap::new();
    for (wepp, area) in hill_areas {
        let topaz = mapping
            .get(&wepp)
            .ok_or_else(|| PyKeyError::new_err(wepp))?;
        let sum = topaz_areas.entry(*topaz).or_default();
        *sum += area.value;
        if !sum.is_finite() {
            return Err(invalid("hillslope watbal area overflow"));
        }
    }
    let empty = flux.is_empty();
    let mut schema = Schema::new(
        OUTPUT
            .iter()
            .enumerate()
            .map(|(i, name)| {
                Field::new(
                    *name,
                    if empty {
                        DataType::Null
                    } else if i < 2 {
                        DataType::Int64
                    } else {
                        DataType::Float64
                    },
                    true,
                )
            })
            .collect::<Vec<_>>(),
    );
    if let Some(metadata) = pandas_metadata {
        schema.metadata.insert("pandas".into(), metadata.into());
    }
    let mut rows: Vec<_> = flux.into_iter().collect();
    rows.sort_unstable_by_key(|(key, _)| *key);
    let rows_written = rows.len();
    // At this boundary all schema/value work is complete: writer failures are I/O.
    let mut sink = ParquetSink::try_new_with_mode(output, schema, output_mode).map_err(io)?;
    for rows in rows.chunks(BATCH_ROWS) {
        let mut topaz = Vec::with_capacity(rows.len());
        let mut years = Vec::with_capacity(rows.len());
        let mut metrics: [Vec<f64>; 6] = std::array::from_fn(|_| Vec::with_capacity(rows.len()));
        for ((id, year), sums) in rows {
            topaz.push(*id);
            years.push(*year);
            metrics[0].push(*topaz_areas.get(id).unwrap_or(&0.0));
            for i in 0..4 {
                metrics[i + 1].push(sums[i].value);
            }
            let evap = (sums[4].value + sums[5].value) + sums[6].value;
            if !evap.is_finite() {
                return Err(invalid("hillslope watbal evaporation overflow"));
            }
            metrics[5].push(evap);
        }
        let mut arrays: Vec<Box<dyn Array>> = vec![
            Box::new(Int64Array::from(topaz)),
            Box::new(Int64Array::from(years)),
        ];
        arrays.extend(
            metrics
                .into_iter()
                .map(|v| Box::new(Float64Array::from(v)) as Box<dyn Array>),
        );
        sink.write_chunk(Chunk::new(arrays)).map_err(io)?;
    }
    sink.finish().map_err(io)?;
    Ok((input_rows, rows_written, ofe_keys))
}

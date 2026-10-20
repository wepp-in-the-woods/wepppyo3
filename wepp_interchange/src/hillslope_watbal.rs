//! Bounded hillslope/year water-balance cache producer. WEPPpy owns mapping policy.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use arrow_array::*;
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ProjectionMask;
use parquet::file::statistics::Statistics;
use pyo3::exceptions::{PyKeyError, PyOSError, PyValueError};
use pyo3::PyResult;

use crate::arrow_support::Chunk;
use crate::parquet::ParquetSink;

const INPUT: [&str; 12] = [
    "wepp_id",
    "ofe_id",
    "water_year",
    "P",
    "Dp",
    "Q",
    "latqcc",
    "Ep",
    "Es",
    "Er",
    "Area",
    "sim_day_index",
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

// A narrow geometry scan lets single-OFE hills retain bounded annual state.
// Their daily weighted depth is exactly the raw depth; only MOFE hills need
// daily denominators. Count after translation so merged IDs cannot take this path.
#[derive(Default)]
struct Geometry {
    areas: BTreeMap<(i64, i64), f64>,
    terminal: HashMap<i64, i64>,
    hills: HashMap<i64, f64>,
    counts: HashMap<i64, usize>,
}

fn geometry(path: &Path, mapping: &HashMap<i64, i64>) -> PyResult<Geometry> {
    let mut result = Geometry::default();
    // Constant exact row-group bounds prove stationary geometry without
    // decoding millions of repeated single-OFE rows. Other layouts scan below.
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path).map_err(io)?).map_err(invalid)?;
    let indices: Option<Vec<_>> = ["wepp_id", "ofe_id", "Area"].iter().map(|name|
        builder.parquet_schema().columns().iter().position(|c| c.path().string() == *name)
    ).collect();
    let signed_keys = ["wepp_id", "ofe_id"].iter().all(|name|
        builder.schema().field_with_name(name).map(|field|
            matches!(field.data_type(), DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64)).unwrap_or(false));
    if let Some(indices) = indices.filter(|_| signed_keys) {
        let records: Option<Vec<_>> = builder.metadata().row_groups().iter().map(|group| {
            let integer = |index| {
                let stats = group.column(index).statistics()?;
                if stats.null_count_opt() != Some(0) || !stats.min_is_exact() || !stats.max_is_exact() { return None; }
                match stats {
                    Statistics::Int32(s) if s.min_opt()? == s.max_opt()? => Some(i64::from(*s.min_opt()?)),
                    Statistics::Int64(s) if s.min_opt()? == s.max_opt()? => Some(*s.min_opt()?),
                    _ => None,
                }
            };
            let stats = group.column(indices[2]).statistics()?;
            if stats.null_count_opt() != Some(0) || !stats.min_is_exact() || !stats.max_is_exact() { return None; }
            let area = match stats {
                Statistics::Double(s) if s.min_opt()? == s.max_opt()? => *s.min_opt()?,
                Statistics::Float(s) if s.min_opt()? == s.max_opt()? => f64::from(*s.min_opt()?),
                _ => return None,
            };
            Some((integer(indices[0])?, integer(indices[1])?, area))
        }).collect();
        if let Some(records) = records {
            for (wepp, ofe, area) in records { add_geometry(&mut result, mapping, wepp, ofe, area)?; }
            return Ok(result);
        }
    }
    for batch in reader(path, &["wepp_id", "ofe_id", "Area"])? {
        let batch = batch.map_err(invalid)?;
        let columns = ["wepp_id", "ofe_id", "Area"].iter().map(|name|
            batch.column_by_name(name).ok_or_else(|| invalid("missing geometry"))
        ).collect::<PyResult<Vec<_>>>()?;
        for row in 0..batch.num_rows() {
            let wepp = key(columns[0].as_ref(), row, false)?.unwrap();
            let ofe = key(columns[1].as_ref(), row, false)?.unwrap();
            let area = number(columns[2].as_ref(), row)?;
            add_geometry(&mut result, mapping, wepp, ofe, area)?;
        }
    }
    Ok(result)
}

fn add_geometry(result: &mut Geometry, mapping: &HashMap<i64, i64>, wepp: i64, ofe: i64, area: f64) -> PyResult<()> {
            if !area.is_finite() || area <= 0.0 {
                return Err(invalid("hillslope watbal Area must be finite and positive"));
            }
            let topaz = mapping.get(&wepp).ok_or_else(|| PyKeyError::new_err(wepp))?;
            if let Some(prior) = result.areas.get(&(wepp, ofe)) {
                if *prior != area { return Err(invalid("changing hillslope watbal Area")); }
            } else {
                result.areas.insert((wepp, ofe), area);
                *result.counts.entry(*topaz).or_default() += 1;
                let total = result.hills.entry(wepp).or_default();
                *total += area;
                if !total.is_finite() { return Err(invalid("hillslope watbal area overflow")); }
            }
            result.terminal.entry(wepp).and_modify(|v| *v = (*v).max(ofe)).or_insert(ofe);
    Ok(())
}

#[derive(Default)]
struct NativeDay {
    local: [Sum; 6],
    outlet: Option<[f64; 2]>,
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
    let geometry = geometry(path, mapping)?;
    let mut flux: HashMap<(i64, i64), [Sum; 7]> = HashMap::new();
    let mut current_observation_key = None;
    let mut current_topaz = 0;
    let mut current_single = false;
    let mut current_days: Vec<i64> = Vec::new();
    let mut observations: HashMap<(i64, i64, i64), Vec<i64>> = HashMap::new();
    let mut daily: HashMap<(i64, i64, i64), NativeDay> = HashMap::new();
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
        let numeric = columns[3..10]
            .iter()
            .map(|a| metrics(a.as_ref()))
            .collect::<PyResult<Vec<_>>>()?;
        for row in 0..batch.num_rows() {
            let wepp =
                key(columns[0].as_ref(), row, false)?.ok_or_else(|| invalid("missing WEPP key"))?;
            let ofe =
                key(columns[1].as_ref(), row, false)?.ok_or_else(|| invalid("missing OFE key"))?;
            let day = key(columns[11].as_ref(), row, false)?
                .ok_or_else(|| invalid("missing simulation day key"))?;
            let year =
                key(columns[2].as_ref(), row, false)?.ok_or_else(|| invalid("missing year key"))?;
            let observation_key = (wepp, ofe, year);
            if current_observation_key != Some(observation_key) {
                if let Some(prior) = current_observation_key {
                    observations.insert(prior, std::mem::take(&mut current_days));
                }
                current_days = observations.remove(&observation_key).unwrap_or_default();
                current_observation_key = Some(observation_key);
                current_topaz = *mapping.get(&wepp).ok_or_else(|| PyKeyError::new_err(wepp))?;
                current_single = geometry.counts.get(&current_topaz) == Some(&1);
            }
            // Canonical rows are chronological. Keep a compact sorted day set;
            // unordered callers retain the same duplicate check without hashing
            // every observation on the normal path.
            if current_days.last().map(|prior| *prior < day).unwrap_or(true) {
                current_days.push(day);
            } else {
                match current_days.binary_search(&day) {
                    Ok(_) => return Err(invalid("duplicate hillslope daily OFE observation")),
                    Err(index) => current_days.insert(index, day),
                }
            }
            let topaz = current_topaz;
            let area = number(columns[10].as_ref(), row)?;
            if !area.is_finite() || area <= 0.0 {
                return Err(invalid("hillslope watbal Area must be finite and positive"));
            }
            if current_single {
                if !number(columns[5].as_ref(), row)?.is_finite()
                    || !number(columns[6].as_ref(), row)?.is_finite() {
                    return Err(invalid("nonfinite hillslope terminal flux"));
                }
                let sums = flux.entry((topaz, year)).or_default();
                for i in 0..7 { sums[i].add(numeric[i][row])?; }
                continue;
            }
            let terminal = geometry.terminal[&wepp];
            let outlet = if ofe == terminal {
                let q = number(columns[5].as_ref(), row)?;
                let lateral = number(columns[6].as_ref(), row)?;
                if !q.is_finite() || !lateral.is_finite() {
                    return Err(invalid("nonfinite hillslope terminal flux"));
                }
                Some([q * geometry.hills[&wepp], lateral * area])
            } else { None };
            let day_sums = daily.entry((wepp, year, day)).or_default();
            for (target, source) in [0, 1, 4, 5, 6].iter().enumerate() {
                day_sums.local[target].add(numeric[*source][row] * area)?;
            }
            day_sums.local[5].add(area)?;
            if let Some(outlet) = outlet {
                if day_sums.outlet.replace(outlet).is_some() {
                    return Err(invalid("duplicate hillslope terminal observation"));
                }
            }
        }
        input_rows += batch.num_rows();
    }
    // Native outlet selection precedes mapping. Local fields keep represented
    // area; outlet volumes use the complete stationary native footprint.
    let mut native_counts: HashMap<i64, usize> = HashMap::new();
    for wepp in geometry.hills.keys() { *native_counts.entry(mapping[wepp]).or_default() += 1; }
    let mut mapped: HashMap<(i64, i64, i64), [Sum; 9]> = HashMap::new();
    for ((wepp, year, day), values) in daily {
        let outlet = values.outlet.ok_or_else(|| invalid("missing known hillslope terminal observation"))?;
        let topaz = mapping[&wepp];
        if native_counts[&topaz] == 1 {
            let annual = flux.entry((topaz, year)).or_default();
            for (source, target) in [0, 1, 4, 5, 6].iter().enumerate() {
                annual[*target].add(values.local[source].value / values.local[5].value)?;
            }
            annual[2].add(outlet[0] / geometry.hills[&wepp])?;
            annual[3].add(outlet[1] / geometry.hills[&wepp])?;
            continue;
        }
        let sums = mapped.entry((topaz, year, day)).or_default();
        for i in 0..6 { sums[i].add(values.local[i].value)?; }
        sums[6].add(outlet[0])?;
        sums[7].add(outlet[1])?;
        sums[8].add(geometry.hills[&wepp])?;
    }
    for ((topaz, year, _), sums) in mapped {
        let annual = flux.entry((topaz, year)).or_default();
        for (source, target) in [0, 1, 4, 5, 6].iter().enumerate() {
            annual[*target].add(sums[source].value / sums[5].value)?;
        }
        annual[2].add(sums[6].value / sums[8].value)?;
        annual[3].add(sums[7].value / sums[8].value)?;
    }
    let ofe_keys = geometry.areas.len();
    let mut topaz_areas: HashMap<i64, f64> = HashMap::new();
    for (wepp, area) in geometry.hills {
        let sum = topaz_areas.entry(mapping[&wepp]).or_default();
        *sum += area;
        if !sum.is_finite() { return Err(invalid("hillslope watbal area overflow")); }
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
    schema
        .metadata
        .insert("hillslope_watbal_aggregation".into(), "mofe-water-v2".into());
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

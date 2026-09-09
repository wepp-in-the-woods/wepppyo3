//! Equal projected-cell area means; domain policy belongs to the caller.
use gdal::{raster::RasterBand, Dataset};
use pyo3::{
    exceptions::{PyIOError, PyValueError},
    prelude::*,
    types::PyDict,
};
use std::collections::{BTreeMap, HashSet};

#[derive(Default)]
struct Accumulator {
    scale: f64,
    sum: f64,
    correction: f64,
    valid: usize,
    missing: usize,
}
impl Accumulator {
    fn add(&mut self, value: f64) {
        let magnitude = value.abs();
        if magnitude > self.scale {
            let ratio = self.scale / magnitude;
            self.sum *= ratio;
            self.correction *= ratio;
            self.scale = magnitude;
        }
        if self.scale != 0.0 {
            let value = value / self.scale;
            let next = self.sum + value;
            self.correction += if self.sum.abs() >= value.abs() {
                (self.sum - next) + value
            } else {
                (value - next) + self.sum
            };
            self.sum = next;
        }
    }
    fn mean(&self) -> f64 {
        self.scale * ((self.sum + self.correction) / (self.valid + self.missing) as f64)
    }
}
fn io(error: gdal::errors::GdalError) -> PyErr {
    PyIOError::new_err(error.to_string())
}
fn read(band: &RasterBand<'_>, shape: (usize, usize)) -> PyResult<(Vec<f64>, Vec<u8>)> {
    let values = band
        .read_as::<f64>((0, 0), shape, shape, None)
        .map_err(io)?
        .data;
    let mask = band
        .open_mask_band()
        .map_err(io)?
        .read_as::<u8>((0, 0), shape, shape, None)
        .map_err(io)?
        .data;
    if values.len() != shape.0 * shape.1 || mask.len() != values.len() {
        return Err(PyValueError::new_err("Raster data/mask length mismatch"));
    }
    Ok((values, mask))
}
fn missing(value: f64, mask: u8, nodata: Option<f64>) -> bool {
    mask == 0
        || !value.is_finite()
        || nodata.is_some_and(|n| value == n || (n.is_nan() && value.is_nan()))
}

#[pyfunction]
#[pyo3(signature = (key_fn, parameter_fn, ignore_channels=true, ignore_keys=None, band_indx=1, default_value=None))]
pub fn identify_area_weighted_mean_single_raster_key(
    py: Python<'_>,
    key_fn: &str,
    parameter_fn: &str,
    ignore_channels: bool,
    ignore_keys: Option<HashSet<i32>>,
    band_indx: isize,
    default_value: Option<f64>,
) -> PyResult<PyObject> {
    if default_value.is_some_and(|d| !d.is_finite()) {
        return Err(PyValueError::new_err("default_value must be finite"));
    }
    let keys = Dataset::open(key_fn).map_err(io)?;
    let parameters = Dataset::open(parameter_fn).map_err(io)?;
    if band_indx < 1 || band_indx > parameters.raster_count() {
        return Err(PyValueError::new_err(
            "band_indx is outside parameter raster bands",
        ));
    }
    let shape = keys.raster_size();
    if shape != parameters.raster_size() {
        return Err(PyValueError::new_err("Raster dimensions mismatch"));
    }
    let key_crs = keys
        .spatial_ref()
        .map_err(|e| PyValueError::new_err(format!("Invalid key CRS: {e}")))?;
    let parameter_crs = parameters
        .spatial_ref()
        .map_err(|e| PyValueError::new_err(format!("Invalid parameter CRS: {e}")))?;
    if !key_crs.is_projected() || !parameter_crs.is_projected() || key_crs != parameter_crs {
        return Err(PyValueError::new_err(
            "Rasters must have equivalent projected CRS",
        ));
    }
    let transform = keys.geo_transform().map_err(io)?;
    let other = parameters.geo_transform().map_err(io)?;
    for t in [transform, other] {
        let determinant = t[1] * t[5] - t[2] * t[4];
        if t.iter().any(|v| !v.is_finite()) || !determinant.is_finite() || determinant == 0.0 {
            return Err(PyValueError::new_err(
                "Affine transform must be finite and nonsingular",
            ));
        }
    }
    if transform != other {
        return Err(PyValueError::new_err("Raster affine transforms mismatch"));
    }
    let key_band = keys.rasterband(1).map_err(io)?;
    let parameter_band = parameters.rasterband(band_indx).map_err(io)?;
    let (key_data, key_mask) = read(&key_band, shape)?;
    let (data, mask) = read(&parameter_band, shape)?;
    let key_nodata = key_band.no_data_value();
    let parameter_nodata = parameter_band.no_data_value();
    let ignored = ignore_keys.unwrap_or_default();
    let mut accumulators: BTreeMap<i32, Accumulator> = BTreeMap::new();
    for i in 0..key_data.len() {
        let raw_key = key_data[i];
        if missing(raw_key, key_mask[i], key_nodata) {
            continue;
        }
        if raw_key.fract() != 0.0 || raw_key < i32::MIN as f64 || raw_key > i32::MAX as f64 {
            return Err(PyValueError::new_err(
                "Key raster must contain signed 32-bit integral identifiers",
            ));
        }
        let key = raw_key as i32;
        if ignored.contains(&key) || (ignore_channels && key % 10 == 4) {
            continue;
        }
        let acc = accumulators.entry(key).or_default();
        if missing(data[i], mask[i], parameter_nodata) {
            acc.missing += 1;
            if let Some(default) = default_value {
                acc.add(default);
            }
        } else {
            acc.valid += 1;
            acc.add(data[i]);
        }
    }
    if default_value.is_none() {
        let affected: Vec<_> = accumulators
            .iter()
            .filter(|(_, a)| a.missing > 0)
            .take(10)
            .map(|(k, a)| format!("{k}: {}/{} missing", a.missing, a.valid + a.missing))
            .collect();
        if !affected.is_empty() {
            return Err(PyValueError::new_err(format!(
                "Missing parameter coverage without default_value (first 10 keys): {}",
                affected.join(", ")
            )));
        }
    }
    let output = PyDict::new_bound(py);
    for (key, acc) in accumulators {
        let mean = acc.mean();
        if !mean.is_finite() {
            return Err(PyValueError::new_err(format!(
                "Nonfinite mean for key {key}"
            )));
        }
        let record = PyDict::new_bound(py);
        record.set_item("mean", mean)?;
        record.set_item("valid_cell_count", acc.valid)?;
        record.set_item("missing_cell_count", acc.missing)?;
        record.set_item("total_cell_count", acc.valid + acc.missing)?;
        output.set_item(key.to_string(), record)?;
    }
    Ok(output.into_py(py))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_means_include_extreme_finite_values() {
        for (values, expected) in [
            (vec![f64::MAX, f64::MAX], f64::MAX),
            (vec![f64::MAX, -f64::MAX], 0.0),
            (vec![0.0001, 0.0001, 0.05], 0.0502 / 3.0),
        ] {
            let mut a = Accumulator::default();
            for v in values {
                a.add(v);
                a.valid += 1;
            }
            assert!((a.mean() - expected).abs() <= expected.abs() * 1e-15);
        }
    }
}

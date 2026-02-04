use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use gdal::errors::GdalError;
use gdal::raster::{
    Buffer, ColorEntry, ColorTable, GdalDataType, PaletteInterpretation, RasterCreationOption,
};
use gdal::spatial_ref::SpatialRef;
use gdal::Dataset;
use gdal_sys::OGRErr::OGRERR_NONE;
use numpy::{PyArray2, PyArrayMethods};
use ordered_float::OrderedFloat;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use pyo3::wrap_pyfunction;
use serde::Deserialize;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum RawValue {
    Int(i64),
    Float(OrderedFloat<f64>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum NormalizedValue {
    Int(i64),
    Float(OrderedFloat<f64>),
}

#[derive(Debug, Clone)]
struct CountInfo {
    count: u64,
    first_seen: usize,
}

#[derive(Debug, Deserialize)]
struct ColorMapFile {
    colors: Vec<ColorMapEntry>,
}

#[derive(Debug, Deserialize)]
struct ColorMapEntry {
    rgb: [i32; 3],
    severity: String,
}

const ALLOWED_SEVERITIES: [&str; 4] = ["unburned", "low", "mod", "high"];

fn default_color_map() -> HashMap<(i32, i32, i32), String> {
    let mut map = HashMap::new();
    map.insert((0, 100, 0), "unburned".to_string());
    map.insert((0, 0, 0), "unburned".to_string());
    map.insert((0, 115, 74), "unburned".to_string());
    map.insert((0, 175, 166), "unburned".to_string());
    map.insert((102, 204, 204), "low".to_string());
    map.insert((102, 205, 205), "low".to_string());
    map.insert((115, 255, 223), "low".to_string());
    map.insert((127, 255, 212), "low".to_string());
    map.insert((0, 255, 255), "low".to_string());
    map.insert((77, 230, 0), "low".to_string());
    map.insert((255, 255, 0), "mod".to_string());
    map.insert((255, 232, 32), "mod".to_string());
    map.insert((255, 0, 0), "high".to_string());
    map
}

fn load_color_map(path: Option<&str>) -> PyResult<HashMap<(i32, i32, i32), String>> {
    match path {
        Some(p) if !p.trim().is_empty() => {
            let contents = fs::read_to_string(p)
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
            let parsed: ColorMapFile = serde_json::from_str(&contents)
                .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{}", e)))?;
            let mut map = HashMap::new();
            for entry in parsed.colors {
                map.insert((entry.rgb[0], entry.rgb[1], entry.rgb[2]), entry.severity);
            }
            Ok(map)
        }
        _ => Ok(default_color_map()),
    }
}

fn validate_srs(dataset: &Dataset) -> bool {
    let wkt = dataset.projection();
    if wkt.trim().is_empty() {
        return false;
    }

    match SpatialRef::from_wkt(&wkt) {
        Ok(srs) => unsafe { gdal_sys::OSRValidate(srs.to_c_hsrs()) == OGRERR_NONE },
        Err(_) => false,
    }
}

#[inline]
fn is_int_value(v: f64) -> bool {
    v.is_finite() && v == v.trunc()
}

fn raw_value_from_f64(v: f64, band_is_float: bool) -> RawValue {
    if band_is_float {
        RawValue::Float(OrderedFloat(v))
    } else {
        RawValue::Int(v as i64)
    }
}

fn normalized_value_from_f64(v: f64) -> NormalizedValue {
    if is_int_value(v) {
        NormalizedValue::Int(v as i64)
    } else {
        NormalizedValue::Float(OrderedFloat(v))
    }
}

fn normalized_value_as_f64(v: &NormalizedValue) -> f64 {
    match v {
        NormalizedValue::Int(i) => *i as f64,
        NormalizedValue::Float(f) => f.0,
    }
}

fn raw_value_to_py(py: Python<'_>, v: &RawValue) -> PyObject {
    match v {
        RawValue::Int(i) => i.into_py(py),
        RawValue::Float(f) => f.0.into_py(py),
    }
}

fn normalized_value_to_py(py: Python<'_>, v: &NormalizedValue) -> PyObject {
    match v {
        NormalizedValue::Int(i) => i.into_py(py),
        NormalizedValue::Float(f) => f.0.into_py(py),
    }
}

fn band_is_float(band_type: GdalDataType) -> bool {
    matches!(band_type, GdalDataType::Float32 | GdalDataType::Float64)
}

fn scan_band_u8(
    band: &gdal::raster::RasterBand<'_>,
) -> Result<(HashMap<RawValue, CountInfo>, HashSet<NormalizedValue>), GdalError> {
    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }
    let chunk_y = if block_y < 256 {
        std::cmp::min(height, 256)
    } else {
        block_y
    };

    let mut counts = vec![0u64; 256];
    let mut first_seen = vec![usize::MAX; 256];
    let mut buffer = vec![0u8; width * chunk_y];
    let mut pixel_index = 0usize;

    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(chunk_y, height - y);
        let slice = &mut buffer[..width * y_size];
        band.read_into_slice(
            (0, y as isize),
            (width, y_size),
            (width, y_size),
            slice,
            None,
        )?;

        for &value in slice.iter() {
            let idx = value as usize;
            if counts[idx] == 0 {
                first_seen[idx] = pixel_index;
            }
            counts[idx] += 1;
            pixel_index += 1;
        }
        y += y_size;
    }

    let mut count_map: HashMap<RawValue, CountInfo> = HashMap::new();
    let mut unique: HashSet<NormalizedValue> = HashSet::new();
    for v in 0..256 {
        if counts[v] > 0 {
            count_map.insert(
                RawValue::Int(v as i64),
                CountInfo {
                    count: counts[v],
                    first_seen: first_seen[v],
                },
            );
            unique.insert(NormalizedValue::Int(v as i64));
        }
    }

    Ok((count_map, unique))
}

fn scan_band_u16(
    band: &gdal::raster::RasterBand<'_>,
) -> Result<(HashMap<RawValue, CountInfo>, HashSet<NormalizedValue>), GdalError> {
    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }
    let chunk_y = if block_y < 256 {
        std::cmp::min(height, 256)
    } else {
        block_y
    };

    let mut counts = vec![0u64; 65536];
    let mut first_seen = vec![usize::MAX; 65536];
    let mut buffer = vec![0u16; width * chunk_y];
    let mut pixel_index = 0usize;

    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(chunk_y, height - y);
        let slice = &mut buffer[..width * y_size];
        band.read_into_slice(
            (0, y as isize),
            (width, y_size),
            (width, y_size),
            slice,
            None,
        )?;

        for &value in slice.iter() {
            let idx = value as usize;
            if counts[idx] == 0 {
                first_seen[idx] = pixel_index;
            }
            counts[idx] += 1;
            pixel_index += 1;
        }
        y += y_size;
    }

    let mut count_map: HashMap<RawValue, CountInfo> = HashMap::new();
    let mut unique: HashSet<NormalizedValue> = HashSet::new();
    for v in 0..65536 {
        if counts[v] > 0 {
            let value = v as i64;
            count_map.insert(
                RawValue::Int(value),
                CountInfo {
                    count: counts[v],
                    first_seen: first_seen[v],
                },
            );
            unique.insert(NormalizedValue::Int(value));
        }
    }

    Ok((count_map, unique))
}

fn scan_band_i16(
    band: &gdal::raster::RasterBand<'_>,
) -> Result<(HashMap<RawValue, CountInfo>, HashSet<NormalizedValue>), GdalError> {
    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }
    let chunk_y = if block_y < 256 {
        std::cmp::min(height, 256)
    } else {
        block_y
    };

    let mut counts = vec![0u64; 65536];
    let mut first_seen = vec![usize::MAX; 65536];
    let mut buffer = vec![0i16; width * chunk_y];
    let mut pixel_index = 0usize;

    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(chunk_y, height - y);
        let slice = &mut buffer[..width * y_size];
        band.read_into_slice(
            (0, y as isize),
            (width, y_size),
            (width, y_size),
            slice,
            None,
        )?;

        for &value in slice.iter() {
            let idx = (value as i32 - i16::MIN as i32) as usize;
            if counts[idx] == 0 {
                first_seen[idx] = pixel_index;
            }
            counts[idx] += 1;
            pixel_index += 1;
        }
        y += y_size;
    }

    let mut count_map: HashMap<RawValue, CountInfo> = HashMap::new();
    let mut unique: HashSet<NormalizedValue> = HashSet::new();
    for v in 0..65536 {
        if counts[v] > 0 {
            let value = v as i64 + i16::MIN as i64;
            count_map.insert(
                RawValue::Int(value),
                CountInfo {
                    count: counts[v],
                    first_seen: first_seen[v],
                },
            );
            unique.insert(NormalizedValue::Int(value));
        }
    }

    Ok((count_map, unique))
}

fn scan_band(
    band: &gdal::raster::RasterBand<'_>,
    band_is_float: bool,
) -> Result<(HashMap<RawValue, CountInfo>, HashSet<NormalizedValue>, bool), GdalError> {
    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }

    let mut counts: HashMap<RawValue, CountInfo> = HashMap::new();
    let mut unique: HashSet<NormalizedValue> = HashSet::new();
    let mut has_non_integer = false;
    let mut first_seen = 0usize;
    let mut track_unique = true;

    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(block_y, height - y);
        let buffer =
            band.read_as::<f64>((0, y as isize), (width, y_size), (width, y_size), None)?;

        for value in buffer.data.iter() {
            let v = *value as f64;
            if !has_non_integer && !is_int_value(v) {
                has_non_integer = true;
            }

            let raw_key = raw_value_from_f64(v, band_is_float);
            let entry = counts.entry(raw_key).or_insert_with(|| {
                let info = CountInfo {
                    count: 0,
                    first_seen,
                };
                first_seen += 1;
                info
            });
            entry.count += 1;

            if track_unique {
                unique.insert(normalized_value_from_f64(v));
                if counts.len() > 512 {
                    track_unique = false;
                    unique.clear();
                }
            }
        }
        y += y_size;
    }

    Ok((counts, unique, has_non_integer))
}

fn summarize_color_table_internal(
    band: &gdal::raster::RasterBand<'_>,
    color_map: &HashMap<(i32, i32, i32), String>,
) -> (bool, Vec<String>, bool) {
    let ct = band.color_table();
    if ct.is_none() {
        return (false, Vec::new(), false);
    }

    let ct = ct.unwrap();
    let mut severities: HashSet<String> = HashSet::new();

    for idx in 0..ct.entry_count() {
        if let Some(entry) = ct.entry_as_rgb(idx) {
            let rgb = (entry.r as i32, entry.g as i32, entry.b as i32);
            if let Some(severity) = color_map.get(&rgb) {
                if ALLOWED_SEVERITIES.contains(&severity.as_str()) {
                    severities.insert(severity.clone());
                }
            }
        }
    }

    let mut severity_list: Vec<String> = severities.into_iter().collect();
    severity_list.sort();
    let valid = severity_list
        .iter()
        .any(|s| matches!(s.as_str(), "low" | "mod" | "high"));

    (true, severity_list, valid)
}

fn parse_ct_dict(ct: &Bound<'_, PyDict>) -> PyResult<HashMap<String, Vec<i32>>> {
    let mut out: HashMap<String, Vec<i32>> = HashMap::new();
    for (key, value) in ct.iter() {
        let key_str: String = key.extract::<String>()?;
        let values: Vec<i32> = value.extract::<Vec<i32>>()?;
        out.insert(key_str, values);
    }
    Ok(out)
}

fn build_color_table_maps(
    band: &gdal::raster::RasterBand<'_>,
    color_map: &HashMap<(i32, i32, i32), String>,
) -> Option<(
    HashMap<String, Vec<i32>>,
    Vec<((i32, i32, i32), Option<String>)>,
    Vec<String>,
)> {
    let ct = band.color_table()?;
    let mut class_index_map: HashMap<String, Vec<i32>> = HashMap::new();
    for sev in ALLOWED_SEVERITIES.iter() {
        class_index_map.insert((*sev).to_string(), Vec::new());
    }

    let mut color_lookup: Vec<((i32, i32, i32), Option<String>)> = Vec::new();
    let mut severity_set: HashSet<String> = HashSet::new();

    for idx in 0..ct.entry_count() {
        if let Some(entry) = ct.entry_as_rgb(idx) {
            let rgb = (entry.r as i32, entry.g as i32, entry.b as i32);
            let severity = color_map.get(&rgb).cloned();
            if let Some(sev) = severity.as_ref() {
                if ALLOWED_SEVERITIES.contains(&sev.as_str()) {
                    severity_set.insert(sev.clone());
                    if let Some(values) = class_index_map.get_mut(sev) {
                        values.push(idx as i32);
                    }
                }
            }
            color_lookup.push((rgb, severity));
        }
    }

    let mut severities: Vec<String> = severity_set.into_iter().collect();
    severities.sort();

    Some((class_index_map, color_lookup, severities))
}

fn build_ct_from_band(
    band: &gdal::raster::RasterBand<'_>,
    color_map: &HashMap<(i32, i32, i32), String>,
) -> Option<HashMap<String, Vec<i32>>> {
    build_color_table_maps(band, color_map)
        .map(|(class_index_map, _color_lookup, _severities)| class_index_map)
}

fn severity_code(severity: &str) -> Option<u8> {
    match severity {
        "unburned" => Some(0),
        "low" => Some(1),
        "mod" => Some(2),
        "high" => Some(3),
        _ => None,
    }
}

fn build_value_map(ct: &HashMap<String, Vec<i32>>) -> HashMap<i64, u8> {
    let mut map = HashMap::new();
    for (severity, values) in ct.iter() {
        if let Some(code) = severity_code(severity) {
            for value in values {
                map.insert(*value as i64, code);
            }
        }
    }
    map
}

fn normalize_offset(offset: i32) -> PyResult<u8> {
    if !(0..=255).contains(&offset) {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "offset must be between 0 and 255",
        ));
    }
    Ok(offset as u8)
}

fn nodata_to_i64(nodata: Option<Vec<f64>>) -> Vec<i64> {
    nodata
        .unwrap_or_default()
        .into_iter()
        .map(|v| v as i64)
        .collect()
}

fn is_nodata(v: f64, nodata: &[i64]) -> bool {
    let v_int = v as i64;
    nodata.iter().any(|nd| *nd == v_int)
}

fn add_offset(offset: u8, code: u8) -> u8 {
    let sum = offset as u16 + code as u16;
    if sum > 255 {
        255
    } else {
        sum as u8
    }
}

fn classify_breaks(v: f64, breaks: &[f64], nodata: &[i64], offset: u8) -> u8 {
    if is_nodata(v, nodata) {
        return offset;
    }
    let mut idx = 0usize;
    for (i, brk) in breaks.iter().enumerate() {
        idx = i;
        if v <= *brk {
            break;
        }
    }
    add_offset(offset, idx as u8)
}

fn classify_ct(v: f64, value_map: &HashMap<i64, u8>, nodata: &[i64], offset: u8) -> u8 {
    if is_nodata(v, nodata) {
        return offset;
    }
    let v_int = v as i64;
    match value_map.get(&v_int) {
        Some(code) => add_offset(offset, *code),
        None => 255,
    }
}

enum ClassifierMode {
    Breaks(Vec<f64>),
    Ct(HashMap<i64, u8>),
}

fn classify_band(
    band: &gdal::raster::RasterBand<'_>,
    classifier: &ClassifierMode,
    nodata: &[i64],
    offset: u8,
    transpose: bool,
) -> Result<Vec<u8>, GdalError> {
    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }

    let mut out = vec![0u8; width * height];
    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(block_y, height - y);
        let buffer =
            band.read_as::<f64>((0, y as isize), (width, y_size), (width, y_size), None)?;

        for row in 0..y_size {
            let row_offset = row * width;
            let out_row = y + row;
            for col in 0..width {
                let v = buffer.data[row_offset + col];
                let class_val = match classifier {
                    ClassifierMode::Breaks(breaks) => classify_breaks(v, breaks, nodata, offset),
                    ClassifierMode::Ct(value_map) => classify_ct(v, value_map, nodata, offset),
                };
                let idx = if transpose {
                    col * height + out_row
                } else {
                    out_row * width + col
                };
                out[idx] = class_val;
            }
        }

        y += y_size;
    }

    Ok(out)
}

#[pyfunction]
#[pyo3(signature = (path, *, color_map_path=None))]
fn summarize_sbs_raster(path: &str, color_map_path: Option<String>) -> PyResult<PyObject> {
    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Python::with_gil(|py| {
            let dict = PyDict::new_bound(py);
            dict.set_item("srs_valid", false)?;
            dict.set_item("class_count", 0)?;
            dict.set_item("unique_classes", PyList::new_bound(py, Vec::<i32>::new()))?;
            dict.set_item("class_counts", PyList::new_bound(py, Vec::<i32>::new()))?;
            dict.set_item("has_non_integer", false)?;
            dict.set_item("has_color_table", false)?;
            dict.set_item(
                "color_table_severities",
                PyList::new_bound(py, Vec::<String>::new()),
            )?;
            dict.set_item("color_table_valid", false)?;
            dict.set_item("sanity_status", 1)?;
            dict.set_item("sanity_message", "File does not exist")?;
            dict.set_item("size_bytes", 0)?;
            Ok(dict.into_py(py))
        });
    }

    let size_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let srs_valid = validate_srs(&dataset);
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let band_type = band.band_type();
    let band_is_float = band_is_float(band_type);

    let (counts, unique, has_non_integer) = match band_type {
        GdalDataType::UInt8 => {
            let (counts, unique) = scan_band_u8(&band)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
            (counts, unique, false)
        }
        GdalDataType::UInt16 => {
            let (counts, unique) = scan_band_u16(&band)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
            (counts, unique, false)
        }
        GdalDataType::Int16 => {
            let (counts, unique) = scan_band_i16(&band)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
            (counts, unique, false)
        }
        _ => scan_band(&band, band_is_float)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?,
    };

    let class_count = counts.len();

    let color_map = load_color_map(color_map_path.as_deref())?;
    let (has_color_table, color_table_severities, color_table_valid) =
        summarize_color_table_internal(&band, &color_map);

    let (sanity_status, sanity_message) = if !srs_valid {
        (
            1,
            "Map contains an invalid projection. Try reprojecting to UTM.",
        )
    } else if class_count > 256 {
        (1, "Map has more than 256 classes")
    } else if has_non_integer {
        (1, "Map has non-integer classes")
    } else if has_color_table {
        if color_table_valid {
            (0, "Map has valid color table")
        } else {
            (1, "Map has no valid color table")
        }
    } else {
        (0, "Map has valid classes")
    };

    Python::with_gil(|py| {
        let dict = PyDict::new_bound(py);

        let unique_classes: Vec<PyObject> = if class_count <= 512 {
            let mut values: Vec<NormalizedValue> = unique.into_iter().collect();
            values.sort_by(|a, b| {
                normalized_value_as_f64(a)
                    .partial_cmp(&normalized_value_as_f64(b))
                    .unwrap_or(Ordering::Equal)
            });
            values
                .iter()
                .map(|v| normalized_value_to_py(py, v))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let class_counts: Vec<PyObject> = if class_count <= 512 {
            let mut entries: Vec<(RawValue, CountInfo)> = counts.into_iter().collect();
            entries.sort_by(|a, b| {
                b.1.count
                    .cmp(&a.1.count)
                    .then(a.1.first_seen.cmp(&b.1.first_seen))
            });
            entries
                .iter()
                .map(|(value, info)| {
                    let tuple = PyTuple::new_bound(
                        py,
                        &[raw_value_to_py(py, value), info.count.into_py(py)],
                    );
                    tuple.into_py(py)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        dict.set_item("srs_valid", srs_valid)?;
        dict.set_item("class_count", class_count)?;
        dict.set_item("unique_classes", PyList::new_bound(py, unique_classes))?;
        dict.set_item("class_counts", PyList::new_bound(py, class_counts))?;
        dict.set_item("has_non_integer", has_non_integer)?;
        dict.set_item("has_color_table", has_color_table)?;
        dict.set_item(
            "color_table_severities",
            PyList::new_bound(py, color_table_severities),
        )?;
        dict.set_item("color_table_valid", color_table_valid)?;
        dict.set_item("sanity_status", sanity_status)?;
        dict.set_item("sanity_message", sanity_message)?;
        dict.set_item("size_bytes", size_bytes)?;
        Ok(dict.into_py(py))
    })
}

#[pyfunction]
#[pyo3(signature = (path, *, color_map_path=None))]
fn read_color_table(
    py: Python<'_>,
    path: &str,
    color_map_path: Option<String>,
) -> PyResult<PyObject> {
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let color_map = load_color_map(color_map_path.as_deref())?;

    let dict = PyDict::new_bound(py);
    if let Some((class_index_map, color_lookup, severities)) =
        build_color_table_maps(&band, &color_map)
    {
        let class_dict = PyDict::new_bound(py);
        for sev in ALLOWED_SEVERITIES.iter() {
            let values = class_index_map.get(*sev).cloned().unwrap_or_default();
            class_dict.set_item(*sev, PyList::new_bound(py, values))?;
        }

        let color_dict = PyDict::new_bound(py);
        for (rgb, severity) in color_lookup {
            let key = PyTuple::new_bound(
                py,
                &[rgb.0.into_py(py), rgb.1.into_py(py), rgb.2.into_py(py)],
            );
            let value: PyObject = match severity {
                Some(sev) => sev.into_py(py),
                None => py.None(),
            };
            color_dict.set_item(key, value)?;
        }

        dict.set_item("has_color_table", true)?;
        dict.set_item("class_index_map", class_dict)?;
        dict.set_item("color_map", color_dict)?;
        dict.set_item("color_table_severities", PyList::new_bound(py, severities))?;
    } else {
        dict.set_item("has_color_table", false)?;
        dict.set_item("class_index_map", py.None())?;
        dict.set_item("color_map", py.None())?;
        dict.set_item(
            "color_table_severities",
            PyList::new_bound(py, Vec::<String>::new()),
        )?;
    }

    Ok(dict.into_py(py))
}

#[pyfunction]
fn unique_values(path: &str) -> PyResult<Vec<PyObject>> {
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let (width, height) = band.size();
    let (_, mut block_y) = band.block_size();
    if block_y == 0 {
        block_y = 1;
    }

    let mut unique: HashSet<NormalizedValue> = HashSet::new();

    let mut y = 0usize;
    while y < height {
        let y_size = std::cmp::min(block_y, height - y);
        let buffer = band
            .read_as::<f64>((0, y as isize), (width, y_size), (width, y_size), None)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

        for value in buffer.data.iter() {
            unique.insert(normalized_value_from_f64(*value));
        }

        y += y_size;
    }

    Python::with_gil(|py| {
        let mut values: Vec<NormalizedValue> = unique.into_iter().collect();
        values.sort_by(|a, b| {
            normalized_value_as_f64(a)
                .partial_cmp(&normalized_value_as_f64(b))
                .unwrap_or(Ordering::Equal)
        });
        Ok(values
            .iter()
            .map(|v| normalized_value_to_py(py, v))
            .collect::<Vec<_>>())
    })
}

#[pyfunction]
#[pyo3(signature = (path, *, color_map_path=None))]
fn summarize_color_table(
    py: Python<'_>,
    path: &str,
    color_map_path: Option<String>,
) -> PyResult<PyObject> {
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let color_map = load_color_map(color_map_path.as_deref())?;

    let dict = PyDict::new_bound(py);
    if let Some((class_index_map, _color_lookup, severities)) =
        build_color_table_maps(&band, &color_map)
    {
        let mut severity_counts: HashMap<String, usize> = HashMap::new();
        for sev in ALLOWED_SEVERITIES.iter() {
            let count = class_index_map
                .get(*sev)
                .map(|values| values.len())
                .unwrap_or(0);
            severity_counts.insert((*sev).to_string(), count);
        }

        let counts_dict = PyDict::new_bound(py);
        for (sev, count) in severity_counts {
            counts_dict.set_item(sev, count)?;
        }

        let class_dict = PyDict::new_bound(py);
        for sev in ALLOWED_SEVERITIES.iter() {
            let values = class_index_map.get(*sev).cloned().unwrap_or_default();
            class_dict.set_item(*sev, PyList::new_bound(py, values))?;
        }

        let valid = severities
            .iter()
            .any(|s| matches!(s.as_str(), "low" | "mod" | "high"));

        dict.set_item("has_color_table", true)?;
        dict.set_item("color_table_severities", PyList::new_bound(py, severities))?;
        dict.set_item("color_table_valid", valid)?;
        dict.set_item("severity_counts", counts_dict)?;
        dict.set_item("class_index_map", class_dict)?;
    } else {
        dict.set_item("has_color_table", false)?;
        dict.set_item(
            "color_table_severities",
            PyList::new_bound(py, Vec::<String>::new()),
        )?;
        dict.set_item("color_table_valid", false)?;
        dict.set_item("severity_counts", PyDict::new_bound(py))?;
        dict.set_item("class_index_map", py.None())?;
    }

    Ok(dict.into_py(py))
}

#[pyfunction]
#[pyo3(signature = (path, *, breaks=None, ct=None, nodata=None, offset=0, color_map_path=None))]
fn reclassify_sbs_raster(
    py: Python<'_>,
    path: &str,
    breaks: Option<Vec<f64>>,
    ct: Option<&Bound<'_, PyDict>>,
    nodata: Option<Vec<f64>>,
    offset: i32,
    color_map_path: Option<String>,
) -> PyResult<Py<PyArray2<u8>>> {
    let offset = normalize_offset(offset)?;
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let classifier = if let Some(breaks) = breaks {
        if breaks.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "breaks must not be empty",
            ));
        }
        ClassifierMode::Breaks(breaks)
    } else if let Some(ct_dict) = ct {
        let ct_map = parse_ct_dict(ct_dict)?;
        ClassifierMode::Ct(build_value_map(&ct_map))
    } else {
        let color_map = load_color_map(color_map_path.as_deref())?;
        let ct_map = build_ct_from_band(&band, &color_map).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "ct and breaks are both None, and the raster has no valid color table",
            )
        })?;
        ClassifierMode::Ct(build_value_map(&ct_map))
    };

    let nodata_vals = nodata_to_i64(nodata);
    let out = classify_band(&band, &classifier, &nodata_vals, offset, true)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let (width, height) = band.size();
    let array = unsafe { PyArray2::<u8>::new_bound(py, [width, height], false) };
    unsafe {
        array
            .as_slice_mut()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?
            .copy_from_slice(&out);
    }

    Ok(array.unbind())
}

#[pyfunction]
#[pyo3(signature = (path, dst_path, *, breaks=None, ct=None, nodata=None, color_map_path=None))]
fn export_sbs_4class(
    path: &str,
    dst_path: &str,
    breaks: Option<Vec<f64>>,
    ct: Option<&Bound<'_, PyDict>>,
    nodata: Option<Vec<f64>>,
    color_map_path: Option<String>,
) -> PyResult<()> {
    let dataset = Dataset::open(path)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let band = dataset
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let classifier = if let Some(breaks) = breaks {
        if breaks.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "breaks must not be empty",
            ));
        }
        ClassifierMode::Breaks(breaks)
    } else if let Some(ct_dict) = ct {
        let ct_map = parse_ct_dict(ct_dict)?;
        ClassifierMode::Ct(build_value_map(&ct_map))
    } else {
        let color_map = load_color_map(color_map_path.as_deref())?;
        let ct_map = build_ct_from_band(&band, &color_map).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(
                "ct and breaks are both None, and the raster has no valid color table",
            )
        })?;
        ClassifierMode::Ct(build_value_map(&ct_map))
    };

    let nodata_vals = nodata_to_i64(nodata);
    let out = classify_band(&band, &classifier, &nodata_vals, 0, false)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let (width, height) = band.size();
    let driver = gdal::DriverManager::get_driver_by_name("GTiff")
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let options = [
        RasterCreationOption {
            key: "COMPRESS",
            value: "LZW",
        },
        RasterCreationOption {
            key: "PHOTOMETRIC",
            value: "PALETTE",
        },
    ];
    let mut dst = driver
        .create_with_band_type_with_options::<u8, _>(
            dst_path,
            width as isize,
            height as isize,
            1,
            &options,
        )
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    if let Ok(transform) = dataset.geo_transform() {
        dst.set_geo_transform(&transform)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    }
    let projection = dataset.projection();
    if !projection.trim().is_empty() {
        dst.set_projection(&projection)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    }

    let mut out_band = dst
        .rasterband(1)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let mut color_table = ColorTable::new(PaletteInterpretation::Rgba);
    color_table.set_color_entry(0, &ColorEntry::rgba(0, 100, 0, 255));
    color_table.set_color_entry(1, &ColorEntry::rgba(127, 255, 212, 255));
    color_table.set_color_entry(2, &ColorEntry::rgba(255, 255, 0, 255));
    color_table.set_color_entry(3, &ColorEntry::rgba(255, 0, 0, 255));
    color_table.set_color_entry(255, &ColorEntry::rgba(255, 255, 255, 0));

    out_band.set_color_table(&color_table);
    out_band
        .set_no_data_value(Some(255.0))
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    let buffer = Buffer::new((width, height), out);
    out_band
        .write((0, 0), (width, height), &buffer)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

    Ok(())
}

#[pymodule]
fn sbs_map_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(summarize_sbs_raster, m)?)?;
    m.add_function(wrap_pyfunction!(read_color_table, m)?)?;
    m.add_function(wrap_pyfunction!(unique_values, m)?)?;
    m.add_function(wrap_pyfunction!(summarize_color_table, m)?)?;
    m.add_function(wrap_pyfunction!(reclassify_sbs_raster, m)?)?;
    m.add_function(wrap_pyfunction!(export_sbs_4class, m)?)?;
    Ok(())
}

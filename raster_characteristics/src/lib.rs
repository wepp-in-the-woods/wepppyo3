#![allow(clippy::legacy_numeric_constants)]
#![allow(clippy::unwrap_or_default)]
#![allow(clippy::useless_conversion)]

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::collections::{BTreeMap, HashMap, HashSet};

use raster::raster::Raster;

fn read_i32_raster(path: &str) -> PyResult<Raster<i32>> {
    Raster::<i32>::read(path)
        .map_err(|err| PyIOError::new_err(format!("Failed to read raster '{}': {}", path, err)))
}

fn validate_equal_shape(
    lhs_name: &str,
    lhs: &Raster<i32>,
    rhs_name: &str,
    rhs: &Raster<i32>,
) -> PyResult<()> {
    if lhs.width != rhs.width || lhs.height != rhs.height {
        return Err(PyValueError::new_err(format!(
            "Raster shape mismatch: {} is {}x{} but {} is {}x{}",
            lhs_name, lhs.width, lhs.height, rhs_name, rhs.width, rhs.height
        )));
    }

    if lhs.data.len() != rhs.data.len() {
        return Err(PyValueError::new_err(format!(
            "Raster data length mismatch: {} has {} cells but {} has {} cells",
            lhs_name,
            lhs.data.len(),
            rhs_name,
            rhs.data.len()
        )));
    }

    Ok(())
}

fn count_intersecting_pairs(
    key_data: &[i32],
    key2_data: &[i32],
    ignore_channels: bool,
    ignore_keys: &HashSet<i32>,
    ignore_keys2: &HashSet<i32>,
) -> BTreeMap<i32, BTreeMap<i32, usize>> {
    let mut pair_counts: BTreeMap<i32, BTreeMap<i32, usize>> = BTreeMap::new();

    for (key, key2) in key_data.iter().zip(key2_data.iter()) {
        if ignore_channels && key % 10 == 4 {
            continue;
        }

        if ignore_keys.contains(key) || ignore_keys2.contains(key2) {
            continue;
        }

        *pair_counts
            .entry(*key)
            .or_default()
            .entry(*key2)
            .or_insert(0) += 1;
    }

    pair_counts
}

#[pyfunction]
fn count_intersecting_raster_key_pairs(
    key_fn: &str,
    key2_fn: &str,
    ignore_channels: bool,
    mut ignore_keys: HashSet<i32>,
    mut ignore_keys2: HashSet<i32>,
) -> PyResult<BTreeMap<String, BTreeMap<String, usize>>> {
    let key_map = read_i32_raster(key_fn)?;
    let key2_map = read_i32_raster(key2_fn)?;
    validate_equal_shape("key_fn", &key_map, "key2_fn", &key2_map)?;

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    if let Some(no_data_value) = key2_map.no_data {
        ignore_keys2.insert(no_data_value);
    }

    ignore_keys.insert(i32::MIN);
    ignore_keys.insert(i32::MAX);
    ignore_keys2.insert(i32::MIN);
    ignore_keys2.insert(i32::MAX);

    let pair_counts = count_intersecting_pairs(
        &key_map.data,
        &key2_map.data,
        ignore_channels,
        &ignore_keys,
        &ignore_keys2,
    );

    let mut result: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (key, sub_map) in pair_counts {
        let mut sub_result: BTreeMap<String, usize> = BTreeMap::new();
        for (key2, count) in sub_map {
            sub_result.insert(key2.to_string(), count);
        }
        result.insert(key.to_string(), sub_result);
    }

    Ok(result)
}

/// Identify the mode (most common) value of each key in a raster dataset.
///
/// Given the file paths to two raster datasets, `key_fn` and `parameter_fn`, this function
/// iterates through each corresponding pair of data points. It keeps count of the occurrence
/// of each unique value (`val`) per unique key (`key`) encountered, ignoring specified keys
/// and/or the designated "no data" value. The mode value is then determined for each key
/// based on these counts. If a key has no valid parameter values, the mode
/// of the full raster (after ignores) is used as a fallback.
///
/// # Arguments
///
/// * `key_fn: &str` - The file path to the raster data to be used as keys.
/// * `parameter_fn: &str` - The file path to the raster data to determine the mode value for each key.
/// * `ignore_channels: bool` - If `true`, keys that end in 4.
/// * `mut ignore_keys: HashSet<i32>` - A set of keys to be ignored during processing. If a "no data"
///    value is defined in `key_map`, it is automatically added to this set.
///
/// # Returns
///
/// `PyResult<HashMap<String, i32>>` - A HashMap where each key represents a unique key from
/// `key_map` and the associated value is the mode (most frequently occurring) value for that key
/// from `parameter_map`.
///
/// # Errors
///
/// Returns `Err` if there is a failure reading the raster data from the provided file paths.
/// Note: The current implementation uses `unwrap()` which may cause panics on errors
/// (to be improved for production use).
///
/// # Example
///
/// ```
/// let key_fn = "path/to/key_map.tif";
/// let parameter_fn = "path/to/parameter_map.tif";
/// let ignore_channels = false;
/// let mut ignore_keys = HashSet::new();
/// ignore_keys.insert(-9999);
///
/// let result = identify_mode_single_raster_key(key_fn, parameter_fn, ignore_channels, ignore_keys);
/// ```
///
/// # Note
///
/// Ensure that the raster datasets provided via `key_fn` and `parameter_fn` are of
/// identical dimensions, as the function does not perform dimensionality checks.
///
/// # Panics
///
/// The function may panic if it is unable to read the raster data from the provided paths.
#[pyfunction]
fn identify_mode_single_raster_key(
    key_fn: &str,
    parameter_fn: &str,
    ignore_channels: bool,
    mut ignore_keys: HashSet<i32>,
    band_indx: isize,
) -> PyResult<HashMap<String, i32>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let parameter_map: Raster<i32> = Raster::<i32>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    ignore_keys.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    let mut count_d: HashMap<i32, HashMap<i32, usize>> = HashMap::new();
    let mut global_val_counts: HashMap<i32, usize> = HashMap::new();
    let mut all_keys: HashSet<i32> = HashSet::new();

    for (key, val) in key_map.data.iter().zip(parameter_map.data.iter()) {
        if ignore_channels && key % 10 == 4 {
            continue;
        }

        if ignore_keys.contains(key) {
            continue;
        }

        all_keys.insert(*key);

        if let Some(no_data_value) = parameter_map.no_data {
            if no_data_value == *val {
                continue;
            }
        }

        *count_d
            .entry(*key)
            .or_insert_with(HashMap::new)
            .entry(*val)
            .or_insert(0) += 1;
        *global_val_counts.entry(*val).or_insert(0) += 1;
    }

    let global_mode = global_val_counts
        .iter()
        .max_by(|(val_a, count_a), (val_b, count_b)| count_a.cmp(count_b).then(val_a.cmp(val_b)))
        .map(|(&val, _)| val);
    let fallback_val = global_mode.or(parameter_map.no_data);

    let mut result: HashMap<String, i32> = HashMap::new();
    for key in all_keys {
        if let Some(sub_map) = count_d.get(&key) {
            if let Some((&val, &_count)) =
                sub_map.iter().max_by(|(val_a, count_a), (val_b, count_b)| {
                    count_a
                        .cmp(count_b)
                        .then(
                            global_val_counts
                                .get(val_a)
                                .copied()
                                .unwrap_or(0)
                                .cmp(&global_val_counts.get(val_b).copied().unwrap_or(0)),
                        )
                        .then(val_a.cmp(val_b)) // stable tie-breaker on value
                })
            {
                result.insert(key.to_string(), val);
                continue;
            }
        }

        if let Some(fallback_val) = fallback_val {
            result.insert(key.to_string(), fallback_val);
        }
    }

    Ok(result)
}

/// Identify the mode (most common) parameter values across intersecting raster key datasets.
///
/// This function analyzes three raster datasets: two providing keys (`key_fn` and `key2_fn`) and
/// one providing parameter values (`parameter_fn`). For each intersecting key pair (from `key_fn`
/// and `key2_fn`), it determines the mode (most common) value from `parameter_fn`, excluding specified
/// keys and/or designated "no data" values. The resulting mode values are returned in a nested
/// HashMap where each entry associates a key from `key_fn` with a HashMap. This inner HashMap, in turn,
/// associates keys from `key2_fn` with their respective mode values. If a key
/// pair has no valid parameter values, the mode of the full raster (after
/// ignores) is used as a fallback.
///
/// # Arguments
///
/// * `key_fn: &str` - File path to the first raster dataset providing key values.
/// * `key2_fn: &str` - File path to the second raster dataset providing key values.
/// * `parameter_fn: &str` - File path to the raster data providing parameter values to calculate the mode for each key pair.
/// * `ignore_channels: bool` - If `true`, keys that are multiples of 10 are ignored during processing.
/// * `mut ignore_keys: HashSet<i32>` - A set of key values to ignore during processing. If a "no data" value is defined in the key raster datasets, it should be added to this set.
/// * `mut ignore_keys2: HashSet<i32>` - A set of key values to ignore during processing. If a "no data" value is defined in the key2 raster datasets, it should be added to this set.
///
/// # Returns
///
/// `PyResult<HashMap<String, HashMap<String, i32>>>` - A nested HashMap where each entry associates a key from `key_fn`
/// with another HashMap. This inner HashMap associates keys from `key2_fn` with the mode parameter value for that key pair.
///
/// # Errors
///
/// Returns `Err` if there is a failure reading the raster data from the provided file paths.
/// Note: In the current implementation using `unwrap()`, the function may panic on errors
/// (improvement recommended for production use).
///
/// # Example
///
/// ```
/// let key_fn = "path/to/key_map.tif";
/// let key2_fn = "path/to/key2_map.tif";
/// let parameter_fn = "path/to/parameter_map.tif";
/// let ignore_channels = false;
/// let mut ignore_keys = HashSet::new();
/// ignore_keys.insert(-9999);
///
/// let result = identify_mode_intersecting_raster_keys(key_fn, key2_fn, parameter_fn, ignore_channels, ignore_keys);
/// ```
///
/// # Note
///
/// Ensure that the raster datasets provided via `key_fn`, `key2_fn`, and `parameter_fn` are of
/// identical dimensions as the function does not perform dimensionality checks.
///
/// # Panics
///
/// The function may panic if it is unable to read the raster data from the provided paths.
#[pyfunction]
fn identify_mode_intersecting_raster_keys(
    key_fn: &str,
    key2_fn: &str,
    parameter_fn: &str,
    ignore_channels: bool,
    mut ignore_keys: HashSet<i32>,
    mut ignore_keys2: HashSet<i32>,
    band_indx: isize,
) -> PyResult<HashMap<String, HashMap<String, i32>>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let key2_map: Raster<i32> = Raster::<i32>::read(key2_fn).unwrap();
    let parameter_map: Raster<i32> = Raster::<i32>::read_band(parameter_fn, band_indx).unwrap();

    // Handle no_data values for key_map and key2_map
    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    ignore_keys.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    if let Some(no_data_value) = key2_map.no_data {
        ignore_keys2.insert(no_data_value);
    }

    ignore_keys2.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys2.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    // Nested HashMap to store count information: key -> key2 -> parameter_value -> count
    let mut count_d: HashMap<i32, HashMap<i32, HashMap<i32, usize>>> = HashMap::new();
    let mut global_val_counts: HashMap<i32, usize> = HashMap::new();
    let mut key2s_by_key: HashMap<i32, HashSet<i32>> = HashMap::new();

    // Iterate through corresponding entries in the three rasters
    for ((key, key2), val) in key_map
        .data
        .iter()
        .zip(key2_map.data.iter())
        .zip(parameter_map.data.iter())
    {
        if ignore_channels && key % 10 == 4 {
            continue;
        }

        if ignore_keys.contains(key) || ignore_keys2.contains(key2) {
            continue;
        }

        key2s_by_key
            .entry(*key)
            .or_insert_with(HashSet::new)
            .insert(*key2);

        if let Some(no_data_value) = parameter_map.no_data {
            if no_data_value == *val {
                continue;
            }
        }

        // Increment the count for the current key, key2, and parameter value
        *count_d
            .entry(*key)
            .or_insert_with(HashMap::new)
            .entry(*key2)
            .or_insert_with(HashMap::new)
            .entry(*val)
            .or_insert(0) += 1;
        *global_val_counts.entry(*val).or_insert(0) += 1;
    }

    let global_mode = global_val_counts
        .iter()
        .max_by(|(val_a, count_a), (val_b, count_b)| count_a.cmp(count_b).then(val_a.cmp(val_b)))
        .map(|(&val, _)| val);
    let fallback_val = global_mode.or(parameter_map.no_data);

    // Determine the mode value for each key, key2 pair
    let mut result: HashMap<String, HashMap<String, i32>> = HashMap::new();
    for (key, key2_set) in key2s_by_key {
        let mut key2_mode_map: HashMap<String, i32> = HashMap::new();
        for key2 in key2_set {
            if let Some(val_count_map) = count_d.get(&key).and_then(|m| m.get(&key2)) {
                if let Some((&val, &_count)) =
                    val_count_map
                        .iter()
                        .max_by(|(val_a, count_a), (val_b, count_b)| {
                            count_a
                                .cmp(count_b)
                                .then(
                                    global_val_counts
                                        .get(val_a)
                                        .copied()
                                        .unwrap_or(0)
                                        .cmp(&global_val_counts.get(val_b).copied().unwrap_or(0)),
                                )
                                .then(val_a.cmp(val_b))
                        })
                {
                    key2_mode_map.insert(key2.to_string(), val);
                    continue;
                }
            }

            if let Some(fallback_val) = fallback_val {
                key2_mode_map.insert(key2.to_string(), fallback_val);
            }
        }
        result.insert(key.to_string(), key2_mode_map);
    }

    Ok(result)
}

/// Identify the median value of each key in a raster dataset.
///
/// Given the file paths to two raster datasets, `key_fn` and `parameter_fn`, this function
/// iterates through each corresponding pair of data points. It keeps count of the occurrence
/// of each unique value (`val`) per unique key (`key`) encountered, ignoring specified keys
/// and/or the designated "no data" value. The median value is then determined for each key
/// based on these counts.
///
/// # Arguments
///
/// * `key_fn: &str` - The file path to the raster data to be used as keys.
/// * `parameter_fn: &str` - The file path to the raster data to determine the mode value for each key.
/// * `ignore_channels: bool` - If `true`, keys that end in 4.
/// * `mut ignore_keys: HashSet<i32>` - A set of keys to be ignored during processing. If a "no data"
///    value is defined in `key_map`, it is automatically added to this set.
///
/// # Returns
///
/// `PyResult<HashMap<String, f64>>` - A HashMap where each key represents a unique key from
/// `key_map` and the associated value is the mode (most frequently occurring) value for that key
/// from `parameter_map`.
///
/// # Errors
///
/// Returns `Err` if there is a failure reading the raster data from the provided file paths.
/// Note: The current implementation uses `unwrap()` which may cause panics on errors
/// (to be improved for production use).
///
/// # Example
///
/// ```
/// let key_fn = "path/to/key_map.tif";
/// let parameter_fn = "path/to/parameter_map.tif";
/// let ignore_channels = false;
/// let mut ignore_keys = HashSet::new();
/// ignore_keys.insert(-9999);
///
/// let result = identify_median_single_raster_key(key_fn, parameter_fn, ignore_channels, ignore_keys);
/// ```
///
/// # Note
///
/// Ensure that the raster datasets provided via `key_fn` and `parameter_fn` are of
/// identical dimensions, as the function does not perform dimensionality checks.
///
/// # Panics
///
/// The function may panic if it is unable to read the raster data from the provided paths.
#[pyfunction]
fn identify_median_single_raster_key(
    key_fn: &str,
    parameter_fn: &str,
    ignore_channels: bool,
    mut ignore_keys: HashSet<i32>,
    band_indx: isize,
) -> PyResult<HashMap<String, f64>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let parameter_map: Raster<f64> = Raster::<f64>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    ignore_keys.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    let mut values_d: HashMap<i32, Vec<f64>> = HashMap::new();

    for (key, &val) in key_map.data.iter().zip(parameter_map.data.iter()) {
        if ignore_channels && key % 10 == 4 {
            continue;
        }

        if let Some(no_data_value) = parameter_map.no_data {
            if (no_data_value - val).abs() < std::f64::EPSILON {
                continue;
            }
        }

        if ignore_keys.contains(key) {
            continue;
        }

        values_d.entry(*key).or_insert_with(Vec::new).push(val);
    }

    let mut result: HashMap<String, f64> = HashMap::new();
    for (key, values) in values_d {
        let median = calculate_median(values);
        result.insert(key.to_string(), median);
    }

    Ok(result)
}

/// Identify the median  parameter values across intersecting raster key datasets.
///
/// This function analyzes three raster datasets: two providing keys (`key_fn` and `key2_fn`) and
/// one providing parameter values (`parameter_fn`). For each intersecting key pair (from `key_fn`
/// and `key2_fn`), it determines the median value from `parameter_fn`, excluding specified
/// keys and/or designated "no data" values. The resulting mode values are returned in a nested
/// HashMap where each entry associates a key from `key_fn` with a HashMap. This inner HashMap, in turn,
/// associates keys from `key2_fn` with their respective mode values.
///
/// # Arguments
///
/// * `key_fn: &str` - File path to the first raster dataset providing key values.
/// * `key2_fn: &str` - File path to the second raster dataset providing key values.
/// * `parameter_fn: &str` - File path to the raster data providing parameter values to calculate the mode for each key pair.
/// * `ignore_channels: bool` - If `true`, keys that are multiples of 10 are ignored during processing.
/// * `mut ignore_keys: HashSet<i32>` - A set of key values to ignore during processing. If a "no data" value is defined in the key raster datasets, it should be added to this set.
/// * `mut ignore_keys2: HashSet<i32>` - A set of key values to ignore during processing. If a "no data" value is defined in the key2 raster datasets, it should be added to this set.
///
/// # Returns
///
/// `PyResult<HashMap<String, HashMap<String, f64>>>` - A nested HashMap where each entry associates a key from `key_fn`
/// with another HashMap. This inner HashMap associates keys from `key2_fn` with the mode parameter value for that key pair.
///
/// # Errors
///
/// Returns `Err` if there is a failure reading the raster data from the provided file paths.
/// Note: In the current implementation using `unwrap()`, the function may panic on errors
/// (improvement recommended for production use).
///
/// # Example
///
/// ```
/// let key_fn = "path/to/key_map.tif";
/// let key2_fn = "path/to/key2_map.tif";
/// let parameter_fn = "path/to/parameter_map.tif";
/// let ignore_channels = false;
/// let mut ignore_keys = HashSet::new();
/// ignore_keys.insert(-9999);
///
/// let result = identify_mode_intersecting_raster_keys(key_fn, key2_fn, parameter_fn, ignore_channels, ignore_keys);
/// ```
///
/// # Note
///
/// Ensure that the raster datasets provided via `key_fn`, `key2_fn`, and `parameter_fn` are of
/// identical dimensions as the function does not perform dimensionality checks.
///
/// # Panics
///
/// The function may panic if it is unable to read the raster data from the provided paths.
#[pyfunction]
fn identify_median_intersecting_raster_keys(
    key_fn: &str,
    key2_fn: &str,
    parameter_fn: &str,
    ignore_channels: bool,
    mut ignore_keys: HashSet<i32>,
    mut ignore_keys2: HashSet<i32>,
    band_indx: isize,
) -> PyResult<HashMap<String, HashMap<String, f64>>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let key2_map: Raster<i32> = Raster::<i32>::read(key2_fn).unwrap();
    let parameter_map: Raster<f64> = Raster::<f64>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    ignore_keys.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    if let Some(no_data_value) = key2_map.no_data {
        ignore_keys2.insert(no_data_value);
    }

    ignore_keys2.insert(i32::MIN); // Ensure we ignore the minimum i32 value
    ignore_keys2.insert(i32::MAX); // Ensure we ignore the maximum i32 value

    // Nested HashMap to store value information: key -> key2 -> parameter_values
    let mut values_d: HashMap<i32, HashMap<i32, Vec<f64>>> = HashMap::new();

    for ((key, key2), &val) in key_map
        .data
        .iter()
        .zip(key2_map.data.iter())
        .zip(parameter_map.data.iter())
    {
        if ignore_channels && key % 10 == 4 {
            continue;
        }

        if let Some(no_data_value) = parameter_map.no_data {
            if (no_data_value - val).abs() < std::f64::EPSILON {
                continue;
            }
        }

        if ignore_keys.contains(key) || ignore_keys2.contains(key2) {
            continue;
        }

        values_d
            .entry(*key)
            .or_insert_with(HashMap::new)
            .entry(*key2)
            .or_insert_with(Vec::new)
            .push(val);
    }

    // Compute the median value for each key, key2 pair
    let mut result: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (key, sub_map) in values_d {
        let mut key2_median_map: HashMap<String, f64> = HashMap::new();
        for (key2, values) in sub_map {
            let median = calculate_median(values);
            key2_median_map.insert(key2.to_string(), median);
        }
        result.insert(key.to_string(), key2_median_map);
    }

    Ok(result)
}

fn calculate_median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = values.len();
    if len % 2 == 1 {
        values[len / 2]
    } else {
        (values[(len - 1) / 2] + values[len / 2]) / 2.0
    }
}

/// A PyO3 module
/// This module is a container for the Python-callable functions we define
#[pymodule]
fn raster_characteristics_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(count_intersecting_raster_key_pairs, m)?)?;
    m.add_function(wrap_pyfunction!(identify_mode_single_raster_key, m)?)?;
    m.add_function(wrap_pyfunction!(identify_mode_intersecting_raster_keys, m)?)?;
    m.add_function(wrap_pyfunction!(identify_median_single_raster_key, m)?)?;
    m.add_function(wrap_pyfunction!(
        identify_median_intersecting_raster_keys,
        m
    )?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_intersecting_pairs_counts_expected_pairs() {
        let key_data = vec![11, 11, 12, 12, 12, 14, -9999];
        let key2_data = vec![1, 2, 1, 1, 2, 1, 1];
        let ignore_keys = HashSet::from([-9999]);
        let ignore_keys2 = HashSet::new();

        let counts =
            count_intersecting_pairs(&key_data, &key2_data, false, &ignore_keys, &ignore_keys2);

        let expected = BTreeMap::from([
            (11, BTreeMap::from([(1, 1), (2, 1)])),
            (12, BTreeMap::from([(1, 2), (2, 1)])),
            (14, BTreeMap::from([(1, 1)])),
        ]);
        assert_eq!(counts, expected);
    }

    #[test]
    fn count_intersecting_pairs_respects_ignore_channels_and_key_filters() {
        let key_data = vec![11, 11, 12, 12, 14, 14];
        let key2_data = vec![1, 2, 1, 2, 1, 2];
        let ignore_keys = HashSet::from([12]);
        let ignore_keys2 = HashSet::from([2]);

        let counts =
            count_intersecting_pairs(&key_data, &key2_data, true, &ignore_keys, &ignore_keys2);

        let expected = BTreeMap::from([(11, BTreeMap::from([(1, 1)]))]);
        assert_eq!(counts, expected);
    }
}

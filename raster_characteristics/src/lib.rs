use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::collections::{HashSet, HashMap};

use raster::raster::Raster;

/// Identify the mode (most common) value of each key in a raster dataset.
///
/// Given the file paths to two raster datasets, `key_fn` and `parameter_fn`, this function 
/// iterates through each corresponding pair of data points. It keeps count of the occurrence 
/// of each unique value (`val`) per unique key (`key`) encountered, ignoring specified keys 
/// and/or the designated "no data" value. The mode value is then determined for each key 
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
    band_indx: isize
) -> PyResult<HashMap<String, i32>> {

    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let parameter_map: Raster<i32> = Raster::<i32>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

    let mut count_d: HashMap<i32, HashMap<i32, usize>> = HashMap::new();

    for (key, val) in key_map.data.iter().zip(parameter_map.data.iter()) {
        if ignore_channels && key % 10 == 4 {
            continue;
        }


        if let Some(no_data_value) = parameter_map.no_data {
            if no_data_value == *val {
                continue;
            }
        }

        if ignore_keys.contains(key) {
            continue;
        }

        *count_d.entry(*key).or_insert_with(HashMap::new).entry(*val).or_insert(0) += 1;
    }

    let mut result: HashMap<String, i32> = HashMap::new();
    for (key, sub_map) in &count_d {
        if let Some((&val, &_count)) = sub_map.iter().max_by_key(|&(_, count)| count) {
            result.insert(key.to_string(), val);
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
    band_indx: isize
) -> PyResult<HashMap<String, HashMap<String, i32>>> {

    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let key2_map: Raster<i32> = Raster::<i32>::read(key2_fn).unwrap();
    let parameter_map: Raster<i32> = Raster::<i32>::read_band(parameter_fn, band_indx).unwrap();
    
    // Handle no_data values for key_map and key2_map
    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }
    if let Some(no_data_value) = key2_map.no_data {
        ignore_keys2.insert(no_data_value);
    }
    
    // Nested HashMap to store count information: key -> key2 -> parameter_value -> count
    let mut count_d: HashMap<i32, HashMap<i32, HashMap<i32, usize>>> = HashMap::new();
    
    // Iterate through corresponding entries in the three rasters
    for ((key, key2), val) in key_map.data.iter().zip(key2_map.data.iter()).zip(parameter_map.data.iter()) {
        if ignore_channels && key % 10 == 4 {
            continue;
        }
        
        if let Some(no_data_value) = parameter_map.no_data {
            if no_data_value == *val {
                continue;
            }
        }
        
        if ignore_keys.contains(key) || ignore_keys2.contains(key2) {
            continue;
        }
        
        // Increment the count for the current key, key2, and parameter value
        *count_d.entry(*key).or_insert_with(HashMap::new)
            .entry(*key2).or_insert_with(HashMap::new)
            .entry(*val).or_insert(0) += 1;
    }
    
    // Determine the mode value for each key, key2 pair
    let mut result: HashMap<String, HashMap<String, i32>> = HashMap::new();
    for (key, sub_map) in &count_d {
        let mut key2_mode_map: HashMap<String, i32> = HashMap::new();
        for (key2, val_count_map) in sub_map {
            if let Some((&val, &_count)) = val_count_map.iter().max_by_key(|&(_, count)| count) {
                key2_mode_map.insert(key2.to_string(), val);
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
    band_indx: isize
) -> PyResult<HashMap<String, f64>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let parameter_map: Raster<f64> = Raster::<f64>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }

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
    band_indx: isize
) -> PyResult<HashMap<String, HashMap<String, f64>>> {
    let key_map: Raster<i32> = Raster::<i32>::read(key_fn).unwrap();
    let key2_map: Raster<i32> = Raster::<i32>::read(key2_fn).unwrap();
    let parameter_map: Raster<f64> = Raster::<f64>::read_band(parameter_fn, band_indx).unwrap();

    if let Some(no_data_value) = key_map.no_data {
        ignore_keys.insert(no_data_value);
    }
    if let Some(no_data_value) = key2_map.no_data {
        ignore_keys2.insert(no_data_value);
    }

    // Nested HashMap to store value information: key -> key2 -> parameter_values
    let mut values_d: HashMap<i32, HashMap<i32, Vec<f64>>> = HashMap::new();

    for ((key, key2), &val) in key_map.data.iter().zip(key2_map.data.iter()).zip(parameter_map.data.iter()) {
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

        values_d.entry(*key).or_insert_with(HashMap::new)
            .entry(*key2).or_insert_with(Vec::new).push(val);
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
fn raster_characteristics_rust(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(identify_mode_single_raster_key, m)?)?;
    m.add_function(wrap_pyfunction!(identify_mode_intersecting_raster_keys, m)?)?;
    m.add_function(wrap_pyfunction!(identify_median_single_raster_key, m)?)?;
    m.add_function(wrap_pyfunction!(identify_median_intersecting_raster_keys, m)?)?;
    Ok(())
}


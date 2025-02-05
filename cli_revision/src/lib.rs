
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, BufRead, Result};
use numpy::{PyReadonlyArray1, PyReadonlyArray3};
use numpy::ndarray::{Array, Array3, Axis};
use numpy::PyUntypedArrayMethods;
use numpy::PyArrayMethods;

// ------------------ HELPER FUNCTIONS (unchanged) ------------------ //
fn find_nearest_index(arr: &[f64], value: f64) -> usize {
    let mut nearest_idx = 0;
    let mut min_dist = f64::MAX;
    for (i, &v) in arr.iter().enumerate() {
        let dist = (v - value).abs();
        if dist < min_dist {
            min_dist = dist;
            nearest_idx = i;
        } else {
            // Because arr is sorted ascending, once distance starts
            // increasing we *could* break, but we won't for clarity.
        }
    }
    nearest_idx
}

fn find_linear_indices_and_t(arr: &[f64], value: f64) -> (usize, usize, f64) {
    let n = arr.len();
    if value < arr[0] || value > arr[n - 1] {
        panic!("Value outside array domain. No extrapolation allowed.");
    }
    let mut left = 0;
    let mut right = n - 1;
    while right - left > 1 {
        let mid = (left + right) / 2;
        if arr[mid] == value {
            return (mid, mid, 0.0);
        } else if arr[mid] < value {
            left = mid;
        } else {
            right = mid;
        }
    }
    let denom = arr[right] - arr[left];
    let t = if denom.abs() < 1e-12 {
        0.0
    } else {
        (value - arr[left]) / denom
    };
    (left, right, t)
}

fn catmull_rom_spline(f0: f64, f1: f64, f2: f64, f3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5
        * ((2.0 * f1)
            + (-f0 + f2) * t
            + (2.0 * f0 - 5.0 * f1 + 4.0 * f2 - f3) * t2
            + (-f0 + 3.0 * f1 - 3.0 * f2 + f3) * t3)
}

fn cubic_neighbor_indices(idx: usize, max_idx: usize) -> (usize, usize, usize, usize) {
    let i0 = if idx == 0 { 0 } else { idx - 1 };
    let i1 = idx;
    let i2 = if idx + 1 > max_idx { max_idx } else { idx + 1 };
    let i3 = if idx + 2 > max_idx { max_idx } else { idx + 2 };
    (i0, i1, i2, i3)
}

fn cubic_interpolate_1d(arr: &[f64], f: &[f64], value: f64) -> f64 {
    let n = arr.len();
    if value < arr[0] || value > arr[n - 1] {
        panic!("Value outside array domain for cubic interpolation.");
    }
    if n < 4 {
        panic!("Need at least 4 points for cubic interpolation.");
    }
    let (left, right, _) = find_linear_indices_and_t(arr, value);
    if left == right {
        return f[left];
    }
    // Use the left as our "center" for Catmull-Rom:
    let center = left;
    let (i0, i1, i2, i3) = cubic_neighbor_indices(center, n - 1);
    let x1 = arr[i1];
    let x2 = arr[i2];
    let span = x2 - x1;
    let local_t = if span.abs() < 1e-12 { 0.0 } else { (value - x1) / span };
    let f0 = f[i0];
    let f1 = f[i1];
    let f2 = f[i2];
    let f3 = f[i3];
    catmull_rom_spline(f0, f1, f2, f3, local_t)
}

// ------------------ 2D SLICE INTERPOLATION (unchanged) ------------------ //
fn interpolate_2d_slice(
    target_e: f64,
    target_n: f64,
    eastings: &[f64],
    northings: &[f64],
    slice_2d: &[f64], // flatten [nx, ny]
    nx: usize,
    ny: usize,
    method: &str,
) -> f64 {
    match method {
        "nearest" => {
            let ix = find_nearest_index(eastings, target_e);
            let iy = find_nearest_index(northings, target_n);
            slice_2d[ix * ny + iy]
        },
        "linear" => {
            let (i0, i1, tx) = find_linear_indices_and_t(eastings, target_e);
            let (j0, j1, ty) = find_linear_indices_and_t(northings, target_n);
            let f00 = slice_2d[i0 * ny + j0];
            let f01 = slice_2d[i0 * ny + j1];
            let f10 = slice_2d[i1 * ny + j0];
            let f11 = slice_2d[i1 * ny + j1];
            // Bilinear
            let f0 = f00 * (1.0 - ty) + f01 * ty;
            let f1 = f10 * (1.0 - ty) + f11 * ty;
            f0 * (1.0 - tx) + f1 * tx
        },
        "cubic" => {
            // Separable cubic in x, then y
            let mut intermediate = vec![0.0; ny];
            for j in 0..ny {
                let mut f_x = vec![0.0; nx];
                for i in 0..nx {
                    f_x[i] = slice_2d[i * ny + j];
                }
                intermediate[j] = cubic_interpolate_1d(eastings, &f_x, target_e);
            }
            // now interpolate in y
            cubic_interpolate_1d(northings, &intermediate, target_n)
        },
        _ => panic!("Unknown interpolation method: {}", method),
    }
}

// -------------- Axis Reversal Helpers -------------- //

/// Reverse axis 0 of a 3D array in-place (shape [nx, ny, nz]).
/// That is, swap row i with row (nx-1 - i).
fn reverse_axis0_in_place(arr: &mut [f64], nx: usize, ny: usize, nz: usize) {
    // Each "row" is size ny*nz. We'll swap row i with row nx-1-i.
    let stride = ny * nz;
    for i in 0..(nx / 2) {
        let j = nx - 1 - i;
        // Swap the entire "row" of length stride
        let start_i = i * stride;
        let start_j = j * stride;
        for k in 0..stride {
            arr.swap(start_i + k, start_j + k);
        }
    }
}

/// Reverse axis 1 of a 3D array in-place (shape [nx, ny, nz]).
/// That is, for each x-slice, swap column y with (ny-1 - y).
fn reverse_axis1_in_place(arr: &mut [f64], nx: usize, ny: usize, nz: usize) {
    // Each x-slice is size ny*nz
    // For each x, we swap row y with row ny-1-y
    for x in 0..nx {
        let base_x = x * ny * nz;
        for y in 0..(ny / 2) {
            let y2 = ny - 1 - y;
            for z in 0..nz {
                let idx1 = base_x + y * nz + z;
                let idx2 = base_x + y2 * nz + z;
                arr.swap(idx1, idx2);
            }
        }
    }
}

// ------------------ MAIN PYTHON-EXPOSED FUNCTION ------------------ //

#[pyfunction]
#[pyo3(signature = (target_easting, target_northing, eastings, northings, data, method, a_min=None, a_max=None))]
#[allow(clippy::too_many_arguments)]
fn interpolate_geospatial(
    target_easting: f64,
    target_northing: f64,
    eastings: PyReadonlyArray1<f64>,
    northings: PyReadonlyArray1<f64>,
    data: PyReadonlyArray3<f64>, // shape = [nx, ny, n_dates]
    method: &str,
    a_min: Option<f64>,
    a_max: Option<f64>,
) -> PyResult<Vec<f64>>  {

    // Convert from NumPy to owned Rust arrays (so we can reverse in-place).
    // We'll also ensure we have a contiguous standard layout for easy index manipulation.
    let mut e_vec = eastings.as_slice()?.to_vec();
    let mut n_vec = northings.as_slice()?.to_vec();
    let shape = data.shape();
    if shape.len() != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "data must be 3D: [nx, ny, n_dates]",
        ));
    }
    let (nx, ny, n_dates) = (shape[0], shape[1], shape[2]);

    // Copy data to a mutable Vec in standard order: i in [0..nx], j in [0..ny], k in [0..n_dates].
    // The data array is presumably row-major from Python (C-contiguous).
    let mut data_buf = data.to_vec()?; // length = nx * ny * n_dates

    // If easting is descending, reverse both the easting array and axis 0 of data.
    if e_vec[0] > e_vec[nx - 1] {
        e_vec.reverse();
        reverse_axis0_in_place(&mut data_buf, nx, ny, n_dates);
    }
    // If northing is descending, reverse both the northing array and axis 1 of data.
    if n_vec[0] > n_vec[ny - 1] {
        n_vec.reverse();
        reverse_axis1_in_place(&mut data_buf, nx, ny, n_dates);
    }

    // Now e_vec and n_vec are guaranteed ascending.
    // Domain checks are straightforward:
    if target_easting < e_vec[0] || target_easting > e_vec[nx - 1]
        || target_northing < n_vec[0] || target_northing > n_vec[ny - 1]
    {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Target easting/northing is outside the grid domain.",
        ));
    }

    // Interpolate for each "date" => we extract [nx, ny] slices from data_buf
    let mut out = vec![0.0; n_dates];
    for date_idx in 0..n_dates {
        // Build the 2D slice for this date
        // data_buf is shape [nx, ny, n_dates] in row-major.
        // Flatten each [nx, ny].
        let mut slice_2d = vec![0.0; nx * ny];
        for i in 0..nx {
            for j in 0..ny {
                slice_2d[i * ny + j] = data_buf[i * ny * n_dates + j * n_dates + date_idx];
            }
        }
        let val = interpolate_2d_slice(
            target_easting,
            target_northing,
            &e_vec,
            &n_vec,
            &slice_2d,
            nx,
            ny,
            method,
        );
        out[date_idx] = val;
    }

    // Clip output if requested
    if let Some(minv) = a_min {
        for v in &mut out {
            if *v < minv {
                *v = minv;
            }
        }
    }
    if let Some(maxv) = a_max {
        for v in &mut out {
            if *v > maxv {
                *v = maxv;
            }
        }
    }

    Ok(out)
}

const HEADER_LINES: usize = 15;
const EXPECTED_TOKENS: usize = 13;

pub fn rust_cli_revision(src_fn: &str, dst_fn: &str, 
    ws_ppts: [f64; 12], ws_tmaxs: [f64; 12], ws_tmins:  [f64; 12],
    hill_ppts: [f64; 12], hill_tmaxs: [f64; 12], hill_tmins:  [f64; 12],
) -> Result<()> {
    let src_f = File::open(src_fn)?;
    let mut src_r = BufReader::new(src_f);

    let dst_f = File::create(dst_fn)?;
    let mut dst_w = BufWriter::new(dst_f);

    let mut line = String::new();
    for _ in 0..HEADER_LINES {
        src_r.read_line(&mut line)?;
        dst_w.write_all(line.as_bytes())?;
        line.clear();
    }

    while src_r.read_line(&mut line)? > 0 {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() == EXPECTED_TOKENS {
            let da = tokens[0];
            let mo: i32 = tokens[1].parse().unwrap();
            let year = tokens[2];
            let mut prcp_f: f64 = tokens[3].parse().unwrap();
            let dur = tokens[4];
            let tp = tokens[5];
            let ip = tokens[6];
            let mut tmax_f: f64 = tokens[7].parse().unwrap();
            let mut tmin_f: f64 = tokens[8].parse().unwrap();
            let rad = tokens[9];
            let w_vl = tokens[10];
            let w_dir = tokens[11];
            let tdew = tokens[12];
        
            let indx = (mo - 1) as usize;
            prcp_f = prcp_f * hill_ppts[indx] / ws_ppts[indx];
            tmax_f = tmax_f - ws_tmaxs[indx] + hill_tmaxs[indx];
            tmin_f = tmin_f - ws_tmins[indx] + hill_tmins[indx];

            let prcp = format!("{:.1}", prcp_f);
            let tmax = format!("{:.1}", tmax_f);
            let tmin = format!("{:.1}", tmin_f);

            dst_w.write_all(format!(
                "{:>3}{:>3}{:>5}{:>6}{:>6}{:>5}{:>7}{:>6}{:>6}{:>5}{:>5}{:>6}{:>6}\n",
                da, mo, year, prcp, dur, tp, ip, tmax, tmin, rad, w_vl, w_dir, tdew
            ).as_bytes())?;
        }
        line.clear();
    }
    Ok(())
}


/// spatializes climate file by biasing between precip, tmin, and tmax values 
/// of the watershed centroid and the hill centroid
/// 
/// inputs:
///   src_fn: str
///       path to climate file to spatialize
///   dst_fn: str
///       path to output spatialized climate file
///   ws_ppts: list of floats
///       list of watershed monthly precip values
///   ws_tmaxs: list of floats
///       list of watershed monthly tmax values
///   ws_tmins: list of floats
///       list of watershed monthly tmin values
///   hill_ppts: list of floats
///       list of hill monthly precip values
///   hill_tmaxs: list of floats
///       list of hill monthly tmax values
///   hill_tmins: list of floats
///       list of hill monthly tmin values
/// 
/// returns:
///  None
#[pyfunction]
fn cli_revision(
    src_fn: &str, 
    dst_fn: &str, 
    ws_ppts: Vec<f64>, 
    ws_tmaxs: Vec<f64>, 
    ws_tmins: Vec<f64>, 
    hill_ppts: Vec<f64>, 
    hill_tmaxs: Vec<f64>, 
    hill_tmins: Vec<f64>
) -> PyResult<()> {
    println!("{}", src_fn);
    println!("{}", dst_fn);

    // Convert Vec<f64> to [f64; 12]
    let convert_array = |v: Vec<f64>| -> PyResult<[f64; 12]> {
        if v.len() == 12 {
            let mut arr = [0.0; 12];
            for (i, &item) in v.iter().enumerate() {
                arr[i] = item;
            }
            Ok(arr)
        } else {
            Err(pyo3::exceptions::PyValueError::new_err("Expected a list of length 12"))
        }
    };

    // Call the original Rust function
    rust_cli_revision(
        src_fn, 
        dst_fn, 
        convert_array(ws_ppts)?, 
        convert_array(ws_tmaxs)?, 
        convert_array(ws_tmins)?, 
        convert_array(hill_ppts)?, 
        convert_array(hill_tmaxs)?, 
        convert_array(hill_tmins)?
    ).map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{}", e)))?;

    Ok(())
}

/// A PyO3 module
/// This module is a container for the Python-callable functions we define
#[pymodule]
fn cli_revision_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(cli_revision, m)?)?;
    m.add_function(wrap_pyfunction!(interpolate_geospatial, m)?)?;
    Ok(())
}

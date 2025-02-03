
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, BufRead, Result};
use numpy::{PyReadonlyArray1, PyReadonlyArray3};

/// Find the closest index in sorted array `arr` for the given `value` (for "nearest" mode).
fn find_nearest_index(arr: &[f64], value: f64) -> usize {
    // Assumes arr is sorted in ascending order.
    let mut nearest_idx = 0;
    let mut min_dist = f64::MAX;
    for (i, &v) in arr.iter().enumerate() {
        let dist = (v - value).abs();
        if dist < min_dist {
            min_dist = dist;
            nearest_idx = i;
        } else {
            // Because arr is sorted, once dist starts increasing, we can break early if desired
        }
    }
    nearest_idx
}

/// For "linear" mode, find the two bounding indices and interpolation factor (t in [0,1]).
fn find_linear_indices_and_t(arr: &[f64], value: f64) -> (usize, usize, f64) {
    // We'll do a binary search for efficiency:
    let n = arr.len();
    // If outside domain, we’ll raise an error. We expect caller checks domain beforehand.
    // But let's do an explicit check to be safe:
    if value < arr[0] || value > arr[n - 1] {
        panic!("Value outside array domain. No extrapolation allowed.");
    }

    // Simple manual binary search:
    let mut left = 0;
    let mut right = n - 1;
    while right - left > 1 {
        let mid = (left + right) / 2;
        if arr[mid] == value {
            return (mid, mid, 0.0); // direct match
        } else if arr[mid] < value {
            left = mid;
        } else {
            right = mid;
        }
    }

    // left, right are consecutive. Compute interpolation ratio:
    let denom = arr[right] - arr[left];
    let t = if denom.abs() < 1e-12 {
        0.0
    } else {
        (value - arr[left]) / denom
    };

    (left, right, t)
}

/// 1D Catmull-Rom (cubic) spline interpolation for four known points f0, f1, f2, f3 at x in [0,1].
/// This is a standard formula: see “Catmull-Rom” or “cardinal splines” for derivation.
fn catmull_rom_spline(f0: f64, f1: f64, f2: f64, f3: f64, t: f64) -> f64 {
    // Catmull-Rom with tension = 0.5 is common, but here's the standard 0 tension formula
    // that often is represented as:
    //   f(t) = f1 + 0.5 * t * (f2 - f0 + t * (2.0*f0 - 5.0*f1 + 4.0*f2 - f3
    //                       + t * (3.0*(f1 - f2) + f3 - f0)))
    // For simplicity, tension=0.0 (which is classic Catmull-Rom).
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * ((2.0 * f1)
        + (-f0 + f2) * t
        + (2.0*f0 - 5.0*f1 + 4.0*f2 - f3) * t2
        + (-f0 + 3.0*f1 - 3.0*f2 + f3) * t3)
}

/// Find four consecutive indices around `center_idx` for cubic interpolation.
/// For points near edges, we clamp so we can still pick 4 neighbors.
fn cubic_neighbor_indices(idx: usize, max_idx: usize) -> (usize, usize, usize, usize) {
    // We want idx-1, idx, idx+1, idx+2, but must clamp in [0, max_idx].
    let i0 = if idx == 0 { 0 } else { idx - 1 };
    let i1 = idx;
    let i2 = if idx + 1 > max_idx { max_idx } else { idx + 1 };
    let i3 = if idx + 2 > max_idx { max_idx } else { idx + 2 };
    (i0, i1, i2, i3)
}

/// 1D cubic interpolation utility:
/// Given a sorted coordinate array `arr`, a desired `value`, and a slice of function values `f`,
/// returns the interpolated value. Here, `f.len() == arr.len()`.
///
/// We do Catmull-Rom between four neighbors. We first locate the 'segment' using linear approach,
/// then pick neighbors around it.
fn cubic_interpolate_1d(arr: &[f64], f: &[f64], value: f64) -> f64 {
    let n = arr.len();
    if value < arr[0] || value > arr[n - 1] {
        panic!("Value outside array domain for cubic interpolation.");
    }
    if n < 4 {
        panic!("Need at least 4 points for cubic interpolation.");
    }

    // Identify the segment [left, right] via linear approach
    let (left, right, _t) = find_linear_indices_and_t(arr, value);
    if left == right {
        // exact match
        return f[left];
    }

    // We will do a Catmull-Rom approach around left, so let's define a 'center' near left
    // so that the interpolation fraction is t in [0,1] between arr[left], arr[right].
    let center = left;
    let (i0, i1, i2, i3) = cubic_neighbor_indices(center, n - 1);

    // Convert "value in [arr[i1], arr[i2]]" to local t in [0,1].
    let x1 = arr[i1];
    let x2 = arr[i2];
    let span = x2 - x1;
    let local_t = if span.abs() < 1e-12 {
        0.0
    } else {
        (value - x1) / span
    };

    let f0 = f[i0];
    let f1 = f[i1];
    let f2 = f[i2];
    let f3 = f[i3];

    catmull_rom_spline(f0, f1, f2, f3, local_t)
}

/// Interpolate a single slice of data (shape = [nx, ny]) at (x, y) using nearest/linear/cubic.
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
            // Find nearest index in eastings, northings
            let ix = find_nearest_index(eastings, target_e);
            let iy = find_nearest_index(northings, target_n);
            slice_2d[ix * ny + iy]
        },
        "linear" => {
            // Bilinear approach: find i0,i1 + t in [0,1], j0,j1 + u in [0,1]
            let (i0, i1, tx) = find_linear_indices_and_t(eastings, target_e);
            let (j0, j1, ty) = find_linear_indices_and_t(northings, target_n);

            let f00 = slice_2d[i0 * ny + j0];
            let f01 = slice_2d[i0 * ny + j1];
            let f10 = slice_2d[i1 * ny + j0];
            let f11 = slice_2d[i1 * ny + j1];

            // Bilinear interpolation
            let f0 = f00 * (1.0 - ty) + f01 * ty;
            let f1 = f10 * (1.0 - ty) + f11 * ty;
            f0 * (1.0 - tx) + f1 * tx
        },
        "cubic" => {
            // We’ll do "separable" 2D cubic:
            // 1) For each y row, do 1D cubic in x dimension -> intermediate array of size ny
            // 2) Then do 1D cubic in y dimension on that intermediate array -> final scalar
            let mut intermediate = vec![0.0; ny];
            for j in 0..ny {
                // extract f(x) along fixed y=j
                let mut f_x = vec![0.0; nx];
                for i in 0..nx {
                    f_x[i] = slice_2d[i * ny + j];
                }
                intermediate[j] = cubic_interpolate_1d(eastings, &f_x, target_e);
            }

            // now interpolate in y dimension at the same target_n
            cubic_interpolate_1d(northings, &intermediate, target_n)
        },
        _ => panic!("Unknown interpolation method: {}", method),
    }
}

/// Python-exposed function for geospatial interpolation.
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
    // Convert inputs from numpy to Rust slices.
    let eastings = eastings.as_slice()?;
    let northings = northings.as_slice()?;
    let data = data.as_array();

    // Basic shape checks:
    let shape = data.shape();
    if shape.len() != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "data array must be 3D: [nx, ny, n_dates]",
        ));
    }
    let nx = shape[0];
    let ny = shape[1];
    let n_dates = shape[2];

    // Domain checks:
    if target_easting < eastings[0] || target_easting > eastings[nx - 1]
        || target_northing < northings[0] || target_northing > northings[ny - 1]
    {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "Target easting/northing is outside the grid domain.",
        ));
    }

    // Prepare output array in Rust
    let mut out = vec![0.0_f64; n_dates];

    // Interpolate each date's 2D slice
    for date_idx in 0..n_dates {
        // slice_2d is shape = [nx, ny]
        // in memory, data is shape [nx, ny, n_dates],
        // so for each i,j => data[[i, j, date_idx]]
        // We can flatten this slice in row-major: [i in 0..nx, j in 0..ny].
        let mut slice_2d = vec![0.0; nx*ny];
        for i in 0..nx {
            for j in 0..ny {
                slice_2d[i * ny + j] = data[[i, j, date_idx]];
            }
        }

        let val = interpolate_2d_slice(
            target_easting,
            target_northing,
            eastings,
            northings,
            &slice_2d,
            nx,
            ny,
            method,
        );
        out[date_idx] = val;
    }

    // Clip if requested
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

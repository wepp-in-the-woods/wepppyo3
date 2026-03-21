use numpy::PyArrayMethods;
use numpy::PyUntypedArrayMethods;
use numpy::{PyReadonlyArray1, PyReadonlyArray3};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::wrap_pyfunction;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Result, Write};

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
    0.5 * ((2.0 * f1)
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
        }
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
        }
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
        }
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
) -> PyResult<Vec<f64>> {
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
    if target_easting < e_vec[0]
        || target_easting > e_vec[nx - 1]
        || target_northing < n_vec[0]
        || target_northing > n_vec[ny - 1]
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
const DEFAULT_IP_CORRECTION: f64 = 0.70;
const DEFAULT_TIME_STEP_MINUTES: f64 = 5.0;

#[derive(Clone, Copy, Debug)]
struct HyetographSegment {
    start_hr: f64,
    end_hr: f64,
    intensity_mm_hr: f64,
}

#[derive(Clone, Debug)]
struct ParsedStormEvent {
    year: i32,
    prcp_mm: f64,
    dur_hr: f64,
    segments: Vec<HyetographSegment>,
}

#[derive(Clone, Debug)]
struct StaticRResult {
    mean_annual_r: f64,
    annual_ei30: BTreeMap<i32, f64>,
    storms_total: usize,
    storms_used: usize,
}

fn make_invalid_data_error(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidData, message.into())
}

fn wepp_eqroot(a: f64) -> Result<f64> {
    if !(0.0 < a && a <= 1.0) {
        return Err(make_invalid_data_error("eqroot expects 0 < a <= 1"));
    }

    if a <= 0.06 {
        return Ok(1.0 / a);
    }

    if a < 0.999 {
        let mut u = if a <= 0.2 {
            1.0 / a
        } else if a <= 0.5 {
            0.968732 / a - 1.55098 * a + 0.431653
        } else if a <= 0.94 {
            1.13243 / a - 0.928240 * a - 0.207111
        } else {
            1.5 - (6.0 * a - 3.75).sqrt()
        };

        loop {
            let e = (-u).exp();
            let f = (1.0 - e) / u;
            let d = a - f;
            let tmpvr1 = (u + 1.0) * f - 1.0;
            let r = a / tmpvr1;
            let s = if r <= 1.0 {
                (d / a).abs()
            } else {
                (d / tmpvr1).abs()
            };
            if s < 0.59e-6 {
                break;
            }
            u *= 1.0 + d / (e - f);
        }
        return Ok(u);
    }

    if a < 1.0 {
        return Ok(1.5 - (6.0 * a - 3.75).sqrt());
    }
    Ok(0.0)
}

fn wepp_dimensionless_hyetograph(
    tp: f64,
    ip: f64,
    ninten: usize,
    use_const: bool,
) -> Result<(Vec<f64>, Vec<f64>)> {
    if ninten < 2 {
        return Err(make_invalid_data_error(
            "ninten must be at least 2 for hyetograph generation",
        ));
    }

    let mut timedl = vec![0.0; ninten];
    let mut intdl = vec![0.0; ninten];
    let deltfq = 1.0 / (ninten - 1) as f64;

    if use_const {
        let mut fqx = 0.0;
        for i in 0..(ninten - 1) {
            fqx += deltfq;
            timedl[i + 1] = fqx;
            intdl[i] = 1.0;
        }
        intdl[ninten - 1] = 0.0;
        return Ok((timedl, intdl));
    }

    let u = wepp_eqroot(1.0 / ip)?;
    let b = u / tp;
    let a = ip * (-u).exp();
    let d = u / (1.0 - tp);

    timedl[ninten - 1] = 1.0;
    let mut fqx = 0.0;
    for i in 0..(ninten - 1) {
        if i < ninten - 2 {
            fqx += deltfq;
            if fqx <= tp {
                timedl[i + 1] = (1.0 / b) * (1.0 + (b / a) * fqx).ln();
            } else {
                timedl[i + 1] = tp - (1.0 / d) * (1.0 - (d / ip) * (fqx - tp)).ln();
            }
        }

        let diff = timedl[i + 1] - timedl[i];
        intdl[i] = if diff > 0.0 { deltfq / diff } else { deltfq / 0.00001 };
    }

    intdl[ninten - 1] = 0.0;
    Ok((timedl, intdl))
}

fn build_hyetograph_non_breakpoint_segments(
    prcp_mm: f64,
    dur_hr: f64,
    tp: f64,
    ip: f64,
    ip_correction: f64,
    min_step_minutes: f64,
) -> Result<Vec<HyetographSegment>> {
    if prcp_mm <= 0.0 || dur_hr <= 0.0 {
        return Ok(Vec::new());
    }
    if min_step_minutes <= 0.0 {
        return Err(make_invalid_data_error(
            "min_step_minutes must be greater than zero",
        ));
    }

    let mut tp_adj = tp;
    let ip_adj = (ip * ip_correction).max(1.0);

    if tp_adj > 1.0 || ip_adj == 1.0 {
        tp_adj = 1.0;
    } else if tp_adj <= 0.0 {
        tp_adj = 0.01;
    }

    let mut ninten: usize = 11;
    let (timedl, intdl) = loop {
        let use_const = tp_adj >= 1.0 && ip_adj <= 1.0;
        let n = if ninten <= 2 { 2 } else { ninten };
        let (timedl, intdl) = wepp_dimensionless_hyetograph(tp_adj, ip_adj, n, n == 2 || use_const)?;

        let mut min_step = f64::MAX;
        for i in 0..(timedl.len() - 1) {
            let step_minutes = (timedl[i + 1] - timedl[i]) * dur_hr * 60.0;
            if step_minutes < min_step {
                min_step = step_minutes;
            }
        }

        if min_step >= min_step_minutes || n <= 2 {
            break (timedl, intdl);
        }
        ninten -= 1;
    };

    let mut segments: Vec<HyetographSegment> = Vec::new();
    for i in 0..(timedl.len() - 1) {
        let start_hr = timedl[i] * dur_hr;
        let end_hr = timedl[i + 1] * dur_hr;
        if end_hr <= start_hr {
            continue;
        }
        let intensity_mm_hr = intdl[i] * prcp_mm / dur_hr;
        segments.push(HyetographSegment {
            start_hr,
            end_hr,
            intensity_mm_hr,
        });
    }
    Ok(segments)
}

fn build_hyetograph_breakpoint_segments(
    breakpoint_times_hr: &[f64],
    breakpoint_cum_depth_mm: &[f64],
) -> Result<Vec<HyetographSegment>> {
    if breakpoint_times_hr.len() != breakpoint_cum_depth_mm.len() {
        return Err(make_invalid_data_error(format!(
            "breakpoint times/depth lengths differ: {} != {}",
            breakpoint_times_hr.len(),
            breakpoint_cum_depth_mm.len()
        )));
    }
    if breakpoint_times_hr.is_empty() {
        return Ok(Vec::new());
    }

    let mut segments: Vec<HyetographSegment> = Vec::new();
    let mut prev_time = 0.0;
    let mut prev_depth = 0.0;

    for (&current_time, &current_depth) in breakpoint_times_hr
        .iter()
        .zip(breakpoint_cum_depth_mm.iter())
    {
        let dt = current_time - prev_time;
        let dd = current_depth - prev_depth;
        if dt > 0.0 && dd > 0.0 {
            segments.push(HyetographSegment {
                start_hr: prev_time,
                end_hr: current_time,
                intensity_mm_hr: dd / dt,
            });
        }
        prev_time = current_time;
        prev_depth = current_depth;
    }

    Ok(segments)
}

fn segments_depth_bins(
    segments: &[HyetographSegment],
    dur_hr: f64,
    time_step_minutes: f64,
) -> Result<(Vec<f64>, f64)> {
    if dur_hr <= 0.0 || time_step_minutes <= 0.0 {
        return Ok((Vec::new(), 0.0));
    }
    let dt_hr = time_step_minutes / 60.0;
    let total_bins = (dur_hr / dt_hr).ceil() as usize;
    if total_bins == 0 {
        return Ok((Vec::new(), dt_hr));
    }

    let mut depths = vec![0.0; total_bins];
    if segments.is_empty() {
        return Ok((depths, dt_hr));
    }

    let mut seg_index = 0usize;
    let seg_count = segments.len();
    let eps = 1.0e-9;
    let mut seg_start = segments[0].start_hr;
    let mut seg_end = segments[0].end_hr;
    let mut seg_intensity = segments[0].intensity_mm_hr;

    for (bin_idx, depth_slot) in depths.iter_mut().enumerate() {
        let bin_start = bin_idx as f64 * dt_hr;
        let bin_end = bin_start + dt_hr;
        let mut remaining_start = bin_start;
        let mut depth = 0.0;

        while seg_index < seg_count {
            if seg_end <= remaining_start + eps {
                seg_index += 1;
                if seg_index < seg_count {
                    seg_start = segments[seg_index].start_hr;
                    seg_end = segments[seg_index].end_hr;
                    seg_intensity = segments[seg_index].intensity_mm_hr;
                }
                continue;
            }

            if seg_start >= bin_end - eps {
                break;
            }

            let overlap_start = remaining_start.max(seg_start);
            let overlap_end = bin_end.min(seg_end);
            if overlap_end > overlap_start {
                depth += seg_intensity * (overlap_end - overlap_start);
                remaining_start = overlap_end;
            }

            if overlap_end >= seg_end - eps {
                seg_index += 1;
                if seg_index < seg_count {
                    seg_start = segments[seg_index].start_hr;
                    seg_end = segments[seg_index].end_hr;
                    seg_intensity = segments[seg_index].intensity_mm_hr;
                }
            }

            if remaining_start >= bin_end - eps {
                break;
            }
        }

        *depth_slot = depth;
    }

    Ok((depths, dt_hr))
}

fn compute_peak_intensities_from_segments_internal(
    segments: &[HyetographSegment],
    storm_depth_mm: f64,
    storm_duration_hours: f64,
    windows_minutes: &[i32],
    time_step_minutes: f64,
) -> Result<Vec<f64>> {
    if storm_depth_mm <= 0.0 || storm_duration_hours <= 0.0 {
        return Ok(vec![0.0; windows_minutes.len()]);
    }
    if time_step_minutes <= 0.0 {
        return Err(make_invalid_data_error(
            "time_step_minutes must be greater than zero",
        ));
    }

    let (depths, dt_hr) = segments_depth_bins(segments, storm_duration_hours, time_step_minutes)?;
    if depths.is_empty() || dt_hr <= 0.0 {
        return Ok(vec![0.0; windows_minutes.len()]);
    }

    let total_bins = depths.len();
    let mut cumulative = vec![0.0; total_bins + 1];
    for i in 0..total_bins {
        cumulative[i + 1] = cumulative[i] + depths[i];
    }

    let mut results: Vec<f64> = Vec::with_capacity(windows_minutes.len());
    for &window_minutes in windows_minutes {
        let window_hr = window_minutes as f64 / 60.0;
        if window_hr <= 0.0 {
            results.push(0.0);
            continue;
        }

        let window_bins = (window_hr / dt_hr).round() as usize;
        if window_bins == 0 {
            results.push(0.0);
            continue;
        }
        if window_bins > total_bins {
            results.push(storm_depth_mm / window_hr);
            continue;
        }

        let mut max_depth = 0.0;
        for start in 0..=(total_bins - window_bins) {
            let depth = cumulative[start + window_bins] - cumulative[start];
            if depth > max_depth {
                max_depth = depth;
            }
        }
        results.push(max_depth / window_hr);
    }

    Ok(results)
}

fn convert_segments_to_tuples(segments: &[HyetographSegment]) -> Vec<(f64, f64, f64)> {
    segments
        .iter()
        .map(|seg| (seg.start_hr, seg.end_hr, seg.intensity_mm_hr))
        .collect()
}

fn convert_tuples_to_segments(tuples: &[(f64, f64, f64)]) -> Vec<HyetographSegment> {
    tuples
        .iter()
        .map(|(start_hr, end_hr, intensity_mm_hr)| HyetographSegment {
            start_hr: *start_hr,
            end_hr: *end_hr,
            intensity_mm_hr: *intensity_mm_hr,
        })
        .collect()
}

fn event_energy_mj_per_ha(segments: &[HyetographSegment]) -> f64 {
    let mut energy = 0.0;
    for seg in segments {
        let duration_hr = seg.end_hr - seg.start_hr;
        if duration_hr <= 0.0 || seg.intensity_mm_hr <= 0.0 {
            continue;
        }
        let depth_mm = seg.intensity_mm_hr * duration_hr;
        if depth_mm <= 0.0 {
            continue;
        }
        let mut unit_energy = 0.119 + 0.0873 * seg.intensity_mm_hr.log10();
        if unit_energy > 0.283 {
            unit_energy = 0.283;
        }
        if unit_energy <= 0.0 {
            continue;
        }
        energy += unit_energy * depth_mm;
    }
    energy
}

fn parse_cli_storms_for_static_r(
    src_fn: &str,
    ip_correction: f64,
    time_step_minutes: f64,
) -> Result<(Vec<ParsedStormEvent>, Vec<i32>)> {
    let src_f = File::open(src_fn)?;
    let src_r = BufReader::new(src_f);
    let lines: Vec<String> = src_r.lines().collect::<std::result::Result<Vec<_>, _>>()?;

    let breakpoint_mode = lines
        .get(1)
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<i32>().ok())
        .map(|value| value > 0)
        .unwrap_or(false);

    let mut header_index: Option<usize> = None;
    for (idx, line) in lines.iter().enumerate() {
        let lower = line.trim().to_ascii_lowercase();
        if lower.starts_with("da ") || lower.starts_with("day ") {
            header_index = Some(idx);
            break;
        }
    }
    let header_index = header_index.ok_or_else(|| {
        make_invalid_data_error("Unable to locate CLI data header line starting with day/month/year")
    })?;
    let mut idx = header_index + 2;

    let mut storms: Vec<ParsedStormEvent> = Vec::new();
    let mut years: BTreeSet<i32> = BTreeSet::new();

    while idx < lines.len() {
        let trimmed = lines[idx].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            idx += 1;
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.is_empty() {
            idx += 1;
            continue;
        }

        if breakpoint_mode {
            if tokens.len() == 2 {
                idx += 1;
                continue;
            }
            if tokens.len() < 4 {
                idx += 1;
                continue;
            }

            let year = match tokens[2].parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    idx += 1;
                    continue;
                }
            };
            let nbrkpt = match tokens[3].parse::<usize>() {
                Ok(value) => value,
                Err(_) => {
                    idx += 1;
                    continue;
                }
            };
            years.insert(year);

            let mut breakpoint_times_hr: Vec<f64> = Vec::with_capacity(nbrkpt);
            let mut breakpoint_cum_depth_mm: Vec<f64> = Vec::with_capacity(nbrkpt);
            for _ in 0..nbrkpt {
                idx += 1;
                if idx >= lines.len() {
                    return Err(make_invalid_data_error(
                        "Reached end of CLI while parsing breakpoint rows",
                    ));
                }
                let bp_tokens: Vec<&str> = lines[idx].split_whitespace().collect();
                if bp_tokens.len() < 2 {
                    return Err(make_invalid_data_error(format!(
                        "Expected breakpoint row with 2 columns, got '{}'",
                        lines[idx]
                    )));
                }
                let time_hr = bp_tokens[0].parse::<f64>().map_err(|_| {
                    make_invalid_data_error(format!("Invalid breakpoint time '{}'", bp_tokens[0]))
                })?;
                let cum_depth_mm = bp_tokens[1].parse::<f64>().map_err(|_| {
                    make_invalid_data_error(format!(
                        "Invalid breakpoint cumulative depth '{}'",
                        bp_tokens[1]
                    ))
                })?;
                breakpoint_times_hr.push(time_hr);
                breakpoint_cum_depth_mm.push(cum_depth_mm);
            }

            let segments =
                build_hyetograph_breakpoint_segments(&breakpoint_times_hr, &breakpoint_cum_depth_mm)?;
            let prcp_mm = breakpoint_cum_depth_mm.last().copied().unwrap_or(0.0);
            let dur_hr = breakpoint_times_hr.last().copied().unwrap_or(0.0);
            storms.push(ParsedStormEvent {
                year,
                prcp_mm,
                dur_hr,
                segments,
            });
            idx += 1;
            continue;
        }

        if tokens.len() < 7 {
            idx += 1;
            continue;
        }

        let year = match tokens[2].parse::<i32>() {
            Ok(value) => value,
            Err(_) => {
                idx += 1;
                continue;
            }
        };
        let prcp_mm = tokens[3].parse::<f64>().map_err(|_| {
            make_invalid_data_error(format!("Invalid precipitation '{}'", tokens[3]))
        })?;
        let dur_hr = tokens[4].parse::<f64>().map_err(|_| {
            make_invalid_data_error(format!("Invalid duration '{}'", tokens[4]))
        })?;
        let tp = tokens[5]
            .parse::<f64>()
            .map_err(|_| make_invalid_data_error(format!("Invalid tp '{}'", tokens[5])))?;
        let ip = tokens[6]
            .parse::<f64>()
            .map_err(|_| make_invalid_data_error(format!("Invalid ip '{}'", tokens[6])))?;
        let segments = build_hyetograph_non_breakpoint_segments(
            prcp_mm,
            dur_hr,
            tp,
            ip,
            ip_correction,
            time_step_minutes,
        )?;

        years.insert(year);
        storms.push(ParsedStormEvent {
            year,
            prcp_mm,
            dur_hr,
            segments,
        });
        idx += 1;
    }

    Ok((storms, years.into_iter().collect()))
}

fn compute_static_r_from_cli_internal(
    src_fn: &str,
    ip_correction: f64,
    time_step_minutes: f64,
    storm_depth_threshold_mm: f64,
) -> Result<StaticRResult> {
    let (storms, years) = parse_cli_storms_for_static_r(src_fn, ip_correction, time_step_minutes)?;
    let mut annual: BTreeMap<i32, f64> = BTreeMap::new();
    for year in years {
        annual.insert(year, 0.0);
    }

    let mut storms_used = 0usize;
    for storm in &storms {
        if storm.prcp_mm < storm_depth_threshold_mm || storm.dur_hr <= 0.0 {
            continue;
        }
        let energy = event_energy_mj_per_ha(&storm.segments);
        if energy <= 0.0 {
            continue;
        }
        let i30 = compute_peak_intensities_from_segments_internal(
            &storm.segments,
            storm.prcp_mm,
            storm.dur_hr,
            &[30],
            time_step_minutes,
        )?[0];
        if i30 <= 0.0 {
            continue;
        }
        let ei30 = energy * i30;
        *annual.entry(storm.year).or_insert(0.0) += ei30;
        storms_used += 1;
    }

    let mean_annual_r = if annual.is_empty() {
        0.0
    } else {
        annual.values().sum::<f64>() / annual.len() as f64
    };

    Ok(StaticRResult {
        mean_annual_r,
        annual_ei30: annual,
        storms_total: storms.len(),
        storms_used,
    })
}

#[pyfunction]
#[pyo3(signature = (prcp_mm, dur_hr, tp, ip, ip_correction=DEFAULT_IP_CORRECTION, min_step_minutes=DEFAULT_TIME_STEP_MINUTES))]
fn build_hyetograph_non_breakpoint(
    prcp_mm: f64,
    dur_hr: f64,
    tp: f64,
    ip: f64,
    ip_correction: f64,
    min_step_minutes: f64,
) -> PyResult<Vec<(f64, f64, f64)>> {
    let segments =
        build_hyetograph_non_breakpoint_segments(prcp_mm, dur_hr, tp, ip, ip_correction, min_step_minutes)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))?;
    Ok(convert_segments_to_tuples(&segments))
}

#[pyfunction]
fn build_hyetograph_breakpoint(
    breakpoint_times_hr: Vec<f64>,
    breakpoint_cum_depth_mm: Vec<f64>,
) -> PyResult<Vec<(f64, f64, f64)>> {
    let segments =
        build_hyetograph_breakpoint_segments(&breakpoint_times_hr, &breakpoint_cum_depth_mm)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))?;
    Ok(convert_segments_to_tuples(&segments))
}

#[pyfunction]
#[pyo3(signature = (segments, storm_depth_mm, storm_duration_hours, windows_minutes=None, time_step_minutes=DEFAULT_TIME_STEP_MINUTES))]
fn compute_peak_intensities_from_hyetograph(
    segments: Vec<(f64, f64, f64)>,
    storm_depth_mm: f64,
    storm_duration_hours: f64,
    windows_minutes: Option<Vec<i32>>,
    time_step_minutes: f64,
) -> PyResult<Vec<f64>> {
    let windows = windows_minutes.unwrap_or_else(|| vec![10, 15, 30, 60]);
    let parsed_segments = convert_tuples_to_segments(&segments);
    compute_peak_intensities_from_segments_internal(
        &parsed_segments,
        storm_depth_mm,
        storm_duration_hours,
        &windows,
        time_step_minutes,
    )
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))
}

#[pyfunction]
#[pyo3(signature = (prcp_mm, dur_hr, tp, ip, windows_minutes=None, ip_correction=DEFAULT_IP_CORRECTION, time_step_minutes=DEFAULT_TIME_STEP_MINUTES))]
fn compute_peak_intensities_non_breakpoint(
    prcp_mm: f64,
    dur_hr: f64,
    tp: f64,
    ip: f64,
    windows_minutes: Option<Vec<i32>>,
    ip_correction: f64,
    time_step_minutes: f64,
) -> PyResult<Vec<f64>> {
    let windows = windows_minutes.unwrap_or_else(|| vec![10, 15, 30, 60]);
    let segments = build_hyetograph_non_breakpoint_segments(
        prcp_mm,
        dur_hr,
        tp,
        ip,
        ip_correction,
        time_step_minutes,
    )
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))?;
    compute_peak_intensities_from_segments_internal(
        &segments,
        prcp_mm,
        dur_hr,
        &windows,
        time_step_minutes,
    )
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))
}

#[pyfunction]
#[pyo3(signature = (breakpoint_times_hr, breakpoint_cum_depth_mm, windows_minutes=None, time_step_minutes=DEFAULT_TIME_STEP_MINUTES))]
fn compute_peak_intensities_breakpoint(
    breakpoint_times_hr: Vec<f64>,
    breakpoint_cum_depth_mm: Vec<f64>,
    windows_minutes: Option<Vec<i32>>,
    time_step_minutes: f64,
) -> PyResult<Vec<f64>> {
    let windows = windows_minutes.unwrap_or_else(|| vec![10, 15, 30, 60]);
    let segments =
        build_hyetograph_breakpoint_segments(&breakpoint_times_hr, &breakpoint_cum_depth_mm)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))?;
    let storm_depth_mm = breakpoint_cum_depth_mm.last().copied().unwrap_or(0.0);
    let storm_duration_hours = breakpoint_times_hr.last().copied().unwrap_or(0.0);
    compute_peak_intensities_from_segments_internal(
        &segments,
        storm_depth_mm,
        storm_duration_hours,
        &windows,
        time_step_minutes,
    )
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))
}

#[pyfunction]
#[pyo3(signature = (src_fn, ip_correction=DEFAULT_IP_CORRECTION, time_step_minutes=DEFAULT_TIME_STEP_MINUTES, storm_depth_threshold_mm=12.5))]
fn compute_static_r_from_cli(
    py: Python<'_>,
    src_fn: &str,
    ip_correction: f64,
    time_step_minutes: f64,
    storm_depth_threshold_mm: f64,
) -> PyResult<PyObject> {
    let result = compute_static_r_from_cli_internal(
        src_fn,
        ip_correction,
        time_step_minutes,
        storm_depth_threshold_mm,
    )
    .map_err(|err| pyo3::exceptions::PyValueError::new_err(format!("{}", err)))?;

    let output = PyDict::new_bound(py);
    output.set_item("mean_annual_r", result.mean_annual_r)?;
    output.set_item("storms_total", result.storms_total)?;
    output.set_item("storms_used", result.storms_used)?;
    output.set_item("storm_depth_threshold_mm", storm_depth_threshold_mm)?;
    output.set_item("energy_equation", "wepp_ah537_log10_capped_v1")?;
    output.set_item("units", "MJ mm ha^-1 h^-1")?;

    let annual_rows = PyList::empty_bound(py);
    for (year, ei30) in result.annual_ei30 {
        let row = PyDict::new_bound(py);
        row.set_item("year", year)?;
        row.set_item("ei30", ei30)?;
        annual_rows.append(row)?;
    }
    output.set_item("annual_ei30", annual_rows)?;
    Ok(output.to_object(py))
}

fn format_storm_value(value: f64) -> String {
    let mut s = format!("{:.6}", value);
    if let Some(pos) = s.find('.') {
        while s.len() > pos + 1 && s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.push('0');
        }
    }
    s
}

pub fn rust_make_rhem_storm_file(src_fn: &str, dst_fn: &str) -> Result<()> {
    let src_f = File::open(src_fn)?;
    let src_r = BufReader::new(src_f);

    let mut events: Vec<(i32, i32, i32, f64, f64, f64, f64)> = Vec::new();
    let mut min_year: Option<i32> = None;

    let mut header_found = false;
    let mut units_skipped = false;

    for line_result in src_r.lines() {
        let line = line_result?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if !header_found {
            if trimmed.to_ascii_lowercase().starts_with("da") {
                header_found = true;
            }
            continue;
        }

        if !units_skipped {
            units_skipped = true;
            continue;
        }

        if trimmed.starts_with('#') {
            continue;
        }

        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if tokens.len() < 7 {
            continue;
        }

        let day = match tokens[0].parse::<i32>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let month = match tokens[1].parse::<i32>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let year = match tokens[2].parse::<i32>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let prcp = match tokens[3].parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if prcp <= 0.0 {
            continue;
        }
        let dur = tokens
            .get(4)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let tp = tokens
            .get(5)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let ip = tokens
            .get(6)
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        if let Some(current_min) = min_year {
            if year < current_min {
                min_year = Some(year);
            }
        } else {
            min_year = Some(year);
        }

        events.push((day, month, year, prcp, dur, tp, ip));
    }

    let min_year = min_year.unwrap_or(0);
    let dst_f = File::create(dst_fn)?;
    let mut dst_w = BufWriter::new(dst_f);

    writeln!(dst_w, "{} # The number of rain events", events.len())?;
    writeln!(dst_w, "0 # Breakpoint data? (0 for no, 1 for yes)")?;
    writeln!(dst_w, "#  id     day  month  year  Rain   Dur    Tp     Ip")?;
    writeln!(dst_w, "#                           (mm)   (h)")?;

    for (idx, (day, month, year, prcp, dur, tp, ip)) in events.iter().enumerate() {
        let relative_year = year - min_year + 1;
        let prcp_str = format_storm_value(*prcp);
        let dur_str = format_storm_value(*dur);
        let tp_str = format_storm_value(*tp);
        let ip_str = format_storm_value(*ip);

        writeln!(
            dst_w,
            "{:<8}{:<6}{:<6}{:<6}{:<7}{:<7}{:<7}{:<7}",
            idx + 1,
            day,
            month,
            relative_year,
            prcp_str,
            dur_str,
            tp_str,
            ip_str
        )?;
    }

    Ok(())
}

#[pyfunction]
fn make_rhem_storm_file(src_fn: &str, dst_fn: &str) -> PyResult<()> {
    rust_make_rhem_storm_file(src_fn, dst_fn)
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{}", e)))
}

#[pyfunction]
pub fn rust_cli_revision(
    src_fn: &str,
    dst_fn: &str,
    mut ws_ppts: [f64; 12],
    ws_tmaxs: [f64; 12],
    ws_tmins: [f64; 12],
    mut hill_ppts: [f64; 12],
    hill_tmaxs: [f64; 12],
    hill_tmins: [f64; 12],
) -> Result<()> {
    // Clip ws_ppts and hill_ppts to minimum of 0.01
    for i in 0..12 {
        ws_ppts[i] = ws_ppts[i].max(0.01);
        hill_ppts[i] = hill_ppts[i].max(0.01);
    }

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
            let mut dur_f: f64 = tokens[4].parse().unwrap();
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

            // Ensure duration > 0.0 when prcp > 0
            if prcp_f > 0.0 && dur_f == 0.0 {
                dur_f = 0.05;
            }

            let prcp = format!("{:.1}", prcp_f);
            let dur = format!("{:.2}", dur_f);
            let tmax = format!("{:.1}", tmax_f);
            let tmin = format!("{:.1}", tmin_f);

            dst_w.write_all(
                format!(
                    "{:>3}{:>3}{:>5}{:>6}{:>6}{:>5}{:>7}{:>6}{:>6}{:>5}{:>5}{:>6}{:>6}\n",
                    da, mo, year, prcp, dur, tp, ip, tmax, tmin, rad, w_vl, w_dir, tdew
                )
                .as_bytes(),
            )?;
        }
        line.clear();
    }
    Ok(())
}

#[pyfunction]
pub fn rust_cli_p_scale_monthlies(src_fn: &str, dst_fn: &str, p_mults: [f64; 12]) -> Result<()> {
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
            let tmax = tokens[7];
            let tmin = tokens[8];
            let rad = tokens[9];
            let w_vl = tokens[10];
            let w_dir = tokens[11];
            let tdew = tokens[12];

            let indx = (mo - 1) as usize;
            prcp_f = prcp_f * p_mults[indx];

            let prcp = format!("{:.1}", prcp_f);

            dst_w.write_all(
                format!(
                    "{:>3}{:>3}{:>5}{:>6}{:>6}{:>5}{:>7}{:>6}{:>6}{:>5}{:>5}{:>6}{:>6}\n",
                    da, mo, year, prcp, dur, tp, ip, tmax, tmin, rad, w_vl, w_dir, tdew
                )
                .as_bytes(),
            )?;
        }
        line.clear();
    }
    Ok(())
}

#[pyfunction]
pub fn rust_cli_p_scale(src_fn: &str, dst_fn: &str, p_mult: f64) -> Result<()> {
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
            let mo = tokens[1];
            let year = tokens[2];
            let mut prcp_f: f64 = tokens[3].parse().unwrap();
            let dur = tokens[4];
            let tp = tokens[5];
            let ip = tokens[6];
            let tmax = tokens[7];
            let tmin = tokens[8];
            let rad = tokens[9];
            let w_vl = tokens[10];
            let w_dir = tokens[11];
            let tdew = tokens[12];

            prcp_f = prcp_f * p_mult;
            let prcp = format!("{:.1}", prcp_f);

            dst_w.write_all(
                format!(
                    "{:>3}{:>3}{:>5}{:>6}{:>6}{:>5}{:>7}{:>6}{:>6}{:>5}{:>5}{:>6}{:>6}\n",
                    da, mo, year, prcp, dur, tp, ip, tmax, tmin, rad, w_vl, w_dir, tdew
                )
                .as_bytes(),
            )?;
        }
        line.clear();
    }
    Ok(())
}

#[pyfunction]
pub fn rust_cli_calculate_p_annual_monthlies_from_lists(
    months: PyReadonlyArray1<i32>,
    precips: PyReadonlyArray1<f64>,
) -> PyResult<Vec<f64>> {
    let mut out: Vec<f64> = Vec::new();

    let mut mo_last: i32 = 0;
    let mut indx: i32 = 0;

    let mut prcp_sum: f64 = 0.0;
    let mut n_days: i32 = 0;

    for (i, &mo) in months.as_slice()?.iter().enumerate() {
        let prcp_f = precips.as_slice()?[i];

        if indx == 0 {
            mo_last = mo;
        }

        if mo != mo_last {
            out.push(prcp_sum / n_days as f64);
            mo_last = mo;
            prcp_sum = 0.0;
            n_days = 0;
        }

        prcp_sum += prcp_f;
        n_days += 1;
        indx += 1;
    }

    out.push(prcp_sum / n_days as f64);

    Ok(out)
}

#[pyfunction]
pub fn rust_cli_calculate_p_annual_monthlies(src_fn: &str) -> PyResult<Vec<f64>> {
    let src_f = File::open(src_fn)?;
    let mut src_r = BufReader::new(src_f);

    let mut line = String::new();
    for _ in 0..HEADER_LINES {
        src_r.read_line(&mut line)?;
        line.clear();
    }

    let mut out: Vec<f64> = Vec::new();

    let mut mo_last: i32 = 0;
    let mut indx: i32 = 0;

    let mut prcp_sum: f64 = 0.0;
    let mut n_days: i32 = 0;
    while src_r.read_line(&mut line)? > 0 {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        if tokens.len() == EXPECTED_TOKENS {
            let mo: i32 = tokens[1].parse().unwrap();

            if indx == 0 {
                mo_last = mo;
            }

            if mo != mo_last {
                out.push(prcp_sum / n_days as f64);
                mo_last = mo;
                prcp_sum = 0.0;
                n_days = 0;
            }

            let prcp_f: f64 = tokens[3].parse().unwrap();
            prcp_sum += prcp_f;
            n_days += 1;
            indx += 1;
        }
        line.clear();
    }

    out.push(prcp_sum / n_days as f64);

    Ok(out)
}

pub fn c_to_f(c: f64) -> f64 {
    c * 9.0 / 5.0 + 32.0
}

#[pyfunction]
pub fn rust_cli_calculate_monthlies(src_fn: &str) -> PyResult<[[f64; 12]; 4]> {
    let src_f = File::open(src_fn)?;
    let mut src_r = BufReader::new(src_f);

    let mut line = String::new();
    for _ in 0..HEADER_LINES {
        src_r.read_line(&mut line)?;
        line.clear();
    }

    let mut out: [[f64; 12]; 4] = [[0.0; 12]; 4];

    let mut yr_last: i32 = -1;
    let mut n_years: f64 = 0.0;

    while src_r.read_line(&mut line)? > 0 {
        let tokens: Vec<&str> = line.split_whitespace().collect();

        if tokens.len() == EXPECTED_TOKENS {
            let mo: usize = tokens[1].parse().unwrap();
            let year: i32 = tokens[2].parse().unwrap();
            let prcp_f: f64 = tokens[3].parse().unwrap();
            let tmax_f: f64 = tokens[7].parse().unwrap();
            let tmin_f: f64 = tokens[8].parse().unwrap();

            if year != yr_last {
                n_years += 1.0;
            }

            out[0][mo - 1] += prcp_f;
            out[1][mo - 1] += tmax_f;
            out[2][mo - 1] += tmin_f;

            if prcp_f > 0.0 {
                out[3][mo - 1] += 1.0; // nwds
            }

            yr_last = year;
        }
        line.clear();
    }

    let days_in_mo: [f64; 12] = [
        31.0, 28.25, 31.0, 30.0, 31.0, 30.0, 31.0, 31.0, 30.0, 31.0, 30.0, 31.0,
    ];

    for i in 0..12 {
        out[0][i] /= n_years;
        out[0][i] *= 0.0393701; // convert to inches/month

        out[1][i] /= n_years * days_in_mo[i];
        out[1][i] = c_to_f(out[1][i]);

        out[2][i] /= n_years * days_in_mo[i];
        out[2][i] = c_to_f(out[2][i]);

        out[3][i] /= n_years;
    }

    Ok(out)
}

#[pyfunction]
pub fn rust_cli_p_scale_annual_monthlies(
    src_fn: &str,
    dst_fn: &str,
    p_mults: Vec<f64>,
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

    let mut mo_last: i32 = 0;
    let mut indx: i32 = 0;
    let mut month_index: usize = 0;

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
            let tmax = tokens[7];
            let tmin = tokens[8];
            let rad = tokens[9];
            let w_vl = tokens[10];
            let w_dir = tokens[11];
            let tdew = tokens[12];

            if indx == 0 {
                mo_last = mo;
            }

            if mo != mo_last {
                month_index += 1;
            }

            prcp_f = prcp_f * p_mults[month_index];

            let prcp = format!("{:.1}", prcp_f);

            dst_w.write_all(
                format!(
                    "{:>3}{:>3}{:>5}{:>6}{:>6}{:>5}{:>7}{:>6}{:>6}{:>5}{:>5}{:>6}{:>6}\n",
                    da, mo, year, prcp, dur, tp, ip, tmax, tmin, rad, w_vl, w_dir, tdew
                )
                .as_bytes(),
            )?;

            mo_last = mo;
            indx += 1;
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
    hill_tmins: Vec<f64>,
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
            Err(pyo3::exceptions::PyValueError::new_err(
                "Expected a list of length 12",
            ))
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
        convert_array(hill_tmins)?,
    )
    .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{}", e)))?;

    Ok(())
}

/// A PyO3 module
/// This module is a container for the Python-callable functions we define
#[pymodule]
fn cli_revision_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(cli_revision, m)?)?;
    m.add_function(wrap_pyfunction!(make_rhem_storm_file, m)?)?;
    m.add_function(wrap_pyfunction!(build_hyetograph_non_breakpoint, m)?)?;
    m.add_function(wrap_pyfunction!(build_hyetograph_breakpoint, m)?)?;
    m.add_function(wrap_pyfunction!(compute_peak_intensities_from_hyetograph, m)?)?;
    m.add_function(wrap_pyfunction!(compute_peak_intensities_non_breakpoint, m)?)?;
    m.add_function(wrap_pyfunction!(compute_peak_intensities_breakpoint, m)?)?;
    m.add_function(wrap_pyfunction!(compute_static_r_from_cli, m)?)?;
    m.add_function(wrap_pyfunction!(interpolate_geospatial, m)?)?;
    m.add_function(wrap_pyfunction!(rust_cli_p_scale_monthlies, m)?)?;
    m.add_function(wrap_pyfunction!(rust_cli_p_scale, m)?)?;
    m.add_function(wrap_pyfunction!(rust_cli_calculate_monthlies, m)?)?;
    m.add_function(wrap_pyfunction!(
        rust_cli_calculate_p_annual_monthlies_from_lists,
        m
    )?)?;
    m.add_function(wrap_pyfunction!(rust_cli_calculate_p_annual_monthlies, m)?)?;
    m.add_function(wrap_pyfunction!(rust_cli_p_scale_annual_monthlies, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_cli_path(filename: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(filename);
        path
    }

    #[test]
    fn non_breakpoint_peak_intensities_are_positive_for_rain_event() {
        let segments = build_hyetograph_non_breakpoint_segments(40.0, 2.0, 0.3, 3.5, 0.70, 5.0)
            .expect("segment build should succeed");
        assert!(!segments.is_empty());

        let peaks = compute_peak_intensities_from_segments_internal(
            &segments,
            40.0,
            2.0,
            &[10, 15, 30, 60],
            5.0,
        )
        .expect("peak intensity computation should succeed");
        assert_eq!(peaks.len(), 4);
        assert!(peaks.iter().all(|value| *value > 0.0));
    }

    #[test]
    fn breakpoint_segments_produce_expected_constant_intensity() {
        let times = vec![0.25, 0.50, 1.00];
        let depths = vec![5.0, 10.0, 20.0];

        let segments = build_hyetograph_breakpoint_segments(&times, &depths)
            .expect("breakpoint segment build should succeed");
        assert_eq!(segments.len(), 3);
        for segment in &segments {
            assert!((segment.intensity_mm_hr - 20.0).abs() < 1.0e-9);
        }

        let peaks = compute_peak_intensities_from_segments_internal(
            &segments,
            20.0,
            1.0,
            &[10, 30, 60],
            5.0,
        )
        .expect("peak intensity computation should succeed");
        assert_eq!(peaks.len(), 3);
        assert!((peaks[0] - 20.0).abs() < 1.0e-6);
        assert!((peaks[1] - 20.0).abs() < 1.0e-6);
        assert!((peaks[2] - 20.0).abs() < 1.0e-6);
    }

    #[test]
    fn static_r_from_cli_returns_annual_totals() {
        let cli_path = temp_cli_path("wepppyo3_static_r_test.cli");
        let cli_text = "\
5.32300
   1   0   0
  Station:  TEST STATION                               CLIGEN VER. 5.32300 -r:    0 -I: 2
 Latitude Longitude Elevation (m) Obs. Years   Beginning year  Years simulated Command Line:
    34.18  -118.57         240          40        1980             2          -itest.par -Ows.prn -owepp.cli -t6 -I2
 Observed monthly ave max temperature (C)
  20.0 20.0 20.0 20.0 20.0 20.0 20.0 20.0 20.0 20.0 20.0 20.0
 Observed monthly ave min temperature (C)
  10.0 10.0 10.0 10.0 10.0 10.0 10.0 10.0 10.0 10.0 10.0 10.0
 Observed monthly ave solar radiation (Langleys/day)
 200.0 200.0 200.0 200.0 200.0 200.0 200.0 200.0 200.0 200.0 200.0 200.0
 Observed monthly ave precipitation (mm)
  50.0 50.0 50.0 50.0 50.0 50.0 50.0 50.0 50.0 50.0 50.0 50.0
 da mo year  prcp  dur   tp     ip  tmax  tmin  rad  w-vl w-dir  tdew
             (mm)  (h)               (C)   (C) (l/d) (m/s)(Deg)   (C)
  1  1 1980  20.0  1.00  0.30  2.0  20.0  10.0  200  2.0   180   8.0
  1  1 1981  20.0  1.00  0.30  2.0  20.0  10.0  200  2.0   180   8.0
";
        fs::write(&cli_path, cli_text).expect("write test CLI");

        let result = compute_static_r_from_cli_internal(
            cli_path.to_string_lossy().as_ref(),
            0.70,
            5.0,
            12.5,
        )
        .expect("static R computation should succeed");

        assert_eq!(result.annual_ei30.len(), 2);
        assert!(result.annual_ei30.values().all(|value| *value > 0.0));
        assert!(result.mean_annual_r > 0.0);
        assert_eq!(result.storms_total, 2);
        assert_eq!(result.storms_used, 2);

        let _ = fs::remove_file(cli_path);
    }
}

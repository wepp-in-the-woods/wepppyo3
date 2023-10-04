use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write, BufRead, Result};

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
fn cli_revision_rust(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(cli_revision, m)?)?;
    Ok(())
}


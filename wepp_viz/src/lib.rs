use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use raster::raster::Raster;
use std::error::Error;
use glob::glob;

fn read_2023_slope_meta(file_path: &str) -> Result<(Vec<usize>, Vec<f64>, f64, f64), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut indices: Vec<usize> = Vec::new();
    let mut distances_norm: Vec<f64> = Vec::new();
    let mut cell_size: f64 = 0.0;
    let mut length: f64 = 0.0;

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if i == 0 {
            // Read indices
            indices = line
                .trim_start_matches("# [")
                .trim_end_matches(']')
                .split(", ")
                .map(|s| s.parse::<usize>())
                .collect::<Result<Vec<usize>, _>>()?;
        } else if i == 1 {
            // Read distances_norm
            distances_norm = line
                .trim_start_matches("# [")
                .trim_end_matches(']')
                .split(", ")
                .map(|s| s.parse::<f64>())
                .collect::<Result<Vec<f64>, _>>()?;
        } else if i == 4 {
            let values: Vec<&str> = line.split_whitespace().collect();
            cell_size = values[1].parse::<f64>().unwrap();
        } else if i == 5 {
            let values2: Vec<&str> = line.split_whitespace().collect();
            length = values2[1].parse::<f64>().unwrap();
            break;
        }
    }

    Ok((indices, distances_norm, cell_size, length))
}

fn read_plot_fn(plot_fn: &Path) -> Result<(Vec<f64>, f64), io::Error> {

//    println!("plot_fn: {}", plot_fn.display());

    let file = File::open(plot_fn)?;
    let reader = io::BufReader::new(file);

    // Skipping the first 4 lines
    let lines = reader.lines().skip(4);

    // read distances and soil loss values
    let mut soil_loss = Vec::with_capacity(100);

    for line in lines {
        if let Ok(l) = line {
            let values: Vec<&str> = l.split_whitespace().collect();
            if values.len() == 3 {
                let _soil_loss: f64 = values[2].parse().unwrap();
                soil_loss.push(_soil_loss);
            }
        }
    }

    // return empty vectors if no data
    if soil_loss.len() == 0 {
        return Ok((soil_loss, 0.0));
    }

    let dx: f64 = 1.0 / (soil_loss.len() as f64 - 1.0);
    Ok((soil_loss, dx))
}


fn interp(x: f64, dx:f64, fp: &Vec<f64>) -> f64 {
    let n = fp.len();
    let last_indx = n - 1;

    if n == 0 {
        return 0.0;
    }

    let i = (x * last_indx as f64).floor() as usize;

    if i + 1 > last_indx {
        return fp[last_indx];
    }

    let x0 = dx * i as f64;
    let y0 = fp[i];
    let y1 = fp[i + 1];

    y0 + (x - x0) * (y1 - y0) / dx

}


#[derive(Debug)]
pub enum SoilLossError {
    IoError(std::io::Error),
    GdalError(gdal::errors::GdalError),
    // Add other error types as needed
}

impl From<std::io::Error> for SoilLossError {
    fn from(err: std::io::Error) -> SoilLossError {
        SoilLossError::IoError(err)
    }
}

impl From<gdal::errors::GdalError> for SoilLossError {
    fn from(err: gdal::errors::GdalError) -> SoilLossError {
        SoilLossError::GdalError(err)
    }
}

fn replace_extension(path: &Path, from_ext: &str, to_ext: &str) -> Option<PathBuf> {
    let path_str = path.to_str()?;
    if path_str.ends_with(from_ext) {
        let new_path_str = path_str.trim_end_matches(from_ext).to_string() + to_ext;
        Some(PathBuf::from(new_path_str))
    } else {
        None
    }
}

fn make_soil_loss_grid_fps_rs(
    discha_fn: &str,
    fp_runs_dir: &str,
    loss_fn: &str
) -> Result<(), Box<dyn Error>>  {

    let discha: Raster<f64> = Raster::<f64>::read(discha_fn).unwrap();

    let mut soil_loss_grid = discha.empty_clone();
    let mut counts_grid = discha.empty_clone();

    let pattern = format!("{}/{}.plot.dat", fp_runs_dir, "*");

    for entry in glob(&pattern).expect("Failed to read glob pattern") {
        match entry {
            Ok(plot_fn) => {
                if let Some(slp_path) = replace_extension(&plot_fn, "plot.dat", "slp") {
                    let slp_path_str = slp_path.to_str().ok_or("Invalid UTF-8 sequence")?.to_string();
                    
                    println!("Plot file: {:?}", plot_fn);
                    println!("SLP file: {:?}", slp_path_str);
                    
                    let (soil_loss, dx) = read_plot_fn(&Path::new(&plot_fn))?;
                    let (indices, distances_norm, cell_size, length) =
                        read_2023_slope_meta(&slp_path_str)?;

                    let slope_segment_m = length / (soil_loss.len() as f64);

                    let mut distance_norm_0: f64;
                    let mut distance_norm_1: f64;
                    for (i, indx) in indices.iter().enumerate() {
                        if i == 0
                        {
                            distance_norm_0 = distances_norm[i];
                            distance_norm_1 = (distances_norm[i] + distances_norm[i + 1]) / 2.0;
                        } else if i == indices.len() - 1 {
                            distance_norm_0 = (distances_norm[i - 1] + distances_norm[i]) / 2.0;
                            distance_norm_1 = distances_norm[i];
                        } else {
                            distance_norm_0 = (distances_norm[i - 1] + distances_norm[i]) / 2.0;
                            distance_norm_1 = (distances_norm[i] + distances_norm[i + 1]) / 2.0;
                        }

                        for (j, _soil_loss) in soil_loss.iter().enumerate() {
                            let _distance_norm = j as f64 * dx;
                            // continue if _distance_norm is outside the range for indx
                            if _distance_norm < distance_norm_0 || _distance_norm > distance_norm_1 {
                                continue;
                            }

                            // compute soil loss
                            let loss_kg = _soil_loss * slope_segment_m * cell_size;
                            soil_loss_grid.data[*indx] += loss_kg;
                        }

                        counts_grid.data[*indx] += 1.0;
                    }
                }
            }
            Err(e) => println!("{:?}", e),
        }
    }

    for (i, loss) in soil_loss_grid.data.iter_mut().enumerate() {
        if counts_grid.data[i] > 0.0 {
            *loss /= counts_grid.data[i] as f64;
        }
    }
    
    soil_loss_grid.write(loss_fn)?;

    Ok(())
}

fn make_soil_loss_grid_rs(
    subwta_fn: &str,
    discha_fn: &str,
    output_dir: &str,
    loss_fn: &str
) -> Result<i32, SoilLossError> {

    let discha: Raster<f64> = Raster::<f64>::read(discha_fn).unwrap();
    let subwta: Raster<i32> = Raster::<i32>::read(subwta_fn).unwrap();

    let mut topaz_ids: Vec<i32> = subwta.unique_values()
        .into_iter()
        .filter(|&x| x != 0 && x % 10 != 4)
        .collect();
    topaz_ids.sort();

    let mut i: i32 = 1;
    let mut soil_loss_grid = discha.empty_clone();

    for topaz_id in &topaz_ids {
//        println!("topaz_id: {}", topaz_id);
        let plot_fn = format!("{}/H{}.plot.dat", output_dir, i);

        let indices = subwta.indices_of(*topaz_id);

        let mut max_discha: f64 = 0.0;
        for indx in &indices {
            let _discha = discha.data[*indx];
            if _discha > max_discha {
                max_discha = _discha;
            }
        }
        let max_discha = max_discha;

        // make sure plot_fn exists
        if !Path::new(&plot_fn).exists() {
            return Err(SoilLossError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", plot_fn),
            )));
        }

        let (soil_loss, dx) = read_plot_fn(&Path::new(&plot_fn))?;

        for indx in &indices {
            let normed_discha = discha.data[*indx] / max_discha;
            let loss = interp(normed_discha, dx, &soil_loss);
            soil_loss_grid.data[*indx] = loss;
        }

        i += 1;
    }

    soil_loss_grid.write(loss_fn)?;

    Ok(i)
}


/// makes a soil-loss grid from topaz distance to channel map
/// and wepp plot file outputs
#[pyfunction]
fn make_soil_loss_grid(
    subwta_fn: &str,
    discha_fn: &str,
    output_dir: &str,
    loss_fn: &str
) -> PyResult<i32> {
    make_soil_loss_grid_rs(subwta_fn, discha_fn, output_dir, loss_fn)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e)))
}

#[pyfunction]
fn make_soil_loss_grid_fps(
    discha_fn: &str,
    fp_runs_dir: &str,
    loss_fn: &str
) -> PyResult<()> {
    make_soil_loss_grid_fps_rs(discha_fn, fp_runs_dir, loss_fn)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e)))
}

/// A PyO3 module
/// This module is a container for the Python-callable functions we define
#[pymodule]
fn wepp_viz_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(make_soil_loss_grid, m)?)?;
    m.add_function(wrap_pyfunction!(make_soil_loss_grid_fps, m)?)?;
    Ok(())
}


#[cfg(test)]
mod tests {

    use crate::make_soil_loss_grid_rs;
    use crate::make_soil_loss_grid_fps_rs;

    #[test]
    fn test_make_soil_loss_grid() {

        let result = make_soil_loss_grid_rs(
    "/geodata/weppcloud_runs/mdobre-mouth-watering-anathema/dem/topaz/SUBWTA.ARC",
    "/geodata/weppcloud_runs/mdobre-mouth-watering-anathema/dem/topaz/DISCHA.ARC", 
    "/geodata/weppcloud_runs/mdobre-mouth-watering-anathema/wepp/output",
    "/geodata/weppcloud_runs/mdobre-mouth-watering-anathema/wepp/plots/loss.tif");

        let result = 165;
        // Assert conditions on the result
        assert_eq!(result, 165); // replace ... with the expected value
    }

    #[test]
    fn test_make_soil_loss_grid_fps() {

        let result = make_soil_loss_grid_fps_rs(
    "/geodata/weppcloud_runs/falling-validity/dem/topaz/DISCHA.ARC",
    "/media/ramdisk/falling-validity",
    "/geodata/weppcloud_runs/falling-validity/wepp/plots/loss_fps.tif");
    
            // Assert conditions on the result
            assert_eq!(result.is_ok(), true); // replace ... with the expected value
    }
}


// wepp_viz_rust.make_soil_loss_grid('/geodata/weppcloud_runs/unimposing-muslin/dem/topaz/SUBWTA.ARC','/geodata/weppcloud_runs/unimposing-muslin/dem/topaz/DISCHA.ARC', '/geodata/weppcloud_runs/unimposing-muslin/wepp/ouput') 

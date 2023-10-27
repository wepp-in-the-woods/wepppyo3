use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use raster::raster::Raster;


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


/// A PyO3 module
/// This module is a container for the Python-callable functions we define
#[pymodule]
fn wepp_viz_rust(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(make_soil_loss_grid, m)?)?;
    Ok(())
}


#[cfg(test)]
mod tests {

    use crate::make_soil_loss_grid_rs;

    #[test]
    fn test_make_soil_loss_grid() {

        let result = make_soil_loss_grid_rs(
    "/geodata/weppcloud_runs/mdobre-womanly-ascot/dem/topaz/SUBWTA.ARC",
    "/geodata/weppcloud_runs/mdobre-womanly-ascot/dem/topaz/DISCHA.ARC", 
    "/geodata/weppcloud_runs/mdobre-womanly-ascot/wepp/output",
    "/home/roger/loss.tif");


        let result = 165;
        // Assert conditions on the result
        assert_eq!(result, 165); // replace ... with the expected value
    }
}


// wepp_viz_rust.make_soil_loss_grid('/geodata/weppcloud_runs/unimposing-muslin/dem/topaz/SUBWTA.ARC','/geodata/weppcloud_runs/unimposing-muslin/dem/topaz/DISCHA.ARC', '/geodata/weppcloud_runs/unimposing-muslin/wepp/ouput') 

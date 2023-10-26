use std::fmt;
use std::error::Error;
use std::collections::HashSet;

use core::any::Any;

use gdal::errors::GdalError;
use gdal::raster::GdalType;
use gdal::spatial_ref::SpatialRef;

use std::str::FromStr;


use proj::Proj;


/// Computes the circular mean of a slice of angles in radians.
///
/// The circular mean is calculated using the trigonometric
/// representation of the angles. The function expects angles
/// to be in radians and returns the mean angle in radians.
///
/// # Arguments
///
/// * `angles` - A slice of angles in radians.
///
/// # Returns
///
/// Returns the circular mean of the given angles in radians.
#[allow(dead_code)]
pub fn circmean(angles: &[f64]) -> f64 {
    let mut sum_sin = 0.0;
    let mut sum_cos = 0.0;

    for &angle in angles {
        sum_sin += angle.sin();
        sum_cos += angle.cos();
    }

    sum_sin /= angles.len() as f64;
    sum_cos /= angles.len() as f64;

    sum_sin.atan2(sum_cos)
}

fn transform_coords(x: f64, y: f64, s_srs: &str, t_srs: &str) -> Result<(f64, f64), Box<dyn Error>> {
    let transformer: Proj= Proj::new_known_crs(&s_srs, &t_srs, None)?;
    Ok(transformer.convert((x, y))?)
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapType {
    BOUND,
    CHNJNT,
    DISCHA,
    DISOUT,
    ELDCHA,
    ELDOUT,
    FLOPAT,
    FLOVEC,
    FVSLOP,
    NETFUL,
    NETW,
    NETWE,
    RELIEF,
    SUBWTA,
    TASPEC,
    UPAREA,
    OTHER,
}

impl FromStr for MapType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BOUND" => Ok(MapType::BOUND),
            "CHNJNT" => Ok(MapType::CHNJNT),
            "DISCHA" => Ok(MapType::DISCHA),
            "DISOUT" => Ok(MapType::DISOUT),
            "ELDCHA" => Ok(MapType::ELDCHA),
            "ELDOUT" => Ok(MapType::ELDOUT),
            "FLOPAT" => Ok(MapType::FLOPAT),
            "FLOVEC" => Ok(MapType::FLOVEC),
            "FVSLOP" => Ok(MapType::FVSLOP),
            "NETFUL" => Ok(MapType::NETFUL),
            "NETW" => Ok(MapType::NETW),
            "NETWE" => Ok(MapType::NETWE),
            "RELIEF" => Ok(MapType::RELIEF),
            "SUBWTA" => Ok(MapType::SUBWTA),
            "TASPEC" => Ok(MapType::TASPEC),
            "UPAREA" => Ok(MapType::UPAREA),
            _ => Ok(MapType::OTHER),
        }
    }
}


#[derive(Debug)]
pub struct Raster<T> {
    pub width: usize,
    pub height: usize,
    pub cellsize: f64,
    pub data: Vec<T>,
    pub no_data: Option<T>,
    pub geo_transform: [f64; 6],
    pub proj4: Option<String>,
    pub path: String,
    pub name: String,
    pub map_type: MapType,
    pub wgs_transform: [f64; 4],
}

// impl new for Raster<T> without wgs_transform
impl<T> Raster<T> {

    #[allow(dead_code)]
    pub fn new(
        width: usize,
        height: usize,
        cellsize: f64,
        data: Vec<T>,
        no_data: Option<T>,
        geo_transform: [f64; 6],
        proj4: Option<String>,
        path: String,
        name: String,
        map_type: MapType,
    ) -> Raster<T> {
        // check if proj4 is not None and build Proj transformer to wgs 84 epsg:4326
        let wgs_transform = match &proj4 {  // Borrow here instead of moving
            Some(proj_str) => {
                // find easting and northing of bottom left corner using geo_transform
                let ll_x: f64 = geo_transform[0];
                let ll_y: f64 = geo_transform[3] + height as f64 * geo_transform[5];

                // find easting and northing of top right corner using geo_transform
                let ur_x: f64 = geo_transform[0] + width as f64 * geo_transform[1];
                let ur_y: f64 = geo_transform[3];

                // transform ll_x, ll_y, ur_x, ur_y to wgs 84 epsg:4326
                let ll_wgs: (f64, f64) = transform_coords(ll_x, ll_y, &proj_str, "+proj=longlat +datum=WGS84 +no_defs").unwrap();
                let ur_wgs: (f64, f64) = transform_coords(ur_x, ur_y, &proj_str, "+proj=longlat +datum=WGS84 +no_defs").unwrap();

                // build wgs_transform to approximate wgs coords from px coords (x, y)
                // (0, 0) is upper left corner
                // lon = ll_wgs.0 + x * (ur_wgs.0 - ll_wgs.0) / width
                // lat = ur_wgs.1 - y * (ur_wgs.1 - ll_wgs.1) / height
                [ll_wgs.0, ur_wgs.1, (ur_wgs.0 - ll_wgs.0) / width as f64, (ur_wgs.1 - ll_wgs.1) / height as f64]
        
            },
            None => [0.0, 0.0, 0.0, 0.0],
        };

        Raster {
            width: width,
            height: height,
            cellsize: cellsize,
            data: data,
            no_data: no_data,
            geo_transform: geo_transform,
            proj4: proj4,
            path: path,
            name: name,
            map_type: map_type,
            wgs_transform: wgs_transform,
        }
    }
}

impl<T: Clone> Clone for Raster<T> {
    fn clone(&self) -> Self {
        Raster {
            width: self.width,
            height: self.height,
            cellsize: self.cellsize,
            data: self.data.clone(),
            no_data: self.no_data.clone(),
            geo_transform: self.geo_transform,
            proj4: self.proj4.clone(),
            path: self.path.clone(),
            name: self.name.clone(),
            map_type: self.map_type.clone(),
            wgs_transform: self.wgs_transform.clone(),
        }
    }
}

pub fn px_to_wgs(wgs_transform: &[f64; 4], px: i32, py: i32) -> (f64, f64) {
    let lon: f64 = wgs_transform[0] + px as f64 * wgs_transform[2];
    let lat: f64 = wgs_transform[1] - py as f64 * wgs_transform[3];
    (lon, lat)
}

pub trait FromF64 {
    fn from_f64(value: f64) -> Self;
}

impl FromF64 for i32 {
    fn from_f64(value: f64) -> Self {
        value as i32 // Do your conversion here
    }
}

impl FromF64 for f64 {
    fn from_f64(value: f64) -> Self {
        value
    }
}


pub trait ToF64 {
    fn to_f64(&self) -> f64;
}

impl ToF64 for i32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for f64 {
    fn to_f64(&self) -> f64 {
        *self
    }
}


impl<T> Raster<T>
where
    T: ToF64, // Constraint for types that can be converted to f64
{
    #[allow(dead_code)]
    fn convert_data_to_f64(&self) -> Vec<f64> {
        self.data.iter().map(|value| value.to_f64()).collect()
    }
}


impl<T: GdalType + Default + Copy + FromF64> Raster<T> {

    #[allow(dead_code)]
    pub fn read(path: &str) -> Result<Raster<T>, GdalError> {
        let dataset = gdal::Dataset::open(path)?;
        let (width, height) = dataset.raster_size();
        let geo_transform = dataset.geo_transform()?;
        let cellsize = geo_transform[1];

        let wkt = dataset.projection();
        let spatial_ref = SpatialRef::from_wkt(&wkt).unwrap();
        let proj4 = spatial_ref.to_proj4().ok();

        //let spatial_ref_result = dataset.spatial_ref();
        //let proj4 = match spatial_ref_result {
        //    Ok(sr) => sr.to_proj4().ok(),
        //    Err(_) => None,
        //};

        let band = dataset.rasterband(1)?;
        let buffer = band.read_as::<T>((0, 0), (width, height), (width, height), None)?;
        let data = buffer.data;

        let no_data_value: Option<f64> = band.no_data_value();
        let no_data = no_data_value.map(|v| T::from_f64(v));

        // find the name by spliting the path and removing the extension from the filename of the file
        let name = path.split("/").last().unwrap().split(".").next().unwrap().to_string();

        // find the map type from the name using from_str
        let map_type = MapType::from_str(&name).unwrap();

        // refactor to use Raster::new

        Ok(Raster::new(
            width,
            height,
            cellsize,
            data,
            no_data,
            geo_transform,
            proj4,
            path.to_string(),
            name,
            map_type,
        ))
    }

    #[allow(dead_code)]
    pub fn read_band(path: &str, band_indx: isize) -> Result<Raster<T>, GdalError> {
        let dataset = gdal::Dataset::open(path)?;
        let (width, height) = dataset.raster_size();
        let geo_transform = dataset.geo_transform()?;
        let cellsize = geo_transform[1];

        let wkt = dataset.projection();
        let spatial_ref = SpatialRef::from_wkt(&wkt).unwrap();
        let proj4 = spatial_ref.to_proj4().ok();

        //let spatial_ref_result = dataset.spatial_ref();
        //let proj4 = match spatial_ref_result {
        //    Ok(sr) => sr.to_proj4().ok(),
        //    Err(_) => None,
        //};

        let band = dataset.rasterband(band_indx)?;
        let buffer = band.read_as::<T>((0, 0), (width, height), (width, height), None)?;
        let data = buffer.data;

        let no_data_value: Option<f64> = band.no_data_value();
        let no_data = no_data_value.map(|v| T::from_f64(v));

        // find the name by spliting the path and removing the extension from the filename of the file
        let name = path.split("/").last().unwrap().split(".").next().unwrap().to_string();

        // find the map type from the name using from_str
        let map_type = MapType::from_str(&name).unwrap();

        // refactor to use Raster::new

        Ok(Raster::new(
            width,
            height,
            cellsize,
            data,
            no_data,
            geo_transform,
            proj4,
            path.to_string(),
            name,
            map_type,
        ))
    }

}

// method to transform usize index to x,y coordinates
impl<T> Raster<T> {
    #[inline(always)]
    pub fn index_to_xy(&self, index: usize) -> (usize, usize) {
        let x = index % self.width;
        let y = index / self.width;
        (x, y)
    }
}

// method to transform x,y coordinates to usize index
impl<T> Raster<T> {
    #[inline(always)]
    pub fn xy_to_index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
}

impl<T> Raster<T> {
    pub fn distance_between(&self, index1: usize, index2: usize) -> f64 {
        let (x1, y1) = self.index_to_xy(index1);
        let (x2, y2) = self.index_to_xy(index2);
        let x_diff = (x2 as f64) - (x1 as f64);
        let y_diff = (y2 as f64) - (y1 as f64);
        let distance = (x_diff.powi(2) + y_diff.powi(2)).sqrt() * self.cellsize;
        distance
    }
}

impl<T> Raster<T> {
    pub fn coordinates_of(&self, indices: &Vec<usize>) -> Vec<Vec<f64>> {
        let mut coords: Vec<Vec<f64>> = Vec::new();
        for index in indices {
            let (x, y) = self.index_to_xy(*index);
            // apply geotransform to x, y 
            let e: f64 = self.geo_transform[0] + x as f64 * self.geo_transform[1] + y as f64 * self.geo_transform[2];
            let n: f64 = self.geo_transform[3] + x as f64 * self.geo_transform[4] + y as f64 * self.geo_transform[5];
            coords.push(vec![e, n]);
        }
        coords
    }
}


impl<T: std::hash::Hash + Eq + Copy> Raster<T> {
    #[allow(dead_code)]
    pub fn mask(&self) -> Vec<bool> {

        let mut the_mask = Vec::new();

        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = self.xy_to_index(x, y);
                let value = &self.data[index];
                let mask_value = !(no_data.is_none() || value != no_data.unwrap());
                // add value to the_mask
                the_mask.push(mask_value);
            }
        }
        the_mask
    }
}


impl<T: std::hash::Hash + Eq + Copy> Raster<T> {
    pub fn unique_values(&self) -> HashSet<T> {

        let mut unique_values = HashSet::new();

        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    unique_values.insert(*value);
                }
            }
        }
        unique_values
    }
}

//impl<T: std::hash::Hash + Eq + Copy> Raster<T> {
impl Raster<i32> {
    pub fn indices_of(&self, target: i32) -> HashSet<usize> {

        let mut indices = HashSet::<usize>::new();

        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if *value == target {
                    indices.insert(index);
                }
            }
        }
        indices
    }
}

pub trait ToIndices {
    fn to_indices(&self) -> Vec<usize>;
}

impl ToIndices for HashSet<usize> {
    fn to_indices(&self) -> Vec<usize> {
        self.iter().cloned().collect()
    }
}

impl ToIndices for Vec<usize> {
    fn to_indices(&self) -> Vec<usize> {
        self.clone()
    }
}

impl<T> Raster<T> {

    #[allow(dead_code)]
    pub fn centroid_of<I: ToIndices>(&self, indices: &I) -> (usize, usize) {
        let indices_vec = indices.to_indices();
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;

        for &index in &indices_vec {
            let (x, y) = self.index_to_xy(index);
            sum_x += x as f64;
            sum_y += y as f64;
        }

        let num_points = indices_vec.len() as f64;
        let centroid_x = (sum_x / num_points).round() as usize;
        let centroid_y = (sum_y / num_points).round() as usize;
        
        (centroid_x, centroid_y)
    }

    #[allow(dead_code)]
    pub fn px_to_lnglat(&self, px: (usize, usize)) -> (f64, f64) {
        let e: f64 = self.geo_transform[0] + px.0 as f64 * self.geo_transform[1] + px.1 as f64 * self.geo_transform[2];
        let n: f64 = self.geo_transform[3] + px.0 as f64 * self.geo_transform[4] + px.1 as f64 * self.geo_transform[5];
    
        let (lng, lat) = transform_coords(e, n, &self.proj4.as_ref().unwrap(), "+proj=longlat +datum=WGS84 +no_defs").unwrap();
        (lng, lat)
    }
    
    
}


impl Raster<f64> {

    #[allow(dead_code)]
    pub fn determine_aspect<I: ToIndices>(&self, indices: &I) -> f64 {
        assert!(self.map_type == MapType::TASPEC, 
            "Raster must be TASPEC type to determine aspect");
    
        let indices_vec = indices.to_indices();
    
        let mut rad_aspects: Vec<f64> = Vec::new();
        for &index in &indices_vec {
            let deg_aspect = self.data[index];
            rad_aspects.push(deg_aspect.to_radians());
        }
        let mut aspect = circmean(rad_aspects.as_slice()).to_degrees();

        if aspect < 0.0 {
            aspect += 360.0;
        }
        aspect
    }
}


impl<T: ToF64> Raster<T> { 
    #[allow(dead_code)]
    pub fn compute_band_statistics(&self) -> BandStatistics {
        // Initialize stats variables. Normally, you'd get these values from your raster data.
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        let mut count = 0;
        let mut sum_of_squares = 0.0;

        let no_data: Option<f64> = self.no_data.as_ref().map(|v| v.to_f64());

        for &value_f64 in &self.convert_data_to_f64() {

            if value_f64 < min {
                min = value_f64;
            }

            if value_f64 > max {
                max = value_f64;
            }

            sum += value_f64;
            sum_of_squares += value_f64 * value_f64;
            if no_data.is_none() || value_f64 != no_data.unwrap() {
                count += 1;
            }
        }

        let mean = sum / count as f64;
        let variance = (sum_of_squares / count as f64) - (mean * mean);
        let std_dev = variance.sqrt();
        let valid_percent = 100.0 * (count as f64) / (self.width * self.height) as f64;

        BandStatistics {
            minimum: min,
            maximum: max,
            mean,
            std_dev,
            valid_percent,
        }
    }
}

impl<T: std::fmt::Display + std::cmp::PartialEq + Any> Raster<T> {
    
    #[allow(dead_code)]
    pub fn display_grid(&self) {
        match self.map_type {
            MapType::SUBWTA => self.display_grid_subwta(),
            MapType::NETFUL => self.display_grid_netful(),
            MapType::BOUND => self.display_grid_bound(),
            MapType::FLOVEC => self.display_grid_flowvec(),
            _ =>  self.display_grid_default()
        }
    }
    
    #[allow(dead_code)]
    fn display_grid_subwta(&self) {
        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    if let Some(int_value) = (value as &dyn Any).downcast_ref::<i32>() {
                        let remainder = *int_value % 10;
                        let color_code = match remainder {
                            0 => 31, // Red
                            1 => 33, // Yellow
                            2 => 32, // Green
                            3 => 35, // Magenta
                            4 => 34, // Blue
                            _ => 37, // White (for remainders 5 through 9)
                        };
                        print!("\x1b[{}m{:<4}\x1b[0m ", color_code, int_value);
                    } else {
                        // Just print the value normally if T isn't i32
                        print!("{:<4} ", value);
                    }
                } else {
                    print!("{:<4} ", ".");
                }
            }
            println!();
        }
    }

    #[allow(dead_code)]
    fn display_grid_bound(&self) {
        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    print!("\x1b[{}m{:<1}\x1b[0m ", 35, value);
                } else {
                    print!("{:<1} ", ".");
                }
            }
            println!();
        }
    }

    #[allow(dead_code)]
    fn display_grid_netful(&self) {
        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    print!("\x1b[{}m{:<1}\x1b[0m ", 34, value);
                } else {
                    print!("{:<1} ", ".");
                }
            }
            println!();
        }
    }

    #[allow(dead_code)]
    fn display_grid_flowvec(&self) {
        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    let character = match (value as &dyn Any).downcast_ref::<i32>() {
                        Some(1) => "↖",
                        Some(2) => "↑",
                        Some(3) => "↗",
                        Some(4) => "←",
                        Some(5) => "-",
                        Some(6) => "→",
                        Some(7) => "↙",
                        Some(8) => "↓",
                        Some(9) => "↘",
                        _ => " ",  // Default for non-matched values
                    };
                    print!("{:<1} ", character);
                } else {
                    print!("{:<1} ", ".");
                }
            }
            println!();
        }
    }

    #[allow(dead_code)]
    fn display_grid_default(&self) {
        let no_data = self.no_data.as_ref();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let value = &self.data[index];
                if no_data.is_none() || value != no_data.unwrap() {
                    print!("{:<4} ", value);
                } else {
                    print!("{:<4} ", ".");
                }
            }
            println!();
        }
    }
}


impl<T: fmt::Display> fmt::Display for Raster<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let no_data_str = match &self.no_data {
            Some(value) => value.to_string(),
            None => "-".to_string(),
        };

        let proj4_str = match &self.proj4 {
            Some(value) => value.to_string(),
            None => "-".to_string(),
        };

        write!(f, "Raster: {}\n Shape: {} x {}\nCellSize: {}\nTransform: {:?}\nNo Data: {}\nProj4: {}", 
               self.name, self.width, self.height, self.cellsize, self.geo_transform, no_data_str, proj4_str)
    }
}


#[derive(Debug)]
pub struct BandStatistics {
    minimum: f64,
    maximum: f64,
    mean: f64,
    std_dev: f64,
    valid_percent: f64,
}


impl fmt::Display for BandStatistics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Min: {}\nMax: {}\nMean: {}\nStd Dev: {}\nValid Percent: {}",
            self.minimum, self.maximum, self.mean, self.std_dev, self.valid_percent
        )
    }
}

#[cfg(test)]
mod tests {
    extern crate maplit;

    use super::Raster;  // Assuming Raster is in the parent module
    use std::collections::HashSet;
    use maplit::hashset;

    #[test]
    fn test_unique_values() {
        let path = "tests/fixtures/watershed_abstraction/litigious-sagacity/dem/topaz/SUBWTA.ARC";
        let raster = Raster::<i32>::read(&path).unwrap();
        let unique_vals = raster.unique_values();

        let expected = hashset!{21, 22, 23, 24};

        assert_eq!(unique_vals, expected);
    }

    #[test]
    fn test_indices_of() {
        let path = "tests/fixtures/watershed_abstraction/litigious-sagacity/dem/topaz/SUBWTA.ARC";
        let raster = Raster::<i32>::read(&path).unwrap();
        let indices = raster.indices_of(21);

        let expected = hashset!{377, 177, 240, 113, 277, 544, 280, 209, 347, 540, 272, 411, 373, 577, 149, 208, 412, 307, 371, 276, 305, 146, 278, 281, 345, 313, 507, 346, 372, 545, 147, 379, 407, 148, 340, 214, 246, 476, 215, 478, 210, 181, 243, 443, 375, 445, 343, 508, 511, 212, 409, 182, 341, 311, 342, 82, 576, 475, 344, 473, 512, 114, 541, 116, 115, 178, 339, 542, 440, 474, 506, 338, 273, 543, 444, 306, 413, 509, 410, 446, 378, 274, 376, 510, 405, 275, 575, 248, 179, 310, 241, 242, 312, 244, 145, 406, 314, 247, 479, 380, 83, 81, 245, 574, 279, 309, 408, 442, 477, 374, 180, 308, 472, 211, 439, 441, 213, 337};

        assert_eq!(indices, expected);
    }


    #[test]
    fn test_mask() {
        let path = "tests/fixtures/watershed_abstraction/small/SUBWTA.ARC";
        let raster = Raster::<i32>::read(&path).unwrap();
        let indices = raster.mask();

        let expected = vec![true, true, true, true, true, true, false, false, false, false, false, false, true, true, false, false];

        assert_eq!(indices, expected);
    }
}

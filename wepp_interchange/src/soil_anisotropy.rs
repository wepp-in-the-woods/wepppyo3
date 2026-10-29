//! SA-01: in-memory soil preparation; Python owns the single file read/write.
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn apply(
    ofes: Vec<Vec<(f64, Option<f64>)>>,
    upper: Option<f64>,
    lower: Option<f64>,
    developer_mode: bool,
) -> Result<Vec<Vec<Option<f64>>>, String> {
    for (name, value, maximum) in [("upper", upper, 100.0), ("lower", lower, 10.0)] {
        if let Some(value) = value {
            if !value.is_finite() || value < 1.0 || (!developer_mode && value > maximum) {
                if developer_mode {
                    return Err(format!(
                        "soil_anisotropy_{name} must be finite and at least 1"
                    ));
                }
                return Err(format!(
                    "soil_anisotropy_{name} must be finite and between 1 and {maximum}"
                ));
            }
        }
    }
    if ofes.is_empty() {
        return Err("Soil must contain at least one OFE".into());
    }
    ofes.into_iter()
        .enumerate()
        .map(|(ofe, horizons)| {
            if horizons.is_empty() {
                return Err(format!("OFE {ofe} has no horizons"));
            }
            let mut previous = 0.0;
            horizons
                .into_iter()
                .map(|(depth, original)| {
                    // Existing ordinary clipping can produce equal terminal depths.
                    if !depth.is_finite() || depth <= 0.0 || depth < previous {
                        return Err(format!(
                            "OFE {ofe} depths must be positive, finite and nondecreasing"
                        ));
                    }
                    previous = depth;
                    Ok(if depth <= 500.0 { upper } else { lower }.or(original))
                })
                .collect()
        })
        .collect()
}

/// Return ratios for each OFE/horizon without mutating inputs or opening files.
#[pyfunction]
#[pyo3(signature = (ofes, upper=None, lower=None, *, developer_mode=false))]
pub fn soil_anisotropy_values(
    ofes: Vec<Vec<(f64, Option<f64>)>>,
    upper: Option<f64>,
    lower: Option<f64>,
    developer_mode: bool,
) -> PyResult<Vec<Vec<Option<f64>>>> {
    apply(ofes, upper, lower, developer_mode).map_err(PyValueError::new_err)
}

#[cfg(test)]
mod tests {
    fn apply(
        ofes: Vec<Vec<(f64, Option<f64>)>>,
        upper: Option<f64>,
        lower: Option<f64>,
    ) -> Result<Vec<Vec<Option<f64>>>, String> {
        super::apply(ofes, upper, lower, false)
    }

    #[test]
    fn developer_overrides_keep_minimum_and_finite_checks() {
        let ofes = vec![vec![(500.0, Some(1.0)), (800.0, Some(2.0))]];
        assert_eq!(
            super::apply(ofes.clone(), Some(101.0), Some(11.0), true).unwrap(),
            vec![vec![Some(101.0), Some(11.0)]]
        );
        for value in [0.0, f64::NAN, f64::INFINITY] {
            assert!(super::apply(ofes.clone(), Some(value), None, true).is_err());
            assert!(super::apply(ofes.clone(), None, Some(value), true).is_err());
        }
    }

    #[test]
    fn boundaries_and_multiple_ofes() {
        let ofes = vec![vec![(100.0, Some(1.0)), (500.0, Some(2.0)), (800.0, Some(3.0))]; 2];
        assert_eq!(
            apply(ofes, Some(100.0), Some(10.0)).unwrap(),
            vec![vec![Some(100.0), Some(100.0), Some(10.0)]; 2]
        );
    }

    #[test]
    fn unchanged_source_values_and_clipped_duplicate_depths() {
        let ofes = vec![vec![
            (100.0, Some(0.0)),
            (500.0, Some(150.0)),
            (500.0, None),
            (800.0, Some(20.0)),
        ]];
        assert_eq!(
            apply(ofes.clone(), None, None).unwrap(),
            vec![vec![Some(0.0), Some(150.0), None, Some(20.0)]]
        );
        assert_eq!(apply(ofes, None, Some(1.0)).unwrap()[0][3], Some(1.0));
    }

    #[test]
    fn invalid_overrides_and_depths() {
        for value in [0.0, 100.1, f64::NAN, f64::INFINITY] {
            assert!(apply(vec![vec![(500.0, Some(1.0))]], Some(value), None).is_err());
        }
        assert!(apply(vec![vec![(500.0, Some(1.0))]], None, Some(10.1)).is_err());
        for depths in [vec![0.0], vec![f64::NAN], vec![600.0, 500.0]] {
            assert!(apply(
                vec![depths.into_iter().map(|depth| (depth, Some(1.0))).collect()],
                Some(1.0),
                None
            )
            .is_err());
        }
        assert!(apply(vec![], None, None).is_err());
        assert!(apply(vec![vec![]], None, None).is_err());
    }
}

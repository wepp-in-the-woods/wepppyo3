#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use numpy::{PyArray2, PyArrayMethods, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

#[pyfunction]
#[pyo3(signature = (subwta, discha, topaz_ids, distance_fractions_by_topaz))]
fn assign_mofe_map(
    py: Python<'_>,
    subwta: PyReadonlyArray2<'_, i32>,
    discha: PyReadonlyArray2<'_, i32>,
    topaz_ids: Vec<i32>,
    distance_fractions_by_topaz: HashMap<i32, Vec<f64>>,
) -> PyResult<Py<PyArray2<i32>>> {
    if topaz_ids.is_empty() {
        let shape = subwta.shape();
        let rows = shape[0];
        let cols = shape[1];
        let array = unsafe { PyArray2::<i32>::new_bound(py, [rows, cols], false) };
        unsafe {
            array
                .as_slice_mut()
                .map_err(|err| {
                    PyValueError::new_err(format!("unable to allocate output array: {err}"))
                })?
                .fill(0);
        }
        return Ok(array.unbind());
    }

    let subwta_shape = subwta.shape();
    let discha_shape = discha.shape();
    if subwta_shape != discha_shape {
        return Err(PyValueError::new_err(format!(
            "subwta/discha shape mismatch: subwta={subwta_shape:?}, discha={discha_shape:?}"
        )));
    }

    let mut unique_ids = HashSet::with_capacity(topaz_ids.len());
    for topaz_id in &topaz_ids {
        if !unique_ids.insert(*topaz_id) {
            return Err(PyValueError::new_err(format!(
                "topaz_ids must be unique; duplicate found: {topaz_id}"
            )));
        }
    }

    let rows = subwta_shape[0];
    let cols = subwta_shape[1];
    let cell_count = rows * cols;

    let subwta_view = subwta.as_array();
    let discha_view = discha.as_array();

    let mut subwta_flat = Vec::with_capacity(cell_count);
    let mut discha_flat = Vec::with_capacity(cell_count);
    for row in 0..rows {
        for col in 0..cols {
            subwta_flat.push(subwta_view[[row, col]]);
            discha_flat.push(discha_view[[row, col]]);
        }
    }

    let mut topaz_indices: HashMap<i32, Vec<usize>> = topaz_ids
        .iter()
        .copied()
        .map(|topaz_id| (topaz_id, Vec::new()))
        .collect();

    for (flat_idx, topaz_value) in subwta_flat.iter().copied().enumerate() {
        if let Some(indices) = topaz_indices.get_mut(&topaz_value) {
            indices.push(flat_idx);
        }
    }

    let mut mofe_flat = vec![0_i32; cell_count];

    for topaz_id in topaz_ids {
        let d_fractions = distance_fractions_by_topaz.get(&topaz_id).ok_or_else(|| {
            PyValueError::new_err(format!(
                "Missing MOFE distance fractions for topaz_id={topaz_id}"
            ))
        })?;

        let indices = topaz_indices.get(&topaz_id).ok_or_else(|| {
            PyValueError::new_err(format!("No topaz index bucket for topaz_id={topaz_id}"))
        })?;

        if indices.is_empty() {
            return Err(PyValueError::new_err(format!(
                "No subwta cells found for topaz_id={topaz_id} while building MOFE map"
            )));
        }

        let mut discha_vals = Vec::with_capacity(indices.len());
        for flat_idx in indices {
            discha_vals.push(discha_flat[*flat_idx]);
        }

        let labels = assign_hillslope_labels(topaz_id, &discha_vals, d_fractions)
            .map_err(PyValueError::new_err)?;

        if labels.len() != indices.len() {
            return Err(PyValueError::new_err(format!(
                "MOFE label count mismatch for topaz_id={topaz_id}: labels={}, cells={}",
                labels.len(),
                indices.len()
            )));
        }

        for (offset, flat_idx) in indices.iter().copied().enumerate() {
            mofe_flat[flat_idx] = labels[offset];
        }
    }

    let array = unsafe { PyArray2::<i32>::new_bound(py, [rows, cols], false) };
    unsafe {
        array
            .as_slice_mut()
            .map_err(|err| PyValueError::new_err(format!("unable to write output array: {err}")))?
            .copy_from_slice(&mofe_flat);
    }

    Ok(array.unbind())
}

fn assign_hillslope_labels(
    topaz_id: i32,
    discha_vals: &[i32],
    d_fractions: &[f64],
) -> Result<Vec<i32>, String> {
    validate_distance_fractions(d_fractions)?;

    let n_cells = discha_vals.len();
    if n_cells == 0 {
        return Err(format!(
            "No discharge cells found for topaz_id={topaz_id} while building MOFE map"
        ));
    }

    let n_ofe = d_fractions.len() - 1;
    if n_ofe == 0 {
        return Err(format!(
            "d_fractions must include at least two points; received {d_fractions:?}"
        ));
    }

    if n_ofe == 1 {
        return Ok(vec![1_i32; n_cells]);
    }

    let max_discha = discha_vals
        .iter()
        .copied()
        .max()
        .ok_or_else(|| format!("No discharge values available for topaz_id={topaz_id}"))?;

    let mut sorted_discha = discha_vals.to_vec();
    sorted_discha.sort_unstable();

    let mut labels = vec![0_i32; n_cells];
    for segment_idx in 0..n_ofe {
        let max_pct = (1.0 - d_fractions[segment_idx]) * 100.0;
        let min_pct = (1.0 - d_fractions[segment_idx + 1]) * 100.0;

        let min_value = percentile_sorted(&sorted_discha, min_pct)?;
        let max_value = percentile_sorted(&sorted_discha, max_pct)?;

        let current_label = (segment_idx + 1) as i32;
        let mut assigned_any = false;

        for cell_idx in 0..n_cells {
            if labels[cell_idx] == 0 {
                let value = discha_vals[cell_idx] as f64;
                if value >= min_value && value <= max_value {
                    labels[cell_idx] = current_label;
                    assigned_any = true;
                }
            }
        }

        if !assigned_any {
            let mut candidates: Vec<usize> = (0..n_cells).filter(|idx| labels[*idx] == 0).collect();
            if candidates.is_empty() {
                candidates = (0..n_cells).collect();
            }

            let target_value = (1.0 - d_fractions[segment_idx]) * (max_discha as f64);
            let mut best_idx = candidates[0];
            let mut best_diff = (target_value - (discha_vals[best_idx] as f64)).abs();

            for candidate in candidates.into_iter().skip(1) {
                let diff = (target_value - (discha_vals[candidate] as f64)).abs();
                if diff < best_diff {
                    best_diff = diff;
                    best_idx = candidate;
                }
            }

            labels[best_idx] = current_label;
        }
    }

    let mut mofe_ids = unique_nonzero(&labels);
    if mofe_ids.len() != n_ofe {
        labels = assign_mofe_ids_by_discharge_rank(discha_vals, d_fractions)?;
        mofe_ids = unique_nonzero(&labels);
    }

    if mofe_ids.len() != n_ofe {
        let expected = (1_i32..=(n_ofe as i32)).collect::<HashSet<_>>();
        let present = mofe_ids.iter().copied().collect::<HashSet<_>>();
        let mut missing = expected.difference(&present).copied().collect::<Vec<_>>();
        missing.sort_unstable();
        return Err(format!(
            "Unable to assign contiguous MOFE ids for topaz_id={topaz_id}: expected=1..{n_ofe} present={mofe_ids:?} missing={missing:?} cells={n_cells}"
        ));
    }

    Ok(labels)
}

fn validate_distance_fractions(d_fractions: &[f64]) -> Result<(), String> {
    if d_fractions.len() < 2 {
        return Err(format!(
            "d_fractions must include at least two points; received {d_fractions:?}"
        ));
    }

    for (idx, value) in d_fractions.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(format!(
                "d_fractions must contain finite values; index={idx} value={value}"
            ));
        }
    }

    for idx in 0..(d_fractions.len() - 1) {
        if d_fractions[idx + 1] < d_fractions[idx] {
            return Err(format!(
                "d_fractions must be non-decreasing; received {d_fractions:?}"
            ));
        }
    }

    Ok(())
}

fn percentile_sorted(sorted_values: &[i32], percentile: f64) -> Result<f64, String> {
    if !(0.0..=100.0).contains(&percentile) {
        return Err(format!(
            "Percentiles must be in the range [0, 100]; received {percentile}"
        ));
    }

    if sorted_values.is_empty() {
        return Err("cannot compute percentile of an empty array".to_string());
    }

    if sorted_values.len() == 1 {
        return Ok(sorted_values[0] as f64);
    }

    let rank = percentile / 100.0 * ((sorted_values.len() - 1) as f64);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;

    if lower == upper {
        return Ok(sorted_values[lower] as f64);
    }

    let lower_value = sorted_values[lower] as f64;
    let upper_value = sorted_values[upper] as f64;
    let weight = rank - (lower as f64);
    Ok(lower_value + (upper_value - lower_value) * weight)
}

fn assign_mofe_ids_by_discharge_rank(
    discha_vals: &[i32],
    d_fractions: &[f64],
) -> Result<Vec<i32>, String> {
    let n_cells = discha_vals.len();
    let counts = compute_mofe_segment_cell_counts(d_fractions, n_cells)?;

    let mut order = (0..n_cells).collect::<Vec<_>>();
    order.sort_by_key(|idx| discha_vals[*idx]);
    order.reverse();

    let mut labels = vec![0_i32; n_cells];
    let mut start = 0_usize;
    for (offset, count) in counts.iter().copied().enumerate() {
        let end = start + (count as usize);
        for order_idx in &order[start..end] {
            labels[*order_idx] = (offset + 1) as i32;
        }
        start = end;
    }

    if start != n_cells {
        return Err(format!(
            "MOFE rank assignment mismatch: assigned={start}, n_cells={n_cells}"
        ));
    }

    Ok(labels)
}

fn compute_mofe_segment_cell_counts(
    d_fractions: &[f64],
    n_cells: usize,
) -> Result<Vec<i32>, String> {
    if n_cells == 0 {
        return Err(format!("n_cells must be positive; received {n_cells}"));
    }

    if d_fractions.len() < 2 {
        return Err(format!(
            "d_fractions must include at least two points; received {d_fractions:?}"
        ));
    }

    let n_ofe = d_fractions.len() - 1;
    if n_cells < n_ofe {
        return Err(format!(
            "cannot assign {n_ofe} OFE ids across only {n_cells} cells"
        ));
    }

    let mut raw_counts = Vec::with_capacity(n_ofe);
    let mut counts = Vec::with_capacity(n_ofe);
    let mut remainders = Vec::with_capacity(n_ofe);

    for idx in 0..n_ofe {
        let length = d_fractions[idx + 1] - d_fractions[idx];
        if length < 0.0 {
            return Err(format!(
                "d_fractions must be non-decreasing; received {d_fractions:?}"
            ));
        }

        let raw = length * (n_cells as f64);
        let floor_count = raw.floor();
        raw_counts.push(raw);
        counts.push(floor_count.max(1.0) as i32);
        remainders.push(raw - floor_count);
    }

    let mut diff = (n_cells as i64) - counts.iter().map(|value| *value as i64).sum::<i64>();
    if diff > 0 {
        let mut order = (0..n_ofe).collect::<Vec<_>>();
        order.sort_by(|left, right| {
            remainders[*right]
                .partial_cmp(&remainders[*left])
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.cmp(right))
        });

        let mut order_idx = 0_usize;
        while diff > 0 {
            counts[order[order_idx % order.len()]] += 1;
            diff -= 1;
            order_idx += 1;
        }
    } else if diff < 0 {
        let mut order = (0..n_ofe).collect::<Vec<_>>();
        order.sort_by(|left, right| {
            remainders[*left]
                .partial_cmp(&remainders[*right])
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.cmp(right))
        });

        for index in order {
            while diff < 0 && counts[index] > 1 {
                counts[index] -= 1;
                diff += 1;
            }
            if diff == 0 {
                break;
            }
        }
    }

    if diff != 0 {
        return Err(format!(
            "unable to reconcile MOFE segment counts for {n_cells} cells: counts={counts:?}, d_fractions={d_fractions:?}"
        ));
    }

    Ok(counts)
}

fn unique_nonzero(values: &[i32]) -> Vec<i32> {
    let mut unique = values
        .iter()
        .copied()
        .filter(|value| *value != 0)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unique.sort_unstable();
    unique
}

#[pymodule]
fn watershed_abstraction_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(assign_mofe_map, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_counts_match_expected_distribution() {
        let d_fractions = vec![0.0, 0.34, 0.67, 1.0];
        let counts = compute_mofe_segment_cell_counts(&d_fractions, 4).unwrap();
        assert_eq!(counts, vec![2, 1, 1]);
    }

    #[test]
    fn repair_path_produces_contiguous_ids_for_flat_discha() {
        let d_fractions = vec![0.0, 0.34, 0.67, 1.0];
        let discha_vals = vec![5, 5, 5, 5];
        let labels = assign_hillslope_labels(171, &discha_vals, &d_fractions).unwrap();
        assert_eq!(labels.len(), 4);
        assert_eq!(unique_nonzero(&labels), vec![1, 2, 3]);
    }

    #[test]
    fn single_ofe_assigns_all_cells_to_one() {
        let d_fractions = vec![0.0, 1.0];
        let discha_vals = vec![4, 2, 3];
        let labels = assign_hillslope_labels(171, &discha_vals, &d_fractions).unwrap();
        assert_eq!(labels, vec![1, 1, 1]);
    }

    #[test]
    fn percentile_matches_linear_interpolation_behavior() {
        let sorted_values = vec![0, 10];
        let pct = percentile_sorted(&sorted_values, 25.0).unwrap();
        assert!((pct - 2.5).abs() < f64::EPSILON);
    }
}

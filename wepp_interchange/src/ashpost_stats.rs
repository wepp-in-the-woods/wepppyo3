//! Faithful scalar NumPy 1.26 indirect sorting and existing AshPost recurrence.
use crate::ashpost::{invalid, Products, Table, TRANSPORT};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
enum Scalar {
    Integer(i64),
    Float(f64),
}
type Entry = BTreeMap<String, Scalar>;
type Periods = BTreeMap<u32, Entry>;
type Measures = BTreeMap<String, Periods>;

fn heap(order: &mut [usize], values: &[f64]) {
    // One-based heap matches the baseline's tie movement, including extraction.
    let mut a = Vec::with_capacity(order.len() + 1);
    a.push(0);
    a.extend_from_slice(order);
    let mut n = order.len();
    for root in (1..=n / 2).rev() {
        let saved = a[root];
        let (mut i, mut j) = (root, root * 2);
        while j <= n {
            if j < n && values[a[j]] < values[a[j + 1]] {
                j += 1;
            }
            if !(values[saved] < values[a[j]]) {
                break;
            }
            a[i] = a[j];
            i = j;
            j *= 2;
        }
        a[i] = saved;
    }
    while n > 1 {
        let saved = a[n];
        a[n] = a[1];
        n -= 1;
        let (mut i, mut j) = (1, 2);
        while j <= n {
            if j < n && values[a[j]] < values[a[j + 1]] {
                j += 1;
            }
            if !(values[saved] < values[a[j]]) {
                break;
            }
            a[i] = a[j];
            i = j;
            j *= 2;
        }
        a[i] = saved;
    }
    order.copy_from_slice(&a[1..]);
}
fn ascending(order: &mut [usize], values: &[f64]) {
    if order.len() < 2 {
        return;
    }
    let (mut lo, mut hi) = (0, order.len() - 1);
    let mut depth = 2 * (usize::BITS - 1 - order.len().leading_zeros()) as i32;
    let mut stack = Vec::new();
    loop {
        if depth < 0 {
            heap(&mut order[lo..=hi], values);
        } else {
            while hi - lo > 15 {
                let mid = lo + (hi - lo) / 2;
                if values[order[mid]] < values[order[lo]] {
                    order.swap(mid, lo);
                }
                if values[order[hi]] < values[order[mid]] {
                    order.swap(hi, mid);
                }
                if values[order[mid]] < values[order[lo]] {
                    order.swap(mid, lo);
                }
                let pivot = values[order[mid]];
                let (mut i, mut j) = (lo, hi - 1);
                order.swap(mid, j);
                loop {
                    i += 1;
                    while values[order[i]] < pivot {
                        i += 1;
                    }
                    j -= 1;
                    while pivot < values[order[j]] {
                        j -= 1;
                    }
                    if i >= j {
                        break;
                    }
                    order.swap(i, j);
                }
                order.swap(i, hi - 1);
                depth -= 1;
                if i - lo < hi - i {
                    stack.push((i + 1, hi, depth));
                    hi = i - 1;
                } else {
                    stack.push((lo, i - 1, depth));
                    lo = i + 1;
                }
            }
            for i in lo + 1..=hi {
                let saved = order[i];
                let mut j = i;
                while j > lo && values[saved] < values[order[j - 1]] {
                    order[j] = order[j - 1];
                    j -= 1;
                }
                order[j] = saved;
            }
        }
        if let Some((l, h, d)) = stack.pop() {
            lo = l;
            hi = h;
            depth = d;
        } else {
            break;
        }
    }
}
fn descending(order: &mut Vec<usize>, values: &[f64]) {
    let nan = order
        .iter()
        .copied()
        .filter(|i| values[*i].is_nan())
        .collect::<Vec<_>>();
    order.retain(|i| !values[*i].is_nan());
    order.reverse();
    ascending(order, values);
    order.reverse();
    order.extend(nan);
}

fn periods(
    table: &Table,
    measure: &str,
    recurrence: &[u32],
    years: f64,
    extract: &[&str],
    order: &mut Vec<usize>,
) -> PyResult<Periods> {
    if years <= 0.0 {
        return Err(invalid("AshPost recurrence requires positive years"));
    }
    let col = table.index(measure)?;
    let values = table.rows.iter().map(|row| row[col]).collect::<Vec<_>>();
    descending(order, &values);
    let mut ranks = vec![f64::NAN; order.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        let rank = (start + 1 + end) as f64 / 2.0;
        for i in start..end {
            ranks[i] = rank;
        }
        start = end;
    }
    let count = (years * 365.25).round_ties_even() as usize;
    let mut allocated = BTreeMap::new();
    let mut selected = recurrence.to_vec();
    selected.sort_unstable();
    for interval in selected {
        if interval == 0 {
            return Err(invalid("AshPost recurrence must be positive"));
        }
        for rank in (1..=count).rev() {
            let period = ((count as f64 + 1.0) / rank as f64) / 365.25;
            if period >= interval as f64 && !allocated.values().any(|index| *index == rank - 1) {
                allocated.insert(interval, rank - 1);
                break;
            }
        }
    }
    let positives = values.iter().filter(|v| **v > 0.0).count();
    let extract = extract
        .iter()
        .map(|n| table.index(n).map(|i| (*n, i)))
        .collect::<PyResult<Vec<_>>>()?;
    let mut result = BTreeMap::new();
    for interval in recurrence {
        let mut row = Entry::new();
        if let Some(index) = allocated.get(interval) {
            let index = if positives == 0 {
                order.len() - 1
            } else {
                (*index).min(positives - 1)
            };
            let source = &table.rows[order[index]];
            let rank = ranks[index];
            let ri = (years + 1.0) / rank;
            let probability = (1.0 - (1.0 - 1.0 / ri)).clamp(0.0, 1.0) * 100.0;
            row.insert(
                measure.into(),
                if measure == "days_from_fire (days)" {
                    Scalar::Integer(source[col] as i64)
                } else {
                    Scalar::Float(source[col])
                },
            );
            for (name, col) in &extract {
                row.insert(
                    (*name).into(),
                    if matches!(*name, "year0" | "year" | "days_from_fire (days)") {
                        Scalar::Integer(source[*col] as i64)
                    } else {
                        Scalar::Float(source[*col])
                    },
                );
            }
            row.insert("rank".into(), Scalar::Integer(rank as i64));
            row.insert("ri".into(), Scalar::Float(ri));
            row.insert("probability".into(), Scalar::Float(probability));
        } else {
            for name in [measure, "rank", "ri", "probability"] {
                row.insert(name.into(), Scalar::Integer(0));
            }
        }
        result.insert(*interval, row);
    }
    Ok(result)
}
fn scalar_value(entry: &Entry, key: &str) -> f64 {
    match entry.get(key) {
        Some(Scalar::Float(v)) => *v,
        Some(Scalar::Integer(v)) => *v as f64,
        None => f64::NAN,
    }
}
fn python(py: Python<'_>, values: &Measures) -> PyResult<PyObject> {
    let result = PyDict::new_bound(py);
    for (measure, periods) in values {
        let p = PyDict::new_bound(py);
        for (interval, row) in periods {
            let dict = PyDict::new_bound(py);
            for (name, value) in row {
                match value {
                    Scalar::Integer(v) => dict.set_item(name, v)?,
                    Scalar::Float(v) => dict.set_item(name, v)?,
                };
            }
            p.set_item(interval, dict)?;
        }
        result.set_item(measure, p)?;
    }
    Ok(result.into_py(py))
}
pub struct Statistics {
    daily: Measures,
    cumulative: Measures,
    classes: BTreeMap<i64, Measures>,
}
impl Statistics {
    pub fn add_to(&self, py: Python<'_>, result: &Bound<'_, PyDict>) -> PyResult<()> {
        result.set_item("return_periods", python(py, &self.daily)?)?;
        result.set_item("cum_return_periods", python(py, &self.cumulative)?)?;
        let classes = PyDict::new_bound(py);
        for (class, measures) in &self.classes {
            classes.set_item(class, python(py, measures)?)?;
        }
        result.set_item("burn_class_return_periods", classes)?;
        Ok(())
    }
}
pub fn calculate(products: &Products, recurrence: &[u32]) -> PyResult<Statistics> {
    let daily = &products.tables[2];
    let class_table = &products.tables[3];
    let cumulative = &products.tables[4];
    let mut order = (0..daily.rows.len()).collect::<Vec<_>>();
    let mut daily_stats = Measures::new();
    let mut classes = BTreeMap::from([
        (1, Measures::new()),
        (2, Measures::new()),
        (3, Measures::new()),
    ]);
    let lookup: HashMap<[i64; 4], usize> = class_table
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| ([r[1] as i64, r[2] as i64, r[3] as i64, r[0] as i64], i))
        .collect();
    for measure in TRANSPORT.iter().map(|m| format!("{m} (tonne)")) {
        let stats = periods(
            daily,
            &measure,
            recurrence,
            daily.rows.len() as f64 / 365.25,
            &["days_from_fire (days)", "year0", "year", "julian"],
            &mut order,
        )?;
        let col = class_table.index(&measure)?;
        for (class, mapping) in &mut classes {
            let mut classified = stats.clone();
            for row in classified.values_mut() {
                let value = if scalar_value(row, &measure) != 0.0 {
                    let key = [
                        scalar_value(row, "year0") as i64,
                        scalar_value(row, "year") as i64,
                        scalar_value(row, "julian") as i64,
                        *class,
                    ];
                    lookup
                        .get(&key)
                        .map(|i| class_table.rows[*i][col])
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                row.insert(measure.clone(), Scalar::Float(value));
            }
            mapping.insert(measure.clone(), classified);
        }
        daily_stats.insert(measure, stats);
    }
    let mut order = (0..cumulative.rows.len()).collect::<Vec<_>>();
    let mut cumulative_stats = Measures::new();
    if !cumulative.rows.is_empty() {
        for measure in TRANSPORT
            .iter()
            .map(|m| format!("cum_{m} (tonne)"))
            .chain(std::iter::once("days_from_fire (days)".into()))
        {
            cumulative_stats.insert(
                measure.clone(),
                periods(
                    cumulative,
                    &measure,
                    recurrence,
                    cumulative.rows.len() as f64,
                    &["year0"],
                    &mut order,
                )?,
            );
        }
    }
    Ok(Statistics {
        daily: daily_stats,
        cumulative: cumulative_stats,
        classes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_ties_match_frozen_pandas() {
        let values = vec![0.0; 20];
        let mut order = (0..20).collect();
        descending(&mut order, &values);
        assert_eq!(
            order,
            vec![0, 1, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 19]
        );
    }
}

#[cfg(test)]
#[path = "ashpost_sort_tests.rs"]
mod frozen_sort_tests;

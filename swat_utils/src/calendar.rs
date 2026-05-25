use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use arrow_array::{
    Array, Int16Array, Int32Array, Int64Array, Int8Array, UInt16Array, UInt32Array, UInt64Array,
    UInt8Array,
};
use arrow_schema::DataType;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use crate::errors::InterchangeError;

#[derive(Debug, Clone)]
pub struct CalendarLookup {
    pub by_year: HashMap<i32, Vec<(i32, i32)>>,
}

impl CalendarLookup {
    pub fn year_len(&self, year: i32) -> Option<usize> {
        self.by_year.get(&year).map(|days| days.len())
    }
}

pub fn load_cli_calendar(path: &Path) -> Result<CalendarLookup, InterchangeError> {
    let reader = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(reader)?;
    let schema = builder.schema().clone();

    let year_name = "year";
    let month_name = if schema.fields().iter().any(|f| f.name() == "month") {
        "month"
    } else {
        "mo"
    };
    let day_name = if schema.fields().iter().any(|f| f.name() == "day_of_month") {
        "day_of_month"
    } else {
        "da"
    };

    let mut file_reader = builder.with_batch_size(1024 * 8).build()?;

    let mut rows: Vec<(i32, i32, i32)> = Vec::new();
    for maybe_batch in &mut file_reader {
        let batch = maybe_batch?;
        if batch.num_rows() == 0 {
            continue;
        }

        let mut year_col: Option<Vec<Option<i32>>> = None;
        let mut month_col: Option<Vec<Option<i32>>> = None;
        let mut day_col: Option<Vec<Option<i32>>> = None;

        for (field, array) in batch.schema().fields().iter().zip(batch.columns()) {
            let name = field.name();
            if name != year_name && name != month_name && name != day_name {
                continue;
            }
            let values =
                primitive_to_i32(array.as_ref()).map_err(|message| InterchangeError::Calendar {
                    message: format!("{} in {}", message, path.display()),
                })?;
            match name.as_str() {
                "year" => year_col = Some(values),
                "month" | "mo" => month_col = Some(values),
                "day_of_month" | "da" => day_col = Some(values),
                _ => {}
            }
        }

        let (years, months, days) = match (year_col, month_col, day_col) {
            (Some(y), Some(m), Some(d)) => (y, m, d),
            _ => continue,
        };

        for ((year, month), day) in years
            .into_iter()
            .zip(months.into_iter())
            .zip(days.into_iter())
        {
            if let (Some(year), Some(month), Some(day)) = (year, month, day) {
                rows.push((year, month, day));
            }
        }
    }

    rows.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
    let mut by_year: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for (year, month, day) in rows {
        by_year.entry(year).or_default().push((month, day));
    }
    Ok(CalendarLookup { by_year })
}

pub fn julian_to_calendar(year: i32, julian: i32, lookup: Option<&CalendarLookup>) -> (i32, i32) {
    if let Some(lookup) = lookup {
        if let Some(days) = lookup.by_year.get(&year) {
            if julian > 0 && (julian as usize) <= days.len() {
                let (month, day) = days[(julian - 1) as usize];
                return (month, day);
            }
        }
    }

    let (month, day) = julian_to_gregorian(year, julian);
    (month, day)
}

pub fn compute_sim_day_index(
    year: i32,
    julian: i32,
    start_year: i32,
    lookup: Option<&CalendarLookup>,
) -> i32 {
    if let Some(lookup) = lookup {
        let mut offset = 0i32;
        let mut exhaustive = true;
        for current_year in start_year..year {
            match lookup.year_len(current_year) {
                Some(days) => offset += days as i32,
                None => {
                    exhaustive = false;
                    break;
                }
            }
        }
        if exhaustive {
            return offset + julian;
        }
    }

    let target = match date_from_julian(year, julian) {
        Some(date) => date,
        None => return julian,
    };
    let base = (start_year, 1, 1);
    days_between(base, target) + 1
}

pub fn determine_wateryear(year: i32, julian: i32) -> i32 {
    let (month, _) = julian_to_gregorian(year, julian);
    if month > 9 {
        year + 1
    } else {
        year
    }
}

fn primitive_to_i32(array: &dyn Array) -> Result<Vec<Option<i32>>, String> {
    match array.data_type() {
        DataType::Int8 => Ok(downcast_int8(array)
            .into_iter()
            .map(|v| v.map(i32::from))
            .collect()),
        DataType::Int16 => Ok(downcast_int16(array)
            .into_iter()
            .map(|v| v.map(i32::from))
            .collect()),
        DataType::Int32 => Ok(downcast_int32(array)),
        DataType::Int64 => Ok(downcast_int64(array)
            .into_iter()
            .map(|v| v.map(|x| x as i32))
            .collect()),
        DataType::UInt8 => Ok(downcast_uint8(array)
            .into_iter()
            .map(|v| v.map(i32::from))
            .collect()),
        DataType::UInt16 => Ok(downcast_uint16(array)
            .into_iter()
            .map(|v| v.map(i32::from))
            .collect()),
        DataType::UInt32 => Ok(downcast_uint32(array)
            .into_iter()
            .map(|v| v.map(|x| x as i32))
            .collect()),
        DataType::UInt64 => Ok(downcast_uint64(array)
            .into_iter()
            .map(|v| v.map(|x| x as i32))
            .collect()),
        other => Err(format!("Unsupported calendar column type: {other:?}")),
    }
}

fn downcast_int8(array: &dyn Array) -> Vec<Option<i8>> {
    array
        .as_any()
        .downcast_ref::<Int8Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_int16(array: &dyn Array) -> Vec<Option<i16>> {
    array
        .as_any()
        .downcast_ref::<Int16Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_int32(array: &dyn Array) -> Vec<Option<i32>> {
    array
        .as_any()
        .downcast_ref::<Int32Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_int64(array: &dyn Array) -> Vec<Option<i64>> {
    array
        .as_any()
        .downcast_ref::<Int64Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_uint8(array: &dyn Array) -> Vec<Option<u8>> {
    array
        .as_any()
        .downcast_ref::<UInt8Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_uint16(array: &dyn Array) -> Vec<Option<u16>> {
    array
        .as_any()
        .downcast_ref::<UInt16Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_uint32(array: &dyn Array) -> Vec<Option<u32>> {
    array
        .as_any()
        .downcast_ref::<UInt32Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn downcast_uint64(array: &dyn Array) -> Vec<Option<u64>> {
    array
        .as_any()
        .downcast_ref::<UInt64Array>()
        .map(|arr| arr.iter().collect())
        .unwrap_or_default()
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn julian_to_gregorian(year: i32, julian: i32) -> (i32, i32) {
    let mut remaining = julian;
    let months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i32;
    for days in months {
        if remaining > days {
            remaining -= days;
            month += 1;
        } else {
            break;
        }
    }
    (month, remaining)
}

fn date_from_julian(year: i32, julian: i32) -> Option<(i32, i32, i32)> {
    if julian < 1 {
        return None;
    }
    let days_in_year = if is_leap_year(year) { 366 } else { 365 };
    if julian > days_in_year {
        return None;
    }
    let (month, day) = julian_to_gregorian(year, julian);
    Some((year, month, day))
}

fn days_between(base: (i32, i32, i32), target: (i32, i32, i32)) -> i32 {
    let base_days = days_from_civil(base.0, base.1, base.2);
    let target_days = days_from_civil(target.0, target.1, target.2);
    (target_days - base_days) as i32
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let mut y = year as i64;
    let m = month as i64;
    let d = day as i64;
    y -= (m <= 2) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrow_support::{BoxedArray, Chunk};
    use crate::parquet::write_single_chunk;
    use arrow_schema::{DataType, Field, Schema};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(stem: &str, extension: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        path.push(format!("swat_utils_calendar_{stem}_{nanos}.{extension}"));
        path
    }

    fn write_calendar_file(path: &Path, rows: &[(i32, i32, i32)]) {
        let years = arrow_array::Int32Array::from(rows.iter().map(|r| r.0).collect::<Vec<_>>());
        let months = arrow_array::Int32Array::from(rows.iter().map(|r| r.1).collect::<Vec<_>>());
        let days = arrow_array::Int32Array::from(rows.iter().map(|r| r.2).collect::<Vec<_>>());
        let schema = Schema::new(vec![
            Field::new("year", DataType::Int32, false),
            Field::new("month", DataType::Int32, false),
            Field::new("day_of_month", DataType::Int32, false),
        ]);
        let chunk = Chunk::new(vec![years.boxed(), months.boxed(), days.boxed()]);
        write_single_chunk(path, schema, chunk).expect("write calendar parquet");
    }

    #[test]
    fn arrow01_calendar_read_contract_swat_utils() {
        let path = temp_path("load", "parquet");
        write_calendar_file(&path, &[(2000, 1, 2), (2000, 1, 1), (2001, 2, 3)]);
        let lookup = load_cli_calendar(&path).expect("load calendar");
        assert_eq!(lookup.year_len(2000), Some(2));
        assert_eq!(lookup.year_len(2001), Some(1));
        assert_eq!(lookup.by_year.get(&2000).cloned(), Some(vec![(1, 1), (1, 2)]));
        assert_eq!(julian_to_calendar(2000, 2, Some(&lookup)), (1, 2));

        std::fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn arrow01_error_mapping_typed_contract_swat_utils_calendar_parquet() {
        let path = temp_path("invalid", "txt");
        std::fs::write(&path, "not a parquet file").expect("write invalid file");
        let err = load_cli_calendar(&path).expect_err("expected parquet parse error");
        match err {
            InterchangeError::Parquet(message) => {
                assert!(
                    !message.is_empty(),
                    "parquet error message should not be empty"
                );
            }
            other => panic!("expected InterchangeError::Parquet, got {other:?}"),
        }

        std::fs::remove_file(path).expect("cleanup invalid file");
    }
}

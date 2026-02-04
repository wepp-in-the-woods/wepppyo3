use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use arrow2::array::{Array, PrimitiveArray};
use arrow2::datatypes::DataType;
use arrow2::io::parquet::read;

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
    let mut reader = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let metadata = read::read_metadata(&mut reader)?;
    let schema = read::infer_schema(&metadata)?;

    let year_name = "year";
    let month_name = if schema.fields.iter().any(|f| f.name == "month") {
        "month"
    } else {
        "mo"
    };
    let day_name = if schema.fields.iter().any(|f| f.name == "day_of_month") {
        "day_of_month"
    } else {
        "da"
    };

    let schema = schema.filter(|_, field| {
        field.name == year_name || field.name == month_name || field.name == day_name
    });

    let row_groups = metadata.row_groups.clone();
    let mut file_reader = read::FileReader::new(
        reader,
        row_groups,
        schema.clone(),
        Some(1024 * 8),
        None,
        None,
    );

    let mut rows: Vec<(i32, i32, i32)> = Vec::new();
    for maybe_chunk in &mut file_reader {
        let chunk = maybe_chunk?;
        if chunk.arrays().len() != schema.fields.len() {
            continue;
        }

        let mut year_col: Option<Vec<Option<i32>>> = None;
        let mut month_col: Option<Vec<Option<i32>>> = None;
        let mut day_col: Option<Vec<Option<i32>>> = None;

        for (field, array) in schema.fields.iter().zip(chunk.arrays()) {
            let values =
                primitive_to_i32(array.as_ref()).map_err(|message| InterchangeError::Calendar {
                    message: format!("{} in {}", message, path.display()),
                })?;
            match field.name.as_str() {
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
        DataType::Int8 => Ok(downcast_numeric::<i8>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::Int16 => Ok(downcast_numeric::<i16>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::Int32 => Ok(downcast_numeric::<i32>(array)),
        DataType::Int64 => Ok(downcast_numeric::<i64>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::UInt8 => Ok(downcast_numeric::<u8>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::UInt16 => Ok(downcast_numeric::<u16>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::UInt32 => Ok(downcast_numeric::<u32>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        DataType::UInt64 => Ok(downcast_numeric::<u64>(array)
            .into_iter()
            .map(|v| v.map(|v| v as i32))
            .collect()),
        other => Err(format!("Unsupported calendar column type: {other:?}")),
    }
}

fn downcast_numeric<T: arrow2::types::NativeType>(array: &dyn Array) -> Vec<Option<T>> {
    array
        .as_any()
        .downcast_ref::<PrimitiveArray<T>>()
        .map(|arr| arr.iter().map(|v| v.copied()).collect())
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
    let yoe = y - era * 400; // [0, 399]
    let mp = m + if m > 2 { -3 } else { 9 }; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe
}

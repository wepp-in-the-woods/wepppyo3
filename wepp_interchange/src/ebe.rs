use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::arrow_support::{BoxedArray, Chunk};

use crate::calendar::{
    compute_sim_day_index, determine_wateryear, load_cli_calendar, CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{watershed_ebe_schema, VersionInfo};

const DEFAULT_CHUNK_ROWS: usize = 250_000;

fn calendar_day_to_julian(
    year: i32,
    month: i32,
    day: i32,
    lookup: Option<&CalendarLookup>,
) -> Result<i32, InterchangeError> {
    if let Some(lookup) = lookup {
        if let Some(days) = lookup.by_year.get(&year) {
            for (idx, (m, d)) in days.iter().enumerate() {
                if *m == month && *d == day {
                    return Ok((idx + 1) as i32);
                }
            }
        } else if !lookup.by_year.is_empty() {
            for days in lookup.by_year.values() {
                for (idx, (m, d)) in days.iter().enumerate() {
                    if *m == month && *d == day {
                        return Ok((idx + 1) as i32);
                    }
                }
            }
        }
        return Err(InterchangeError::Calendar {
            message: format!(
                "Date {year}-{month}-{day} not found in CLI calendar lookup (years available: {:?})",
                lookup.by_year.keys().collect::<Vec<_>>()
            ),
        });
    }

    let date =
        chrono_date(year, month, day).map_err(|message| InterchangeError::Calendar { message })?;
    Ok(date)
}

fn chrono_date(year: i32, month: i32, day: i32) -> Result<i32, String> {
    if month < 1 || month > 12 {
        return Err(format!("Invalid month {month}"));
    }
    let max_day = days_in_month(year, month);
    if day < 1 || day > max_day {
        return Err(format!("Invalid day {day} for {year}-{month}"));
    }
    let mut julian = 0i32;
    for m in 1..month {
        julian += days_in_month(year, m);
    }
    julian += day;
    Ok(julian)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    let leap = is_leap_year(year);
    match month {
        1 => 31,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn watershed_ebe_to_parquet(
    ebe_path: &Path,
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
    start_year: Option<i32>,
    legacy_element_id: Option<i32>,
    chan_path: Option<&Path>,
    chunk_rows: Option<usize>,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = watershed_ebe_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    let chunk_size = chunk_rows.unwrap_or(DEFAULT_CHUNK_ROWS);
    let resolved_legacy_element_id = match legacy_element_id {
        Some(element_id) => Some(element_id),
        None => infer_outlet_element_id(ebe_path, chan_path)?,
    };

    let calendar_start_year = lookup
        .as_ref()
        .and_then(|cal| cal.by_year.keys().min().copied());
    let resolved_start_year = start_year.or(calendar_start_year);
    let normalize_sim_years = resolved_start_year.is_some();
    let mut sim_start_year = resolved_start_year;

    let file = File::open(ebe_path).map_err(|err| InterchangeError::io(ebe_path, err))?;
    let reader = BufReader::new(file);

    let mut years: Vec<i16> = Vec::new();
    let mut sim_day_index: Vec<i32> = Vec::new();
    let mut simulation_year: Vec<i16> = Vec::new();
    let mut months: Vec<i8> = Vec::new();
    let mut days: Vec<i8> = Vec::new();
    let mut julians: Vec<i16> = Vec::new();
    let mut water_years: Vec<i16> = Vec::new();
    let mut precip: Vec<f64> = Vec::new();
    let mut runoff_volume: Vec<f64> = Vec::new();
    let mut peak_runoff: Vec<f64> = Vec::new();
    let mut sediment_yield: Vec<f64> = Vec::new();
    let mut soluble_pollutant: Vec<f64> = Vec::new();
    let mut particulate_pollutant: Vec<f64> = Vec::new();
    let mut total_pollutant: Vec<f64> = Vec::new();
    let mut element_id: Vec<Option<i32>> = Vec::new();

    let mut row_counter = 0usize;
    let mut nonzero_peak_rows = 0usize;

    for (line_no, line) in reader.lines().enumerate() {
        let raw_line = line.map_err(|err| InterchangeError::io(ebe_path, err))?;
        let stripped = raw_line.trim();
        if stripped.is_empty()
            || stripped.starts_with("WATERSHED")
            || stripped.starts_with('(')
            || stripped.starts_with("Day")
            || stripped.starts_with('-')
            || stripped.starts_with("Month")
            || stripped.starts_with("Year")
        {
            continue;
        }

        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() != 10 && tokens.len() != 11 {
            if tokens
                .first()
                .is_some_and(|token| token.chars().all(|c| c.is_ascii_digit()))
            {
                return Err(InterchangeError::parse(
                    ebe_path,
                    Some(line_no + 1),
                    format!(
                        "Unsupported EBE record width: expected 10 or 11 fields, found {}",
                        tokens.len()
                    ),
                    Some(raw_line.clone()),
                ));
            }
            continue;
        }

        let day_of_month: i32 = tokens[0].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid day token",
                Some(raw_line.clone()),
            )
        })?;
        let month: i32 = tokens[1].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid month token",
                Some(raw_line.clone()),
            )
        })?;
        let sim_year: i32 = tokens[2].parse().map_err(|_| {
            InterchangeError::parse(
                ebe_path,
                Some(line_no + 1),
                "Invalid simulation year token",
                Some(raw_line.clone()),
            )
        })?;

        let year = if normalize_sim_years {
            if sim_year < 1000 {
                resolved_start_year.unwrap_or(sim_year) + sim_year - 1
            } else {
                sim_year
            }
        } else {
            sim_year
        };
        if sim_start_year.is_none() {
            sim_start_year = Some(year);
        }

        let julian = calendar_day_to_julian(year, month, day_of_month, lookup.as_ref())?;
        let sim_day = compute_sim_day_index(
            year,
            julian,
            sim_start_year.unwrap_or(year),
            lookup.as_ref(),
        );
        let water_year = determine_wateryear(year, julian);

        years.push(year as i16);
        sim_day_index.push(sim_day);
        simulation_year.push(sim_year as i16);
        months.push(month as i8);
        days.push(day_of_month as i8);
        julians.push(julian as i16);
        water_years.push(water_year as i16);

        precip.push(parse_required_float(tokens[3]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        runoff_volume.push(parse_required_float(tokens[4]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        let peak_runoff_value = parse_required_float(tokens[5]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?;
        if peak_runoff_value != 0.0 {
            nonzero_peak_rows += 1;
        }
        peak_runoff.push(peak_runoff_value);
        sediment_yield.push(parse_required_float(tokens[6]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        soluble_pollutant.push(parse_required_float(tokens[7]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        particulate_pollutant.push(parse_required_float(tokens[8]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);
        total_pollutant.push(parse_required_float(tokens[9]).map_err(|msg| {
            InterchangeError::parse(ebe_path, Some(line_no + 1), msg, Some(raw_line.clone()))
        })?);

        let element_value = if tokens.len() == 11 {
            Some(tokens[10].parse::<i32>().map_err(|_| {
                InterchangeError::parse(
                    ebe_path,
                    Some(line_no + 1),
                    "Invalid element id token",
                    Some(raw_line.clone()),
                )
            })?)
        } else {
            resolved_legacy_element_id
        };
        element_id.push(element_value);

        row_counter += 1;
        if row_counter % chunk_size == 0 {
            flush_chunk(
                &mut sink,
                &schema,
                &mut years,
                &mut sim_day_index,
                &mut simulation_year,
                &mut months,
                &mut days,
                &mut julians,
                &mut water_years,
                &mut precip,
                &mut runoff_volume,
                &mut peak_runoff,
                &mut sediment_yield,
                &mut soluble_pollutant,
                &mut particulate_pollutant,
                &mut total_pollutant,
                &mut element_id,
            )?;
        }
    }

    if row_counter == 0 {
        sink.write_chunk(empty_chunk(&schema))?;
    } else if !years.is_empty() {
        flush_chunk(
            &mut sink,
            &schema,
            &mut years,
            &mut sim_day_index,
            &mut simulation_year,
            &mut months,
            &mut days,
            &mut julians,
            &mut water_years,
            &mut precip,
            &mut runoff_volume,
            &mut peak_runoff,
            &mut sediment_yield,
            &mut soluble_pollutant,
            &mut particulate_pollutant,
            &mut total_pollutant,
            &mut element_id,
        )?;
    }

    if row_counter > 0 && nonzero_peak_rows == 0 {
        if let Some(chan_path) = chan_path.filter(|path| path.exists()) {
            let positive_chan_peak_rows = count_positive_chan_peak_rows(chan_path)?;
            if positive_chan_peak_rows > 0 {
                return Err(InterchangeError::parse(
                    ebe_path,
                    None,
                    format!(
                        "Detected peak runoff regression signature: ebe_pw0 peak_runoff is all-zero \
                         ({row_counter}/{row_counter}) while chan.out has positive peaks \
                         ({positive_chan_peak_rows} rows)."
                    ),
                    None,
                ));
            }
        }
    }

    sink.finish()
}

fn infer_outlet_element_id(
    ebe_path: &Path,
    chan_path: Option<&Path>,
) -> Result<Option<i32>, InterchangeError> {
    if let Some(base) = ebe_path.parent() {
        let mut maximum_hillslope_id: Option<i32> = None;
        let entries = fs::read_dir(base).map_err(|err| InterchangeError::io(base, err))?;
        for entry in entries {
            let entry = entry.map_err(|err| InterchangeError::io(base, err))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(id_text) = name
                .strip_prefix('H')
                .and_then(|value| value.strip_suffix(".ebe.dat"))
            else {
                continue;
            };
            if id_text.is_empty() || !id_text.chars().all(|character| character.is_ascii_digit()) {
                continue;
            }
            let Ok(hillslope_id) = id_text.parse::<i32>() else {
                continue;
            };
            maximum_hillslope_id = Some(
                maximum_hillslope_id.map_or(hillslope_id, |current| current.max(hillslope_id)),
            );
        }
        if let Some(maximum_hillslope_id) = maximum_hillslope_id {
            return Ok(Some(maximum_hillslope_id + 1));
        }
    }

    let Some(chan_path) = chan_path.filter(|path| path.exists()) else {
        return Ok(None);
    };
    let file = File::open(chan_path).map_err(|err| InterchangeError::io(chan_path, err))?;
    let reader = BufReader::new(file);
    let mut fallback = None;
    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(chan_path, err))?;
        let stripped = raw_line.trim();
        if stripped.is_empty()
            || stripped.starts_with("Channel")
            || stripped.starts_with("Muskingum")
            || stripped.starts_with("Peak")
            || stripped.starts_with("Year")
        {
            continue;
        }
        let tokens = stripped.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 4 {
            continue;
        }
        let (Ok(element_id), Ok(channel_id)) = (tokens[2].parse::<i32>(), tokens[3].parse::<i32>())
        else {
            continue;
        };
        if channel_id == 1 {
            return Ok(Some(element_id));
        }
        fallback.get_or_insert(element_id);
    }
    Ok(fallback)
}

fn count_positive_chan_peak_rows(chan_path: &Path) -> Result<usize, InterchangeError> {
    let file = File::open(chan_path).map_err(|err| InterchangeError::io(chan_path, err))?;
    let reader = BufReader::new(file);
    let mut data_section = false;
    let mut positive_rows = 0usize;
    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(chan_path, err))?;
        let stripped = raw_line.trim();
        if !data_section {
            if stripped.starts_with("Year") && stripped.contains("Elmt_ID") {
                data_section = true;
            }
            continue;
        }
        if stripped.is_empty() {
            continue;
        }
        let tokens = stripped.split_whitespace().collect::<Vec<_>>();
        if tokens.len() != 6 {
            continue;
        }
        let Ok(peak) = parse_required_float(tokens[5]) else {
            continue;
        };
        if peak > 0.0 {
            positive_rows += 1;
        }
    }
    Ok(positive_rows)
}

#[allow(clippy::too_many_arguments)]
fn flush_chunk(
    sink: &mut ParquetSink,
    _schema: &arrow_schema::Schema,
    years: &mut Vec<i16>,
    sim_day_index: &mut Vec<i32>,
    simulation_year: &mut Vec<i16>,
    months: &mut Vec<i8>,
    days: &mut Vec<i8>,
    julians: &mut Vec<i16>,
    water_years: &mut Vec<i16>,
    precip: &mut Vec<f64>,
    runoff_volume: &mut Vec<f64>,
    peak_runoff: &mut Vec<f64>,
    sediment_yield: &mut Vec<f64>,
    soluble_pollutant: &mut Vec<f64>,
    particulate_pollutant: &mut Vec<f64>,
    total_pollutant: &mut Vec<f64>,
    element_id: &mut Vec<Option<i32>>,
) -> Result<(), InterchangeError> {
    if years.is_empty() {
        return Ok(());
    }
    let chunk = Chunk::new(vec![
        arrow_array::Int16Array::from(std::mem::take(years)).boxed(),
        arrow_array::Int32Array::from(std::mem::take(sim_day_index)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(simulation_year)).boxed(),
        arrow_array::Int8Array::from(std::mem::take(months)).boxed(),
        arrow_array::Int8Array::from(std::mem::take(days)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(julians)).boxed(),
        arrow_array::Int16Array::from(std::mem::take(water_years)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(precip)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(runoff_volume)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(peak_runoff)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(sediment_yield)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(soluble_pollutant)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(particulate_pollutant)).boxed(),
        arrow_array::Float64Array::from(std::mem::take(total_pollutant)).boxed(),
        arrow_array::Int32Array::from(std::mem::take(element_id)).boxed(),
    ]);
    sink.write_chunk(chunk)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(stem: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wepp_ebe_{stem}_{nanos}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_text(path: &Path, contents: &str) {
        let mut file = File::create(path).expect("create fixture");
        file.write_all(contents.as_bytes()).expect("write fixture");
    }

    #[test]
    fn outlet_inference_prefers_maximum_hillslope_id() {
        let dir = temp_dir("hillslope_outlet");
        let ebe_path = dir.join("ebe_pw0.txt");
        let chan_path = dir.join("chan.out");
        write_text(&ebe_path, "");
        write_text(&dir.join("H2.ebe.dat"), "");
        write_text(&dir.join("H9.ebe.dat"), "");
        write_text(&chan_path, "2000 1 41 1 0 2.0\n");

        assert_eq!(
            infer_outlet_element_id(&ebe_path, Some(&chan_path)).expect("infer outlet"),
            Some(10)
        );
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn outlet_inference_uses_channel_one_then_first_valid_channel() {
        let dir = temp_dir("channel_outlet");
        let ebe_path = dir.join("ebe_pw0.txt");
        let chan_path = dir.join("chan.out");
        write_text(&ebe_path, "");
        write_text(
            &chan_path,
            "Channel header\n2000 1 41 3 0 2.0\n2000 1 52 1 0 3.0\n",
        );
        assert_eq!(
            infer_outlet_element_id(&ebe_path, Some(&chan_path)).expect("infer channel one"),
            Some(52)
        );

        write_text(&chan_path, "2000 1 41 3 0 2.0\n2000 1 52 2 0 3.0\n");
        assert_eq!(
            infer_outlet_element_id(&ebe_path, Some(&chan_path)).expect("infer first channel"),
            Some(41)
        );
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn all_zero_ebe_peaks_fail_when_channel_peaks_are_positive() {
        let dir = temp_dir("peak_audit");
        let ebe_path = dir.join("ebe_pw0.txt");
        let chan_path = dir.join("chan.out");
        let output_path = dir.join("watershed_ebe.parquet");
        write_text(&ebe_path, "1 1 2000 1.0 2.0 0.0 4.0 5.0 6.0 7.0\n");
        write_text(
            &chan_path,
            "Year J Elmt_ID Chan_ID Time Peak\n2000 1 41 1 0 2.0\n",
        );

        let error = watershed_ebe_to_parquet(
            &ebe_path,
            &output_path,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
            Some(&chan_path),
            None,
        )
        .expect_err("peak regression must fail");
        assert!(error.display_message().contains("all-zero"));
        assert!(!output_path.exists());
        fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn nonzero_ebe_peak_passes_channel_peak_audit() {
        let dir = temp_dir("peak_audit_ok");
        let ebe_path = dir.join("ebe_pw0.txt");
        let chan_path = dir.join("chan.out");
        let output_path = dir.join("watershed_ebe.parquet");
        write_text(&ebe_path, "1 1 2000 1.0 2.0 0.5 4.0 5.0 6.0 7.0\n");
        write_text(
            &chan_path,
            "Year J Elmt_ID Chan_ID Time Peak\n2000 1 41 1 0 2.0\n",
        );

        let summary = watershed_ebe_to_parquet(
            &ebe_path,
            &output_path,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
            Some(&chan_path),
            None,
        )
        .expect("write audited ebe");
        assert_eq!(summary.rows_written, 1);
        assert!(output_path.exists());
        fs::remove_dir_all(dir).expect("cleanup");
    }
}

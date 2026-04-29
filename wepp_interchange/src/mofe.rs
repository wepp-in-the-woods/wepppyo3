use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::errors::InterchangeError;

#[derive(Debug)]
struct SingleOfeSlope {
    azm: f64,
    fwidth: f64,
    length: f64,
    distances: Vec<f64>,
    slopes: Vec<f64>,
}

pub fn segment_single_ofe_slope(
    src_fn: &str,
    dst_fn: Option<&str>,
    target_length: f64,
    apply_buffer: bool,
    buffer_length: f64,
    _min_length: f64,
    max_ofes: i64,
) -> Result<i64, InterchangeError> {
    if !target_length.is_finite() || target_length <= 0.0 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!("target_length must be finite and > 0; received {target_length}"),
            None,
        ));
    }

    let slope = parse_single_ofe_slope(src_fn)?;
    if !slope.length.is_finite() || slope.length <= 0.0 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!(
                "slope length must be finite and > 0; received {}",
                slope.length
            ),
            None,
        ));
    }

    if max_ofes < 1 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!("max_ofes must be >= 1; received {max_ofes}"),
            None,
        ));
    }

    let mut d_d = vec![0.0f64];
    let mut n_mofes: i64;
    let mut n_buffer = 0i64;
    let mut effective_buffer_length = if buffer_length.is_finite() {
        buffer_length
    } else {
        0.0
    };

    if apply_buffer {
        if slope.length <= effective_buffer_length {
            n_mofes = 1;
            effective_buffer_length = slope.length;
        } else if slope.length <= effective_buffer_length + target_length {
            n_mofes = 2;
        } else {
            n_mofes = round_half_even((slope.length - effective_buffer_length) / target_length)
                as i64
                + 1;
            if n_mofes < 2 {
                return Err(InterchangeError::parse(
                    src_fn,
                    None,
                    format!("computed n_mofes must be >= 2 with buffer mode; received {n_mofes}"),
                    None,
                ));
            }
        }

        n_buffer = 1;
        let d_buffer = effective_buffer_length / slope.length;
        d_d.push(d_buffer);
    } else {
        n_mofes = round_half_even(slope.length / target_length) as i64;
        effective_buffer_length = 0.0;
    }

    if n_mofes == 0 {
        n_mofes = 1;
    }
    if n_mofes > max_ofes {
        n_mofes = max_ofes;
    }

    let trailing_ofes = n_mofes - n_buffer;
    let ofe_length = if trailing_ofes == 0 {
        0.0
    } else {
        (slope.length - effective_buffer_length) / trailing_ofes as f64
    };
    let d_step = ofe_length / slope.length;
    for _ in 0..trailing_ofes {
        d_d.push(d_step);
    }

    let mut cumulative = Vec::with_capacity(d_d.len());
    let mut running = 0.0f64;
    for value in d_d {
        running += value;
        cumulative.push(running);
    }

    let last = *cumulative.last().ok_or_else(|| {
        InterchangeError::parse(src_fn, None, "failed to compute MOFE boundaries", None)
    })?;
    if (last - 1.0).abs() >= 0.0001 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!("MOFE boundary must end at 1.0; received {last}"),
            None,
        ));
    }
    if cumulative.len() as i64 != n_mofes + 1 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!(
                "MOFE boundary count mismatch; got {}, expected {}",
                cumulative.len(),
                n_mofes + 1
            ),
            None,
        ));
    }

    let mut output_lines = Vec::new();
    output_lines.push("97.5".to_string());
    output_lines.push(n_mofes.to_string());
    output_lines.push(format!("{:?} {:?}", slope.azm, slope.fwidth));

    for i in 0..(n_mofes as usize) {
        let d0 = cumulative[i];
        let dend = cumulative[i + 1];

        let mut distance_p = vec![d0];
        for d in slope.distances.iter().copied() {
            if d0 < d && d < dend {
                distance_p.push(d);
            }
        }

        let last_distance = *distance_p.last().unwrap_or(&d0);
        if round_ndigits_half_even(last_distance, 4) < round_ndigits_half_even(dend, 4) {
            distance_p.push(dend);
        }

        let segment_length = (dend - d0) * slope.length;
        let mut profile: Vec<(String, String)> = Vec::new();
        for d in distance_p.iter().copied() {
            let slope_value = interp_slope(&slope.distances, &slope.slopes, d);
            let normalized_d = (d - d0) / (dend - d0);
            let d_fmt = format!("{normalized_d:.4}");
            let s_fmt = format!("{slope_value:.4}");
            if let Some(last_pair) = profile.last_mut() {
                if last_pair.0 == d_fmt {
                    *last_pair = (d_fmt, s_fmt);
                    continue;
                }
            }
            profile.push((d_fmt, s_fmt));
        }

        output_lines.push(format!("{} {:.2}", profile.len(), segment_length));
        let row = profile
            .iter()
            .map(|(d, s)| format!("{d}, {s}"))
            .collect::<Vec<_>>()
            .join(" ");
        output_lines.push(format!("  {row}"));
    }

    let output = output_lines.join("\n");
    let dst_path = resolve_dst_path(src_fn, dst_fn);
    let mut file = File::create(&dst_path).map_err(|err| InterchangeError::io(&dst_path, err))?;
    file.write_all(output.as_bytes())
        .map_err(|err| InterchangeError::io(&dst_path, err))?;

    Ok(n_mofes)
}

fn resolve_dst_path(src_fn: &str, dst_fn: Option<&str>) -> PathBuf {
    if let Some(dst) = dst_fn {
        if !dst.trim().is_empty() {
            return PathBuf::from(dst);
        }
    }
    PathBuf::from(src_fn.replace(".slp", ".mofe.slp"))
}

fn parse_single_ofe_slope(src_fn: &str) -> Result<SingleOfeSlope, InterchangeError> {
    let src_path = Path::new(src_fn);
    let text =
        std::fs::read_to_string(src_path).map_err(|err| InterchangeError::io(src_path, err))?;
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if lines.len() < 5 {
        return Err(InterchangeError::parse(
            src_fn,
            None,
            format!(
                "expected at least 5 non-comment lines; found {}",
                lines.len()
            ),
            None,
        ));
    }

    let n_ofes = parse_i64(src_fn, 2, &lines[1], "n_ofes")?;
    if n_ofes != 1 {
        return Err(InterchangeError::parse(
            src_fn,
            Some(2),
            format!("expecting 1 OFE in source slope file; received {n_ofes}"),
            Some(lines[1].clone()),
        ));
    }

    let header_tokens = lines[2].split_whitespace().collect::<Vec<_>>();
    if header_tokens.len() < 2 {
        return Err(InterchangeError::parse(
            src_fn,
            Some(3),
            "invalid slope metadata line; expected at least azimuth and width",
            Some(lines[2].clone()),
        ));
    }
    let azm = parse_f64(src_fn, 3, header_tokens[0], "azimuth")?;
    let fwidth = parse_f64(src_fn, 3, header_tokens[1], "flow width")?;

    let segment_tokens = lines[3].split_whitespace().collect::<Vec<_>>();
    if segment_tokens.len() < 2 {
        return Err(InterchangeError::parse(
            src_fn,
            Some(4),
            "invalid segment line; expected nSegments and length",
            Some(lines[3].clone()),
        ));
    }
    let n_segments = parse_i64(src_fn, 4, segment_tokens[0], "nSegments")?;
    if n_segments < 2 {
        return Err(InterchangeError::parse(
            src_fn,
            Some(4),
            format!("nSegments must be >= 2; received {n_segments}"),
            Some(lines[3].clone()),
        ));
    }
    let length = parse_f64(src_fn, 4, segment_tokens[1], "slope length")?;

    let row_tokens = lines[4].replace(',', " ");
    let row_values = row_tokens.split_whitespace().collect::<Vec<_>>();
    if row_values.len() != (n_segments as usize) * 2 {
        return Err(InterchangeError::parse(
            src_fn,
            Some(5),
            format!(
                "expected {} profile values, found {}",
                (n_segments as usize) * 2,
                row_values.len()
            ),
            Some(lines[4].clone()),
        ));
    }

    let mut distances = Vec::with_capacity(n_segments as usize);
    let mut slopes = Vec::with_capacity(n_segments as usize);
    for i in 0..(n_segments as usize) {
        distances.push(parse_f64(src_fn, 5, row_values[i * 2], "distance")?);
        slopes.push(parse_f64(src_fn, 5, row_values[i * 2 + 1], "slope")?);
    }

    Ok(SingleOfeSlope {
        azm,
        fwidth,
        length,
        distances,
        slopes,
    })
}

fn parse_i64(src_fn: &str, line: usize, token: &str, field: &str) -> Result<i64, InterchangeError> {
    token.parse::<i64>().map_err(|_| {
        InterchangeError::parse(
            src_fn,
            Some(line),
            format!("invalid integer for {field}: {token}"),
            Some(token.to_string()),
        )
    })
}

fn parse_f64(src_fn: &str, line: usize, token: &str, field: &str) -> Result<f64, InterchangeError> {
    token.parse::<f64>().map_err(|_| {
        InterchangeError::parse(
            src_fn,
            Some(line),
            format!("invalid float for {field}: {token}"),
            Some(token.to_string()),
        )
    })
}

fn searchsorted_left(values: &[f64], target: f64) -> usize {
    let mut lo = 0usize;
    let mut hi = values.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if values[mid] < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn interp_slope(distances: &[f64], slopes: &[f64], d: f64) -> f64 {
    let clipped = if d > 1.0 { 1.0 } else { d };
    let mut idx = searchsorted_left(distances, clipped);
    if idx >= slopes.len() {
        idx = slopes.len() - 1;
    }
    slopes[idx]
}

fn round_half_even(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let sign = if value.is_sign_negative() { -1.0 } else { 1.0 };
    let abs_value = value.abs();
    let floor = abs_value.floor();
    let frac = abs_value - floor;
    let rounded_abs = if frac < 0.5 {
        floor
    } else if frac > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    };
    rounded_abs * sign
}

fn round_ndigits_half_even(value: f64, ndigits: i32) -> f64 {
    let factor = 10f64.powi(ndigits);
    round_half_even(value * factor) / factor
}

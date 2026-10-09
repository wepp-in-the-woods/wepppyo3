//! Validation for the in-band, current-day PASS source extension.
use std::path::Path;

use crate::errors::InterchangeError;

pub(crate) fn error(path: &Path, line: &str) -> InterchangeError {
    InterchangeError::parse(
        path,
        None,
        "Invalid PASS components record",
        Some(line.into()),
    )
}

pub(crate) fn header(line: &str, path: &Path) -> Result<bool, InterchangeError> {
    if line.trim_end() == "WEPP_PASS_COMPONENTS 3" {
        Ok(true)
    } else if line.contains("WEPP_PASS_COMPONENTS") {
        Err(error(path, line))
    } else {
        Ok(false)
    }
}

fn prefix<'a>(
    line: &'a str,
    label: &str,
    year: i32,
    day: i32,
    path: &Path,
) -> Result<(Vec<&'a str>, usize), InterchangeError> {
    let t: Vec<_> = line.split_whitespace().collect();
    if t.len() < 4
        || t[0] != label
        || t[1].parse::<i32>().ok() != Some(year)
        || t[2].parse::<i32>().ok() != Some(day)
    {
        return Err(error(path, line));
    }
    let index = t[3].parse::<usize>().map_err(|_| error(path, line))?;
    Ok((t, index))
}

pub(crate) fn count(
    line: &str,
    year: i32,
    day: i32,
    hills: usize,
    path: &Path,
) -> Result<usize, InterchangeError> {
    let (t, n) = prefix(line, "SOURCES3", year, day, path)?;
    if t.len() != 4 || n > hills {
        return Err(error(path, line));
    }
    Ok(n)
}

pub(crate) fn source(
    line: &str,
    year: i32,
    day: i32,
    master: bool,
    path: &Path,
) -> Result<usize, InterchangeError> {
    record(line, year, day, master, path).map(|(index, _)| index)
}

pub(crate) fn record(
    line: &str,
    year: i32,
    day: i32,
    master: bool,
    path: &Path,
) -> Result<(usize, Option<[f64; 27]>), InterchangeError> {
    let (t, index) = prefix(line, "SRC3", year, day, path)?;
    if !master && index == 0 && t.len() == 4 {
        return Ok((0, None));
    }
    if index == 0 || (!master && index != 1) || t.len() != 31 {
        return Err(error(path, line));
    }
    let mut values = [0.0_f64; 27];
    for (v, token) in values.iter_mut().zip(&t[4..]) {
        if !token.bytes().all(|b| b"0123456789+-.EeDd".contains(&b)) {
            return Err(error(path, line));
        }
        *v = token
            .replace(['D', 'd'], "E")
            .parse()
            .map_err(|_| error(path, line))?;
        // The producer's supplemental representation is native REAL.
        if !v.is_finite() || *v < 0.0 || *v > f32::MAX as f64 {
            return Err(error(path, line));
        }
    }
    if values[2] <= 0.0
        || values[3..].iter().sum::<f64>() <= 0.0
        || (values[0] == 0.0 && values[1] != 0.0)
    {
        return Err(error(path, line));
    }
    Ok((index, Some(values)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active() -> String {
        format!(
            "SRC3 1980 47 1 0.000002 3.63E-8 0.03 {}",
            vec!["0.001"; 24].join(" ")
        )
    }
    #[test]
    fn component_contract() {
        let p = Path::new("synthetic.pass");
        assert!(header("WEPP_PASS_COMPONENTS 3", p).unwrap());
        assert!(!header("legacy", p).unwrap());
        assert!(header("WEPP_PASS_COMPONENTS 1", p).is_err());
        assert!(header("WEPP_PASS_COMPONENTS 2", p).is_err());
        assert!(header("WEPP_PASS_COMPONENTS 4", p).is_err());
        assert_eq!(source("SRC3 1980 47 0", 1980, 47, false, p).unwrap(), 0);
        assert_eq!(source(&active(), 1980, 47, false, p).unwrap(), 1);
        for line in [
            active() + " 0",
            active().replace("0.03", "NaN"),
            active().replace("0.03", "-1"),
            active().replace("0.03", "1e99"),
            active().replace("0.000002", "0"),
            active().replace("0.001", "0"),
            active().replace("1980", "1981"),
            active().replace("47 1", "47 2"),
        ] {
            assert!(source(&line, 1980, 47, false, p).is_err(), "{line}");
        }
        assert!(count("SOURCES3 1980 47 4", 1980, 47, 3, p).is_err());
    }
}

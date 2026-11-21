//! Dated forcing and final CLI validation for the opt-in daily PRISM revision.
use super::{EXPECTED_TOKENS, HEADER_LINES};
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind, Result};

pub(super) fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

pub(super) fn number(token: &str) -> Result<f64> {
    token
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid("Invalid or nonfinite PRISM weather field"))
}

pub(super) struct Floor<'a> {
    dates: &'a [i32],
    raw: &'a [f64],
    row: usize,
}

impl<'a> Floor<'a> {
    pub fn new(dates: &'a [i32], raw: &'a [f64]) -> Result<Self> {
        if dates.is_empty() || dates.len() != raw.len() || dates.windows(2).any(|w| w[0] >= w[1]) {
            return Err(invalid("PRISM dewpoint calendar mismatch"));
        }
        Ok(Self { dates, raw, row: 0 })
    }

    fn check_date(&self, row: usize, tokens: &[&str]) -> Result<()> {
        let mismatch = || invalid("PRISM dewpoint calendar mismatch");
        let parse = |s: &str| s.parse::<i32>().map_err(|_| mismatch());
        let (day, month, year) = (parse(tokens[0])?, parse(tokens[1])?, parse(tokens[2])?);
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let days = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            1..=12 => 31,
            _ => return Err(mismatch()),
        };
        if !(1..=9999).contains(&year)
            || !(1..=days).contains(&day)
            || self.dates.get(row) != Some(&(year * 10000 + month * 100 + day))
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub fn check_source(&self, tokens: &[&str]) -> Result<()> {
        self.check_date(self.row, tokens)?;
        for value in &tokens[3..12] {
            number(value)?;
        }
        Ok(())
    }

    pub fn derive(&mut self, serialized_tmin: &str) -> Result<String> {
        let raw = self.raw[self.row];
        if !raw.is_finite() {
            return Err(invalid("Nonfinite PRISM raw tdmean"));
        }
        let tmin = number(serialized_tmin)?;
        // NumPy maximum selects its second operand on ties, including signed zero.
        let derived = if raw > tmin { raw } else { tmin };
        self.row += 1;
        Ok(format!("{derived:.1}"))
    }

    pub fn finish(&self, dst: &str) -> Result<()> {
        if self.row != self.dates.len() {
            return Err(invalid("PRISM dewpoint calendar mismatch"));
        }
        let mut count = 0;
        for line in BufReader::new(File::open(dst)?).lines().skip(HEADER_LINES) {
            let line = line?;
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != EXPECTED_TOKENS {
                return Err(invalid("Malformed revised PRISM CLI row"));
            }
            self.check_date(count, &fields)?;
            let rain = number(fields[3])?;
            let tmax = number(fields[7])?;
            let tmin = number(fields[8])?;
            let radiation = number(fields[9])?;
            let speed = number(fields[10])?;
            number(fields[11])?;
            let dew = number(fields[12])?;
            if rain < 0.0 || radiation < 0.0 || speed < 0.0 || tmin > tmax || dew < tmin {
                return Err(invalid("Invalid PRISM-derived CLI weather"));
            }
            count += 1;
        }
        if count != self.row {
            return Err(invalid("Incomplete revised PRISM CLI"));
        }
        Ok(())
    }
}

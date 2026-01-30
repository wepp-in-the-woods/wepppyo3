use memchr::{memchr, memchr2};

pub fn parse_float_loose(token: &str) -> Option<f64> {
    let stripped = token.trim();
    if stripped.is_empty() {
        return Some(0.0);
    }
    let bytes = stripped.as_bytes();
    if memchr(b'*', bytes).is_some() && bytes.iter().all(|b| *b == b'*') {
        return Some(f64::NAN);
    }

    let mut owned = String::new();
    let candidate = if stripped.starts_with('.') {
        owned.push('0');
        owned.push_str(stripped);
        owned.as_str()
    } else {
        stripped
    };

    if let Ok(value) = fast_float::parse::<f64, _>(candidate) {
        return Some(value);
    }

    let bytes = candidate.as_bytes();
    if memchr(b'e', bytes).is_none() && memchr(b'E', bytes).is_none() {
        if let Some(pos) = memchr2(b'+', b'-', &bytes[1..]).map(|idx| idx + 1) {
            let mut with_exp = String::with_capacity(candidate.len() + 1);
            with_exp.push_str(&candidate[..pos]);
            with_exp.push('E');
            with_exp.push_str(&candidate[pos..]);
            if let Ok(value) = fast_float::parse::<f64, _>(&with_exp) {
                return Some(value);
            }
        }
    }

    fast_float::parse::<f64, _>(candidate).ok()
}

pub fn parse_required_float(token: &str) -> Result<f64, String> {
    parse_float_loose(token).ok_or_else(|| format!("Unable to parse float from '{token}'"))
}

pub fn tokenize_numeric_line(line: &str) -> Vec<f64> {
    let bytes = line.as_bytes();
    let mut tokens: Vec<&str> = Vec::new();
    let mut start: Option<usize> = None;
    let mut idx = 0usize;
    while idx < bytes.len() {
        let b = bytes[idx];
        if b.is_ascii_whitespace() {
            if let Some(s) = start.take() {
                tokens.push(&line[s..idx]);
            }
            idx += 1;
            continue;
        }
        if start.is_none() {
            start = Some(idx);
            idx += 1;
            continue;
        }
        if (b == b'+' || b == b'-') && !matches!(bytes[idx.saturating_sub(1)], b'e' | b'E') {
            if let Some(s) = start.take() {
                tokens.push(&line[s..idx]);
            }
            start = Some(idx);
        }
        idx += 1;
    }
    if let Some(s) = start {
        tokens.push(&line[s..]);
    }

    let mut out = Vec::new();
    for token in tokens {
        if let Some(value) = parse_float_loose(token) {
            out.push(value);
        }
    }
    out
}

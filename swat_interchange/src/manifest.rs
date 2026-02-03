use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{Reason, SwatError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub category: String,
    pub filename: String,
    pub source_line: String,
    pub line_no: usize,
}

pub fn read_manifest(path: &Path) -> Result<Vec<ManifestEntry>, SwatError> {
    let file = File::open(path).map_err(|err| SwatError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut entries = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line_no = idx + 1;
        let raw = line.map_err(|err| SwatError::io(path, err))?;
        if line_no == 1 {
            continue;
        }
        if raw.trim().is_empty() {
            continue;
        }
        if raw.trim_start().starts_with('#') {
            continue;
        }
        let stripped = strip_inline_comment(&raw);
        let tokens: Vec<&str> = stripped.split_whitespace().collect();
        if tokens.len() < 2 {
            continue;
        }
        let category = tokens[0].to_string();
        let filename = tokens[1].to_string();
        entries.push(ManifestEntry {
            category,
            filename,
            source_line: raw,
            line_no,
        });
    }

    Ok(entries)
}

fn strip_inline_comment(raw: &str) -> String {
    let mut prev_whitespace = false;
    for (idx, ch) in raw.char_indices() {
        if ch == '#' && prev_whitespace {
            return raw[..idx].trim_end().to_string();
        }
        prev_whitespace = ch.is_whitespace();
    }
    raw.to_string()
}

pub fn validate_basename(filename: &str) -> Result<(), Reason> {
    if filename.contains('/') || filename.contains('\\') {
        return Err(Reason::PathInvalid);
    }
    if filename.starts_with("..") || filename.contains("..") {
        return Err(Reason::PathInvalid);
    }
    if PathBuf::from(filename).is_absolute() {
        return Err(Reason::PathInvalid);
    }
    Ok(())
}

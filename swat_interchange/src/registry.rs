use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    String,
    Float64,
    Int32,
    Int64,
}

#[derive(Debug, Clone)]
pub struct SwatTableSpec {
    pub pattern: &'static str,
    pub skip_lines: usize,
    pub header_line_index: usize,
    pub units_line_index: Option<usize>,
    pub header_merge: bool,
    pub column_types: HashMap<String, ColumnType>,
    pub column_descriptions: HashMap<String, String>,
    pub units_overrides: HashMap<String, String>,
    pub sentinel_overrides: HashMap<String, Vec<String>>,
    pub table_description: Option<String>,
}

impl SwatTableSpec {
    pub fn default_spec() -> Self {
        Self {
            pattern: "*",
            skip_lines: 1,
            header_line_index: 0,
            units_line_index: Some(1),
            header_merge: false,
            column_types: HashMap::new(),
            column_descriptions: HashMap::new(),
            units_overrides: HashMap::new(),
            sentinel_overrides: HashMap::new(),
            table_description: None,
        }
    }
}

pub fn resolve_spec(basename: &str) -> SwatTableSpec {
    let mut best: Option<(bool, usize, SwatTableSpec)> = None;
    for spec in registry_specs() {
        if !matches_pattern(spec.pattern, basename) {
            continue;
        }
        let is_exact = !spec.pattern.contains('*');
        let len = spec.pattern.len();
        match &best {
            Some((best_exact, best_len, _)) => {
                if *best_exact && !is_exact {
                    continue;
                }
                if *best_exact == is_exact && *best_len >= len {
                    continue;
                }
            }
            None => {}
        }
        best = Some((is_exact, len, spec));
    }

    best.map(|(_, _, spec)| spec).unwrap_or_else(SwatTableSpec::default_spec)
}

fn matches_pattern(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }
    let mut remainder = text;
    let mut parts = pattern.split('*').peekable();
    let starts_with_wildcard = pattern.starts_with('*');
    let ends_with_wildcard = pattern.ends_with('*');

    if let Some(first) = parts.next() {
        if !first.is_empty() && !starts_with_wildcard {
            if !remainder.starts_with(first) {
                return false;
            }
            remainder = &remainder[first.len()..];
        }
    }

    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(index) = remainder.find(part) {
            remainder = &remainder[index + part.len()..];
        } else {
            return false;
        }
    }

    if !ends_with_wildcard {
        if let Some(last) = pattern.split('*').last() {
            return text.ends_with(last);
        }
    }
    true
}

fn registry_specs() -> Vec<SwatTableSpec> {
    let mut specs = Vec::new();

    let mut checker_types = HashMap::new();
    checker_types.insert("sname".to_string(), ColumnType::String);
    checker_types.insert("hydgrp".to_string(), ColumnType::String);
    checker_types.insert("tiledrain".to_string(), ColumnType::Int32);

    let mut checker_desc = HashMap::new();
    checker_desc.insert("sname".to_string(), "sname".to_string());
    checker_desc.insert("hydgrp".to_string(), "hydgrp".to_string());
    checker_desc.insert(
        "tiledrain".to_string(),
        "tile drainage flag (0=notile;1=tile)".to_string(),
    );

    let mut checker_units = HashMap::new();
    checker_units.insert("sname".to_string(), "".to_string());
    checker_units.insert("hydgrp".to_string(), "".to_string());
    checker_units.insert("tiledrain".to_string(), "".to_string());

    specs.push(SwatTableSpec {
        pattern: "checker.out",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: true,
        column_types: checker_types,
        column_descriptions: checker_desc,
        units_overrides: checker_units,
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ checker output".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "files_out.out",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: None,
        header_merge: false,
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT output manifest".to_string()),
    });

    let mut lu_change_types = HashMap::new();
    lu_change_types.insert("hru".to_string(), ColumnType::Int32);
    lu_change_types.insert("year".to_string(), ColumnType::Int32);
    lu_change_types.insert("mon".to_string(), ColumnType::Int32);
    lu_change_types.insert("day".to_string(), ColumnType::Int32);
    lu_change_types.insert("operation".to_string(), ColumnType::String);
    lu_change_types.insert("lu_before".to_string(), ColumnType::String);
    lu_change_types.insert("lu_after".to_string(), ColumnType::String);

    let mut lu_change_desc = HashMap::new();
    lu_change_desc.insert("hru".to_string(), "hru".to_string());
    lu_change_desc.insert("year".to_string(), "year".to_string());
    lu_change_desc.insert("mon".to_string(), "mon".to_string());
    lu_change_desc.insert("day".to_string(), "day".to_string());
    lu_change_desc.insert("operation".to_string(), "operation".to_string());
    lu_change_desc.insert("lu_before".to_string(), "lu_before".to_string());
    lu_change_desc.insert("lu_after".to_string(), "lu_after".to_string());

    specs.push(SwatTableSpec {
        pattern: "lu_change_out.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: None,
        header_merge: false,
        column_types: lu_change_types,
        column_descriptions: lu_change_desc,
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ land use change operations".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "crop_yld_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ crop yield outputs".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "recall_aa.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ recall annual averages".to_string()),
    });

    specs
}

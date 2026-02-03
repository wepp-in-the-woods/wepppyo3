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
    pub whitespace_delimited: bool,
    pub column_names_override: Option<Vec<&'static str>>,
    pub merge_column: Option<&'static str>,
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
            whitespace_delimited: false,
            column_names_override: None,
            merge_column: None,
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

    best.map(|(_, _, spec)| spec)
        .unwrap_or_else(SwatTableSpec::default_spec)
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
        whitespace_delimited: false,
        column_names_override: None,
        merge_column: None,
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
        whitespace_delimited: false,
        column_names_override: None,
        merge_column: None,
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
        whitespace_delimited: false,
        column_names_override: None,
        merge_column: None,
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
        whitespace_delimited: false,
        column_names_override: None,
        merge_column: None,
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ crop yield outputs".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "hru_soilcarb_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ soil carbon gains/losses".to_string()),
    });

    let rescarb_columns = vec![
        "jday",
        "mon",
        "day",
        "yr",
        "unit",
        "gis_id",
        "name",
        "plant_surf_c",
        "plant_root_c",
        "rsd_surfdecay_c",
        "rsd_rootdecay_c",
        "harv_stov_c",
        "emit_c",
    ];
    let mut rescarb_units = HashMap::new();
    for key in [
        "plant_surf_c",
        "plant_root_c",
        "rsd_surfdecay_c",
        "rsd_rootdecay_c",
        "harv_stov_c",
        "emit_c",
    ] {
        rescarb_units.insert(key.to_string(), "kg C/ha".to_string());
    }
    specs.push(SwatTableSpec {
        pattern: "hru_rescarb_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: Some(rescarb_columns),
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: rescarb_units,
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ residue carbon gains/losses".to_string()),
    });

    let plcarb_columns = vec![
        "jday",
        "mon",
        "day",
        "yr",
        "unit",
        "gis_id",
        "name",
        "npp_c",
        "harv_abgr_c",
        "harv_root_c",
        "drop_c",
        "grazeat_c",
        "emit_c",
    ];
    let mut plcarb_units = HashMap::new();
    for key in [
        "npp_c",
        "harv_abgr_c",
        "harv_root_c",
        "drop_c",
        "grazeat_c",
        "emit_c",
    ] {
        plcarb_units.insert(key.to_string(), "kg C/ha".to_string());
    }
    specs.push(SwatTableSpec {
        pattern: "hru_plcarb_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: Some(plcarb_columns),
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: plcarb_units,
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ plant carbon gains/losses".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "hru_scf_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ soil carbon transformations".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "hru_ls_*.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("mgt_ops"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ HRU losses outputs".to_string()),
    });

    let basin_wb_aa_columns = vec![
        "jday",
        "mon",
        "day",
        "yr",
        "unit",
        "gis_id",
        "name",
        "precip",
        "snofall",
        "snomlt",
        "surq_gen",
        "latq",
        "wateryld",
        "perc",
        "et",
        "ecanopy",
        "eplant",
        "esoil",
        "surq_cont",
        "cn",
        "sw_init",
        "sw_final",
        "sw_ave",
        "sw_300",
        "sno_init",
        "sno_final",
        "snopack",
        "pet",
        "qtile",
        "irr",
        "surq_runon",
        "latq_runon",
        "overbank",
        "surq_cha",
        "surq_res",
        "surq_ls",
        "latq_cha",
        "latq_res",
        "latq_ls",
        "gwsoilq",
        "satex",
        "satex_chan",
        "sw_change",
        "lagsurf",
        "laglatq",
        "lagsatex",
        "wet_evap",
        "wet_oflo",
        "wet_stor",
        "cal_sim",
        "cal_adj",
    ];
    let mut basin_wb_aa_units = HashMap::new();
    basin_wb_aa_units.insert("cal_sim".to_string(), "".to_string());
    basin_wb_aa_units.insert("cal_adj".to_string(), "".to_string());
    let mut basin_wb_aa_types = HashMap::new();
    basin_wb_aa_types.insert("cal_sim".to_string(), ColumnType::String);
    specs.push(SwatTableSpec {
        pattern: "basin_wb_aa.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: Some(basin_wb_aa_columns),
        merge_column: Some("cal_sim"),
        column_types: basin_wb_aa_types,
        column_descriptions: HashMap::new(),
        units_overrides: basin_wb_aa_units,
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ basin water balance (average annual)".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "recall_aa.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ recall annual averages".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "basin_psc_aa.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some(
            "SWAT+ basin point source contributions (average annual)".to_string(),
        ),
    });

    specs.push(SwatTableSpec {
        pattern: "ru_day.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ routing unit daily outputs".to_string()),
    });

    specs.push(SwatTableSpec {
        pattern: "ru_aa.txt",
        skip_lines: 1,
        header_line_index: 0,
        units_line_index: Some(1),
        header_merge: false,
        whitespace_delimited: true,
        column_names_override: None,
        merge_column: Some("name"),
        column_types: HashMap::new(),
        column_descriptions: HashMap::new(),
        units_overrides: HashMap::new(),
        sentinel_overrides: HashMap::new(),
        table_description: Some("SWAT+ routing unit average annual outputs".to_string()),
    });

    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_beats_glob() {
        let spec = resolve_spec("checker.out");
        assert_eq!(spec.pattern, "checker.out");
        assert!(spec.header_merge);
    }

    #[test]
    fn glob_match_for_crop_yield() {
        let spec = resolve_spec("crop_yld_001.txt");
        assert_eq!(spec.pattern, "crop_yld_*.txt");
    }

    #[test]
    fn default_spec_for_unknown() {
        let spec = resolve_spec("unknown.txt");
        assert_eq!(spec.pattern, "*");
        assert_eq!(spec.skip_lines, 1);
        assert_eq!(spec.header_line_index, 0);
        assert_eq!(spec.units_line_index, Some(1));
        assert!(!spec.header_merge);
    }

    #[test]
    fn wildcard_matching_variants() {
        assert!(matches_pattern("foo*.txt", "foobar.txt"));
        assert!(matches_pattern("*bar.txt", "foobar.txt"));
        assert!(matches_pattern("foo*bar.txt", "foobazbar.txt"));
        assert!(!matches_pattern("foo*bar.txt", "foobaz.txt"));
    }
}

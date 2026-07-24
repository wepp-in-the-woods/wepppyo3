use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use arrow_array::{Float64Array, Int16Array, Int32Array, Int8Array};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::ag_fields::{self, Source as AgFieldsSource};
use crate::arrow_support::{BoxedArray, Chunk};
use crate::calendar::{
    compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar,
    CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::floats::parse_required_float;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::schema::{hill_wat_schema, VersionInfo};

const RAW_HEADER_SUBSTITUTIONS: [(&str, &str); 5] = [
    (" -", ""),
    ("#", "(#)"),
    (" mm", ""),
    ("Water(mm)", "Water"),
    ("m^2", "(m^2)"),
];

const WAT_BASE_COLUMN_NAMES: [&str; 20] = [
    "OFE",
    "J",
    "Y",
    "P",
    "RM",
    "Q",
    "Ep",
    "Es",
    "Er",
    "Dp",
    "UpStrmQ",
    "SubRIn",
    "latqcc",
    "Total-Soil Water",
    "frozwt",
    "Snow-Water",
    "QOFE",
    "Tile",
    "Irr",
    "Area",
];

const WAT_OPTIONAL_COLUMN_NAMES: [&str; 6] = [
    "SoilWaterTotal",
    "ProfileDepth",
    "ProfilePorosityCap",
    "ProfileFCStore",
    "ProfileWPStore",
    "InterceptionStorage",
];

fn header_aliases() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("OFE (#)", "OFE"),
        ("OFE", "OFE"),
        ("P (mm)", "P"),
        ("RM (mm)", "RM"),
        ("Q (mm)", "Q"),
        ("Ep (mm)", "Ep"),
        ("Es (mm)", "Es"),
        ("Er (mm)", "Er"),
        ("Dp (mm)", "Dp"),
        ("UpStrmQ (mm)", "UpStrmQ"),
        ("SubRIn (mm)", "SubRIn"),
        ("latqcc (mm)", "latqcc"),
        ("Total-Soil Water (mm)", "Total-Soil Water"),
        ("frozwt (mm)", "frozwt"),
        ("Snow-Water (mm)", "Snow-Water"),
        ("QOFE (mm)", "QOFE"),
        ("Tile (mm)", "Tile"),
        ("Irr (mm)", "Irr"),
        ("Area (m^2)", "Area"),
        ("SoilWaterTotal (mm)", "SoilWaterTotal"),
        ("ProfileDepth (mm)", "ProfileDepth"),
        ("ProfilePorosityCap (mm)", "ProfilePorosityCap"),
        ("ProfileFCStore (mm)", "ProfileFCStore"),
        ("ProfileWPStore (mm)", "ProfileWPStore"),
        ("InterceptionStorage (mm)", "InterceptionStorage"),
    ])
}

#[derive(Debug)]
pub struct WatColumns {
    wepp_id: Vec<i32>,
    ofe_id: Vec<i16>,
    year: Vec<i16>,
    sim_day_index: Vec<i32>,
    julian: Vec<i16>,
    month: Vec<i8>,
    day_of_month: Vec<i8>,
    water_year: Vec<i16>,
    ofe: Vec<i16>,
    p: Vec<f64>,
    rm: Vec<f64>,
    q: Vec<f64>,
    ep: Vec<f64>,
    es: Vec<f64>,
    er: Vec<f64>,
    dp: Vec<f64>,
    upstrmq: Vec<f64>,
    subrin: Vec<f64>,
    latqcc: Vec<f64>,
    total_soil_water: Vec<f64>,
    frozwt: Vec<f64>,
    snow_water: Vec<f64>,
    qofe: Vec<f64>,
    tile: Vec<f64>,
    irr: Vec<f64>,
    area: Vec<f64>,
    soil_water_total: Vec<Option<f64>>,
    profile_depth: Vec<Option<f64>>,
    profile_porosity_cap: Vec<Option<f64>>,
    profile_fc_store: Vec<Option<f64>>,
    profile_wp_store: Vec<Option<f64>>,
    interception_storage: Vec<Option<f64>>,
}

impl WatColumns {
    fn new() -> Self {
        Self {
            wepp_id: Vec::new(),
            ofe_id: Vec::new(),
            year: Vec::new(),
            sim_day_index: Vec::new(),
            julian: Vec::new(),
            month: Vec::new(),
            day_of_month: Vec::new(),
            water_year: Vec::new(),
            ofe: Vec::new(),
            p: Vec::new(),
            rm: Vec::new(),
            q: Vec::new(),
            ep: Vec::new(),
            es: Vec::new(),
            er: Vec::new(),
            dp: Vec::new(),
            upstrmq: Vec::new(),
            subrin: Vec::new(),
            latqcc: Vec::new(),
            total_soil_water: Vec::new(),
            frozwt: Vec::new(),
            snow_water: Vec::new(),
            qofe: Vec::new(),
            tile: Vec::new(),
            irr: Vec::new(),
            area: Vec::new(),
            soil_water_total: Vec::new(),
            profile_depth: Vec::new(),
            profile_porosity_cap: Vec::new(),
            profile_fc_store: Vec::new(),
            profile_wp_store: Vec::new(),
            interception_storage: Vec::new(),
        }
    }

    pub fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("wepp_id", self.wepp_id).unwrap();
        dict.set_item("ofe_id", self.ofe_id).unwrap();
        dict.set_item("year", self.year).unwrap();
        dict.set_item("sim_day_index", self.sim_day_index).unwrap();
        dict.set_item("julian", self.julian).unwrap();
        dict.set_item("month", self.month).unwrap();
        dict.set_item("day_of_month", self.day_of_month).unwrap();
        dict.set_item("water_year", self.water_year).unwrap();
        dict.set_item("OFE", self.ofe).unwrap();
        dict.set_item("P", self.p).unwrap();
        dict.set_item("RM", self.rm).unwrap();
        dict.set_item("Q", self.q).unwrap();
        dict.set_item("Ep", self.ep).unwrap();
        dict.set_item("Es", self.es).unwrap();
        dict.set_item("Er", self.er).unwrap();
        dict.set_item("Dp", self.dp).unwrap();
        dict.set_item("UpStrmQ", self.upstrmq).unwrap();
        dict.set_item("SubRIn", self.subrin).unwrap();
        dict.set_item("latqcc", self.latqcc).unwrap();
        dict.set_item("Total-Soil Water", self.total_soil_water)
            .unwrap();
        dict.set_item("frozwt", self.frozwt).unwrap();
        dict.set_item("Snow-Water", self.snow_water).unwrap();
        dict.set_item("QOFE", self.qofe).unwrap();
        dict.set_item("Tile", self.tile).unwrap();
        dict.set_item("Irr", self.irr).unwrap();
        dict.set_item("Area", self.area).unwrap();
        dict.set_item("SoilWaterTotal", self.soil_water_total)
            .unwrap();
        dict.set_item("ProfileDepth", self.profile_depth).unwrap();
        dict.set_item("ProfilePorosityCap", self.profile_porosity_cap)
            .unwrap();
        dict.set_item("ProfileFCStore", self.profile_fc_store)
            .unwrap();
        dict.set_item("ProfileWPStore", self.profile_wp_store)
            .unwrap();
        dict.set_item("InterceptionStorage", self.interception_storage)
            .unwrap();
        dict.into_py(py)
    }

    pub(crate) fn into_chunk(self) -> Chunk<Box<dyn arrow_array::Array>> {
        Chunk::new(vec![
            Int32Array::from(self.wepp_id).boxed(),
            Int16Array::from(self.ofe_id).boxed(),
            Int16Array::from(self.year).boxed(),
            Int32Array::from(self.sim_day_index).boxed(),
            Int16Array::from(self.julian).boxed(),
            Int8Array::from(self.month).boxed(),
            Int8Array::from(self.day_of_month).boxed(),
            Int16Array::from(self.water_year).boxed(),
            Int16Array::from(self.ofe).boxed(),
            Float64Array::from(self.p).boxed(),
            Float64Array::from(self.rm).boxed(),
            Float64Array::from(self.q).boxed(),
            Float64Array::from(self.ep).boxed(),
            Float64Array::from(self.es).boxed(),
            Float64Array::from(self.er).boxed(),
            Float64Array::from(self.dp).boxed(),
            Float64Array::from(self.upstrmq).boxed(),
            Float64Array::from(self.subrin).boxed(),
            Float64Array::from(self.latqcc).boxed(),
            Float64Array::from(self.total_soil_water).boxed(),
            Float64Array::from(self.frozwt).boxed(),
            Float64Array::from(self.snow_water).boxed(),
            Float64Array::from(self.qofe).boxed(),
            Float64Array::from(self.tile).boxed(),
            Float64Array::from(self.irr).boxed(),
            Float64Array::from(self.area).boxed(),
            Float64Array::from(self.soil_water_total).boxed(),
            Float64Array::from(self.profile_depth).boxed(),
            Float64Array::from(self.profile_porosity_cap).boxed(),
            Float64Array::from(self.profile_fc_store).boxed(),
            Float64Array::from(self.profile_wp_store).boxed(),
            Float64Array::from(self.interception_storage).boxed(),
        ])
    }
}

pub fn hillslope_wat_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
) -> Result<WatColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    hillslope_wat_to_columns_with_lookup(path, lookup.as_ref(), version)
}

fn hillslope_wat_to_columns_with_lookup(
    path: &Path,
    lookup: Option<&CalendarLookup>,
    _version: &VersionInfo,
) -> Result<WatColumns, InterchangeError> {
    let wepp_id = extract_wepp_id(path)?;

    let file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let reader = BufReader::new(file);
    let mut header_rows: Vec<Vec<String>> = Vec::new();
    let mut header: Option<Vec<String>> = None;
    let mut column_positions: HashMap<String, usize> = HashMap::new();
    let mut out = WatColumns::new();
    let mut start_year: Option<i32> = None;

    enum ParseState {
        SeekingHeaderStart,
        InHeader,
        SkipAfterHeader(usize),
        Data,
    }

    let mut state = ParseState::SeekingHeaderStart;

    for line in reader.lines() {
        let raw_line = line.map_err(|err| InterchangeError::io(path, err))?;
        let stripped = raw_line.trim();
        match state {
            ParseState::SeekingHeaderStart => {
                if stripped.is_empty() {
                    continue;
                }
                if stripped.starts_with('-') {
                    state = ParseState::InHeader;
                }
            }
            ParseState::InHeader => {
                if stripped.is_empty() {
                    continue;
                }
                if stripped.starts_with('-') {
                    let parsed_header = build_header_from_rows(&header_rows, path)?;
                    column_positions = parsed_header
                        .iter()
                        .enumerate()
                        .map(|(idx, name)| (name.clone(), idx))
                        .collect();
                    header = Some(parsed_header);
                    state = ParseState::SkipAfterHeader(1);
                } else {
                    header_rows.push(raw_line.split_whitespace().map(|s| s.to_string()).collect());
                }
            }
            ParseState::SkipAfterHeader(skip) => {
                if skip > 1 {
                    state = ParseState::SkipAfterHeader(skip - 1);
                } else {
                    state = ParseState::Data;
                }
            }
            ParseState::Data => {
                if stripped.is_empty() {
                    continue;
                }
                let header = header.as_ref().ok_or_else(|| {
                    InterchangeError::parse(path, None, "Missing WAT header rows", None)
                })?;
                let tokens: Vec<&str> = raw_line.split_whitespace().collect();
                if tokens.len() != header.len() {
                    return Err(InterchangeError::parse(
                        path,
                        None,
                        format!(
                            "Unsupported hillslope WAT record width: expected {} fields, found {}",
                            header.len(),
                            tokens.len()
                        ),
                        Some(raw_line.clone()),
                    ));
                }

                let julian_val: i32 = tokens[*column_positions.get("J").unwrap()]
                    .parse()
                    .map_err(|_| {
                        InterchangeError::parse(
                            path,
                            None,
                            "Invalid julian token",
                            Some(raw_line.clone()),
                        )
                    })?;
                let year_val: i32 = tokens[*column_positions.get("Y").unwrap()]
                    .parse()
                    .map_err(|_| {
                        InterchangeError::parse(
                            path,
                            None,
                            "Invalid year token",
                            Some(raw_line.clone()),
                        )
                    })?;
                let (month, day_of_month) = julian_to_calendar(year_val, julian_val, lookup);
                let water_year = determine_wateryear(year_val, julian_val);
                let first_year = *start_year.get_or_insert(year_val);
                let sim_day_index = compute_sim_day_index(year_val, julian_val, first_year, lookup);
                if sim_day_index < 1 {
                    return Err(InterchangeError::parse(
                        path,
                        None,
                        format!("Computed negative simulation day index ({sim_day_index})"),
                        Some(raw_line.clone()),
                    ));
                }
                let ofe_val: i32 = tokens[*column_positions.get("OFE").unwrap()]
                    .parse()
                    .map_err(|_| {
                        InterchangeError::parse(
                            path,
                            None,
                            "Invalid OFE token",
                            Some(raw_line.clone()),
                        )
                    })?;

                out.wepp_id.push(wepp_id);
                out.ofe_id.push(ofe_val as i16);
                out.year.push(year_val as i16);
                out.sim_day_index.push(sim_day_index);
                out.julian.push(julian_val as i16);
                out.month.push(month as i8);
                out.day_of_month.push(day_of_month as i8);
                out.water_year.push(water_year as i16);
                out.ofe.push(ofe_val as i16);

                for name in WAT_BASE_COLUMN_NAMES.iter().skip(3) {
                    let token = tokens[*column_positions.get(*name).unwrap()];
                    let value = parse_required_float(token).map_err(|msg| {
                        InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                    })?;
                    match *name {
                        "P" => out.p.push(value),
                        "RM" => out.rm.push(value),
                        "Q" => out.q.push(value),
                        "Ep" => out.ep.push(value),
                        "Es" => out.es.push(value),
                        "Er" => out.er.push(value),
                        "Dp" => out.dp.push(value),
                        "UpStrmQ" => out.upstrmq.push(value),
                        "SubRIn" => out.subrin.push(value),
                        "latqcc" => out.latqcc.push(value),
                        "Total-Soil Water" => out.total_soil_water.push(value),
                        "frozwt" => out.frozwt.push(value),
                        "Snow-Water" => out.snow_water.push(value),
                        "QOFE" => out.qofe.push(value),
                        "Tile" => out.tile.push(value),
                        "Irr" => out.irr.push(value),
                        "Area" => out.area.push(value),
                        _ => {}
                    }
                }

                for name in WAT_OPTIONAL_COLUMN_NAMES.iter() {
                    if let Some(position) = column_positions.get(*name) {
                        let token = tokens[*position];
                        let value = parse_required_float(token).map_err(|msg| {
                            InterchangeError::parse(path, None, msg, Some(raw_line.clone()))
                        })?;
                        match *name {
                            "SoilWaterTotal" => out.soil_water_total.push(Some(value)),
                            "ProfileDepth" => out.profile_depth.push(Some(value)),
                            "ProfilePorosityCap" => out.profile_porosity_cap.push(Some(value)),
                            "ProfileFCStore" => out.profile_fc_store.push(Some(value)),
                            "ProfileWPStore" => out.profile_wp_store.push(Some(value)),
                            "InterceptionStorage" => out.interception_storage.push(Some(value)),
                            _ => {}
                        }
                    } else {
                        match *name {
                            "SoilWaterTotal" => out.soil_water_total.push(None),
                            "ProfileDepth" => out.profile_depth.push(None),
                            "ProfilePorosityCap" => out.profile_porosity_cap.push(None),
                            "ProfileFCStore" => out.profile_fc_store.push(None),
                            "ProfileWPStore" => out.profile_wp_store.push(None),
                            "InterceptionStorage" => out.interception_storage.push(None),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if header.is_none() {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unable to locate WAT header delimiters",
            None,
        ));
    }

    Ok(out)
}

pub fn hillslope_wat_files_to_parquet(
    paths: &[PathBuf],
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = hill_wat_schema(version);
    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    if paths.is_empty() {
        sink.write_chunk(empty_chunk(&schema))?;
    } else {
        for path in paths {
            let columns = hillslope_wat_to_columns_with_lookup(path, lookup.as_ref(), version)?;
            sink.write_chunk(columns.into_chunk())?;
        }
    }
    sink.finish()
}

pub fn ag_fields_hillslope_wat_files_to_parquet(
    sources: &[AgFieldsSource],
    output_path: &Path,
    cli_calendar_path: Option<&Path>,
    version: &VersionInfo,
) -> Result<WriteSummary, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };
    let schema = ag_fields::schema_from_hillslope(hill_wat_schema(version));
    ag_fields::write_sources(sources, output_path, schema, |path| {
        hillslope_wat_to_columns_with_lookup(path, lookup.as_ref(), version)
            .map(WatColumns::into_chunk)
    })
}

fn build_header_from_rows(
    raw_header_rows: &[Vec<String>],
    path: &Path,
) -> Result<Vec<String>, InterchangeError> {
    let mut header: Vec<String> = Vec::new();
    let min_len = raw_header_rows
        .iter()
        .map(|row| row.len())
        .min()
        .unwrap_or(0);
    for col_idx in 0..min_len {
        let mut merged = raw_header_rows
            .iter()
            .map(|row| row[col_idx].clone())
            .collect::<Vec<_>>()
            .join(" ");
        for (old, new) in RAW_HEADER_SUBSTITUTIONS.iter() {
            merged = merged.replace(old, new);
        }
        header.push(merged.trim().to_string());
    }

    let aliases = header_aliases();
    let canonical_header: Vec<String> = header
        .iter()
        .map(|value| {
            aliases
                .get(value.as_str())
                .unwrap_or(&value.as_str())
                .to_string()
        })
        .collect();

    let base_len = WAT_BASE_COLUMN_NAMES.len();
    let canonical_refs: Vec<&str> = canonical_header.iter().map(|s| s.as_str()).collect();
    if canonical_refs.len() < base_len || canonical_refs[..base_len] != WAT_BASE_COLUMN_NAMES {
        return Err(InterchangeError::parse(
            path,
            None,
            format!("Unexpected WAT column layout: {header:?}"),
            None,
        ));
    }

    let optional_header = &canonical_refs[base_len..];
    if optional_header.len() > WAT_OPTIONAL_COLUMN_NAMES.len() {
        return Err(InterchangeError::parse(
            path,
            None,
            format!(
                "Unexpected WAT column layout: {header:?}; optional columns must be trailing approved terms {:?}",
                WAT_OPTIONAL_COLUMN_NAMES
            ),
            None,
        ));
    }
    let expected_optional = &WAT_OPTIONAL_COLUMN_NAMES[..optional_header.len()];
    if optional_header != expected_optional {
        return Err(InterchangeError::parse(
            path,
            None,
            format!(
                "Unexpected WAT column layout: {header:?}; optional columns must be trailing approved terms {:?}",
                WAT_OPTIONAL_COLUMN_NAMES
            ),
            None,
        ));
    }

    Ok(canonical_header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "wepp_interchange_hill_wat_{label}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&dir).expect("failed to create temp directory");
        dir
    }

    fn write_wat(path: &Path, header: &str, row: &str) {
        let mut payload = String::new();
        payload.push_str(header);
        payload.push_str(row);
        payload.push('\n');
        fs::write(path, payload).expect("failed to write wat file");
    }

    const HEADER_BASE: &str = " ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  OFE    J    Y      P      RM     Q                Ep      Es      Er     Dp       UpStrmQ   SubRIn    latqcc Total-Soil frozwt Snow-Water QOFE            Tile    Irr        Area
  #      -    -      mm     mm     mm               mm      mm      mm       mm      mm           mm      mm   Water(mm)   mm        mm      mm             mm      mm         m^2
 ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

";

    const HEADER_ENRICHED: &str = " ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  OFE    J    Y      P      RM     Q                Ep      Es      Er     Dp       UpStrmQ   SubRIn    latqcc Total-Soil frozwt Snow-Water QOFE            Tile    Irr        Area SoilWaterTotal ProfileDepth ProfilePorosityCap ProfileFCStore ProfileWPStore InterceptionStorage
  #      -    -      mm     mm     mm               mm      mm      mm       mm      mm           mm      mm   Water(mm)   mm        mm      mm             mm      mm         m^2             mm           mm                 mm             mm             mm                  mm
 ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

";
    const HEADER_ENRICHED_NO_INTERCEPTION: &str = " ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  OFE    J    Y      P      RM     Q                Ep      Es      Er     Dp       UpStrmQ   SubRIn    latqcc Total-Soil frozwt Snow-Water QOFE            Tile    Irr        Area SoilWaterTotal ProfileDepth ProfilePorosityCap ProfileFCStore ProfileWPStore
  #      -    -      mm     mm     mm               mm      mm      mm       mm      mm           mm      mm   Water(mm)   mm        mm      mm             mm      mm         m^2             mm           mm                 mm             mm             mm
 ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

";

    #[test]
    fn parses_legacy_layout_with_null_optional_terms() {
        let temp_dir = make_temp_dir("legacy");
        let wat_path = temp_dir.join("H1.wat.dat");
        write_wat(
            &wat_path,
            HEADER_BASE,
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00",
        );

        let version = VersionInfo::new(1, 0);
        let cols = hillslope_wat_to_columns(&wat_path, None, &version).expect("parse failed");
        assert_eq!(cols.p.len(), 1);
        assert_eq!(cols.dp[0], 0.40);
        assert_eq!(cols.total_soil_water[0], 100.0);
        assert_eq!(cols.soil_water_total[0], None);
        assert_eq!(cols.interception_storage[0], None);
    }

    #[test]
    fn parses_widened_scientific_dp_without_shifting_following_fields() {
        let temp_dir = make_temp_dir("widened_dp");
        let wat_path = temp_dir.join("H1.wat.dat");
        write_wat(
            &wat_path,
            HEADER_BASE,
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30   0.9158381E-02   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00",
        );

        let version = VersionInfo::new(1, 0);
        let cols = hillslope_wat_to_columns(&wat_path, None, &version).expect("parse failed");
        assert_eq!(cols.dp[0], 0.009158381);
        assert_eq!(cols.upstrmq[0], 0.0);
        assert_eq!(cols.subrin[0], 0.0);
        assert_eq!(cols.latqcc[0], 0.50);
        assert_eq!(cols.total_soil_water[0], 100.0);
    }

    #[test]
    fn parses_enriched_layout_with_interception_storage() {
        let temp_dir = make_temp_dir("enriched");
        let wat_path = temp_dir.join("H1.wat.dat");
        write_wat(
            &wat_path,
            HEADER_ENRICHED,
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00         101.25      1000.00             510.00         310.00         130.00               0.45",
        );

        let version = VersionInfo::new(1, 0);
        let cols = hillslope_wat_to_columns(&wat_path, None, &version).expect("parse failed");
        assert_eq!(cols.soil_water_total[0], Some(101.25));
        assert_eq!(cols.profile_depth[0], Some(1000.0));
        assert_eq!(cols.profile_porosity_cap[0], Some(510.0));
        assert_eq!(cols.profile_fc_store[0], Some(310.0));
        assert_eq!(cols.profile_wp_store[0], Some(130.0));
        assert_eq!(cols.interception_storage[0], Some(0.45));
    }

    #[test]
    fn parses_enriched_layout_without_interception_storage() {
        let temp_dir = make_temp_dir("enriched_without_interception");
        let wat_path = temp_dir.join("H1.wat.dat");
        write_wat(
            &wat_path,
            HEADER_ENRICHED_NO_INTERCEPTION,
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00         101.25      1000.00             510.00         310.00         130.00",
        );

        let version = VersionInfo::new(1, 0);
        let cols = hillslope_wat_to_columns(&wat_path, None, &version).expect("parse failed");
        assert_eq!(cols.soil_water_total[0], Some(101.25));
        assert_eq!(cols.profile_depth[0], Some(1000.0));
        assert_eq!(cols.profile_porosity_cap[0], Some(510.0));
        assert_eq!(cols.profile_fc_store[0], Some(310.0));
        assert_eq!(cols.profile_wp_store[0], Some(130.0));
        assert_eq!(cols.interception_storage[0], None);
    }

    #[test]
    fn writes_multiple_files_directly_with_calendar_day_indices() {
        let temp_dir = make_temp_dir("direct_parquet");
        let first_path = temp_dir.join("H1.wat.dat");
        let second_path = temp_dir.join("H2.wat.dat");
        let output_path = temp_dir.join("H.wat.parquet");
        let rows = "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00
     2    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      75.00
     1    2 2000   11.00   11.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00
     2    2 2000   11.00   11.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      75.00";
        write_wat(&first_path, HEADER_BASE, rows);
        write_wat(&second_path, HEADER_BASE, rows);

        let version = VersionInfo::new(1, 0);
        let summary = hillslope_wat_files_to_parquet(
            &[first_path, second_path],
            &output_path,
            None,
            &version,
        )
        .expect("direct WAT parquet failed");

        assert_eq!(summary.rows_written, 8);
        assert_eq!(summary.row_groups, 2);
        let builder = ParquetRecordBatchReaderBuilder::try_new(
            File::open(&output_path).expect("open direct parquet"),
        )
        .expect("build direct parquet reader");
        assert_eq!(builder.schema().as_ref(), &hill_wat_schema(&version));
        let reader = builder.build().expect("build record batch reader");
        let mut sim_days: Vec<i32> = Vec::new();
        for batch in reader {
            let batch = batch.expect("read record batch");
            let values = batch
                .column(3)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("sim_day_index Int32");
            sim_days.extend(values.values().iter().copied());
        }
        assert_eq!(sim_days, [1, 1, 2, 2, 1, 1, 2, 2]);
    }

    #[test]
    fn ag_fields_writer_preserves_all_wat_values_and_coupled_identity() {
        let temp_dir = make_temp_dir("ag_fields_parity");
        let paths = [
            temp_dir.join("H2.wat.dat"),
            temp_dir.join("H1.wat.dat"),
            temp_dir.join("H3.wat.dat"),
        ];
        let rows = "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00
     2    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      75.00";
        for path in &paths {
            write_wat(path, HEADER_BASE, rows);
        }
        let ordinary = temp_dir.join("ordinary.wat.parquet");
        let ag_output = temp_dir.join("ag_fields.wat.parquet");
        let version = VersionInfo::new(1, 2);
        let ordinary_summary = hillslope_wat_files_to_parquet(&paths, &ordinary, None, &version)
            .expect("write ordinary WAT parquet");
        let sources = vec![
            AgFieldsSource::new(paths[0].clone(), 90, 2),
            AgFieldsSource::new(paths[1].clone(), 90, 1),
            AgFieldsSource::new(paths[2].clone(), 91, 3),
        ];
        let ag_summary =
            ag_fields_hillslope_wat_files_to_parquet(&sources, &ag_output, None, &version)
                .expect("write AgFields WAT parquet");

        assert_eq!(ordinary_summary.rows_written, ag_summary.rows_written);
        assert_eq!(ordinary_summary.row_groups, ag_summary.row_groups);
        crate::ag_fields::assert_parquet_parity(&ordinary, &ag_output, &sources);
    }

    #[test]
    fn rejects_unknown_optional_column() {
        let temp_dir = make_temp_dir("unknown_header");
        let wat_path = temp_dir.join("H1.wat.dat");
        write_wat(
            &wat_path,
            &HEADER_ENRICHED.replace("ProfileWPStore", "UnexpectedExtra"),
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00         101.25      1000.00             510.00         310.00         130.00               0.45",
        );

        let version = VersionInfo::new(1, 0);
        let err = hillslope_wat_to_columns(&wat_path, None, &version)
            .expect_err("expected layout parse failure");
        let msg = err.display_message();
        assert!(msg.contains("Unexpected WAT column layout"));
    }

    #[test]
    fn rejects_overlong_optional_layout() {
        let temp_dir = make_temp_dir("overlong_optional_header");
        let wat_path = temp_dir.join("H1.wat.dat");
        let header_overlong = " ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  OFE    J    Y      P      RM     Q                Ep      Es      Er     Dp       UpStrmQ   SubRIn    latqcc Total-Soil frozwt Snow-Water QOFE            Tile    Irr        Area SoilWaterTotal ProfileDepth ProfilePorosityCap ProfileFCStore ProfileWPStore InterceptionStorage ExtraOptional
  #      -    -      mm     mm     mm               mm      mm      mm       mm      mm           mm      mm   Water(mm)   mm        mm      mm             mm      mm         m^2             mm           mm                 mm             mm             mm                  mm            mm
 ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

";
        write_wat(
            &wat_path,
            header_overlong,
            "     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00         101.25      1000.00             510.00         310.00         130.00               0.45               9.99",
        );

        let version = VersionInfo::new(1, 0);
        let err = hillslope_wat_to_columns(&wat_path, None, &version)
            .expect_err("expected layout parse failure");
        let msg = err.display_message();
        assert!(msg.contains("Unexpected WAT column layout"));
    }
}

fn extract_wepp_id(path: &Path) -> Result<i32, InterchangeError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| InterchangeError::parse(path, None, "Missing filename", None))?;
    let mut chars = name.chars();
    if chars.next().map(|c| c.to_ascii_uppercase()) != Some('H') {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unrecognized WAT filename pattern",
            Some(name.to_string()),
        ));
    }
    let mut digits = String::new();
    for c in chars {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return Err(InterchangeError::parse(
            path,
            None,
            "Unrecognized WAT filename pattern",
            Some(name.to_string()),
        ));
    }
    digits.parse::<i32>().map_err(|_| {
        InterchangeError::parse(path, None, "Invalid hillslope id", Some(name.to_string()))
    })
}

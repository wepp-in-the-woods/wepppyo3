use std::collections::HashMap;

use arrow_schema::{DataType, Field, Schema};

#[derive(Debug, Clone, Copy)]
pub struct VersionInfo {
    pub major: u32,
    pub minor: u32,
}

impl VersionInfo {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn dataset_version(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }
}

pub fn field_with_meta(
    name: &str,
    data_type: DataType,
    units: Option<&str>,
    description: Option<&str>,
) -> Field {
    let mut metadata: HashMap<String, String> = HashMap::new();
    if let Some(units) = units {
        metadata.insert("units".to_string(), units.to_string());
    }
    if let Some(description) = description {
        metadata.insert("description".to_string(), description.to_string());
    }
    Field::new(name, data_type, true).with_metadata(metadata)
}

pub fn schema_with_version(schema: Schema, version: &VersionInfo) -> Schema {
    let mut metadata = schema.metadata().clone();
    metadata.insert("dataset_version".to_string(), version.dataset_version());
    metadata.insert(
        "dataset_version_major".to_string(),
        version.major.to_string(),
    );
    metadata.insert(
        "dataset_version_minor".to_string(),
        version.minor.to_string(),
    );
    metadata.insert("schema_version".to_string(), version.major.to_string());
    schema.with_metadata(metadata)
}

pub fn watershed_ebe_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("year", DataType::Int16, None, Some("Calendar year")),
        field_with_meta(
            "sim_day_index",
            DataType::Int32,
            None,
            Some("1-indexed simulation day"),
        ),
        field_with_meta(
            "simulation_year",
            DataType::Int16,
            None,
            Some("WEPP simulation year reported in output"),
        ),
        field_with_meta("month", DataType::Int8, None, Some("Calendar month")),
        field_with_meta(
            "day_of_month",
            DataType::Int8,
            None,
            Some("Calendar day of month"),
        ),
        field_with_meta(
            "julian",
            DataType::Int16,
            None,
            Some("Julian day from WEPP output"),
        ),
        field_with_meta(
            "water_year",
            DataType::Int16,
            None,
            Some("Water year derived from year/julian"),
        ),
        field_with_meta(
            "precip",
            DataType::Float64,
            Some("mm"),
            Some("Watershed precipitation depth for the event"),
        ),
        field_with_meta(
            "runoff_volume",
            DataType::Float64,
            Some("m^3"),
            Some("Watershed runoff volume for the event"),
        ),
        field_with_meta(
            "peak_runoff",
            DataType::Float64,
            Some("m^3/s"),
            Some("Peak watershed discharge"),
        ),
        field_with_meta(
            "sediment_yield",
            DataType::Float64,
            Some("kg"),
            Some("Sediment yield at the watershed outlet"),
        ),
        field_with_meta(
            "soluble_pollutant",
            DataType::Float64,
            Some("kg"),
            Some("Soluble pollutant mass delivered at watershed outlet"),
        ),
        field_with_meta(
            "particulate_pollutant",
            DataType::Float64,
            Some("kg"),
            Some("Particulate pollutant mass delivered at watershed outlet"),
        ),
        field_with_meta(
            "total_pollutant",
            DataType::Float64,
            Some("kg"),
            Some("Total pollutant mass delivered (soluble + particulate)"),
        ),
        field_with_meta(
            "element_id",
            DataType::Int32,
            None,
            Some("Channel element identifier (Elmt_ID)"),
        ),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn watershed_chanwb_schema(version: &VersionInfo) -> Schema {
    let mut fields = vec![
        field_with_meta("year", DataType::Int16, None, Some("Calendar year")),
        field_with_meta(
            "simulation_year",
            DataType::Int16,
            None,
            Some("Simulation year from chanwb.out"),
        ),
        field_with_meta(
            "julian",
            DataType::Int16,
            None,
            Some("Julian day reported by WEPP"),
        ),
        field_with_meta(
            "month",
            DataType::Int8,
            None,
            Some("Calendar month derived from Julian day"),
        ),
        field_with_meta(
            "day_of_month",
            DataType::Int8,
            None,
            Some("Calendar day-of-month derived from Julian day"),
        ),
        field_with_meta(
            "water_year",
            DataType::Int16,
            None,
            Some("Water year computed from Julian day"),
        ),
        field_with_meta(
            "Elmt_ID",
            DataType::Int32,
            None,
            Some("Channel element identifier"),
        ),
        field_with_meta(
            "Chan_ID",
            DataType::Int32,
            None,
            Some("Channel ID reported by WEPP"),
        ),
    ];

    let measurements = vec![
        (
            "Inflow (m^3)",
            "m^3",
            "Total inflow above channel outlet, includes baseflow, all sources",
        ),
        ("Outflow (m^3)", "m^3", "Water flow out of channel outlet"),
        (
            "Storage (m^3)",
            "m^3",
            "Water surface storage at the end of the day",
        ),
        ("Baseflow (m^3)", "m^3", "Portion of inflow from baseflow"),
        (
            "Loss (m^3)",
            "m^3",
            "Transmission loss in channel, infiltration",
        ),
        (
            "Balance (m^3)",
            "m^3",
            "Water balance error at end of day (inflow - outflow - loss - Δstorage)",
        ),
    ];

    for (name, units, description) in measurements {
        fields.push(field_with_meta(
            name,
            DataType::Float64,
            Some(units),
            Some(description),
        ));
    }

    schema_with_version(Schema::new(fields), version)
}

pub fn watershed_chnwb_schema(version: &VersionInfo) -> Schema {
    let mut fields = vec![
        field_with_meta(
            "wepp_id",
            DataType::Int32,
            None,
            Some("Channel (OFE) identifier"),
        ),
        field_with_meta("julian", DataType::Int16, None, Some("Julian day")),
        field_with_meta("year", DataType::Int16, None, Some("Calendar year")),
        field_with_meta(
            "simulation_year",
            DataType::Int16,
            None,
            Some("Simulation year value from input file"),
        ),
        field_with_meta("month", DataType::Int8, None, Some("Calendar month")),
        field_with_meta(
            "day_of_month",
            DataType::Int8,
            None,
            Some("Calendar day of month"),
        ),
        field_with_meta(
            "water_year",
            DataType::Int16,
            None,
            Some("Computed water year"),
        ),
        field_with_meta("OFE", DataType::Int16, None, Some("Channel OFE index")),
        field_with_meta("J", DataType::Int16, None, Some("Julian day as reported")),
        field_with_meta(
            "Y",
            DataType::Int16,
            None,
            Some("Simulation year as reported"),
        ),
    ];

    let measurements = vec![
        ("P (mm)", "mm", "precipitation"),
        ("RM (mm)", "mm", "rainfall + irrigation + snowmelt"),
        ("Q (mm)", "mm", "daily runoff over effective length"),
        ("Ep (mm)", "mm", "plant transpiration"),
        ("Es (mm)", "mm", "soil evaporation"),
        ("Er (mm)", "mm", "residue evaporation"),
        ("Dp (mm)", "mm", "deep percolation"),
        ("UpStrmQ (mm)", "mm", "Runon added to OFE"),
        ("SubRIn (mm)", "mm", "Subsurface runon added to OFE"),
        ("latqcc (mm)", "mm", "lateral subsurface flow"),
        (
            "Total Soil Water (mm)",
            "mm",
            "Unfrozen water in soil profile",
        ),
        ("frozwt (mm)", "mm", "Frozen water in soil profile"),
        ("Snow Water (mm)", "mm", "Water in surface snow"),
        ("QOFE (mm)", "mm", "Daily runoff scaled to single OFE"),
        ("Tile (mm)", "mm", "Tile drainage"),
        ("Irr (mm)", "mm", "Irrigation"),
        ("Surf (mm)", "mm", "Surface storage"),
        ("Base (mm)", "mm", "Portion of runon from external baseflow"),
        ("Area (m^2)", "m^2", "Area that depths apply over"),
    ];

    for (name, units, description) in measurements {
        fields.push(field_with_meta(
            name,
            DataType::Float64,
            Some(units),
            Some(description),
        ));
    }

    schema_with_version(Schema::new(fields), version)
}

pub fn hill_pass_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta(
            "event",
            DataType::Utf8,
            None,
            Some("Record type: EVENT, SUBEVENT, NO EVENT"),
        ),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta(
            "sim_day_index",
            DataType::Int32,
            None,
            Some("1-indexed simulation day since start year"),
        ),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta("dur", DataType::Float64, Some("s"), Some("Storm duration")),
        field_with_meta(
            "tcs",
            DataType::Float64,
            Some("h"),
            Some("Overland flow time of concentration"),
        ),
        field_with_meta(
            "oalpha",
            DataType::Float64,
            Some("unitless"),
            Some("Overland flow alpha parameter"),
        ),
        field_with_meta("runoff", DataType::Float64, Some("m"), Some("Runoff depth")),
        field_with_meta(
            "runvol",
            DataType::Float64,
            Some("m^3"),
            Some("Runoff volume"),
        ),
        field_with_meta(
            "sbrunf",
            DataType::Float64,
            Some("m"),
            Some("Subsurface runoff depth"),
        ),
        field_with_meta(
            "sbrunv",
            DataType::Float64,
            Some("m^3"),
            Some("Subsurface runoff volume"),
        ),
        field_with_meta(
            "drainq",
            DataType::Float64,
            Some("m/day"),
            Some("Drainage flux"),
        ),
        field_with_meta(
            "drrunv",
            DataType::Float64,
            Some("m^3"),
            Some("Tile Drainage volume"),
        ),
        field_with_meta(
            "peakro",
            DataType::Float64,
            Some("m^3/s"),
            Some("Peak runoff rate"),
        ),
        field_with_meta(
            "tdet",
            DataType::Float64,
            Some("kg"),
            Some("Total detachment"),
        ),
        field_with_meta(
            "tdep",
            DataType::Float64,
            Some("kg"),
            Some("Total deposition"),
        ),
        field_with_meta(
            "sedcon_1",
            DataType::Float64,
            Some("kg/m^3"),
            Some("Sediment concentration 1"),
        ),
        field_with_meta(
            "sedcon_2",
            DataType::Float64,
            Some("kg/m^3"),
            Some("Sediment concentration 2"),
        ),
        field_with_meta(
            "sedcon_3",
            DataType::Float64,
            Some("kg/m^3"),
            Some("Sediment concentration 3"),
        ),
        field_with_meta(
            "sedcon_4",
            DataType::Float64,
            Some("kg/m^3"),
            Some("Sediment concentration 4"),
        ),
        field_with_meta(
            "sedcon_5",
            DataType::Float64,
            Some("kg/m^3"),
            Some("Sediment concentration 5"),
        ),
        field_with_meta(
            "clot",
            DataType::Float64,
            Some("m^3/s"),
            Some("Friction flow 1"),
        ),
        field_with_meta(
            "slot",
            DataType::Float64,
            Some("%"),
            Some("% of exiting sediment in the silt size class"),
        ),
        field_with_meta(
            "saot",
            DataType::Float64,
            Some("%"),
            Some("% of exiting sediment in the small aggregate size class"),
        ),
        field_with_meta(
            "laot",
            DataType::Float64,
            Some("%"),
            Some("% of exiting sediment in the large aggregate size class"),
        ),
        field_with_meta(
            "sdot",
            DataType::Float64,
            Some("%"),
            Some("% of exiting sediment in the sand size class"),
        ),
        field_with_meta(
            "gwbfv",
            DataType::Float64,
            None,
            Some("Groundwater baseflow"),
        ),
        field_with_meta(
            "gwdsv",
            DataType::Float64,
            None,
            Some("Groundwater deep seepage"),
        ),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn hill_ebe_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta(
            "sim_day_index",
            DataType::Int32,
            None,
            Some("1-indexed simulation day"),
        ),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta(
            "Precip",
            DataType::Float64,
            Some("mm"),
            Some("Storm precipitation depth"),
        ),
        field_with_meta(
            "Runoff",
            DataType::Float64,
            Some("mm"),
            Some("Runoff depth scaled by effective flow length"),
        ),
        field_with_meta(
            "IR-det",
            DataType::Float64,
            Some("kg/m^2"),
            Some("Weighted interrill detachment over the hillslope"),
        ),
        field_with_meta(
            "Av-det",
            DataType::Float64,
            Some("kg/m^2"),
            Some("Average soil detachment across detachment regions"),
        ),
        field_with_meta(
            "Mx-det",
            DataType::Float64,
            Some("kg/m^2"),
            Some("Maximum soil detachment across detachment regions"),
        ),
        field_with_meta(
            "Det-point",
            DataType::Float64,
            Some("m"),
            Some("Location of maximum soil detachment along hillslope"),
        ),
        field_with_meta(
            "Av-dep",
            DataType::Float64,
            Some("kg/m^2"),
            Some("Average sediment deposition across deposition regions"),
        ),
        field_with_meta(
            "Max-dep",
            DataType::Float64,
            Some("kg/m^2"),
            Some("Maximum sediment deposition across deposition regions"),
        ),
        field_with_meta(
            "Dep-point",
            DataType::Float64,
            Some("m"),
            Some("Location of maximum sediment deposition along hillslope"),
        ),
        field_with_meta(
            "Sed.Del",
            DataType::Float64,
            Some("kg/m"),
            Some("Storm sediment load per unit width at hillslope outlet"),
        ),
        field_with_meta(
            "ER",
            DataType::Float64,
            None,
            Some("Specific surface enrichment ratio for event sediment"),
        ),
        field_with_meta(
            "Det-Len",
            DataType::Float64,
            Some("m"),
            Some("Effective detachment flow length"),
        ),
        field_with_meta(
            "Dep-Len",
            DataType::Float64,
            Some("m"),
            Some("Effective deposition flow length"),
        ),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn hill_element_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("ofe_id", DataType::Int16, None, None),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta("OFE", DataType::Int16, None, None),
        field_with_meta("Precip", DataType::Float64, Some("mm"), None),
        field_with_meta("Runoff", DataType::Float64, Some("mm"), None),
        field_with_meta(
            "EffInt",
            DataType::Float64,
            Some("mm/h"),
            Some("Effective rainfall intensity"),
        ),
        field_with_meta(
            "PeakRO",
            DataType::Float64,
            Some("mm/h"),
            Some("Peak runoff rate"),
        ),
        field_with_meta("EffDur", DataType::Float64, Some("h"), None),
        field_with_meta(
            "Enrich",
            DataType::Float64,
            None,
            Some("Sediment enrichment ratio"),
        ),
        field_with_meta(
            "Keff",
            DataType::Float64,
            Some("mm/h"),
            Some("Effective hydraulic conductivity"),
        ),
        field_with_meta("Sm", DataType::Float64, Some("mm"), None),
        field_with_meta("LeafArea", DataType::Float64, None, Some("Leaf area index")),
        field_with_meta(
            "CanHgt",
            DataType::Float64,
            Some("m"),
            Some("Canopy height"),
        ),
        field_with_meta("Cancov", DataType::Float64, Some("%"), Some("Canopy cover")),
        field_with_meta(
            "IntCov",
            DataType::Float64,
            Some("%"),
            Some("Interrill cover"),
        ),
        field_with_meta("RilCov", DataType::Float64, Some("%"), Some("Rill cover")),
        field_with_meta("LivBio", DataType::Float64, Some("kg/m^2"), None),
        field_with_meta("DeadBio", DataType::Float64, Some("kg/m^2"), None),
        field_with_meta(
            "Ki",
            DataType::Float64,
            Some("kg s/m^4"),
            Some("Interrill erodibility"),
        ),
        field_with_meta(
            "Kr",
            DataType::Float64,
            Some("s/m"),
            Some("Rill erodibility"),
        ),
        field_with_meta("Tcrit", DataType::Float64, None, None),
        field_with_meta("RilWid", DataType::Float64, Some("m"), None),
        field_with_meta("SedLeave", DataType::Float64, Some("kg/m"), None),
        field_with_meta("QRain", DataType::Float64, Some("mm"), None),
        field_with_meta("QSnow", DataType::Float64, Some("mm"), None),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn hill_loss_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("class_id", DataType::Int8, None, None),
        field_with_meta(
            "Class",
            DataType::Int8,
            None,
            Some("Sediment particle size class"),
        ),
        field_with_meta("Diameter", DataType::Float64, Some("mm"), None),
        field_with_meta("Specific Gravity", DataType::Float64, None, None),
        field_with_meta("% Sand", DataType::Float64, Some("%"), None),
        field_with_meta("% Silt", DataType::Float64, Some("%"), None),
        field_with_meta("% Clay", DataType::Float64, Some("%"), None),
        field_with_meta("% O.M.", DataType::Float64, Some("%"), None),
        field_with_meta("Sediment Fraction", DataType::Float64, None, None),
        field_with_meta("In Flow Exiting", DataType::Float64, None, None),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn hill_soil_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("ofe_id", DataType::Int16, None, None),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta(
            "sim_day_index",
            DataType::Int32,
            None,
            Some("1-indexed simulation day"),
        ),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta("OFE", DataType::Int16, None, None),
        field_with_meta("Poros", DataType::Float64, Some("%"), Some("Soil porosity")),
        field_with_meta(
            "Keff",
            DataType::Float64,
            Some("mm/hr"),
            Some("Effective hydraulic conductivity"),
        ),
        field_with_meta(
            "Suct",
            DataType::Float64,
            Some("mm"),
            Some("Suction across wetting front"),
        ),
        field_with_meta(
            "FC",
            DataType::Float64,
            Some("mm/mm"),
            Some("Field capacity"),
        ),
        field_with_meta(
            "WP",
            DataType::Float64,
            Some("mm/mm"),
            Some("Wilting point"),
        ),
        field_with_meta(
            "Rough",
            DataType::Float64,
            Some("mm"),
            Some("Surface roughness"),
        ),
        field_with_meta(
            "Ki",
            DataType::Float64,
            Some("adjsmt"),
            Some("Interrill erodibility adjustment factor"),
        ),
        field_with_meta(
            "Kr",
            DataType::Float64,
            Some("adjsmt"),
            Some("Rill erodibility adjustment factor"),
        ),
        field_with_meta(
            "Tauc",
            DataType::Float64,
            Some("adjsmt"),
            Some("Critical shear stress adjustment factor"),
        ),
        field_with_meta(
            "Saturation",
            DataType::Float64,
            Some("frac"),
            Some("Saturation as fraction (10mm profile)"),
        ),
        field_with_meta(
            "TSW",
            DataType::Float64,
            Some("mm"),
            Some("Total soil water"),
        ),
    ];
    schema_with_version(Schema::new(fields), version)
}

pub fn hill_wat_schema(version: &VersionInfo) -> Schema {
    let fields = vec![
        field_with_meta("wepp_id", DataType::Int32, None, None),
        field_with_meta("ofe_id", DataType::Int16, None, None),
        field_with_meta("year", DataType::Int16, None, None),
        field_with_meta(
            "sim_day_index",
            DataType::Int32,
            None,
            Some("1-indexed simulation day"),
        ),
        field_with_meta("julian", DataType::Int16, None, None),
        field_with_meta("month", DataType::Int8, None, None),
        field_with_meta("day_of_month", DataType::Int8, None, None),
        field_with_meta("water_year", DataType::Int16, None, None),
        field_with_meta("OFE", DataType::Int16, None, None),
        field_with_meta("P", DataType::Float64, Some("mm"), Some("Precipitation")),
        field_with_meta(
            "RM",
            DataType::Float64,
            Some("mm"),
            Some("Rainfall+Irrigation+Snowmelt"),
        ),
        field_with_meta(
            "Q",
            DataType::Float64,
            Some("mm"),
            Some("Daily runoff over eff length"),
        ),
        field_with_meta(
            "Ep",
            DataType::Float64,
            Some("mm"),
            Some("Plant transpiration"),
        ),
        field_with_meta(
            "Es",
            DataType::Float64,
            Some("mm"),
            Some("Soil evaporation"),
        ),
        field_with_meta(
            "Er",
            DataType::Float64,
            Some("mm"),
            Some("Residue evaporation"),
        ),
        field_with_meta(
            "Dp",
            DataType::Float64,
            Some("mm"),
            Some("Deep percolation"),
        ),
        field_with_meta(
            "UpStrmQ",
            DataType::Float64,
            Some("mm"),
            Some("Runon added to OFE"),
        ),
        field_with_meta(
            "SubRIn",
            DataType::Float64,
            Some("mm"),
            Some("Subsurface runon added to OFE"),
        ),
        field_with_meta(
            "latqcc",
            DataType::Float64,
            Some("mm"),
            Some("Lateral subsurface flow"),
        ),
        field_with_meta(
            "Total-Soil Water",
            DataType::Float64,
            Some("mm"),
            Some("Unfrozen water in soil profile"),
        ),
        field_with_meta(
            "frozwt",
            DataType::Float64,
            Some("mm"),
            Some("Frozen water in soil profile"),
        ),
        field_with_meta(
            "Snow-Water",
            DataType::Float64,
            Some("mm"),
            Some("Water in surface snow"),
        ),
        field_with_meta(
            "QOFE",
            DataType::Float64,
            Some("mm"),
            Some("Daily runoff scaled to single OFE"),
        ),
        field_with_meta("Tile", DataType::Float64, Some("mm"), Some("Tile drainage")),
        field_with_meta("Irr", DataType::Float64, Some("mm"), Some("Irrigation")),
        field_with_meta(
            "Area",
            DataType::Float64,
            Some("m^2"),
            Some("Area that depths apply over"),
        ),
        field_with_meta(
            "SoilWaterTotal",
            DataType::Float64,
            Some("mm"),
            Some("Full-profile soil water depth (watcon + frozwt), optional producer-authoritative term"),
        ),
        field_with_meta(
            "ProfileDepth",
            DataType::Float64,
            Some("mm"),
            Some("Full soil profile depth (solthk(nsl)), optional producer-authoritative term"),
        ),
        field_with_meta(
            "ProfilePorosityCap",
            DataType::Float64,
            Some("mm"),
            Some("Full-profile porosity storage capacity (sum(por * dg)), optional producer-authoritative term"),
        ),
        field_with_meta(
            "ProfileFCStore",
            DataType::Float64,
            Some("mm"),
            Some("Full-profile field-capacity storage (sum(thetfc * dg)), optional producer-authoritative term"),
        ),
        field_with_meta(
            "ProfileWPStore",
            DataType::Float64,
            Some("mm"),
            Some("Full-profile wilting-point storage (sum(thetdr * dg)), optional producer-authoritative term"),
        ),
        field_with_meta(
            "InterceptionStorage",
            DataType::Float64,
            Some("mm"),
            Some("Plant/residue interception carryover storage (pintlv + resint), optional producer-authoritative term"),
        ),
    ];
    schema_with_version(Schema::new(fields), version)
}

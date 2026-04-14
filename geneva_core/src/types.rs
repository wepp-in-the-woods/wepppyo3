#[derive(Debug, Clone, PartialEq)]
pub struct HruRow {
    pub hru_id: String,
    pub area_m2: f64,
    pub cn_arc_ii: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StormEvent {
    pub storm_id: String,
    pub duration_minutes: f64,
    pub depth_mm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    pub kernel_schema_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StormResult {
    pub storm_id: String,
    pub peak_discharge_cms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchResult {
    pub storms_total: usize,
    pub storms_completed: usize,
}

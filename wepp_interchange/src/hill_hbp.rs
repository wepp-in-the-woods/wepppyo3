use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::calendar::{
    compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar,
    CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::hill_pass::{extract_wepp_id, PassColumns};
use crate::schema::VersionInfo;

const MAGIC: &[u8; 8] = b"WFPHBP01";
const FOOTER_MAGIC: &[u8; 8] = b"ENDHBP01";
const SUPPORTED_MAJOR: u16 = 1;
const SUPPORTED_MINOR: u16 = 0;
const SCALE_I64: f64 = 1e-9;
const DIM_SCALAR: u8 = 0;
const DIM_NOFE: u8 = 1;
const DIM_NOFE_LAYERS: u8 = 2;

const REQUIRED_STATE_IDS: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 100, 101, 102, 103, 104, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209,
    210, 300, 900, 901,
];

#[derive(Clone, Copy)]
struct YearEntry {
    sim_year_index: u32,
    calendar_year: i32,
    days_in_year: u16,
    first_julian_day: u16,
    last_julian_day: u16,
}

#[derive(Clone, Copy)]
struct DirectoryEntry {
    sim_year_index: u32,
    calendar_year: i32,
    julian_day: u16,
    event_kind: u8,
    payload_offset: usize,
    payload_length: usize,
    payload_crc32c: u32,
}

struct Layout {
    begin_year: i32,
    npart: usize,
    nofe: u32,
    max_layers: u32,
    years: Vec<YearEntry>,
    entries: Vec<DirectoryEntry>,
    directory_start: usize,
    directory_end: usize,
    footer_start: usize,
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn require(&self, count: usize) -> Result<(), &'static str> {
        if self.pos + count > self.data.len() {
            return Err("truncated payload");
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        self.require(1)?;
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        self.require(2)?;
        let value = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        self.require(4)?;
        let value = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(value)
    }

    fn i32(&mut self) -> Result<i32, &'static str> {
        self.require(4)?;
        let value = i32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(value)
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        self.require(8)?;
        let value = u64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(value)
    }

    fn i64(&mut self) -> Result<i64, &'static str> {
        self.require(8)?;
        let value = i64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(value)
    }

    fn f64(&mut self) -> Result<f64, &'static str> {
        self.require(8)?;
        let value = f64::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
            self.data[self.pos + 4],
            self.data[self.pos + 5],
            self.data[self.pos + 6],
            self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(value)
    }

    fn raw(&mut self, count: usize) -> Result<&'a [u8], &'static str> {
        self.require(count)?;
        let start = self.pos;
        let end = start + count;
        self.pos = end;
        Ok(&self.data[start..end])
    }

    fn string(&mut self) -> Result<String, &'static str> {
        let length = self.u32()? as usize;
        let raw = self.raw(length)?;
        std::str::from_utf8(raw)
            .map(|value| value.to_string())
            .map_err(|_| "invalid utf8 string")
    }
}

fn parse_error(path: &Path, message: impl Into<String>) -> InterchangeError {
    InterchangeError::parse(path, None, message.into(), None)
}

fn crc32c(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for value in data {
        crc ^= *value as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0x82F63B78;
            } else {
                crc >>= 1;
            }
            crc &= 0xFFFF_FFFF;
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn expected_state_schema(state_id: u16) -> Option<(u8, u8, u16, u8, u8)> {
    match state_id {
        1 => Some((1, 1, 1, 1, DIM_NOFE)),
        2 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        3 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        4 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        5 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        6 => Some((1, 2, 3, 2, DIM_NOFE_LAYERS)),
        7 => Some((1, 2, 3, 2, DIM_NOFE_LAYERS)),
        100 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        101 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        102 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        103 => Some((1, 1, 2, 1, DIM_NOFE)),
        104 => Some((1, 1, 2, 1, DIM_NOFE)),
        200 => Some((1, 1, 2, 1, DIM_NOFE)),
        201 => Some((1, 2, 4, 1, DIM_NOFE)),
        202 => Some((1, 1, 2, 1, DIM_NOFE)),
        203 => Some((1, 1, 2, 1, DIM_NOFE)),
        204 => Some((1, 1, 2, 1, DIM_NOFE)),
        205 => Some((1, 1, 2, 1, DIM_NOFE)),
        206 => Some((1, 1, 2, 1, DIM_NOFE)),
        207 => Some((1, 1, 2, 1, DIM_NOFE)),
        208 => Some((1, 1, 2, 1, DIM_NOFE)),
        209 => Some((1, 1, 2, 1, DIM_NOFE)),
        210 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        300 => Some((1, 1, 5, 0, DIM_SCALAR)),
        900 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        901 => Some((1, 1, 2, 2, DIM_NOFE_LAYERS)),
        _ => None,
    }
}

fn expected_dims(kind: u8, layout: &Layout) -> Result<Vec<u32>, String> {
    match kind {
        DIM_SCALAR => Ok(vec![]),
        DIM_NOFE => Ok(vec![layout.nofe]),
        DIM_NOFE_LAYERS => Ok(vec![layout.nofe, layout.max_layers]),
        _ => Err("unknown registry dimension kind".to_string()),
    }
}

fn key_in_year_table(entry: &DirectoryEntry, years: &[YearEntry]) -> bool {
    years.iter().any(|year| {
        entry.sim_year_index == year.sim_year_index
            && entry.calendar_year == year.calendar_year
            && entry.julian_day >= year.first_julian_day
            && entry.julian_day <= year.last_julian_day
    })
}

fn validate_year_table(
    path: &Path,
    years: &[YearEntry],
    nyear: u32,
) -> Result<u32, InterchangeError> {
    if years.len() != nyear as usize {
        return Err(parse_error(path, "year table count mismatch"));
    }

    let mut expected_record_count = 0u32;
    for (index, year) in years.iter().enumerate() {
        if year.sim_year_index != (index + 1) as u32 {
            return Err(parse_error(
                path,
                "year table sim_year_index must be one-based and ordered",
            ));
        }
        if year.days_in_year < 1 {
            return Err(parse_error(
                path,
                "year table days_in_year must be positive",
            ));
        }
        if year.first_julian_day < 1 || year.last_julian_day < year.first_julian_day {
            return Err(parse_error(path, "year table julian-day range is invalid"));
        }
        if year.days_in_year != (year.last_julian_day - year.first_julian_day + 1) {
            return Err(parse_error(
                path,
                "year table days_in_year must match julian-day range",
            ));
        }
        expected_record_count += year.days_in_year as u32;
    }

    Ok(expected_record_count)
}

fn parse_layout(data: &[u8], path: &Path) -> Result<Layout, InterchangeError> {
    let mut cursor = Cursor::new(data, 0);

    let magic = cursor.raw(8).map_err(|msg| parse_error(path, msg))?;
    if magic != MAGIC {
        return Err(parse_error(path, "bad magic"));
    }

    let schema_major = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    let schema_minor = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    if schema_major != SUPPORTED_MAJOR {
        return Err(parse_error(path, "unsupported schema major"));
    }
    if schema_minor > SUPPORTED_MINOR {
        return Err(parse_error(path, "unsupported schema minor"));
    }

    let endianness = cursor.u8().map_err(|msg| parse_error(path, msg))?;
    if endianness != 1 {
        return Err(parse_error(path, "unsupported endianness"));
    }

    let header_bytes = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
    if header_bytes > data.len() {
        return Err(parse_error(path, "header length exceeds file length"));
    }

    let _compatibility_id = cursor.raw(32).map_err(|msg| parse_error(path, msg))?;
    let _artifact_role = cursor.u8().map_err(|msg| parse_error(path, msg))?;
    let _producer = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _run_id = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _created_utc = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _unit_policy_id = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _state_registry_id = cursor.raw(32).map_err(|msg| parse_error(path, msg))?;

    let header_crc_pos = cursor.pos;
    let header_crc = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    if cursor.pos != header_bytes {
        return Err(parse_error(path, "header length mismatch"));
    }

    let mut header_region = data[..header_bytes].to_vec();
    header_region[header_crc_pos..header_crc_pos + 4].fill(0);
    if crc32c(&header_region) != header_crc {
        return Err(parse_error(path, "header crc mismatch"));
    }

    let _hillslope_id = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let nyear = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let begin_year = cursor.i32().map_err(|msg| parse_error(path, msg))?;
    let npart = cursor.u16().map_err(|msg| parse_error(path, msg))? as usize;
    let nofe = cursor.u16().map_err(|msg| parse_error(path, msg))? as u32;
    let max_layers = cursor.u16().map_err(|msg| parse_error(path, msg))? as u32;
    let _calendar_policy_id = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _event_enum_version = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    let _simulation_mode = cursor.u8().map_err(|msg| parse_error(path, msg))?;

    let _climate_file_name = cursor.string().map_err(|msg| parse_error(path, msg))?;
    let _hillslope_area_i64 = cursor.i64().map_err(|msg| parse_error(path, msg))?;
    let particle_count = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
    for _ in 0..particle_count {
        let _ = cursor.f64().map_err(|msg| parse_error(path, msg))?;
    }
    if particle_count != npart {
        return Err(parse_error(path, "event sediment count mismatch"));
    }
    let _srp = cursor.f64().map_err(|msg| parse_error(path, msg))?;
    let _slfp = cursor.f64().map_err(|msg| parse_error(path, msg))?;
    let _bfp = cursor.f64().map_err(|msg| parse_error(path, msg))?;
    let _scp = cursor.f64().map_err(|msg| parse_error(path, msg))?;

    let year_count = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let mut years = Vec::with_capacity(year_count as usize);
    for _ in 0..year_count {
        let entry = YearEntry {
            sim_year_index: cursor.u32().map_err(|msg| parse_error(path, msg))?,
            calendar_year: cursor.i32().map_err(|msg| parse_error(path, msg))?,
            days_in_year: cursor.u16().map_err(|msg| parse_error(path, msg))?,
            first_julian_day: cursor.u16().map_err(|msg| parse_error(path, msg))?,
            last_julian_day: cursor.u16().map_err(|msg| parse_error(path, msg))?,
        };
        let _single_storm_flag = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        years.push(entry);
    }

    let expected_record_count = validate_year_table(path, &years, nyear)?;

    let registry_count = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let mut registry_state_ids = Vec::with_capacity(registry_count as usize);
    let mut registry_seen: HashSet<u16> = HashSet::new();

    for _ in 0..registry_count {
        let state_id = cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let required_flag = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let representation_class = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let unit_class = cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let rank = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let dims_kind = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let _name = cursor.string().map_err(|msg| parse_error(path, msg))?;

        if !registry_seen.insert(state_id) {
            return Err(parse_error(path, "duplicate registry state id"));
        }
        registry_state_ids.push(state_id);

        if let Some(expected) = expected_state_schema(state_id) {
            if expected
                != (
                    required_flag,
                    representation_class,
                    unit_class,
                    rank,
                    dims_kind,
                )
            {
                return Err(parse_error(
                    path,
                    "state registry block does not match PS-03 schema",
                ));
            }
        }
    }

    let registry_set: HashSet<u16> = registry_state_ids.into_iter().collect();
    if let Some(missing) = REQUIRED_STATE_IDS
        .iter()
        .find(|state_id| !registry_set.contains(state_id))
    {
        return Err(parse_error(
            path,
            format!("registry required state id missing: {missing}"),
        ));
    }

    let directory_start = cursor.pos;
    let record_count = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
    let mut entries = Vec::with_capacity(record_count);

    for _ in 0..record_count {
        let entry_offset = cursor.pos;
        let sim_year_index = cursor.u32().map_err(|msg| parse_error(path, msg))?;
        let calendar_year = cursor.i32().map_err(|msg| parse_error(path, msg))?;
        let julian_day = cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let event_kind = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let payload_offset = cursor.u64().map_err(|msg| parse_error(path, msg))? as usize;
        let payload_length = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
        let payload_crc32c = cursor.u32().map_err(|msg| parse_error(path, msg))?;
        let _ = entry_offset;
        entries.push(DirectoryEntry {
            sim_year_index,
            calendar_year,
            julian_day,
            event_kind,
            payload_offset,
            payload_length,
            payload_crc32c,
        });
    }

    let directory_end = cursor.pos;
    if entries.is_empty() {
        return Err(parse_error(path, "empty day directory"));
    }
    if record_count != expected_record_count as usize {
        return Err(parse_error(
            path,
            "directory record count must equal sum of year-table days",
        ));
    }

    let mut previous_key: Option<(u32, u16)> = None;
    let mut expected_payload_offset = directory_end;
    for entry in &entries {
        let key = (entry.sim_year_index, entry.julian_day);
        if !key_in_year_table(entry, &years) {
            return Err(parse_error(path, "directory key is outside the year table"));
        }
        if let Some(previous) = previous_key {
            if key <= previous {
                return Err(parse_error(
                    path,
                    "directory keys must be deterministic and strictly ordered",
                ));
            }
        }
        if entry.payload_offset != expected_payload_offset {
            return Err(parse_error(path, "payload offsets are not deterministic"));
        }
        if entry.payload_length < 1 {
            return Err(parse_error(path, "payload length must be positive"));
        }
        expected_payload_offset += entry.payload_length;
        previous_key = Some(key);
    }

    let footer_start = expected_payload_offset;
    if footer_start + 20 > data.len() {
        return Err(parse_error(path, "truncated footer"));
    }

    let mut footer_cursor = Cursor::new(data, footer_start);
    let directory_crc = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let file_crc_pos = footer_cursor.pos;
    let file_crc = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let footer_record_count = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let footer_magic = footer_cursor.raw(8).map_err(|msg| parse_error(path, msg))?;

    if crc32c(&data[directory_start..directory_end]) != directory_crc {
        return Err(parse_error(path, "directory crc mismatch"));
    }

    let mut file_region = data.to_vec();
    file_region[file_crc_pos..file_crc_pos + 4].fill(0);
    if crc32c(&file_region) != file_crc {
        return Err(parse_error(path, "file crc mismatch"));
    }

    if footer_record_count != expected_record_count {
        return Err(parse_error(
            path,
            "footer record count must equal sum of year-table days",
        ));
    }

    if footer_magic != FOOTER_MAGIC {
        return Err(parse_error(path, "bad footer magic"));
    }

    Ok(Layout {
        begin_year,
        npart,
        nofe,
        max_layers,
        years,
        entries,
        directory_start,
        directory_end,
        footer_start,
    })
}

fn scaled_i64(value: i64) -> f64 {
    value as f64 * SCALE_I64
}

#[derive(Default)]
struct HbpRow {
    event: String,
    year: i16,
    sim_day_index: i32,
    julian: i16,
    month: i8,
    day_of_month: i8,
    water_year: i16,
    dur: f64,
    tcs: f64,
    oalpha: f64,
    runoff: f64,
    runvol: f64,
    sbrunf: f64,
    sbrunv: f64,
    drainq: f64,
    drrunv: f64,
    peakro: f64,
    tdet: f64,
    tdep: f64,
    sedcon_1: f64,
    sedcon_2: f64,
    sedcon_3: f64,
    sedcon_4: f64,
    sedcon_5: f64,
    clot: f64,
    slot: f64,
    saot: f64,
    laot: f64,
    sdot: f64,
    gwbfv: f64,
    gwdsv: f64,
}

impl HbpRow {
    fn push_into(self, out: &mut PassColumns, wepp_id: i32) {
        out.wepp_id.push(wepp_id);
        out.event.push(self.event);
        out.year.push(self.year);
        out.sim_day_index.push(self.sim_day_index);
        out.julian.push(self.julian);
        out.month.push(self.month);
        out.day_of_month.push(self.day_of_month);
        out.water_year.push(self.water_year);
        out.dur.push(self.dur);
        out.tcs.push(self.tcs);
        out.oalpha.push(self.oalpha);
        out.runoff.push(self.runoff);
        out.runvol.push(self.runvol);
        out.sbrunf.push(self.sbrunf);
        out.sbrunv.push(self.sbrunv);
        out.drainq.push(self.drainq);
        out.drrunv.push(self.drrunv);
        out.peakro.push(self.peakro);
        out.tdet.push(self.tdet);
        out.tdep.push(self.tdep);
        out.sedcon_1.push(self.sedcon_1);
        out.sedcon_2.push(self.sedcon_2);
        out.sedcon_3.push(self.sedcon_3);
        out.sedcon_4.push(self.sedcon_4);
        out.sedcon_5.push(self.sedcon_5);
        out.clot.push(self.clot);
        out.slot.push(self.slot);
        out.saot.push(self.saot);
        out.laot.push(self.laot);
        out.sdot.push(self.sdot);
        out.gwbfv.push(self.gwbfv);
        out.gwdsv.push(self.gwdsv);
    }
}

fn parse_payload_into(
    data: &[u8],
    entry: &DirectoryEntry,
    layout: &Layout,
    path: &Path,
    lookup: Option<&CalendarLookup>,
    out: &mut PassColumns,
    wepp_id: i32,
) -> Result<(), InterchangeError> {
    if entry.payload_offset + entry.payload_length > data.len() {
        return Err(parse_error(path, "truncated payload"));
    }

    let payload = &data[entry.payload_offset..entry.payload_offset + entry.payload_length];
    if crc32c(payload) != entry.payload_crc32c {
        return Err(parse_error(path, "payload crc mismatch"));
    }

    let mut cursor = Cursor::new(payload, 0);
    let sim_year_index = cursor.u32().map_err(|msg| parse_error(path, msg))?;
    let calendar_year = cursor.i32().map_err(|msg| parse_error(path, msg))?;
    let julian_day = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    let event_kind = cursor.u8().map_err(|msg| parse_error(path, msg))?;
    let payload_schema_minor = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    let state_snapshot_count = cursor.u16().map_err(|msg| parse_error(path, msg))? as usize;

    if (sim_year_index, calendar_year, julian_day, event_kind)
        != (
            entry.sim_year_index,
            entry.calendar_year,
            entry.julian_day,
            entry.event_kind,
        )
    {
        return Err(parse_error(path, "payload and directory key mismatch"));
    }

    if payload_schema_minor > SUPPORTED_MINOR {
        return Err(parse_error(path, "unsupported payload minor"));
    }

    let month_day = julian_to_calendar(calendar_year, julian_day as i32, lookup);
    let water_year = determine_wateryear(calendar_year, julian_day as i32);
    let sim_day_index =
        compute_sim_day_index(calendar_year, julian_day as i32, layout.begin_year, lookup);
    if sim_day_index < 1 {
        return Err(parse_error(
            path,
            format!("Computed negative simulation day index ({sim_day_index})"),
        ));
    }

    let mut row = HbpRow {
        event: match event_kind {
            0 => "NO EVENT".to_string(),
            1 => "SUBEVENT".to_string(),
            2 => "EVENT".to_string(),
            _ => return Err(parse_error(path, "unsupported event kind")),
        },
        year: calendar_year as i16,
        sim_day_index,
        julian: julian_day as i16,
        month: month_day.0 as i8,
        day_of_month: month_day.1 as i8,
        water_year: water_year as i16,
        ..Default::default()
    };

    match event_kind {
        0 => {
            row.gwbfv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.gwdsv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
        }
        1 => {
            row.sbrunf = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.sbrunv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.drainq = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.drrunv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.gwbfv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.gwdsv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
        }
        2 => {
            row.dur = cursor.f64().map_err(|msg| parse_error(path, msg))?;
            row.tcs = cursor.f64().map_err(|msg| parse_error(path, msg))?;
            row.oalpha = cursor.f64().map_err(|msg| parse_error(path, msg))?;
            row.runoff = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.runvol = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.sbrunf = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.sbrunv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.drainq = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.drrunv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.peakro = cursor.f64().map_err(|msg| parse_error(path, msg))?;
            row.tdet = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.tdep = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);

            let sediment_count = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
            if sediment_count != layout.npart {
                return Err(parse_error(path, "event sediment count mismatch"));
            }
            if sediment_count > 5 {
                return Err(parse_error(path, "unsupported sediment class count"));
            }
            let mut sed_values = Vec::with_capacity(sediment_count);
            for _ in 0..sediment_count {
                sed_values.push(cursor.f64().map_err(|msg| parse_error(path, msg))?);
            }

            let fraction_count = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
            if fraction_count != layout.npart {
                return Err(parse_error(path, "event particle fraction count mismatch"));
            }
            if fraction_count > 5 {
                return Err(parse_error(path, "unsupported particle fraction count"));
            }
            let mut fraction_values = Vec::with_capacity(fraction_count);
            for _ in 0..fraction_count {
                fraction_values.push(cursor.f64().map_err(|msg| parse_error(path, msg))?);
            }

            row.sedcon_1 = *sed_values.first().unwrap_or(&0.0);
            row.sedcon_2 = *sed_values.get(1).unwrap_or(&0.0);
            row.sedcon_3 = *sed_values.get(2).unwrap_or(&0.0);
            row.sedcon_4 = *sed_values.get(3).unwrap_or(&0.0);
            row.sedcon_5 = *sed_values.get(4).unwrap_or(&0.0);
            row.clot = *fraction_values.first().unwrap_or(&0.0);
            row.slot = *fraction_values.get(1).unwrap_or(&0.0);
            row.saot = *fraction_values.get(2).unwrap_or(&0.0);
            row.laot = *fraction_values.get(3).unwrap_or(&0.0);
            row.sdot = *fraction_values.get(4).unwrap_or(&0.0);

            row.gwbfv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
            row.gwdsv = scaled_i64(cursor.i64().map_err(|msg| parse_error(path, msg))?);
        }
        _ => unreachable!(),
    }

    let mut state_ids_seen: HashSet<u16> = HashSet::new();

    for _ in 0..state_snapshot_count {
        let state_id = cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let entry_length = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
        let entry_end = cursor.pos + entry_length;
        if entry_end > payload.len() {
            return Err(parse_error(path, "truncated state entry"));
        }
        if !state_ids_seen.insert(state_id) {
            return Err(parse_error(path, "duplicate state id"));
        }

        let mut state_cursor = Cursor::new(payload, cursor.pos);
        let required_flag = state_cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let representation_class = state_cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let unit_class = state_cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let rank = state_cursor.u8().map_err(|msg| parse_error(path, msg))? as usize;

        let mut dims = Vec::with_capacity(rank);
        for _ in 0..rank {
            dims.push(state_cursor.u32().map_err(|msg| parse_error(path, msg))?);
        }

        if let Some(expected) = expected_state_schema(state_id) {
            if required_flag != expected.0 {
                return Err(parse_error(
                    path,
                    "state required flag does not match registry",
                ));
            }
            if representation_class != expected.1 {
                return Err(parse_error(
                    path,
                    "state representation does not match registry",
                ));
            }
            if unit_class != expected.2 {
                return Err(parse_error(
                    path,
                    "state unit class does not match registry",
                ));
            }
            if rank as u8 != expected.3 {
                return Err(parse_error(path, "state rank does not match registry"));
            }
            let expected_dims =
                expected_dims(expected.4, layout).map_err(|message| parse_error(path, message))?;
            if dims != expected_dims {
                return Err(parse_error(path, "state dimensions do not match registry"));
            }
        }

        let mut value_count: usize = 1;
        for dim in &dims {
            value_count = value_count.saturating_mul(*dim as usize);
        }

        match representation_class {
            1 => {
                for _ in 0..value_count {
                    let _ = state_cursor.i64().map_err(|msg| parse_error(path, msg))?;
                }
            }
            2 => {
                for _ in 0..value_count {
                    let _ = state_cursor.f64().map_err(|msg| parse_error(path, msg))?;
                }
            }
            _ => return Err(parse_error(path, "unsupported state representation")),
        }

        if state_cursor.pos != entry_end {
            return Err(parse_error(path, "state entry length mismatch"));
        }

        if required_flag != 1 {
            return Err(parse_error(path, "required state marked optional"));
        }

        cursor.pos = entry_end;
    }

    if cursor.pos != payload.len() {
        return Err(parse_error(path, "payload has trailing bytes"));
    }

    if let Some(missing) = REQUIRED_STATE_IDS
        .iter()
        .find(|state_id| !state_ids_seen.contains(state_id))
    {
        return Err(parse_error(
            path,
            format!("required state id missing: {missing}"),
        ));
    }

    row.push_into(out, wepp_id);
    Ok(())
}

pub fn hillslope_hbp_to_columns(
    path: &Path,
    cli_calendar_path: Option<&Path>,
    _version: &VersionInfo,
) -> Result<PassColumns, InterchangeError> {
    let lookup = match cli_calendar_path {
        Some(path) => Some(load_cli_calendar(path)?),
        None => None,
    };

    let wepp_id = extract_wepp_id(path)?;

    let mut file = File::open(path).map_err(|err| InterchangeError::io(path, err))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|err| InterchangeError::io(path, err))?;

    let layout = parse_layout(&data, path)?;
    let _ = (
        layout.directory_start,
        layout.directory_end,
        layout.footer_start,
        layout.years.len(),
    );

    let mut out = PassColumns::new();
    for entry in &layout.entries {
        parse_payload_into(
            &data,
            entry,
            &layout,
            path,
            lookup.as_ref(),
            &mut out,
            wepp_id,
        )?;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::hillslope_hbp_to_columns;
    use crate::errors::InterchangeError;
    use crate::schema::VersionInfo;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_hbp(bytes: &[u8]) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wepp_interchange_hbp_tests_{nonce}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        let path = dir.join("H1.hbp");
        fs::write(&path, bytes).expect("fixture should be written");
        path
    }

    fn assert_parse_message(err: InterchangeError, expected_fragment: &str) {
        match err {
            InterchangeError::Parse { message, .. } => {
                assert!(
                    message.contains(expected_fragment),
                    "expected parse message to contain '{expected_fragment}', got '{message}'"
                );
            }
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_bad_magic() {
        let path = write_temp_hbp(b"BADHBP00");
        let version = VersionInfo::new(7, 0);

        let err = match hillslope_hbp_to_columns(&path, None, &version) {
            Ok(_) => panic!("bad magic must fail"),
            Err(err) => err,
        };

        assert_parse_message(err, "bad magic");
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_unsupported_endianness() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"WFPHBP01");
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.push(0u8); // unsupported endianness marker
        let path = write_temp_hbp(&payload);
        let version = VersionInfo::new(7, 0);

        let err = match hillslope_hbp_to_columns(&path, None, &version) {
            Ok(_) => panic!("unsupported endianness must fail"),
            Err(err) => err,
        };

        assert_parse_message(err, "unsupported endianness");
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_truncated_payload() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"WFPHBP01");
        payload.extend_from_slice(&1u16.to_le_bytes());
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.push(1u8); // little endian
        let path = write_temp_hbp(&payload);
        let version = VersionInfo::new(7, 0);

        let err = match hillslope_hbp_to_columns(&path, None, &version) {
            Ok(_) => panic!("truncated header must fail"),
            Err(err) => err,
        };

        assert_parse_message(err, "truncated payload");
    }
}

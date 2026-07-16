use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;

use crate::calendar::{
    compute_sim_day_index, determine_wateryear, julian_to_calendar, load_cli_calendar,
    CalendarLookup,
};
use crate::errors::InterchangeError;
use crate::hill_pass::{extract_wepp_id, PassColumns};
use crate::schema::VersionInfo;

const MAGIC: &[u8; 8] = b"WFPHBP01";
const FOOTER_MAGIC: &[u8; 8] = b"ENDHBP01";
const SUPPORTED_MAJOR_V1: u16 = 1;
const SUPPORTED_MINOR_V1: u16 = 0;
const SUPPORTED_MAJOR_V2: u16 = 2;
const SUPPORTED_MINOR_V2: u16 = 0;
const SCALE_I64: f64 = 1e-9;
const DIM_SCALAR: u8 = 0;
const DIM_NOFE: u8 = 1;
const DIM_NOFE_LAYERS: u8 = 2;
const PAYLOAD_CODEC_ZLIB: u8 = 1;

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
    single_storm_flag: u8,
}

#[derive(Clone, Copy)]
struct DirectoryEntry {
    sim_year_index: u32,
    calendar_year: i32,
    julian_day: u16,
    event_kind: u8,
    payload: EntryPayload,
}

#[derive(Clone, Copy)]
enum EntryPayload {
    SchemaV1 {
        payload_offset: usize,
        payload_length: usize,
        payload_crc32c: u32,
    },
    SchemaV2 {
        payload_block_id: usize,
        day_in_block_index: u16,
        raw_payload_offset: usize,
        raw_payload_length: usize,
        raw_payload_crc32c: u32,
    },
}

#[derive(Clone, Copy)]
struct PayloadBlockEntry {
    sim_year_index: u32,
    stored_block_offset: usize,
    stored_block_length: usize,
    raw_block_length: usize,
    payload_codec: u8,
    stored_block_crc32c: u32,
    raw_block_crc32c: u32,
}

struct Layout {
    schema_major: u16,
    schema_minor: u16,
    begin_year: i32,
    npart: usize,
    nofe: u32,
    max_layers: u32,
    years: Vec<YearEntry>,
    entries: Vec<DirectoryEntry>,
    directory_start: usize,
    directory_end: usize,
    footer_start: usize,
    payload_blocks: Vec<PayloadBlockEntry>,
    raw_payload_blocks: Vec<Vec<u8>>,
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
    schema_major: u16,
    simulation_mode: u8,
) -> Result<u32, InterchangeError> {
    if years.len() != nyear as usize {
        return Err(parse_error(path, "year table count mismatch"));
    }

    if schema_major == SUPPORTED_MAJOR_V2 && simulation_mode != 1 {
        return Err(parse_error(path, "schema 2.0 requires simulation_mode = 1"));
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
        if schema_major == SUPPORTED_MAJOR_V2 {
            if year.days_in_year != 366 {
                return Err(parse_error(
                    path,
                    "schema 2.0 year table days_in_year must be 366",
                ));
            }
            if year.first_julian_day != 1 || year.last_julian_day != 366 {
                return Err(parse_error(
                    path,
                    "schema 2.0 year table range must be 1..366",
                ));
            }
            if year.single_storm_flag != 0 {
                return Err(parse_error(path, "schema 2.0 single_storm_flag must be 0"));
            }
        }
        expected_record_count += year.days_in_year as u32;
    }

    Ok(expected_record_count)
}

fn decode_zlib_block(
    path: &Path,
    source: &[u8],
    expected_raw_length: usize,
) -> Result<Vec<u8>, InterchangeError> {
    let mut decoder = ZlibDecoder::new(source);
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .map_err(|_| parse_error(path, "schema 2.x zlib decode failed"))?;
    if raw.len() != expected_raw_length {
        return Err(parse_error(path, "schema 2.x zlib decoded length mismatch"));
    }
    Ok(raw)
}

fn u64_to_usize(path: &Path, value: u64, field_name: &str) -> Result<usize, InterchangeError> {
    usize::try_from(value)
        .map_err(|_| parse_error(path, format!("{field_name} exceeds platform limits")))
}

fn parse_layout(data: &[u8], path: &Path) -> Result<Layout, InterchangeError> {
    let mut cursor = Cursor::new(data, 0);

    let magic = cursor.raw(8).map_err(|msg| parse_error(path, msg))?;
    if magic != MAGIC {
        return Err(parse_error(path, "bad magic"));
    }

    let schema_major = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    let schema_minor = cursor.u16().map_err(|msg| parse_error(path, msg))?;
    match schema_major {
        SUPPORTED_MAJOR_V1 => {
            if schema_minor > SUPPORTED_MINOR_V1 {
                return Err(parse_error(path, "unsupported schema minor"));
            }
        }
        SUPPORTED_MAJOR_V2 => {
            if schema_minor > SUPPORTED_MINOR_V2 {
                return Err(parse_error(path, "unsupported schema minor"));
            }
        }
        _ => return Err(parse_error(path, "unsupported schema major")),
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
    let artifact_role = cursor.u8().map_err(|msg| parse_error(path, msg))?;
    if artifact_role != 1 {
        return Err(parse_error(path, "unsupported artifact role"));
    }
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
    let simulation_mode = cursor.u8().map_err(|msg| parse_error(path, msg))?;

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
            single_storm_flag: cursor.u8().map_err(|msg| parse_error(path, msg))?,
        };
        years.push(entry);
    }

    let expected_record_count =
        validate_year_table(path, &years, nyear, schema_major, simulation_mode)?;

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
        let sim_year_index = cursor.u32().map_err(|msg| parse_error(path, msg))?;
        let calendar_year = cursor.i32().map_err(|msg| parse_error(path, msg))?;
        let julian_day = cursor.u16().map_err(|msg| parse_error(path, msg))?;
        let event_kind = cursor.u8().map_err(|msg| parse_error(path, msg))?;
        let payload = match schema_major {
            SUPPORTED_MAJOR_V1 => {
                let payload_offset = u64_to_usize(
                    path,
                    cursor.u64().map_err(|msg| parse_error(path, msg))?,
                    "payload_offset_bytes",
                )?;
                let payload_length = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                if payload_length < 1 {
                    return Err(parse_error(path, "payload length must be positive"));
                }
                let payload_crc32c = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                EntryPayload::SchemaV1 {
                    payload_offset,
                    payload_length,
                    payload_crc32c,
                }
            }
            SUPPORTED_MAJOR_V2 => {
                let payload_block_id = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                let day_in_block_index = cursor.u16().map_err(|msg| parse_error(path, msg))?;
                let raw_payload_offset =
                    cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                let raw_payload_length =
                    cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                let raw_payload_crc32c = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                EntryPayload::SchemaV2 {
                    payload_block_id,
                    day_in_block_index,
                    raw_payload_offset,
                    raw_payload_length,
                    raw_payload_crc32c,
                }
            }
            _ => return Err(parse_error(path, "unsupported schema major")),
        };

        entries.push(DirectoryEntry {
            sim_year_index,
            calendar_year,
            julian_day,
            event_kind,
            payload,
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
        previous_key = Some(key);
    }

    match schema_major {
        SUPPORTED_MAJOR_V1 => {
            let mut expected_payload_offset = directory_end;
            for entry in &entries {
                let EntryPayload::SchemaV1 {
                    payload_offset,
                    payload_length,
                    ..
                } = entry.payload
                else {
                    return Err(parse_error(path, "unsupported schema major"));
                };
                if payload_offset != expected_payload_offset {
                    return Err(parse_error(path, "payload offsets are not deterministic"));
                }
                expected_payload_offset = expected_payload_offset
                    .checked_add(payload_length)
                    .ok_or_else(|| parse_error(path, "truncated payload"))?;
            }

            let footer_start = expected_payload_offset;
            let footer_end = footer_start.checked_add(20);
            if footer_end.is_none() || footer_end.unwrap_or(usize::MAX) > data.len() {
                return Err(parse_error(path, "truncated payload"));
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
                schema_major,
                schema_minor,
                begin_year,
                npart,
                nofe,
                max_layers,
                years,
                entries,
                directory_start,
                directory_end,
                footer_start,
                payload_blocks: Vec::new(),
                raw_payload_blocks: Vec::new(),
            })
        }
        SUPPORTED_MAJOR_V2 => {
            let payload_block_table_start = cursor.pos;
            let payload_block_count = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
            if payload_block_count != nyear as usize {
                return Err(parse_error(
                    path,
                    "schema 2.x block count must equal year table count",
                ));
            }

            let mut payload_blocks = Vec::with_capacity(payload_block_count);
            for block_index in 0..payload_block_count {
                let payload_block_id = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                if payload_block_id != block_index as u32 {
                    return Err(parse_error(
                        path,
                        "schema 2.x payload_block_id must be contiguous and ordered",
                    ));
                }
                let block_sim_year_index = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                if block_sim_year_index != (block_index + 1) as u32 {
                    return Err(parse_error(
                        path,
                        "schema 2.x payload block sim_year_index mismatch",
                    ));
                }
                let block_day_slot_count = cursor.u16().map_err(|msg| parse_error(path, msg))?;
                let represented_day_count = cursor.u16().map_err(|msg| parse_error(path, msg))?;
                if block_day_slot_count != 366 || represented_day_count != 366 {
                    return Err(parse_error(
                        path,
                        "schema 2.0 payload block day counts must be 366",
                    ));
                }
                let stored_block_offset = u64_to_usize(
                    path,
                    cursor.u64().map_err(|msg| parse_error(path, msg))?,
                    "stored_block_offset_bytes",
                )?;
                let stored_block_length =
                    cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                let raw_block_length = cursor.u32().map_err(|msg| parse_error(path, msg))? as usize;
                let payload_codec = cursor.u8().map_err(|msg| parse_error(path, msg))?;
                if payload_codec != PAYLOAD_CODEC_ZLIB {
                    return Err(parse_error(path, "schema 2.x payload codec is unsupported"));
                }
                let stored_block_crc32c = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                let raw_block_crc32c = cursor.u32().map_err(|msg| parse_error(path, msg))?;
                if stored_block_length < 1 || raw_block_length < 1 {
                    return Err(parse_error(
                        path,
                        "schema 2.x payload block lengths must be positive",
                    ));
                }
                payload_blocks.push(PayloadBlockEntry {
                    sim_year_index: block_sim_year_index,
                    stored_block_offset,
                    stored_block_length,
                    raw_block_length,
                    payload_codec,
                    stored_block_crc32c,
                    raw_block_crc32c,
                });
            }
            let payload_block_table_end = cursor.pos;

            if data.len() < 28 || payload_block_table_end > data.len() - 28 {
                return Err(parse_error(path, "truncated payload"));
            }
            let footer_start = data.len() - 28;
            let mut footer_cursor = Cursor::new(data, footer_start);
            let directory_crc = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
            let payload_block_table_crc =
                footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
            let file_crc_pos = footer_cursor.pos;
            let file_crc = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
            let footer_record_count = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
            let footer_block_count = footer_cursor.u32().map_err(|msg| parse_error(path, msg))?;
            let footer_magic = footer_cursor.raw(8).map_err(|msg| parse_error(path, msg))?;

            if footer_magic != FOOTER_MAGIC {
                return Err(parse_error(path, "bad footer magic"));
            }
            if footer_record_count != expected_record_count {
                return Err(parse_error(
                    path,
                    "footer record count must equal sum of year-table days",
                ));
            }
            if footer_record_count != 366 * nyear {
                return Err(parse_error(
                    path,
                    "schema 2.0 record count must equal 366 * nyear",
                ));
            }
            if footer_block_count != payload_block_count as u32 {
                return Err(parse_error(path, "schema 2.x footer block count mismatch"));
            }
            if footer_block_count != nyear {
                return Err(parse_error(path, "schema 2.0 block count must equal nyear"));
            }
            if crc32c(&data[directory_start..directory_end]) != directory_crc {
                return Err(parse_error(path, "directory crc mismatch"));
            }
            if crc32c(&data[payload_block_table_start..payload_block_table_end])
                != payload_block_table_crc
            {
                return Err(parse_error(path, "payload block table crc mismatch"));
            }
            let mut file_region = data.to_vec();
            file_region[file_crc_pos..file_crc_pos + 4].fill(0);
            if crc32c(&file_region) != file_crc {
                return Err(parse_error(path, "file crc mismatch"));
            }

            let mut raw_payload_blocks = Vec::with_capacity(payload_blocks.len());
            for block in &payload_blocks {
                if block.payload_codec != PAYLOAD_CODEC_ZLIB {
                    return Err(parse_error(path, "schema 2.x payload codec is unsupported"));
                }
                let stored_end = block
                    .stored_block_offset
                    .checked_add(block.stored_block_length)
                    .ok_or_else(|| parse_error(path, "truncated payload"))?;
                if stored_end > footer_start {
                    return Err(parse_error(path, "truncated payload"));
                }
                let stored_block = &data[block.stored_block_offset..stored_end];
                if crc32c(stored_block) != block.stored_block_crc32c {
                    return Err(parse_error(path, "payload block stored crc mismatch"));
                }
                let raw_block = decode_zlib_block(path, stored_block, block.raw_block_length)?;
                if crc32c(&raw_block) != block.raw_block_crc32c {
                    return Err(parse_error(path, "payload block raw crc mismatch"));
                }
                raw_payload_blocks.push(raw_block);
            }

            let mut block_prev_day_index = vec![-1i32; payload_blocks.len()];
            let mut block_prev_raw_end = vec![0usize; payload_blocks.len()];
            let mut block_seen_count = vec![0usize; payload_blocks.len()];
            for entry in &entries {
                let EntryPayload::SchemaV2 {
                    payload_block_id,
                    day_in_block_index,
                    raw_payload_offset,
                    raw_payload_length,
                    ..
                } = entry.payload
                else {
                    return Err(parse_error(path, "unsupported schema major"));
                };
                if payload_block_id >= payload_blocks.len() {
                    return Err(parse_error(
                        path,
                        "schema 2.x directory block id is out of range",
                    ));
                }
                let block = payload_blocks[payload_block_id];
                if block.sim_year_index != entry.sim_year_index {
                    return Err(parse_error(
                        path,
                        "schema 2.x block sim_year_index must match directory key",
                    ));
                }
                if day_in_block_index > 365 {
                    return Err(parse_error(
                        path,
                        "schema 2.x day_in_block_index is out of range",
                    ));
                }
                if entry.julian_day == 0 || day_in_block_index != entry.julian_day - 1 {
                    return Err(parse_error(
                        path,
                        "schema 2.0 day_in_block_index must equal julian_day - 1",
                    ));
                }
                if raw_payload_length < 1 {
                    return Err(parse_error(path, "schema 2.x raw payload slice is invalid"));
                }
                let raw_payload_end = raw_payload_offset
                    .checked_add(raw_payload_length)
                    .ok_or_else(|| {
                        parse_error(path, "schema 2.x day slice exceeds raw block bounds")
                    })?;
                if raw_payload_end > block.raw_block_length {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slice exceeds raw block bounds",
                    ));
                }
                if day_in_block_index as i32 != block_prev_day_index[payload_block_id] + 1 {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slots must be contiguous in each block",
                    ));
                }
                if raw_payload_offset < block_prev_raw_end[payload_block_id] {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slices overlap in raw block",
                    ));
                }
                if raw_payload_offset > block_prev_raw_end[payload_block_id] {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slices must cover raw block without gaps",
                    ));
                }
                block_prev_raw_end[payload_block_id] = raw_payload_end;
                block_prev_day_index[payload_block_id] = day_in_block_index as i32;
                block_seen_count[payload_block_id] += 1;
            }
            for block_index in 0..payload_blocks.len() {
                if block_seen_count[block_index] != 366 {
                    return Err(parse_error(
                        path,
                        "schema 2.0 payload block must represent 366 day slots",
                    ));
                }
                if block_prev_day_index[block_index] != 365 {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slots must terminate at index 365",
                    ));
                }
                if block_prev_raw_end[block_index] != payload_blocks[block_index].raw_block_length {
                    return Err(parse_error(
                        path,
                        "schema 2.x day slices must cover raw block without gaps",
                    ));
                }
            }

            Ok(Layout {
                schema_major,
                schema_minor,
                begin_year,
                npart,
                nofe,
                max_layers,
                years,
                entries,
                directory_start,
                directory_end,
                footer_start,
                payload_blocks,
                raw_payload_blocks,
            })
        }
        _ => Err(parse_error(path, "unsupported schema major")),
    }
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
    let payload = match entry.payload {
        EntryPayload::SchemaV1 {
            payload_offset,
            payload_length,
            payload_crc32c,
        } => {
            let payload_end = payload_offset
                .checked_add(payload_length)
                .ok_or_else(|| parse_error(path, "truncated payload"))?;
            if payload_end > data.len() {
                return Err(parse_error(path, "truncated payload"));
            }
            let payload = &data[payload_offset..payload_end];
            if crc32c(payload) != payload_crc32c {
                return Err(parse_error(path, "payload crc mismatch"));
            }
            payload.to_vec()
        }
        EntryPayload::SchemaV2 {
            payload_block_id,
            raw_payload_offset,
            raw_payload_length,
            raw_payload_crc32c,
            ..
        } => {
            if payload_block_id >= layout.raw_payload_blocks.len() {
                return Err(parse_error(
                    path,
                    "schema 2.x directory block id is out of range",
                ));
            }
            let raw_block = &layout.raw_payload_blocks[payload_block_id];
            let payload_end = raw_payload_offset
                .checked_add(raw_payload_length)
                .ok_or_else(|| {
                    parse_error(path, "schema 2.x day slice exceeds raw block bounds")
                })?;
            if payload_end > raw_block.len() {
                return Err(parse_error(
                    path,
                    "schema 2.x day slice exceeds raw block bounds",
                ));
            }
            let payload = &raw_block[raw_payload_offset..payload_end];
            if crc32c(payload) != raw_payload_crc32c {
                return Err(parse_error(path, "raw payload crc mismatch"));
            }
            payload.to_vec()
        }
    };

    let mut cursor = Cursor::new(&payload, 0);
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

    let supported_payload_minor = match layout.schema_major {
        SUPPORTED_MAJOR_V1 => SUPPORTED_MINOR_V1,
        SUPPORTED_MAJOR_V2 => SUPPORTED_MINOR_V2,
        _ => return Err(parse_error(path, "unsupported schema major")),
    };
    if payload_schema_minor > supported_payload_minor {
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

        let mut state_cursor = Cursor::new(&payload, cursor.pos);
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
    use super::{
        crc32c, expected_state_schema, hillslope_hbp_to_columns, EntryPayload, REQUIRED_STATE_IDS,
        SUPPORTED_MAJOR_V1, SUPPORTED_MAJOR_V2,
    };
    use crate::ag_fields::Source as AgFieldsSource;
    use crate::errors::InterchangeError;
    use crate::hill_pass::{
        ag_fields_hillslope_pass_files_to_parquet, hillslope_pass_files_to_parquet,
    };
    use crate::schema::VersionInfo;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    const DIR_V2_ROW_SIZE: usize = 29;
    const TABLE_V2_ENTRY_SIZE: usize = 37;

    struct Schema2Fixture {
        bytes: Vec<u8>,
        directory_start: usize,
        directory_len: usize,
        table_start: usize,
        table_len: usize,
        footer_start: usize,
    }

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

    fn put_u8(buf: &mut Vec<u8>, value: u8) {
        buf.push(value);
    }

    fn put_u16(buf: &mut Vec<u8>, value: u16) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_i32(buf: &mut Vec<u8>, value: i32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_u64(buf: &mut Vec<u8>, value: u64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_i64(buf: &mut Vec<u8>, value: i64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_f64(buf: &mut Vec<u8>, value: f64) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn put_string(buf: &mut Vec<u8>, value: &str) {
        put_u32(buf, value.len() as u32);
        buf.extend_from_slice(value.as_bytes());
    }

    fn put_u32_at(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u16_at(buf: &mut [u8], offset: usize, value: u16) {
        buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn read_u32_at(buf: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ])
    }

    fn state_dims(dims_kind: u8, nofe: u32, max_layers: u32) -> Vec<u32> {
        match dims_kind {
            0 => vec![],
            1 => vec![nofe],
            2 => vec![nofe, max_layers],
            _ => panic!("unknown dims_kind {dims_kind}"),
        }
    }

    fn build_state_entry(state_id: u16, nofe: u32, max_layers: u32) -> Vec<u8> {
        let (required_flag, representation_class, unit_class, rank, dims_kind) =
            expected_state_schema(state_id).expect("required state schema should exist");
        let dims = state_dims(dims_kind, nofe, max_layers);
        assert_eq!(dims.len(), rank as usize);

        let mut entry = Vec::new();
        put_u8(&mut entry, required_flag);
        put_u8(&mut entry, representation_class);
        put_u16(&mut entry, unit_class);
        put_u8(&mut entry, rank);
        for dim in &dims {
            put_u32(&mut entry, *dim);
        }
        let value_count = dims.iter().copied().product::<u32>().max(1) as usize;
        match representation_class {
            1 => {
                for _ in 0..value_count {
                    put_i64(&mut entry, 0);
                }
            }
            2 => {
                for _ in 0..value_count {
                    put_f64(&mut entry, 0.0);
                }
            }
            _ => panic!("unsupported representation_class {representation_class}"),
        }

        let mut out = Vec::new();
        put_u16(&mut out, state_id);
        put_u32(&mut out, entry.len() as u32);
        out.extend_from_slice(&entry);
        out
    }

    fn build_no_event_payload(
        sim_year_index: u32,
        calendar_year: i32,
        julian_day: u16,
        nofe: u32,
        max_layers: u32,
        payload_minor: u16,
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        put_u32(&mut payload, sim_year_index);
        put_i32(&mut payload, calendar_year);
        put_u16(&mut payload, julian_day);
        put_u8(&mut payload, 0); // NO_EVENT
        put_u16(&mut payload, payload_minor);
        put_u16(&mut payload, REQUIRED_STATE_IDS.len() as u16);
        put_i64(&mut payload, 0); // baseflow_volume_m3
        put_i64(&mut payload, 0); // dissolved_storage_volume_m3
        for state_id in REQUIRED_STATE_IDS {
            payload.extend_from_slice(&build_state_entry(*state_id, nofe, max_layers));
        }
        payload
    }

    fn append_common_prefix(
        schema_major: u16,
        schema_minor: u16,
        nyear: u32,
        begin_year: i32,
        simulation_mode: u8,
    ) -> Vec<u8> {
        let mut file = Vec::new();

        let mut header = Vec::new();
        header.extend_from_slice(b"WFPHBP01");
        put_u16(&mut header, schema_major);
        put_u16(&mut header, schema_minor);
        put_u8(&mut header, 1); // little endian
        let header_bytes_pos = header.len();
        put_u32(&mut header, 0); // header_bytes placeholder
        header.extend_from_slice(&[0u8; 32]); // compatibility_id
        put_u8(&mut header, 1); // artifact_role hillslope_shard
        put_string(&mut header, "ps15-wepppyo3-test");
        put_string(&mut header, "ps15");
        put_string(&mut header, "2026-05-14T00:00:00Z");
        put_string(&mut header, "metric-v1");
        header.extend_from_slice(&[0u8; 32]); // state_registry_id
        let header_crc_pos = header.len();
        put_u32(&mut header, 0); // header_crc32c placeholder
        let header_bytes = header.len() as u32;
        put_u32_at(&mut header, header_bytes_pos, header_bytes);
        let header_crc = crc32c(&header);
        put_u32_at(&mut header, header_crc_pos, header_crc);
        file.extend_from_slice(&header);

        let npart = 1u16;
        let nofe = 1u16;
        let max_layers = 1u16;
        put_u32(&mut file, 1); // hillslope_id
        put_u32(&mut file, nyear);
        put_i32(&mut file, begin_year);
        put_u16(&mut file, npart);
        put_u16(&mut file, nofe);
        put_u16(&mut file, max_layers);
        put_string(&mut file, "gregorian");
        put_u16(&mut file, 1); // event_enum_version
        put_u8(&mut file, simulation_mode);

        put_string(&mut file, "p1.cli");
        put_i64(&mut file, 0); // area scaled
        put_u32(&mut file, npart as u32);
        put_f64(&mut file, 0.001);
        put_f64(&mut file, 0.0);
        put_f64(&mut file, 0.0);
        put_f64(&mut file, 0.0);
        put_f64(&mut file, 0.0);

        put_u32(&mut file, nyear);
        for y in 0..nyear {
            put_u32(&mut file, y + 1);
            put_i32(&mut file, begin_year + y as i32);
            if schema_major == SUPPORTED_MAJOR_V2 {
                put_u16(&mut file, 366);
                put_u16(&mut file, 1);
                put_u16(&mut file, 366);
                put_u8(&mut file, 0);
            } else {
                put_u16(&mut file, 1);
                put_u16(&mut file, 1);
                put_u16(&mut file, 1);
                put_u8(&mut file, 0);
            }
        }

        put_u32(&mut file, REQUIRED_STATE_IDS.len() as u32);
        for state_id in REQUIRED_STATE_IDS {
            let (required_flag, representation_class, unit_class, rank, dims_kind) =
                expected_state_schema(*state_id).expect("required state schema should exist");
            put_u16(&mut file, *state_id);
            put_u8(&mut file, required_flag);
            put_u8(&mut file, representation_class);
            put_u16(&mut file, unit_class);
            put_u8(&mut file, rank);
            put_u8(&mut file, dims_kind);
            put_string(&mut file, &format!("state_{state_id}"));
        }

        file
    }

    fn build_schema1_fixture() -> Vec<u8> {
        let mut file = append_common_prefix(SUPPORTED_MAJOR_V1, 0, 1, 2004, 1);
        let payload = build_no_event_payload(1, 2004, 1, 1, 1, 0);
        let payload_crc = crc32c(&payload);

        let directory_start = file.len();
        let directory_len = 4 + 27;
        let payload_offset = directory_start + directory_len;
        let mut directory = Vec::new();
        put_u32(&mut directory, 1); // record_count
        put_u32(&mut directory, 1); // sim_year_index
        put_i32(&mut directory, 2004);
        put_u16(&mut directory, 1);
        put_u8(&mut directory, 0); // NO_EVENT
        put_u64(&mut directory, payload_offset as u64);
        put_u32(&mut directory, payload.len() as u32);
        put_u32(&mut directory, payload_crc);

        file.extend_from_slice(&directory);
        file.extend_from_slice(&payload);

        let directory_crc = crc32c(&directory);
        put_u32(&mut file, directory_crc);
        let file_crc_pos = file.len();
        put_u32(&mut file, 0);
        put_u32(&mut file, 1); // record_count
        file.extend_from_slice(b"ENDHBP01");
        let file_crc = crc32c(&file);
        put_u32_at(&mut file, file_crc_pos, file_crc);
        file
    }

    fn build_schema2_fixture() -> Schema2Fixture {
        let nyear = 1u32;
        let begin_year = 2004i32;
        let mut file = append_common_prefix(SUPPORTED_MAJOR_V2, 0, nyear, begin_year, 1);

        let mut raw_offsets = Vec::with_capacity(366);
        let mut raw_lengths = Vec::with_capacity(366);
        let mut raw_payload_crcs = Vec::with_capacity(366);
        let mut raw_block = Vec::new();
        for day in 1..=366u16 {
            let payload = build_no_event_payload(1, begin_year, day, 1, 1, 0);
            raw_offsets.push(raw_block.len() as u32);
            raw_lengths.push(payload.len() as u32);
            raw_payload_crcs.push(crc32c(&payload));
            raw_block.extend_from_slice(&payload);
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(&raw_block)
            .expect("schema2 raw block should compress");
        let stored_block = encoder.finish().expect("zlib encoder should finish");
        let stored_crc = crc32c(&stored_block);
        let raw_block_crc = crc32c(&raw_block);

        let directory_start = file.len();
        let directory_len = 4 + 366 * DIR_V2_ROW_SIZE;
        let table_start = directory_start + directory_len;
        let table_len = 4 + TABLE_V2_ENTRY_SIZE;
        let payload_block_region_start = table_start + table_len;
        let stored_block_offset = payload_block_region_start as u64;

        let mut directory = Vec::new();
        put_u32(&mut directory, 366);
        for day in 1..=366u16 {
            let idx = (day - 1) as usize;
            put_u32(&mut directory, 1); // sim_year_index
            put_i32(&mut directory, begin_year);
            put_u16(&mut directory, day);
            put_u8(&mut directory, 0); // NO_EVENT
            put_u32(&mut directory, 0); // payload_block_id
            put_u16(&mut directory, day - 1); // day_in_block_index
            put_u32(&mut directory, raw_offsets[idx]);
            put_u32(&mut directory, raw_lengths[idx]);
            put_u32(&mut directory, raw_payload_crcs[idx]);
        }

        let mut table = Vec::new();
        put_u32(&mut table, 1); // block_count
        put_u32(&mut table, 0); // payload_block_id
        put_u32(&mut table, 1); // sim_year_index
        put_u16(&mut table, 366); // block_day_slot_count
        put_u16(&mut table, 366); // represented_day_count
        put_u64(&mut table, stored_block_offset);
        put_u32(&mut table, stored_block.len() as u32);
        put_u32(&mut table, raw_block.len() as u32);
        put_u8(&mut table, 1); // payload_codec zlib
        put_u32(&mut table, stored_crc);
        put_u32(&mut table, raw_block_crc);

        file.extend_from_slice(&directory);
        file.extend_from_slice(&table);
        file.extend_from_slice(&stored_block);
        let footer_start = file.len();

        let directory_crc = crc32c(&directory);
        let table_crc = crc32c(&table);
        put_u32(&mut file, directory_crc);
        put_u32(&mut file, table_crc);
        let file_crc_pos = file.len();
        put_u32(&mut file, 0);
        put_u32(&mut file, 366); // record_count
        put_u32(&mut file, 1); // block_count
        file.extend_from_slice(b"ENDHBP01");
        let file_crc = crc32c(&file);
        put_u32_at(&mut file, file_crc_pos, file_crc);

        Schema2Fixture {
            bytes: file,
            directory_start,
            directory_len,
            table_start,
            table_len,
            footer_start,
        }
    }

    fn schema2_row_start(fixture: &Schema2Fixture, day_slot: usize) -> usize {
        fixture.directory_start + 4 + day_slot * DIR_V2_ROW_SIZE
    }

    fn schema2_block_entry_start(fixture: &Schema2Fixture) -> usize {
        fixture.table_start + 4
    }

    fn refresh_schema2_crc_fields(fixture: &mut Schema2Fixture) {
        let directory_end = fixture.directory_start + fixture.directory_len;
        let table_end = fixture.table_start + fixture.table_len;
        let directory_crc = crc32c(&fixture.bytes[fixture.directory_start..directory_end]);
        let table_crc = crc32c(&fixture.bytes[fixture.table_start..table_end]);
        put_u32_at(&mut fixture.bytes, fixture.footer_start, directory_crc);
        put_u32_at(&mut fixture.bytes, fixture.footer_start + 4, table_crc);
        put_u32_at(&mut fixture.bytes, fixture.footer_start + 8, 0);
        let file_crc = crc32c(&fixture.bytes);
        put_u32_at(&mut fixture.bytes, fixture.footer_start + 8, file_crc);
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

    #[test]
    fn hillslope_hbp_to_columns_reads_schema1_fixture() {
        let bytes = build_schema1_fixture();
        let path = write_temp_hbp(&bytes);
        let version = VersionInfo::new(7, 0);

        let out =
            hillslope_hbp_to_columns(&path, None, &version).expect("schema1 fixture should parse");
        assert_eq!(out.event.len(), 1);
        assert_eq!(out.event[0], "NO EVENT");
        assert_eq!(out.julian[0], 1);
        assert_eq!(out.year[0], 2004);
    }

    #[test]
    fn hillslope_hbp_to_columns_reads_schema2_fixture() {
        let fixture = build_schema2_fixture();
        let path = write_temp_hbp(&fixture.bytes);
        let version = VersionInfo::new(7, 0);

        let out =
            hillslope_hbp_to_columns(&path, None, &version).expect("schema2 fixture should parse");
        assert_eq!(out.event.len(), 366);
        assert_eq!(out.event[0], "NO EVENT");
        assert_eq!(out.julian[0], 1);
        assert_eq!(out.julian[365], 366);
        assert_eq!(out.year[0], 2004);
    }

    #[test]
    fn ag_fields_pass_writer_preserves_schema2_hbp_values_and_identity() {
        let fixture = build_schema2_fixture();
        let first = write_temp_hbp(&fixture.bytes);
        let second = first.parent().expect("fixture parent").join("H2.hbp");
        fs::write(&second, &fixture.bytes).expect("write second HBP fixture");
        let paths = [second.clone(), first.clone()];
        let ordinary = first.parent().unwrap().join("ordinary.pass.parquet");
        let ag_output = first.parent().unwrap().join("ag_fields.pass.parquet");
        let version = VersionInfo::new(1, 2);

        let ordinary_summary =
            hillslope_pass_files_to_parquet(&paths, &ordinary, None, &version, Some("hbp"))
                .expect("write ordinary HBP PASS parquet");
        let sources = vec![
            AgFieldsSource::new(second, 100, 2),
            AgFieldsSource::new(first, 101, 1),
        ];
        let ag_summary = ag_fields_hillslope_pass_files_to_parquet(
            &sources,
            &ag_output,
            None,
            &version,
            Some("hbp"),
        )
        .expect("write AgFields HBP PASS parquet");

        assert_eq!(ordinary_summary.rows_written, ag_summary.rows_written);
        assert_eq!(ordinary_summary.row_groups, ag_summary.row_groups);
        crate::ag_fields::assert_parquet_parity(&ordinary, &ag_output, &sources);
    }

    fn mutate_schema2_and_expect_reject(
        mut fixture: Schema2Fixture,
        expected_message: &str,
        mutate: impl FnOnce(&mut Schema2Fixture),
    ) {
        mutate(&mut fixture);
        let path = write_temp_hbp(&fixture.bytes);
        let version = VersionInfo::new(7, 0);

        let err = match hillslope_hbp_to_columns(&path, None, &version) {
            Ok(_) => panic!("schema2 mutation should fail: {expected_message}"),
            Err(err) => err,
        };
        assert_parse_message(err, expected_message);
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_schema2_bad_codec() {
        mutate_schema2_and_expect_reject(
            build_schema2_fixture(),
            "schema 2.x payload codec is unsupported",
            |fixture| {
                let block_entry_start = schema2_block_entry_start(fixture);
                fixture.bytes[block_entry_start + 28] = 2;
                refresh_schema2_crc_fields(fixture);
            },
        );
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_schema2_bad_day_slot() {
        mutate_schema2_and_expect_reject(
            build_schema2_fixture(),
            "schema 2.0 day_in_block_index must equal julian_day - 1",
            |fixture| {
                let row1_start = schema2_row_start(fixture, 1);
                put_u16_at(&mut fixture.bytes, row1_start + 15, 0);
                refresh_schema2_crc_fields(fixture);
            },
        );
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_schema2_bad_slice_bounds() {
        mutate_schema2_and_expect_reject(
            build_schema2_fixture(),
            "schema 2.x day slice exceeds raw block bounds",
            |fixture| {
                let raw_block_len =
                    read_u32_at(&fixture.bytes, schema2_block_entry_start(fixture) + 24);
                let row1_start = schema2_row_start(fixture, 1);
                put_u32_at(&mut fixture.bytes, row1_start + 21, raw_block_len);
                refresh_schema2_crc_fields(fixture);
            },
        );
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_schema2_slice_gap() {
        mutate_schema2_and_expect_reject(
            build_schema2_fixture(),
            "schema 2.x day slices must cover raw block without gaps",
            |fixture| {
                let row1_start = schema2_row_start(fixture, 1);
                let current_offset = read_u32_at(&fixture.bytes, row1_start + 17);
                put_u32_at(&mut fixture.bytes, row1_start + 17, current_offset + 1);
                refresh_schema2_crc_fields(fixture);
            },
        );
    }

    #[test]
    fn hillslope_hbp_to_columns_rejects_schema2_stored_crc() {
        mutate_schema2_and_expect_reject(
            build_schema2_fixture(),
            "payload block stored crc mismatch",
            |fixture| {
                let block_entry_start = schema2_block_entry_start(fixture);
                let stored_crc = read_u32_at(&fixture.bytes, block_entry_start + 29);
                put_u32_at(&mut fixture.bytes, block_entry_start + 29, stored_crc ^ 1);
                refresh_schema2_crc_fields(fixture);
            },
        );
    }

    #[test]
    fn schema2_fixture_has_expected_directory_shape() {
        let fixture = build_schema2_fixture();
        let first_row = schema2_row_start(&fixture, 0);
        let first_payload = EntryPayload::SchemaV2 {
            payload_block_id: read_u32_at(&fixture.bytes, first_row + 11) as usize,
            day_in_block_index: u16::from_le_bytes([
                fixture.bytes[first_row + 15],
                fixture.bytes[first_row + 16],
            ]),
            raw_payload_offset: read_u32_at(&fixture.bytes, first_row + 17) as usize,
            raw_payload_length: read_u32_at(&fixture.bytes, first_row + 21) as usize,
            raw_payload_crc32c: read_u32_at(&fixture.bytes, first_row + 25),
        };
        match first_payload {
            EntryPayload::SchemaV2 {
                payload_block_id,
                day_in_block_index,
                raw_payload_offset,
                raw_payload_length,
                raw_payload_crc32c,
            } => {
                assert_eq!(payload_block_id, 0);
                assert_eq!(day_in_block_index, 0);
                assert_eq!(raw_payload_offset, 0);
                assert!(raw_payload_length > 0);
                assert_ne!(raw_payload_crc32c, 0);
            }
            EntryPayload::SchemaV1 { .. } => unreachable!(),
        }
    }
}

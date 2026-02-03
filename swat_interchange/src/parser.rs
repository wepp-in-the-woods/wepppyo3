use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use arrow2::array::{Array, Float64Array, Int32Array, Int64Array, Utf8Array};
use arrow2::chunk::Chunk;
use arrow2::datatypes::{DataType, Field, Metadata, Schema};
use arrow2::io::parquet::write::CompressionOptions;

use crate::errors::SwatError;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};
use crate::registry::{ColumnType, SwatTableSpec};

const DEFAULT_NUMERIC_SENTINELS: [&str; 3] = ["-9999", "-999", "-99"];
const DEFAULT_TEXT_SENTINELS: [&str; 4] = ["na", "n/a", "null", "---"];
const MAX_INFERENCE_SAMPLES: usize = 10_000;
const MAX_INFERENCE_ROWS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub source_name: String,
    pub registry_key: String,
    pub name: String,
    pub data_type: ColumnType,
    pub units: String,
    pub description: String,
    pub sentinels: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct TableSchema {
    pub schema: Schema,
    pub columns: Vec<ColumnInfo>,
    pub boundaries: Vec<usize>,
    pub header_len: usize,
    pub data_start_line: usize,
    pub whitespace_delimited: bool,
    pub merge_column: Option<usize>,
}

pub fn table_schema_from_file(
    path: &Path,
    spec: &SwatTableSpec,
    dataset_metadata: Metadata,
) -> Result<TableSchema, SwatError> {
    let header_info = read_header_info(path, spec)?;
    let mut columns = build_column_info(&header_info, spec)?;

    if columns.is_empty() {
        return Err(SwatError::header_error(
            path,
            Some(header_info.header_line_no),
            "No columns detected in header",
            Some(header_info.header_line.clone()),
        ));
    }

    if needs_inference(spec, &columns) {
        let inferred = infer_column_types(path, &header_info, &columns)?;
        for (col, inferred_type) in columns.iter_mut().zip(inferred.into_iter()) {
            if !spec.column_types.contains_key(&col.registry_key) {
                col.data_type = inferred_type;
            }
        }
    }

    let fields = columns
        .iter()
        .map(|col| {
            let mut field = Field::new(&col.name, column_type_to_arrow(col.data_type), true);
            let mut meta = Metadata::new();
            meta.insert("units".to_string(), col.units.clone());
            meta.insert("description".to_string(), col.description.clone());
            meta.insert("source_name".to_string(), col.source_name.clone());
            field.metadata = meta;
            field
        })
        .collect::<Vec<_>>();

    let schema = Schema {
        fields,
        metadata: dataset_metadata,
    };

    let merge_column = if spec.merge_column.is_some() {
        if header_info.merge_column.is_none() {
            return Err(SwatError::header_error(
                path,
                Some(header_info.header_line_no),
                format!(
                    "Merge column '{}' not found in header",
                    spec.merge_column.unwrap_or("")
                ),
                Some(header_info.header_line.clone()),
            ));
        }
        header_info.merge_column
    } else {
        None
    };

    Ok(TableSchema {
        schema,
        columns,
        boundaries: header_info.boundaries,
        header_len: header_info.header_len,
        data_start_line: header_info.data_start_line,
        whitespace_delimited: spec.whitespace_delimited,
        merge_column,
    })
}

pub fn parse_table_to_parquet(
    source_path: &Path,
    output_path: &Path,
    schema: TableSchema,
    chunk_rows: usize,
    compression: CompressionOptions,
) -> Result<WriteSummary, SwatError> {
    let file = File::open(source_path).map_err(|err| SwatError::io(source_path, err))?;
    let mut reader = BufReader::new(file);

    let mut sink = ParquetSink::try_new(output_path, schema.schema.clone(), compression)?;
    let mut buffers = ColumnBuffers::new(&schema.columns);

    let mut rows_in_chunk = 0usize;
    let mut total_rows = 0usize;

    let mut idx = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| decode_or_io(source_path, err))?;
        if bytes == 0 {
            break;
        }
        idx += 1;
        let line_no = idx;
        if line_no < schema.data_start_line {
            continue;
        }
        let line = strip_newline(&line);
        if line.trim().is_empty() {
            continue;
        }
        let values = if schema.whitespace_delimited {
            parse_whitespace_values(
                source_path,
                line_no,
                &line,
                schema.columns.len(),
                schema.merge_column,
            )?
        } else {
            if overflow_non_whitespace(&line, schema.header_len) {
                return Err(SwatError::column_mismatch(
                    source_path,
                    Some(line_no),
                    "Non-whitespace data beyond last header boundary",
                    Some(line.clone()),
                ));
            }
            slice_line(&line, &schema.boundaries, schema.header_len)
        };
        if values.len() != schema.columns.len() {
            return Err(SwatError::column_mismatch(
                source_path,
                Some(line_no),
                format!(
                    "Column count mismatch: expected {}, got {}",
                    schema.columns.len(),
                    values.len()
                ),
                Some(line.clone()),
            ));
        }
        for (idx, (col, value)) in schema.columns.iter().zip(values.iter()).enumerate() {
            buffers.push_value(idx, col, value, source_path, line_no)?;
        }

        rows_in_chunk += 1;
        total_rows += 1;
        if rows_in_chunk >= chunk_rows {
            let chunk = buffers.to_chunk()?;
            sink.write_chunk(chunk)?;
            buffers.clear();
            rows_in_chunk = 0;
        }
    }

    if total_rows == 0 {
        let chunk = empty_chunk(&schema.schema);
        sink.write_chunk(chunk)?;
    } else if rows_in_chunk > 0 {
        let chunk = buffers.to_chunk()?;
        sink.write_chunk(chunk)?;
    }

    sink.finish()
}

fn needs_inference(spec: &SwatTableSpec, columns: &[ColumnInfo]) -> bool {
    columns
        .iter()
        .any(|col| !spec.column_types.contains_key(&col.registry_key))
}

fn infer_column_types(
    path: &Path,
    header: &HeaderInfo,
    columns: &[ColumnInfo],
) -> Result<Vec<ColumnType>, SwatError> {
    let column_count = columns.len();
    if column_count == 0 {
        return Ok(Vec::new());
    }
    let file = File::open(path).map_err(|err| SwatError::io(path, err))?;
    let mut reader = BufReader::new(file);

    let mut numeric_possible = vec![true; column_count];
    let mut samples = vec![0usize; column_count];
    let mut done = 0usize;

    let sentinels = columns
        .iter()
        .map(|col| col.sentinels.clone())
        .collect::<Vec<_>>();
    let mut idx = 0usize;
    let mut rows_scanned = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|err| decode_or_io(path, err))?;
        if bytes == 0 {
            break;
        }
        idx += 1;
        let line_no = idx;
        if line_no < header.data_start_line {
            continue;
        }
        let line = strip_newline(&line);
        if line.trim().is_empty() {
            continue;
        }
        rows_scanned += 1;
        let values = if header.whitespace_delimited {
            parse_whitespace_values(path, line_no, &line, column_count, header.merge_column)?
        } else {
            if overflow_non_whitespace(&line, header.header_len) {
                return Err(SwatError::column_mismatch(
                    path,
                    Some(line_no),
                    "Non-whitespace data beyond last header boundary",
                    Some(line.clone()),
                ));
            }
            slice_line(&line, &header.boundaries, header.header_len)
        };
        for (idx, value) in values.iter().enumerate() {
            if !numeric_possible[idx] || samples[idx] >= MAX_INFERENCE_SAMPLES {
                continue;
            }
            if is_null_numeric(value, &sentinels[idx]) {
                continue;
            }
            samples[idx] += 1;
            if value.parse::<f64>().is_err() {
                numeric_possible[idx] = false;
                done += 1;
            }
        }
        if done == column_count {
            break;
        }
        if samples.iter().all(|count| *count >= MAX_INFERENCE_SAMPLES) {
            break;
        }
        if rows_scanned >= MAX_INFERENCE_ROWS {
            break;
        }
    }

    let types = numeric_possible
        .into_iter()
        .map(|is_numeric| {
            if is_numeric {
                ColumnType::Float64
            } else {
                ColumnType::String
            }
        })
        .collect::<Vec<_>>();
    Ok(types)
}

fn build_column_info(
    header: &HeaderInfo,
    spec: &SwatTableSpec,
) -> Result<Vec<ColumnInfo>, SwatError> {
    let source_names = &header.source_names;
    let registry_keys = registry_keys(source_names);
    let normalized = normalize_names(source_names);

    let mut columns = Vec::new();
    for (idx, ((source_name, registry_key), name)) in source_names
        .iter()
        .zip(registry_keys.iter())
        .zip(normalized.iter())
        .enumerate()
    {
        let data_type = spec
            .column_types
            .get(registry_key)
            .copied()
            .unwrap_or(ColumnType::Float64);

        let units = spec
            .units_overrides
            .get(registry_key)
            .cloned()
            .or_else(|| header.units.get(idx).cloned())
            .unwrap_or_else(String::new);

        let description = spec
            .column_descriptions
            .get(registry_key)
            .cloned()
            .unwrap_or_else(|| source_name.to_string());

        let sentinels = if let Some(values) = spec.sentinel_overrides.get(registry_key) {
            values.iter().map(|value| value.to_lowercase()).collect()
        } else {
            default_numeric_sentinels()
        };

        columns.push(ColumnInfo {
            source_name: source_name.to_string(),
            registry_key: registry_key.to_string(),
            name: name.to_string(),
            data_type,
            units,
            description,
            sentinels,
        });
    }

    Ok(columns)
}

fn normalize_names(names: &[String]) -> Vec<String> {
    let mut normalized = Vec::with_capacity(names.len());
    let mut counts: HashMap<String, usize> = HashMap::new();

    for (idx, name) in names.iter().enumerate() {
        let mut normalized_name = normalize_name(name);
        if normalized_name.is_empty() {
            normalized_name = format!("col_{idx}");
        }
        if normalized_name
            .chars()
            .next()
            .map(|ch| ch.is_ascii_digit())
            .unwrap_or(false)
        {
            normalized_name = format!("col_{normalized_name}");
        }
        let count = counts.entry(normalized_name.clone()).or_insert(0);
        if *count > 0 {
            normalized_name = format!("{}_{}", normalized_name, *count + 1);
        }
        *count += 1;
        normalized.push(normalized_name);
    }

    normalized
}

fn normalize_name(name: &str) -> String {
    let trimmed = name.trim();
    let mut out = String::new();
    let mut last_was_underscore = false;

    for ch in trimmed.chars() {
        let is_alnum = ch.is_ascii_alphanumeric();
        if is_alnum {
            out.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }

    let trimmed = out.trim_matches('_');
    trimmed.to_string()
}

fn registry_keys(names: &[String]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut keys = Vec::with_capacity(names.len());

    for name in names {
        let count = seen.entry(name.clone()).or_insert(0);
        let key = if *count == 0 {
            name.clone()
        } else {
            format!("{}#{}", name, *count + 1)
        };
        *count += 1;
        keys.push(key);
    }

    keys
}

fn default_numeric_sentinels() -> HashSet<String> {
    let mut values = HashSet::new();
    for sentinel in DEFAULT_NUMERIC_SENTINELS.iter() {
        values.insert((*sentinel).to_string());
    }
    for sentinel in DEFAULT_TEXT_SENTINELS.iter() {
        values.insert((*sentinel).to_string());
    }
    values
}

fn is_null_numeric(value: &str, sentinels: &HashSet<String>) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lower = trimmed.to_lowercase();
    sentinels.contains(trimmed) || sentinels.contains(&lower)
}

fn column_type_to_arrow(column_type: ColumnType) -> DataType {
    match column_type {
        ColumnType::String => DataType::Utf8,
        ColumnType::Float64 => DataType::Float64,
        ColumnType::Int32 => DataType::Int32,
        ColumnType::Int64 => DataType::Int64,
    }
}

struct HeaderInfo {
    source_names: Vec<String>,
    boundaries: Vec<usize>,
    header_len: usize,
    units: Vec<String>,
    data_start_line: usize,
    header_line_no: usize,
    header_line: String,
    whitespace_delimited: bool,
    merge_column: Option<usize>,
}

fn read_header_info(path: &Path, spec: &SwatTableSpec) -> Result<HeaderInfo, SwatError> {
    let file = File::open(path).map_err(|err| SwatError::io(path, err))?;
    let mut reader = BufReader::new(file);

    let mut line_no = 0usize;
    let mut buffer = String::new();

    for _ in 0..spec.skip_lines {
        buffer.clear();
        if reader
            .read_line(&mut buffer)
            .map_err(|err| decode_or_io(path, err))?
            == 0
        {
            return Err(SwatError::header_error(
                path,
                None,
                "Unexpected EOF while skipping header lines",
                None,
            ));
        }
        line_no += 1;
    }

    let max_idx = std::cmp::max(
        spec.header_line_index,
        spec.units_line_index.unwrap_or(spec.header_line_index),
    );

    let mut lines = Vec::new();
    for _ in 0..=max_idx {
        buffer.clear();
        if reader
            .read_line(&mut buffer)
            .map_err(|err| decode_or_io(path, err))?
            == 0
        {
            return Err(SwatError::header_error(
                path,
                Some(line_no),
                "Unexpected EOF while reading header lines",
                None,
            ));
        }
        line_no += 1;
        lines.push(strip_newline(&buffer));
    }

    let header_line = lines
        .get(spec.header_line_index)
        .cloned()
        .ok_or_else(|| SwatError::header_error(path, Some(line_no), "Missing header line", None))?;
    let units_line = spec
        .units_line_index
        .and_then(|idx| lines.get(idx).cloned());

    let (mut source_names, mut boundaries, header_len) = if spec.header_merge {
        let units_line = units_line.clone().ok_or_else(|| {
            SwatError::header_error(
                path,
                Some(line_no),
                "Missing units line for header merge",
                None,
            )
        })?;
        let header_tokens = parse_tokens(&header_line);
        let units_tokens = parse_tokens(&units_line);
        let (names, boundaries) = merge_header_tokens(&header_tokens, &units_tokens);
        let header_len = std::cmp::max(header_line.len(), units_line.len());
        (names, boundaries, header_len)
    } else {
        let header_tokens = parse_tokens(&header_line);
        tokens_to_columns(&header_tokens, header_line.len())
    };

    if let Some(overrides) = &spec.column_names_override {
        source_names = overrides.iter().map(|name| (*name).to_string()).collect();
        if source_names.len() > boundaries.len() {
            boundaries
                .extend(std::iter::repeat(header_len).take(source_names.len() - boundaries.len()));
        } else if source_names.len() < boundaries.len() {
            boundaries.truncate(source_names.len());
        }
    }

    let mut units = vec![String::new(); source_names.len()];
    if let Some(units_line) = units_line {
        let units_values = slice_line(&units_line, &boundaries, header_len);
        for (idx, value) in units_values.into_iter().enumerate() {
            if idx < units.len() {
                units[idx] = value.trim().to_string();
            }
        }
    }

    let data_start_line = spec.skip_lines + max_idx + 2;

    let merge_column = spec
        .merge_column
        .and_then(|name| source_names.iter().position(|col| col == name));

    Ok(HeaderInfo {
        source_names,
        boundaries,
        header_len,
        units,
        data_start_line,
        header_line_no: spec.skip_lines + spec.header_line_index + 1,
        header_line,
        whitespace_delimited: spec.whitespace_delimited,
        merge_column,
    })
}

fn strip_newline(line: &str) -> String {
    let mut trimmed = line.to_string();
    if trimmed.ends_with('\n') {
        trimmed.pop();
        if trimmed.ends_with('\r') {
            trimmed.pop();
        }
    }
    trimmed
}

fn parse_tokens(line: &str) -> Vec<(usize, String)> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    let mut idx = 0usize;

    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() {
            break;
        }
        let start = idx;
        while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let token = line[start..idx].to_string();
        tokens.push((start, token));
    }

    tokens
}

fn tokens_to_columns(
    tokens: &[(usize, String)],
    header_len: usize,
) -> (Vec<String>, Vec<usize>, usize) {
    let mut names = Vec::new();
    let mut boundaries = Vec::new();
    for (start, name) in tokens.iter() {
        boundaries.push(*start);
        names.push(name.clone());
    }
    (names, boundaries, header_len)
}

fn merge_header_tokens(
    header_tokens: &[(usize, String)],
    units_tokens: &[(usize, String)],
) -> (Vec<String>, Vec<usize>) {
    let header_map: HashMap<usize, String> = header_tokens.iter().cloned().collect();
    let units_map: HashMap<usize, String> = units_tokens.iter().cloned().collect();
    if header_tokens.is_empty() {
        let mut names = Vec::new();
        let mut positions = Vec::new();
        for (start, name) in units_tokens.iter() {
            positions.push(*start);
            names.push(name.clone());
        }
        return (names, positions);
    }
    let first_header = header_tokens.first().map(|(pos, _)| *pos).unwrap_or(0);
    let mut positions: Vec<usize> = header_map.keys().copied().collect();
    for (pos, _) in units_tokens.iter() {
        if header_map.contains_key(pos) {
            continue;
        }
        if *pos < first_header {
            positions.push(*pos);
        }
    }
    positions.sort_unstable();
    positions.dedup();

    let mut names = Vec::new();
    for pos in positions.iter() {
        if let Some(name) = header_map.get(pos) {
            names.push(name.clone());
        } else if let Some(name) = units_map.get(pos) {
            names.push(name.clone());
        }
    }

    (names, positions)
}

fn slice_line(line: &str, boundaries: &[usize], header_len: usize) -> Vec<String> {
    let mut values = Vec::with_capacity(boundaries.len());
    let line_len = line.len();

    for (idx, start) in boundaries.iter().enumerate() {
        let end = if idx + 1 < boundaries.len() {
            boundaries[idx + 1]
        } else {
            header_len
        };
        let start = (*start).min(line_len);
        let end = end.min(line_len);
        if start >= end {
            values.push(String::new());
        } else {
            values.push(line[start..end].trim().to_string());
        }
    }

    values
}

fn parse_whitespace_values(
    path: &Path,
    line_no: usize,
    line: &str,
    column_count: usize,
    merge_column: Option<usize>,
) -> Result<Vec<String>, SwatError> {
    let mut tokens = line
        .split_whitespace()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    if tokens.len() < column_count {
        tokens.extend(std::iter::repeat(String::new()).take(column_count - tokens.len()));
        return Ok(tokens);
    }
    if tokens.len() == column_count {
        return Ok(tokens);
    }

    if let Some(merge_idx) = merge_column {
        if merge_idx >= column_count {
            return Err(SwatError::column_mismatch(
                path,
                Some(line_no),
                format!("Merge column index {merge_idx} out of bounds for {column_count} columns"),
                Some(line.to_string()),
            ));
        }
        let extra = tokens.len() - column_count;
        let merge_end = merge_idx + extra;
        if merge_end >= tokens.len() {
            return Err(SwatError::column_mismatch(
                path,
                Some(line_no),
                "Merge column span exceeds token count",
                Some(line.to_string()),
            ));
        }
        let mut merged = Vec::with_capacity(column_count);
        merged.extend(tokens[..merge_idx].iter().cloned());
        let merged_value = tokens[merge_idx..=merge_end].join(" ");
        merged.push(merged_value);
        merged.extend(tokens[merge_end + 1..].iter().cloned());
        if merged.len() == column_count {
            return Ok(merged);
        }
    }

    Err(SwatError::column_mismatch(
        path,
        Some(line_no),
        format!(
            "Column count mismatch: expected {}, got {}",
            column_count,
            tokens.len()
        ),
        Some(line.to_string()),
    ))
}

fn overflow_non_whitespace(line: &str, header_len: usize) -> bool {
    if line.len() <= header_len {
        return false;
    }
    line[header_len..].chars().any(|ch| !ch.is_whitespace())
}

fn decode_or_io(path: &Path, err: std::io::Error) -> SwatError {
    if err.kind() == std::io::ErrorKind::InvalidData {
        SwatError::decode(path, "Invalid UTF-8 sequence")
    } else {
        SwatError::io(path, err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;

    fn temp_path(filename: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let suffix = format!(
            "swat_interchange_{:?}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            filename
        );
        path.push(suffix);
        path
    }

    fn write_temp(contents: &str) -> PathBuf {
        let path = temp_path("table.txt");
        fs::write(&path, contents).expect("write temp file");
        path
    }

    fn write_temp_with<F>(filename: &str, write_fn: F) -> PathBuf
    where
        F: FnOnce(&mut BufWriter<fs::File>),
    {
        let path = temp_path(filename);
        let file = fs::File::create(&path).expect("create temp file");
        let mut writer = BufWriter::new(file);
        write_fn(&mut writer);
        writer.flush().expect("flush temp file");
        path
    }

    fn base_spec(skip_lines: usize, units_line_index: Option<usize>) -> SwatTableSpec {
        SwatTableSpec {
            pattern: "*",
            skip_lines,
            header_line_index: 0,
            units_line_index,
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

    #[test]
    fn units_are_bound_per_occurrence() {
        let contents = "a         a         b\nm         s         k\n1         2         3\n";
        let path = write_temp(contents);
        let spec = base_spec(0, Some(1));
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[0].units, "m");
        assert_eq!(schema.columns[1].units, "s");
        assert_eq!(schema.columns[2].units, "k");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn inference_respects_sentinel_overrides() {
        let contents = "value    \nMISS\nMISS\n1.5\n";
        let path = write_temp(contents);
        let mut spec = base_spec(0, None);
        spec.sentinel_overrides
            .insert("value".to_string(), vec!["MISS".to_string()]);
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns.len(), 1);
        assert_eq!(schema.columns[0].data_type, ColumnType::Float64);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn normalized_names_and_metadata_preserved() {
        let contents = "A         A         ----      1B\nm         s         kg        -\n1         2         3         4\n";
        let path = write_temp(contents);
        let spec = base_spec(0, Some(1));
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        let names = schema
            .columns
            .iter()
            .map(|col| col.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["a", "a_2", "col_2", "col_1b"]);
        assert_eq!(schema.columns[0].units, "m");
        assert_eq!(schema.columns[2].units, "kg");
        assert_eq!(schema.columns[2].description, "----");

        let field = &schema.schema.fields[2];
        assert_eq!(
            field.metadata.get("source_name").map(|v| v.as_str()),
            Some("----")
        );
        assert_eq!(
            field.metadata.get("description").map(|v| v.as_str()),
            Some("----")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn units_row_blank_preserved() {
        let contents = "a         b         c\nm                   k\n1         2         3\n";
        let path = write_temp(contents);
        let spec = base_spec(0, Some(1));
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[0].units, "m");
        assert_eq!(schema.columns[1].units, "");
        assert_eq!(schema.columns[2].units, "k");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn units_and_description_overrides_precedence() {
        let contents = "A   B\nm   s\n1   2\n";
        let path = write_temp(contents);
        let mut spec = base_spec(0, Some(1));
        spec.units_overrides
            .insert("A".to_string(), "ft".to_string());
        spec.column_descriptions
            .insert("A".to_string(), "Alpha".to_string());
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns[0].units, "ft");
        assert_eq!(schema.columns[1].units, "s");
        assert_eq!(schema.columns[0].description, "Alpha");
        assert_eq!(schema.columns[1].description, "B");

        let field = &schema.schema.fields[0];
        assert_eq!(field.metadata.get("units").map(|v| v.as_str()), Some("ft"));
        assert_eq!(
            field.metadata.get("description").map(|v| v.as_str()),
            Some("Alpha")
        );
        assert_eq!(
            field.metadata.get("source_name").map(|v| v.as_str()),
            Some("A")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn header_merge_requires_units_line() {
        let contents = "a   b\n1   2\n";
        let path = write_temp(contents);
        let mut spec = base_spec(0, None);
        spec.header_merge = true;
        let metadata = Metadata::new();
        let err = table_schema_from_file(&path, &spec, metadata).expect_err("header error");

        match err {
            SwatError::Parse {
                reason, message, ..
            } => {
                assert_eq!(reason, crate::errors::Reason::HeaderError);
                assert!(message.contains("Missing units line for header merge"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn short_line_slices_to_empty_trailing() {
        let header = "a   b   c";
        let tokens = parse_tokens(header);
        let (_, boundaries, header_len) = tokens_to_columns(&tokens, header.len());
        let values = slice_line("1   2", &boundaries, header_len);

        assert_eq!(values.len(), 3);
        assert_eq!(values[0], "1");
        assert_eq!(values[1], "2");
        assert_eq!(values[2], "");
    }

    #[test]
    fn overflow_non_whitespace_triggers_column_mismatch() {
        let contents = "a   b\n1   2   3\n";
        let path = write_temp(contents);
        let spec = base_spec(0, None);
        let metadata = Metadata::new();
        let err = table_schema_from_file(&path, &spec, metadata).expect_err("column mismatch");

        match err {
            SwatError::Parse {
                reason, message, ..
            } => {
                assert_eq!(reason, crate::errors::Reason::ColumnMismatch);
                assert!(message.contains("Non-whitespace data beyond last header boundary"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let _ = fs::remove_file(path);
    }

    #[test]
    fn whitespace_merge_column_resolves_extra_tokens() {
        let contents = "id   name   value\n1    alpha beta   2\n";
        let path = write_temp(contents);
        let mut spec = base_spec(0, None);
        spec.whitespace_delimited = true;
        spec.merge_column = Some("name");
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[1].data_type, ColumnType::String);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn column_override_extends_schema_for_whitespace() {
        let contents = "a   b\nm   s\n1   2   3\n";
        let path = write_temp(contents);
        let mut spec = base_spec(0, Some(1));
        spec.whitespace_delimited = true;
        spec.column_names_override = Some(vec!["a", "b", "c"]);
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns.len(), 3);
        assert_eq!(schema.columns[0].units, "m");
        assert_eq!(schema.columns[1].units, "s");
        assert_eq!(schema.columns[2].units, "");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn default_numeric_sentinels_case_insensitive() {
        let sentinels = default_numeric_sentinels();
        assert!(is_null_numeric("NA", &sentinels));
        assert!(is_null_numeric("n/a", &sentinels));
        assert!(is_null_numeric("Null", &sentinels));
        assert!(is_null_numeric("---", &sentinels));
        assert!(is_null_numeric("-999", &sentinels));
    }

    #[test]
    fn inference_stops_after_sample_cap() {
        let path = write_temp_with("sample_cap.txt", |writer| {
            writeln!(writer, "value").expect("write header");
            for _ in 0..MAX_INFERENCE_SAMPLES {
                writeln!(writer, "1.0").expect("write sample");
            }
            writeln!(writer, "oops").expect("write non-numeric");
        });
        let spec = base_spec(0, None);
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns[0].data_type, ColumnType::Float64);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn inference_stops_after_row_cap() {
        let path = write_temp_with("row_cap.txt", |writer| {
            writeln!(writer, "value").expect("write header");
            for idx in 0..MAX_INFERENCE_ROWS {
                if idx % 50 == 0 {
                    writeln!(writer, "1.0").expect("write numeric");
                } else {
                    writeln!(writer, "NA").expect("write null");
                }
            }
            writeln!(writer, "oops").expect("write non-numeric");
        });
        let spec = base_spec(0, None);
        let metadata = Metadata::new();
        let schema = table_schema_from_file(&path, &spec, metadata).expect("schema");

        assert_eq!(schema.columns[0].data_type, ColumnType::Float64);

        let _ = fs::remove_file(path);
    }
}

struct ColumnBuffers {
    buffers: Vec<ColumnBuffer>,
}

impl ColumnBuffers {
    fn new(columns: &[ColumnInfo]) -> Self {
        let buffers = columns
            .iter()
            .map(|col| ColumnBuffer::new(col.data_type))
            .collect::<Vec<_>>();
        Self { buffers }
    }

    fn push_value(
        &mut self,
        column_index: usize,
        column: &ColumnInfo,
        value: &str,
        path: &Path,
        line_no: usize,
    ) -> Result<(), SwatError> {
        let buffer = self.buffers.get_mut(column_index).expect("buffer index");
        buffer.push(value, column, path, line_no)
    }

    fn to_chunk(&self) -> Result<Chunk<Box<dyn Array>>, SwatError> {
        let arrays = self
            .buffers
            .iter()
            .map(|buffer| buffer.to_array())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Chunk::new(arrays))
    }

    fn clear(&mut self) {
        for buffer in self.buffers.iter_mut() {
            buffer.clear();
        }
    }
}

#[derive(Debug)]
struct ColumnBuffer {
    data_type: ColumnType,
    floats: Vec<Option<f64>>,
    int32s: Vec<Option<i32>>,
    int64s: Vec<Option<i64>>,
    strings: Vec<Option<String>>,
}

impl ColumnBuffer {
    fn new(data_type: ColumnType) -> Self {
        Self {
            data_type,
            floats: Vec::new(),
            int32s: Vec::new(),
            int64s: Vec::new(),
            strings: Vec::new(),
        }
    }

    fn push(
        &mut self,
        value: &str,
        column: &ColumnInfo,
        path: &Path,
        line_no: usize,
    ) -> Result<(), SwatError> {
        match self.data_type {
            ColumnType::Float64 => {
                let trimmed = value.trim();
                if is_null_numeric(trimmed, &column.sentinels) {
                    self.floats.push(None);
                } else {
                    let parsed = fast_float::parse::<f64, _>(trimmed).map_err(|_| {
                        SwatError::parse(
                            path,
                            Some(line_no),
                            format!("Unable to parse float '{trimmed}'"),
                            Some(value.to_string()),
                        )
                    })?;
                    self.floats.push(Some(parsed));
                }
            }
            ColumnType::Int32 => {
                let trimmed = value.trim();
                if is_null_numeric(trimmed, &column.sentinels) {
                    self.int32s.push(None);
                } else {
                    let parsed = trimmed.parse::<i32>().map_err(|_| {
                        SwatError::parse(
                            path,
                            Some(line_no),
                            format!("Unable to parse int32 '{trimmed}'"),
                            Some(value.to_string()),
                        )
                    })?;
                    self.int32s.push(Some(parsed));
                }
            }
            ColumnType::Int64 => {
                let trimmed = value.trim();
                if is_null_numeric(trimmed, &column.sentinels) {
                    self.int64s.push(None);
                } else {
                    let parsed = trimmed.parse::<i64>().map_err(|_| {
                        SwatError::parse(
                            path,
                            Some(line_no),
                            format!("Unable to parse int64 '{trimmed}'"),
                            Some(value.to_string()),
                        )
                    })?;
                    self.int64s.push(Some(parsed));
                }
            }
            ColumnType::String => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    self.strings.push(None);
                } else {
                    self.strings.push(Some(trimmed.to_string()));
                }
            }
        }
        Ok(())
    }

    fn to_array(&self) -> Result<Box<dyn Array>, SwatError> {
        match self.data_type {
            ColumnType::Float64 => Ok(Float64Array::from(self.floats.clone()).boxed()),
            ColumnType::Int32 => Ok(Int32Array::from(self.int32s.clone()).boxed()),
            ColumnType::Int64 => Ok(Int64Array::from(self.int64s.clone()).boxed()),
            ColumnType::String => Ok(Utf8Array::<i32>::from(self.strings.clone()).boxed()),
        }
    }

    fn clear(&mut self) {
        self.floats.clear();
        self.int32s.clear();
        self.int64s.clear();
        self.strings.clear();
    }
}

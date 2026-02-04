use std::collections::HashSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use arrow2::io::parquet::read;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use time::OffsetDateTime;

use crate::errors::InterchangeError;

const SUPPORTED_EXTENSIONS: [&str; 7] = [
    ".nodb", ".tsv", ".csv", ".tif", ".parquet", ".json", ".geojson",
];

pub fn catalog_scan(base: &Path) -> Result<Vec<PyObject>, InterchangeError> {
    let entries = build_catalog(base);
    Python::with_gil(|py| {
        let mut out: Vec<PyObject> = Vec::new();
        for entry in entries {
            out.push(entry.into_pydict(py));
        }
        Ok(out)
    })
}

fn build_catalog(base: &Path) -> Vec<CatalogEntry> {
    let mut entries: Vec<CatalogEntry> = Vec::new();
    for path in iter_catalog_files(base, None) {
        if let Some(entry) = build_entry(base, &path, None) {
            entries.push(entry);
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn build_entry(base: &Path, path: &Path, catalog_path: Option<&str>) -> Option<CatalogEntry> {
    let suffix = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))?;
    if !SUPPORTED_EXTENSIONS.contains(&suffix.as_str()) {
        return None;
    }

    let stat = match path.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return None,
    };

    let base_len = base.to_string_lossy().len() + 1;
    let rel_path = if let Some(catalog_path) = catalog_path {
        catalog_path.to_string()
    } else if let Ok(rel) = path.strip_prefix(base) {
        rel.to_string_lossy().to_string()
    } else {
        path.to_string_lossy()
            .get(base_len..)
            .unwrap_or("")
            .to_string()
    };

    let mut entry = CatalogEntry {
        path: rel_path.replace(std::path::MAIN_SEPARATOR, "/"),
        extension: suffix.clone(),
        size_bytes: stat.len(),
        modified: format_timestamp(stat.modified().ok()),
        schema: None,
    };

    if suffix == ".parquet" {
        entry.schema = read_parquet_schema(path);
    } else if suffix == ".geojson" {
        entry.schema = None;
    }

    Some(entry)
}

fn read_parquet_schema(path: &Path) -> Option<SchemaInfo> {
    let mut reader = File::open(path).ok()?;
    let metadata = read::read_metadata(&mut reader).ok()?;
    let schema = read::infer_schema(&metadata).ok()?;

    let mut fields = Vec::new();
    for field in schema.fields {
        let mut info = FieldInfo {
            name: field.name.clone(),
            r#type: data_type_to_pyarrow(&field.data_type),
            units: None,
            description: None,
        };
        if let Some(meta) = field.metadata.get("units") {
            info.units = Some(meta.clone());
        }
        if let Some(meta) = field.metadata.get("description") {
            info.description = Some(meta.clone());
        }
        fields.push(info);
    }
    Some(SchemaInfo { fields })
}

fn data_type_to_pyarrow(data_type: &arrow2::datatypes::DataType) -> String {
    use arrow2::datatypes::DataType;
    match data_type {
        DataType::Int8 => "int8".to_string(),
        DataType::Int16 => "int16".to_string(),
        DataType::Int32 => "int32".to_string(),
        DataType::Int64 => "int64".to_string(),
        DataType::UInt8 => "uint8".to_string(),
        DataType::UInt16 => "uint16".to_string(),
        DataType::UInt32 => "uint32".to_string(),
        DataType::UInt64 => "uint64".to_string(),
        DataType::Float32 => "float".to_string(),
        DataType::Float64 => "double".to_string(),
        DataType::Utf8 => "string".to_string(),
        DataType::LargeUtf8 => "large_string".to_string(),
        DataType::Boolean => "bool".to_string(),
        other => format!("{other:?}"),
    }
}

fn format_timestamp(time: Option<SystemTime>) -> String {
    let time = time.unwrap_or(SystemTime::UNIX_EPOCH);
    let datetime: OffsetDateTime = time.into();
    let datetime = datetime.to_offset(time::UtcOffset::UTC);
    let micros = datetime.microsecond();
    if micros == 0 {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+00:00",
            datetime.year(),
            u8::from(datetime.month()),
            datetime.day(),
            datetime.hour(),
            datetime.minute(),
            datetime.second(),
        )
    } else {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            datetime.year(),
            u8::from(datetime.month()),
            datetime.day(),
            datetime.hour(),
            datetime.minute(),
            datetime.second(),
            micros,
        )
    }
}

#[derive(Debug)]
struct FieldInfo {
    name: String,
    r#type: String,
    units: Option<String>,
    description: Option<String>,
}

#[derive(Debug)]
struct SchemaInfo {
    fields: Vec<FieldInfo>,
}

#[derive(Debug)]
struct CatalogEntry {
    path: String,
    extension: String,
    size_bytes: u64,
    modified: String,
    schema: Option<SchemaInfo>,
}

impl CatalogEntry {
    fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("path", self.path).unwrap();
        dict.set_item("extension", self.extension).unwrap();
        dict.set_item("size_bytes", self.size_bytes).unwrap();
        dict.set_item("modified", self.modified).unwrap();
        if let Some(schema) = self.schema {
            dict.set_item("schema", schema.into_pydict(py)).unwrap();
        }
        dict.into_py(py)
    }
}

impl SchemaInfo {
    fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        let mut field_entries: Vec<PyObject> = Vec::new();
        for field in self.fields {
            field_entries.push(field.into_pydict(py));
        }
        dict.set_item("fields", field_entries).unwrap();
        dict.into_py(py)
    }
}

impl FieldInfo {
    fn into_pydict(self, py: Python<'_>) -> PyObject {
        let dict = PyDict::new_bound(py);
        dict.set_item("name", self.name).unwrap();
        dict.set_item("type", self.r#type).unwrap();
        if let Some(units) = self.units {
            dict.set_item("units", units).unwrap();
        }
        if let Some(description) = self.description {
            dict.set_item("description", description).unwrap();
        }
        dict.into_py(py)
    }
}

fn iter_catalog_files(base: &Path, directory: Option<&Path>) -> Vec<PathBuf> {
    let start = directory.unwrap_or(base);
    let mut stack: Vec<PathBuf> = vec![start.to_path_buf()];
    let mut visited_paths: HashSet<PathBuf> = HashSet::new();
    let mut visited_real_dirs: HashSet<PathBuf> = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();

    while let Some(current) = stack.pop() {
        let resolved_current = match current.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };
        if visited_paths.contains(&current) {
            continue;
        }
        visited_paths.insert(current.clone());

        if !is_allowed_target(base, &resolved_current) {
            continue;
        }

        let is_symlink_dir = current
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if !is_symlink_dir {
            if visited_real_dirs.contains(&resolved_current) {
                continue;
            }
            visited_real_dirs.insert(resolved_current.clone());
        }

        let read_dir = match fs::read_dir(&current) {
            Ok(dir) => dir,
            Err(_) => continue,
        };
        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            let resolved_entry = match entry_path.canonicalize() {
                Ok(path) => path,
                Err(_) => continue,
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                stack.push(entry_path);
                continue;
            }

            if file_type.is_symlink() {
                if resolved_entry.is_dir() {
                    if !is_allowed_target(base, &resolved_entry) {
                        continue;
                    }
                    if visited_real_dirs.contains(&resolved_entry) {
                        continue;
                    }
                    stack.push(entry_path);
                    continue;
                }
                if !resolved_entry.is_file() {
                    continue;
                }
            }

            if resolved_entry.is_file() && is_allowed_target(base, &resolved_entry) {
                files.push(entry_path);
            }
        }
    }

    files
}

fn is_allowed_target(base: &Path, target: &Path) -> bool {
    if is_within_directory(base, target) {
        return true;
    }
    if let Some(parent_root) = find_parent_run_root(base) {
        return is_within_directory(&parent_root, target);
    }
    false
}

fn is_within_directory(base: &Path, target: &Path) -> bool {
    target.strip_prefix(base).is_ok()
}

fn find_parent_run_root(base: &Path) -> Option<PathBuf> {
    let parts: Vec<&std::ffi::OsStr> = base.iter().collect();
    let mut idx: Option<usize> = None;
    for (i, part) in parts.iter().enumerate() {
        if part == &std::ffi::OsStr::new("_pups") {
            idx = Some(i);
            break;
        }
    }
    let idx = idx?;
    if idx == 0 {
        return None;
    }
    Some(parts[..idx].iter().collect())
}

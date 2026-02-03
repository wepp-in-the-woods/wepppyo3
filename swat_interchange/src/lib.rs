use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

use arrow2::chunk::Chunk;
use arrow2::datatypes::Metadata;
use arrow2::io::parquet::write::CompressionOptions;
use pyo3::exceptions::{PyIOError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

mod errors;
mod manifest;
mod parquet;
mod parser;
mod registry;

use crate::errors::{Reason, SwatError};
use crate::manifest::{read_manifest, validate_basename, ManifestEntry};
use crate::parquet::{write_single_chunk, WriteSummary};
use crate::parser::{parse_table_to_parquet, table_schema_from_file};
use crate::registry::resolve_spec;

const SPEC_NAME: &str = "swat-interchange-v1";
const GENERATOR: &str = "wepppyo3.swat_interchange";
const GENERATOR_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorEntry {
    filename: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterchangeVersion {
    spec: String,
    generator: String,
    generator_version: String,
    status: String,
    created_utc: String,
    run_output_dir: String,
    run_id: Option<String>,
    files_total: Option<usize>,
    files_written: Option<usize>,
    files_skipped: Option<usize>,
    errors: Option<Vec<ErrorEntry>>,
}

#[derive(Debug, Clone)]
struct SkipEntry {
    filename: String,
    reason: Reason,
}

#[pyfunction]
#[pyo3(
    signature = (
        run_output_dir,
        *,
        interchange_dir=None,
        manifest_path=None,
        ncpu=None,
        chunk_rows=None,
        delete_after_interchange=false,
        dry_run=false,
        delete_manifest=false,
        fail_fast=false,
        include=None,
        exclude=None,
        write_manifest=true,
        compression="snappy",
        stale_after_hours=None,
        overwrite=false,
    )
)]
fn swat_outputs_to_parquet(
    run_output_dir: String,
    interchange_dir: Option<String>,
    manifest_path: Option<String>,
    ncpu: Option<i32>,
    chunk_rows: Option<usize>,
    delete_after_interchange: bool,
    dry_run: bool,
    delete_manifest: bool,
    fail_fast: bool,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    write_manifest: bool,
    compression: &str,
    stale_after_hours: Option<f64>,
    overwrite: bool,
) -> PyResult<PyObject> {
    let start = Instant::now();
    let ncpu = resolve_ncpu(ncpu)?;
    let chunk_rows = validate_chunk_rows(chunk_rows)?;
    let compression = compression_from_str(compression)?;
    validate_stale_after(stale_after_hours)?;

    let run_output_dir = PathBuf::from(run_output_dir);
    let interchange_dir = interchange_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| run_output_dir.join("interchange"));
    let manifest_path = manifest_path
        .map(PathBuf::from)
        .unwrap_or_else(|| run_output_dir.join("files_out.out"));

    let run_id = read_run_id(&run_output_dir);
    let version_path = interchange_dir.join("interchange_version.json");

    if let Some(version) = read_version(&version_path) {
        if !overwrite {
            match version.status.as_str() {
                "in_progress" => {
                    let allow_override = if let Some(hours) = stale_after_hours {
                        match parse_created_utc(&version.created_utc) {
                            Ok(created) => is_stale(created, hours),
                            Err(_) => {
                                return Err(runtime_error(
                                    Reason::InterchangeCreatedUtcInvalid,
                                    "created_utc is missing or invalid",
                                ))
                            }
                        }
                    } else {
                        false
                    };
                    if !allow_override {
                        return Err(runtime_error(
                            Reason::InterchangeInProgress,
                            "interchange already in progress",
                        ));
                    }
                }
                "complete" => {
                    let summary = summary_for_existing_complete(
                        &run_output_dir,
                        &manifest_path,
                        include.as_ref(),
                        exclude.as_ref(),
                        &interchange_dir,
                        start.elapsed().as_millis() as u64,
                    )?;
                    return Ok(summary);
                }
                "partial" => {
                    return Err(runtime_error(
                        Reason::InterchangePartial,
                        "interchange already marked partial",
                    ))
                }
                "failed" => {
                    return Err(runtime_error(
                        Reason::InterchangeFailed,
                        "interchange already marked failed",
                    ))
                }
                _ => {}
            }
        }
    }

    if let Err(err) = fs::create_dir_all(&interchange_dir) {
        return Err(PyIOError::new_err(format!(
            "Unable to create interchange_dir: {err}"
        )));
    }

    let mut version = InterchangeVersion {
        spec: SPEC_NAME.to_string(),
        generator: GENERATOR.to_string(),
        generator_version: GENERATOR_VERSION.to_string(),
        status: "in_progress".to_string(),
        created_utc: now_rfc3339(),
        run_output_dir: run_output_dir.to_string_lossy().into_owned(),
        run_id: run_id.clone(),
        files_total: None,
        files_written: None,
        files_skipped: None,
        errors: None,
    };

    let mut errors = Vec::new();

    let manifest_entries = if manifest_path.exists() {
        match read_manifest(&manifest_path) {
            Ok(entries) => entries,
            Err(_) => {
                record_run_error(
                    &mut version,
                    &mut errors,
                    Reason::ManifestUnreadable,
                    &interchange_dir,
                    &version_path,
                )?;
                return Err(runtime_error(
                    Reason::ManifestUnreadable,
                    "files_out.out is unreadable",
                ));
            }
        }
    } else {
        if include.is_none() {
            record_run_error(
                &mut version,
                &mut errors,
                Reason::ManifestMissing,
                &interchange_dir,
                &version_path,
            )?;
            return Err(runtime_error(
                Reason::ManifestMissing,
                "files_out.out is missing",
            ));
        }
        Vec::new()
    };

    if write_manifest && manifest_path.exists() {
        let manifest_output = interchange_dir.join("files_out.parquet");
        if overwrite || !manifest_output.exists() {
            if let Err(err) = write_manifest_parquet(&manifest_output, &manifest_entries) {
                return Err(PyRuntimeError::new_err(err.display_message()));
            }
        }
    }

    let (candidates, mut skipped_by_order, include_missing) = if manifest_entries.is_empty() && include.is_some() {
        build_candidates_from_include(include.as_ref(), exclude.as_ref())
    } else {
        build_candidates(&manifest_entries, include.as_ref(), exclude.as_ref())
    };

    let files_total = candidates.len();

    version.files_total = Some(files_total);
    if let Err(err) = write_version(&version_path, &version) {
        return Err(PyRuntimeError::new_err(err.display_message()));
    }

    let mut files_written = 0usize;
    let mut rows_written = 0usize;
    let mut row_groups = 0usize;
    let mut output_paths = Vec::new();
    let mut had_error_class = false;

    let log_path = interchange_dir.join("interchange.log");

    let (work_items, work_by_index) = prepare_work_items(
        &candidates,
        &run_output_dir,
        &interchange_dir,
        overwrite,
        delete_after_interchange,
        &log_path,
        &mut skipped_by_order,
    )?;

    if fail_fast || ncpu <= 1 || work_items.len() <= 1 {
        for item in work_items.iter() {
            let result = process_work_item(item, chunk_rows, compression, run_id.as_deref(), overwrite);
            match result {
                Ok(WorkOutcome::Written(summary)) => {
                    files_written += 1;
                    rows_written += summary.rows_written;
                    row_groups += summary.row_groups;
                    output_paths.push(item.output_path.to_string_lossy().into_owned());
                    log_event(&log_path, "convert", &item.candidate.filename, None)?;
                    if delete_after_interchange && item.candidate.filename != "files_out.out" {
                        handle_delete(
                            &item.source_path,
                            &item.candidate.filename,
                            dry_run,
                            &log_path,
                        )?;
                    }
                }
                Ok(WorkOutcome::Skipped(reason)) => {
                    set_skip(
                        &mut skipped_by_order,
                        item.index,
                        SkipEntry {
                            filename: item.candidate.filename.clone(),
                            reason,
                        },
                    );
                    log_event(&log_path, "skip", &item.candidate.filename, Some(reason))?;
                    if delete_after_interchange {
                        log_event(&log_path, "delete_skipped", &item.candidate.filename, Some(reason))?;
                    }
                }
                Err(err) => {
                    handle_file_error(
                        err,
                        &item.candidate.filename,
                        item.index,
                        fail_fast,
                        &mut skipped_by_order,
                        &mut errors,
                        &mut had_error_class,
                        &mut version,
                        &version_path,
                    )?;
                    if fail_fast {
                        return Err(PyRuntimeError::new_err("conversion failed"));
                    }
                }
            }
        }
    } else {
        let results = run_parallel(
            work_items,
            ncpu,
            chunk_rows,
            compression,
            run_id.clone(),
            work_by_index.len(),
            overwrite,
        );
        for (idx, result) in results.into_iter().enumerate() {
            let Some(item) = work_by_index.get(idx).and_then(|entry| entry.as_ref()) else {
                continue;
            };
            let Some(result) = result else {
                continue;
            };
            match result {
                Ok(WorkOutcome::Written(summary)) => {
                    files_written += 1;
                    rows_written += summary.rows_written;
                    row_groups += summary.row_groups;
                    output_paths.push(item.output_path.to_string_lossy().into_owned());
                    log_event(&log_path, "convert", &item.candidate.filename, None)?;
                    if delete_after_interchange && item.candidate.filename != "files_out.out" {
                        handle_delete(
                            &item.source_path,
                            &item.candidate.filename,
                            dry_run,
                            &log_path,
                        )?;
                    }
                }
                Ok(WorkOutcome::Skipped(reason)) => {
                    set_skip(
                        &mut skipped_by_order,
                        item.index,
                        SkipEntry {
                            filename: item.candidate.filename.clone(),
                            reason,
                        },
                    );
                    log_event(&log_path, "skip", &item.candidate.filename, Some(reason))?;
                    if delete_after_interchange {
                        log_event(&log_path, "delete_skipped", &item.candidate.filename, Some(reason))?;
                    }
                }
                Err(err) => {
                    handle_file_error(
                        err,
                        &item.candidate.filename,
                        item.index,
                        false,
                        &mut skipped_by_order,
                        &mut errors,
                        &mut had_error_class,
                        &mut version,
                        &version_path,
                    )?;
                }
            }
        }
    }

    if delete_manifest && manifest_path.exists() {
        if dry_run {
            log_event(&log_path, "delete_skipped", "files_out.out", None)?;
        } else if let Err(err) = fs::remove_file(&manifest_path) {
            log_event(&log_path, "error", "files_out.out", None)?;
            let _ = err;
        } else {
            log_event(&log_path, "delete", "files_out.out", None)?;
        }
    }

    let skipped = collect_skipped(skipped_by_order, include_missing);

    version.files_written = Some(files_written);
    version.files_skipped = Some(skipped.len());
    version.errors = if errors.is_empty() { None } else { Some(errors) };
    version.status = if had_error_class {
        "partial".to_string()
    } else {
        "complete".to_string()
    };

    if let Err(err) = write_version(&version_path, &version) {
        return Err(PyRuntimeError::new_err(err.display_message()));
    }

    let summary = build_summary_dict(
        start.elapsed().as_millis() as u64,
        &run_output_dir,
        &interchange_dir,
        files_total,
        files_written,
        skipped.len(),
        rows_written,
        row_groups,
        output_paths,
        skipped,
    );
    Ok(summary)
}

#[pyfunction]
#[pyo3(
    signature = (
        source_path,
        output_path,
        *,
        category=None,
        chunk_rows=None,
        delete_after_interchange=false,
        allow_external_delete=false,
        dry_run=false,
        compression="snappy",
        overwrite=false,
    )
)]
fn swat_output_to_parquet(
    source_path: String,
    output_path: String,
    category: Option<String>,
    chunk_rows: Option<usize>,
    delete_after_interchange: bool,
    allow_external_delete: bool,
    dry_run: bool,
    compression: &str,
    overwrite: bool,
) -> PyResult<PyObject> {
    let start = Instant::now();
    let chunk_rows = validate_chunk_rows(chunk_rows)?;
    let compression = compression_from_str(compression)?;

    let source_path = PathBuf::from(source_path);
    let output_path = PathBuf::from(output_path);

    if delete_after_interchange {
        let run_output_dir = detect_run_output_dir(&source_path);
        if run_output_dir.is_none() && !allow_external_delete {
            return Err(PyValueError::new_err(
                "delete_after_interchange requires a SWAT run output directory or allow_external_delete=True",
            ));
        }
    }

    if output_path.exists() && !overwrite {
        if delete_after_interchange {
            let log_name = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .unwrap_or_else(|| source_path.to_string_lossy().into_owned());
            let log_path = if let Some(run_output_dir) = detect_run_output_dir(&source_path) {
                run_output_dir.join("interchange").join("interchange.log")
            } else {
                output_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("interchange.log")
            };
            log_event(&log_path, "delete_skipped", &log_name, Some(Reason::Exists))?;
        }
        let summary = build_single_summary(
            start.elapsed().as_millis() as u64,
            &source_path,
            &output_path,
            0,
            0,
            category,
        );
        return Ok(summary);
    }

    let spec = resolve_spec(
        source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(""),
    );

    let metadata = build_dataset_metadata(
        source_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(""),
        category.as_deref(),
        None,
    );

    let schema = table_schema_from_file(&source_path, &spec, metadata).map_err(to_py_err)?;
    let summary = parse_table_to_parquet(&source_path, &output_path, schema, chunk_rows, compression)
        .map_err(to_py_err)?;

    if delete_after_interchange && source_path.file_name().and_then(|name| name.to_str()) != Some("files_out.out") {
        let log_name = source_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .unwrap_or_else(|| source_path.to_string_lossy().into_owned());
        let log_path = if let Some(run_output_dir) = detect_run_output_dir(&source_path) {
            run_output_dir.join("interchange").join("interchange.log")
        } else {
            output_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("interchange.log")
        };
        handle_delete(&source_path, &log_name, dry_run, &log_path)?;
    }

    let summary = build_single_summary(
        start.elapsed().as_millis() as u64,
        &source_path,
        &output_path,
        summary.rows_written,
        summary.row_groups,
        category,
    );
    Ok(summary)
}

#[pymodule]
fn swat_interchange_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(swat_outputs_to_parquet, m)?)?;
    m.add_function(wrap_pyfunction!(swat_output_to_parquet, m)?)?;
    Ok(())
}

fn build_single_summary(
    elapsed_ms: u64,
    source_path: &Path,
    output_path: &Path,
    rows_written: usize,
    row_groups: usize,
    category: Option<String>,
) -> PyObject {
    Python::with_gil(|py| {
        let dict = PyDict::new_bound(py);
        dict.set_item("elapsed_ms", elapsed_ms).unwrap();
        dict.set_item("source_path", source_path.to_string_lossy().into_owned())
            .unwrap();
        dict.set_item("output_paths", vec![output_path.to_string_lossy().into_owned()])
            .unwrap();
        dict.set_item("rows_written", rows_written).unwrap();
        dict.set_item("row_groups", row_groups).unwrap();
        dict.set_item("category", category).unwrap();
        dict.into_py(py)
    })
}

fn build_summary_dict(
    elapsed_ms: u64,
    run_output_dir: &Path,
    interchange_dir: &Path,
    files_total: usize,
    files_written: usize,
    files_skipped: usize,
    rows_written: usize,
    row_groups: usize,
    output_paths: Vec<String>,
    skipped: Vec<SkipEntry>,
) -> PyObject {
    Python::with_gil(|py| {
        let dict = PyDict::new_bound(py);
        dict.set_item("elapsed_ms", elapsed_ms).unwrap();
        dict.set_item("run_output_dir", run_output_dir.to_string_lossy().into_owned())
            .unwrap();
        dict.set_item("interchange_dir", interchange_dir.to_string_lossy().into_owned())
            .unwrap();
        dict.set_item("files_total", files_total).unwrap();
        dict.set_item("files_written", files_written).unwrap();
        dict.set_item("files_skipped", files_skipped).unwrap();
        dict.set_item("rows_written", rows_written).unwrap();
        dict.set_item("row_groups", row_groups).unwrap();
        dict.set_item("output_paths", output_paths).unwrap();
        let skipped_list = skipped
            .into_iter()
            .map(|entry| {
                let item = PyDict::new_bound(py);
                item.set_item("filename", entry.filename).unwrap();
                item.set_item("reason", entry.reason.as_str()).unwrap();
                item
            })
            .collect::<Vec<_>>();
        dict.set_item("skipped", skipped_list).unwrap();
        dict.into_py(py)
    })
}

fn resolve_ncpu(ncpu: Option<i32>) -> PyResult<usize> {
    if let Some(ncpu) = ncpu {
        if ncpu < 0 {
            return Err(PyValueError::new_err("ncpu must be >= 0"));
        }
        if ncpu > 32 {
            return Err(PyValueError::new_err("ncpu must be <= 32"));
        }
        if ncpu == 0 {
            return Ok(1);
        }
        return Ok(ncpu as usize);
    }

    let available = thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1);
    Ok(std::cmp::min(available, 4))
}

fn validate_chunk_rows(chunk_rows: Option<usize>) -> PyResult<usize> {
    let chunk_rows = chunk_rows.unwrap_or(100_000);
    if chunk_rows == 0 {
        return Err(PyValueError::new_err("chunk_rows must be > 0"));
    }
    Ok(chunk_rows)
}

fn validate_stale_after(stale_after_hours: Option<f64>) -> PyResult<()> {
    if let Some(value) = stale_after_hours {
        if value <= 0.0 {
            return Err(PyValueError::new_err("stale_after_hours must be > 0"));
        }
    }
    Ok(())
}

fn compression_from_str(compression: &str) -> PyResult<CompressionOptions> {
    match compression.to_lowercase().as_str() {
        "snappy" => Ok(CompressionOptions::Snappy),
        "zstd" => Ok(CompressionOptions::Zstd(None)),
        "gzip" => Ok(CompressionOptions::Gzip(None)),
        "none" => Ok(CompressionOptions::Uncompressed),
        _ => Err(PyValueError::new_err(format!(
            "Unsupported compression '{compression}'"
        ))),
    }
}

fn build_dataset_metadata(
    source_file: &str,
    category: Option<&str>,
    run_id: Option<&str>,
) -> Metadata {
    let mut metadata = BTreeMap::new();
    metadata.insert("swat_interchange_version".to_string(), SPEC_NAME.to_string());
    metadata.insert("source_file".to_string(), source_file.to_string());
    if let Some(category) = category {
        metadata.insert("category".to_string(), category.to_string());
    }
    if let Some(run_id) = run_id {
        metadata.insert("run_id".to_string(), run_id.to_string());
    }
    metadata
}

fn read_run_id(run_output_dir: &Path) -> Option<String> {
    let path = run_output_dir.join("index.json");
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("run_id").and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn replace_extension(filename: &str, new_ext: &str) -> String {
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(filename);
    format!("{stem}.{new_ext}")
}

fn write_version(path: &Path, version: &InterchangeVersion) -> Result<(), SwatError> {
    let data = serde_json::to_string_pretty(version).map_err(|err| SwatError::value(err.to_string()))?;
    fs::write(path, data).map_err(|err| SwatError::io(path, err))
}

fn read_version(path: &Path) -> Option<InterchangeVersion> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn parse_created_utc(created: &str) -> Result<OffsetDateTime, SwatError> {
    OffsetDateTime::parse(created, &Rfc3339)
        .map_err(|err| SwatError::value(format!("Invalid created_utc: {err}")))
}

fn is_stale(created: OffsetDateTime, stale_after_hours: f64) -> bool {
    let age = OffsetDateTime::now_utc() - created;
    age.whole_seconds() as f64 / 3600.0 > stale_after_hours
}

fn summary_for_existing_complete(
    run_output_dir: &Path,
    manifest_path: &Path,
    include: Option<&Vec<String>>,
    exclude: Option<&Vec<String>>,
    interchange_dir: &Path,
    elapsed_ms: u64,
) -> PyResult<PyObject> {
    let manifest_entries = if manifest_path.exists() {
        read_manifest(manifest_path).unwrap_or_default()
    } else {
        Vec::new()
    };

    let (candidates, mut skipped_by_order, include_missing) =
        if manifest_entries.is_empty() && include.is_some() {
            build_candidates_from_include(include, exclude)
        } else {
            build_candidates(&manifest_entries, include, exclude)
        };
    let files_total = candidates.len();
    for candidate in candidates {
        set_skip(
            &mut skipped_by_order,
            candidate.order_index,
            SkipEntry {
                filename: candidate.filename,
                reason: Reason::InterchangeComplete,
            },
        );
    }
    let skipped = collect_skipped(skipped_by_order, include_missing);
    let files_skipped = skipped.len();

    let summary = build_summary_dict(
        elapsed_ms,
        run_output_dir,
        interchange_dir,
        files_total,
        0,
        files_skipped,
        0,
        0,
        Vec::new(),
        skipped,
    );
    Ok(summary)
}

fn build_candidates(
    manifest_entries: &[ManifestEntry],
    include: Option<&Vec<String>>,
    exclude: Option<&Vec<String>>,
) -> (Vec<Candidate>, Vec<Option<SkipEntry>>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut skipped_by_order = Vec::new();
    let mut seen = HashSet::new();

    let include_list = include.map(dedupe_list).unwrap_or_default();
    let include_filtered = if let Some(exclude_list) = exclude {
        include_list
            .iter()
            .filter(|item| !exclude_list.contains(item))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        include_list.clone()
    };
    let include_set: HashSet<String> = include_filtered.iter().cloned().collect();

    let mut order_index = 0usize;
    for entry in manifest_entries.iter() {
        if !include_set.is_empty() && !include_set.contains(&entry.filename) {
            continue;
        }
        if let Some(exclude_list) = exclude {
            if exclude_list.contains(&entry.filename) {
                continue;
            }
        }
        let index = order_index;
        order_index += 1;
        skipped_by_order.push(None);
        if validate_basename(&entry.filename).is_err() {
            set_skip(
                &mut skipped_by_order,
                index,
                SkipEntry {
                    filename: entry.filename.clone(),
                    reason: Reason::PathInvalid,
                },
            );
            continue;
        }
        if seen.contains(&entry.filename) {
            set_skip(
                &mut skipped_by_order,
                index,
                SkipEntry {
                    filename: entry.filename.clone(),
                    reason: Reason::Duplicate,
                },
            );
            continue;
        }
        seen.insert(entry.filename.clone());
        candidates.push(Candidate {
            filename: entry.filename.clone(),
            category: Some(entry.category.clone()),
            order_index: index,
        });
    }

    let mut include_missing = Vec::new();
    if !include_filtered.is_empty() {
        let manifest_names: HashSet<String> = manifest_entries
            .iter()
            .map(|entry| entry.filename.clone())
            .collect();
        for item in include_filtered {
            if !manifest_names.contains(&item) {
                include_missing.push(item);
            }
        }
    }

    (candidates, skipped_by_order, include_missing)
}

fn build_candidates_from_include(
    include: Option<&Vec<String>>,
    exclude: Option<&Vec<String>>,
) -> (Vec<Candidate>, Vec<Option<SkipEntry>>, Vec<String>) {
    let include_list = include.map(dedupe_list).unwrap_or_default();
    let include_filtered = if let Some(exclude_list) = exclude {
        include_list
            .into_iter()
            .filter(|item| !exclude_list.contains(item))
            .collect::<Vec<_>>()
    } else {
        include_list
    };
    let mut candidates = Vec::new();
    let mut skipped_by_order = vec![None; include_filtered.len()];
    for (index, item) in include_filtered.into_iter().enumerate() {
        if validate_basename(&item).is_err() {
            set_skip(
                &mut skipped_by_order,
                index,
                SkipEntry {
                    filename: item.clone(),
                    reason: Reason::PathInvalid,
                },
            );
            continue;
        }
        candidates.push(Candidate {
            filename: item.clone(),
            category: None,
            order_index: index,
        });
    }
    (candidates, skipped_by_order, Vec::new())
}

fn dedupe_list(values: &Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut output = Vec::new();
    for item in values {
        if seen.insert(item.clone()) {
            output.push(item.clone());
        }
    }
    output
}

fn set_skip(skipped_by_order: &mut Vec<Option<SkipEntry>>, index: usize, entry: SkipEntry) {
    if index >= skipped_by_order.len() {
        return;
    }
    if skipped_by_order[index].is_none() {
        skipped_by_order[index] = Some(entry);
    }
}

fn collect_skipped(
    skipped_by_order: Vec<Option<SkipEntry>>,
    include_missing: Vec<String>,
) -> Vec<SkipEntry> {
    let mut skipped = Vec::new();
    for entry in skipped_by_order.into_iter().flatten() {
        skipped.push(entry);
    }
    for missing in include_missing {
        skipped.push(SkipEntry {
            filename: missing,
            reason: Reason::NotInManifest,
        });
    }
    skipped
}

fn record_run_error(
    version: &mut InterchangeVersion,
    errors: &mut Vec<ErrorEntry>,
    reason: Reason,
    interchange_dir: &Path,
    version_path: &Path,
) -> PyResult<()> {
    errors.push(ErrorEntry {
        filename: "<run>".to_string(),
        reason: reason.as_str().to_string(),
    });
    version.status = "failed".to_string();
    version.errors = Some(errors.clone());
    if fs::create_dir_all(interchange_dir).is_ok() {
        let _ = write_version(version_path, version);
    }
    Ok(())
}

fn handle_file_error(
    err: SwatError,
    filename: &str,
    order_index: usize,
    fail_fast: bool,
    skipped_by_order: &mut Vec<Option<SkipEntry>>,
    errors: &mut Vec<ErrorEntry>,
    had_error_class: &mut bool,
    version: &mut InterchangeVersion,
    version_path: &Path,
) -> PyResult<()> {
    let reason = err.reason();
    set_skip(
        skipped_by_order,
        order_index,
        SkipEntry {
            filename: filename.to_string(),
            reason,
        },
    );
    if reason.is_error_class() {
        *had_error_class = true;
    }
    errors.push(ErrorEntry {
        filename: filename.to_string(),
        reason: reason.as_str().to_string(),
    });
    if fail_fast {
        version.status = "failed".to_string();
        version.errors = Some(errors.clone());
        if let Err(err) = write_version(version_path, version) {
            return Err(PyRuntimeError::new_err(err.display_message()));
        }
    }
    Ok(())
}

fn handle_delete(
    source_path: &Path,
    filename: &str,
    dry_run: bool,
    log_path: &Path,
) -> PyResult<()> {
    if dry_run {
        log_event(log_path, "delete_skipped", filename, None)?;
        return Ok(());
    }
    match fs::remove_file(source_path) {
        Ok(()) => log_event(log_path, "delete", filename, None)?,
        Err(err) => {
            log_event(log_path, "error", filename, None)?;
            let _ = err;
        }
    }
    Ok(())
}

fn log_event(log_path: &Path, action: &str, filename: &str, reason: Option<Reason>) -> PyResult<()> {
    let mut event = serde_json::Map::new();
    event.insert("timestamp".to_string(), serde_json::Value::String(now_rfc3339()));
    event.insert("action".to_string(), serde_json::Value::String(action.to_string()));
    event.insert("file".to_string(), serde_json::Value::String(filename.to_string()));
    if let Some(reason) = reason {
        event.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.as_str().to_string()),
        );
    }
    let event = serde_json::Value::Object(event);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|err| PyIOError::new_err(format!("Failed to open interchange.log: {err}")))?;
    writeln!(file, "{}", event.to_string())
        .map_err(|err| PyIOError::new_err(format!("Failed to write interchange.log: {err}")))?;
    Ok(())
}

fn detect_run_output_dir(path: &Path) -> Option<PathBuf> {
    let components: Vec<_> = path.components().collect();
    for (idx, component) in components.iter().enumerate() {
        let name = component.as_os_str().to_string_lossy();
        if name.starts_with("run_") && idx >= 2 {
            let parent = components.get(idx - 1)?.as_os_str().to_string_lossy();
            let grandparent = components.get(idx - 2)?.as_os_str().to_string_lossy();
            if parent == "outputs" && grandparent == "swat" {
                let mut run_path = PathBuf::new();
                for comp in &components[..=idx] {
                    run_path.push(comp.as_os_str());
                }
                return Some(run_path);
            }
        }
    }
    None
}

fn file_changed(path: &Path, before: &fs::Metadata) -> Result<bool, SwatError> {
    let after = path.metadata().map_err(|err| SwatError::io(path, err))?;
    let before_mtime = before.modified().ok();
    let after_mtime = after.modified().ok();
    Ok(before.len() != after.len() || before_mtime != after_mtime)
}

fn write_manifest_parquet(path: &Path, entries: &[ManifestEntry]) -> Result<WriteSummary, SwatError> {
    let categories = entries
        .iter()
        .map(|entry| Some(entry.category.clone()))
        .collect::<Vec<_>>();
    let filenames = entries
        .iter()
        .map(|entry| Some(entry.filename.clone()))
        .collect::<Vec<_>>();
    let source_lines = entries
        .iter()
        .map(|entry| Some(entry.source_line.clone()))
        .collect::<Vec<_>>();
    let line_nos = entries
        .iter()
        .map(|entry| Some(entry.line_no as i32))
        .collect::<Vec<_>>();

    let fields = vec![
        manifest_field(
            "category",
            arrow2::datatypes::DataType::Utf8,
            "",
            "SWAT output category",
        ),
        manifest_field(
            "filename",
            arrow2::datatypes::DataType::Utf8,
            "",
            "SWAT output filename",
        ),
        manifest_field(
            "source_line",
            arrow2::datatypes::DataType::Utf8,
            "",
            "Original manifest line",
        ),
        manifest_field(
            "line_no",
            arrow2::datatypes::DataType::Int32,
            "",
            "1-based line number in files_out.out",
        ),
    ];

    let mut metadata = BTreeMap::new();
    metadata.insert("swat_interchange_version".to_string(), SPEC_NAME.to_string());
    metadata.insert("source_file".to_string(), "files_out.out".to_string());

    let schema = arrow2::datatypes::Schema {
        fields,
        metadata,
    };

    let arrays: Vec<Box<dyn arrow2::array::Array>> = vec![
        arrow2::array::Utf8Array::<i32>::from(categories).boxed(),
        arrow2::array::Utf8Array::<i32>::from(filenames).boxed(),
        arrow2::array::Utf8Array::<i32>::from(source_lines).boxed(),
        arrow2::array::Int32Array::from(line_nos).boxed(),
    ];
    let chunk = Chunk::new(arrays);
    write_single_chunk(path, schema, chunk, CompressionOptions::Snappy)
}

fn manifest_field(
    name: &str,
    data_type: arrow2::datatypes::DataType,
    units: &str,
    description: &str,
) -> arrow2::datatypes::Field {
    let mut field = arrow2::datatypes::Field::new(name, data_type, true);
    let mut meta = BTreeMap::new();
    meta.insert("units".to_string(), units.to_string());
    meta.insert("description".to_string(), description.to_string());
    field.metadata = meta;
    field
}

fn runtime_error(reason: Reason, message: &str) -> PyErr {
    PyRuntimeError::new_err(format!("{}: {}", reason.as_str(), message))
}

fn to_py_err(err: SwatError) -> PyErr {
    match err {
        SwatError::Io { .. } => PyIOError::new_err(err.display_message()),
        SwatError::Parse { .. }
        | SwatError::Value { .. }
        | SwatError::Decode { .. } => PyValueError::new_err(err.display_message()),
        _ => PyRuntimeError::new_err(err.display_message()),
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    filename: String,
    category: Option<String>,
    order_index: usize,
}

#[derive(Debug, Clone)]
struct WorkItem {
    index: usize,
    candidate: Candidate,
    source_path: PathBuf,
    output_path: PathBuf,
}

#[derive(Debug)]
struct WorkResult {
    index: usize,
    result: Result<WorkOutcome, SwatError>,
}

#[derive(Debug)]
enum WorkOutcome {
    Written(WriteSummary),
    Skipped(Reason),
}

fn prepare_work_items(
    candidates: &[Candidate],
    run_output_dir: &Path,
    interchange_dir: &Path,
    overwrite: bool,
    delete_after_interchange: bool,
    log_path: &Path,
    skipped_by_order: &mut Vec<Option<SkipEntry>>,
) -> PyResult<(Vec<WorkItem>, Vec<Option<WorkItem>>)> {
    let mut work_items = Vec::new();
    let mut work_by_index = vec![None; skipped_by_order.len()];

    for candidate in candidates.iter() {
        let index = candidate.order_index;
        let source_path = run_output_dir.join(&candidate.filename);
        if !source_path.exists() {
            set_skip(
                skipped_by_order,
                index,
                SkipEntry {
                    filename: candidate.filename.clone(),
                    reason: Reason::Missing,
                },
            );
            log_event(log_path, "skip", &candidate.filename, Some(Reason::Missing))?;
            continue;
        }

        let output_path = interchange_dir.join(replace_extension(&candidate.filename, "parquet"));
        if output_path.exists() && !overwrite {
            set_skip(
                skipped_by_order,
                index,
                SkipEntry {
                    filename: candidate.filename.clone(),
                    reason: Reason::Exists,
                },
            );
            log_event(log_path, "skip", &candidate.filename, Some(Reason::Exists))?;
            if delete_after_interchange {
                log_event(log_path, "delete_skipped", &candidate.filename, Some(Reason::Exists))?;
            }
            continue;
        }

        let item = WorkItem {
            index,
            candidate: candidate.clone(),
            source_path,
            output_path,
        };
        if index < work_by_index.len() {
            work_by_index[index] = Some(item.clone());
        }
        work_items.push(item);
    }

    Ok((work_items, work_by_index))
}

fn process_work_item(
    item: &WorkItem,
    chunk_rows: usize,
    compression: CompressionOptions,
    run_id: Option<&str>,
    overwrite: bool,
) -> Result<WorkOutcome, SwatError> {
    if item.output_path.exists() && !overwrite {
        return Ok(WorkOutcome::Skipped(Reason::Exists));
    }

    let before_meta = item
        .source_path
        .metadata()
        .map_err(|err| SwatError::io(&item.source_path, err))?;

    let spec = resolve_spec(&item.candidate.filename);
    let metadata = build_dataset_metadata(
        &item.candidate.filename,
        item.candidate.category.as_deref(),
        run_id,
    );
    let schema = table_schema_from_file(&item.source_path, &spec, metadata)?;
    let summary = parse_table_to_parquet(
        &item.source_path,
        &item.output_path,
        schema,
        chunk_rows,
        compression,
    )?;

    if file_changed(&item.source_path, &before_meta)? {
        return Err(SwatError::file_changed(
            &item.source_path,
            "Source file changed during conversion",
        ));
    }

    Ok(WorkOutcome::Written(summary))
}

fn run_parallel(
    work_items: Vec<WorkItem>,
    ncpu: usize,
    chunk_rows: usize,
    compression: CompressionOptions,
    run_id: Option<String>,
    total: usize,
    overwrite: bool,
) -> Vec<Option<Result<WorkOutcome, SwatError>>> {
    let queue = Arc::new(Mutex::new(VecDeque::from(work_items.clone())));
    let (tx, rx) = mpsc::channel::<WorkResult>();
    let run_id = Arc::new(run_id);

    let mut handles = Vec::new();
    for _ in 0..ncpu {
        let queue = Arc::clone(&queue);
        let tx = tx.clone();
        let run_id = Arc::clone(&run_id);
        handles.push(thread::spawn(move || loop {
            let item = {
                let mut queue = queue.lock().expect("queue lock");
                queue.pop_front()
            };
            let Some(item) = item else {
                break;
            };
            let result = process_work_item(&item, chunk_rows, compression, run_id.as_deref(), overwrite);
            let _ = tx.send(WorkResult {
                index: item.index,
                result,
            });
        }));
    }
    drop(tx);

    let mut results: Vec<Option<Result<WorkOutcome, SwatError>>> = Vec::with_capacity(total);
    results.resize_with(total, || None);
    for _ in 0..work_items.len() {
        if let Ok(res) = rx.recv() {
            if res.index < results.len() {
                results[res.index] = Some(res.result);
            }
        }
    }

    for handle in handles {
        let _ = handle.join();
    }

    results
}

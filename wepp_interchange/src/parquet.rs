use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::arrow_support::{BoxedArray, Chunk};
use arrow_array::{new_empty_array, Array, RecordBatch};
use arrow_schema::Schema;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};

use crate::errors::InterchangeError;

#[derive(Debug, Clone)]
pub struct WriteSummary {
    pub rows_written: usize,
    pub row_groups: usize,
}

pub struct ParquetSink {
    writer: Option<ArrowWriter<File>>,
    schema: Arc<Schema>,
    tmp_path: Option<PathBuf>,
    final_path: PathBuf,
    rows_written: usize,
    row_groups: usize,
}

pub(crate) struct StagedParquet {
    tmp_path: Option<PathBuf>,
    final_path: PathBuf,
    summary: WriteSummary,
}

struct BackupPaths(Vec<Option<PathBuf>>);

struct TempPathGuard(Option<PathBuf>);

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl ParquetSink {
    pub fn try_new(path: &Path, schema: Schema) -> Result<Self, InterchangeError> {
        let (tmp_path, file) = create_unique_temp_file(path)?;

        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .set_dictionary_enabled(true)
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_statistics_enabled(EnabledStatistics::Chunk)
            .build();

        let schema = Arc::new(schema);
        let mut temp_guard = TempPathGuard(Some(tmp_path));
        let writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props))?;
        let tmp_path = temp_guard.0.take().expect("guarded staging path");

        Ok(Self {
            writer: Some(writer),
            schema,
            tmp_path: Some(tmp_path),
            final_path: path.to_path_buf(),
            rows_written: 0,
            row_groups: 0,
        })
    }

    pub fn write_chunk(&mut self, chunk: Chunk<Box<dyn Array>>) -> Result<(), InterchangeError> {
        let rows = chunk.len();
        let arrays = chunk
            .into_arrays()
            .into_iter()
            .map(Arc::<dyn Array>::from)
            .collect::<Vec<_>>();

        if rows > 0 {
            let batch = RecordBatch::try_new(Arc::clone(&self.schema), arrays)?;
            let writer = self.writer.as_mut().ok_or_else(|| {
                InterchangeError::Parquet("Parquet sink is already closed".to_string())
            })?;
            writer.write(&batch)?;
            // Each caller-provided chunk is an intentional row-group boundary. The
            // ordered multi-file APIs use one chunk per source file.
            writer.flush()?;
            self.row_groups += 1;
        }

        self.rows_written += rows;
        Ok(())
    }

    pub(crate) fn finish_staged(mut self) -> Result<StagedParquet, InterchangeError> {
        let writer = self.writer.take().ok_or_else(|| {
            InterchangeError::Parquet("Parquet sink is already closed".to_string())
        })?;
        writer.close()?;
        let tmp_path = self.tmp_path.take().ok_or_else(|| {
            InterchangeError::Parquet("Parquet sink has no staged path".to_string())
        })?;
        Ok(StagedParquet {
            tmp_path: Some(tmp_path),
            final_path: self.final_path.clone(),
            summary: WriteSummary {
                rows_written: self.rows_written,
                row_groups: self.row_groups,
            },
        })
    }

    pub fn finish(self) -> Result<WriteSummary, InterchangeError> {
        let staged = self.finish_staged()?;
        let mut summaries = commit_staged(vec![staged])?;
        Ok(summaries.remove(0))
    }
}

impl Drop for ParquetSink {
    fn drop(&mut self) {
        // Close the handle before unlinking an incomplete stage. Drop cannot
        // surface cleanup errors, so removal is necessarily best-effort here.
        self.writer.take();
        if let Some(tmp_path) = self.tmp_path.take() {
            let _ = fs::remove_file(tmp_path);
        }
    }
}

impl Drop for StagedParquet {
    fn drop(&mut self) {
        if let Some(tmp_path) = self.tmp_path.take() {
            let _ = fs::remove_file(tmp_path);
        }
    }
}

impl Drop for BackupPaths {
    fn drop(&mut self) {
        for backup in &mut self.0 {
            if let Some(path) = backup.take() {
                let _ = fs::remove_file(path);
            }
        }
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn commit_staged(
    staged: Vec<StagedParquet>,
) -> Result<Vec<WriteSummary>, InterchangeError> {
    commit_staged_impl(staged, None, None)
}

#[cfg(test)]
pub(crate) fn commit_staged_with_failure(
    staged: Vec<StagedParquet>,
    fail_before_publish: usize,
) -> Result<Vec<WriteSummary>, InterchangeError> {
    commit_staged_impl(staged, Some(fail_before_publish), None)
}

#[cfg(test)]
pub(crate) fn commit_staged_with_rollback_failure(
    staged: Vec<StagedParquet>,
    fail_before_publish: usize,
    fail_restore: usize,
) -> Result<Vec<WriteSummary>, InterchangeError> {
    commit_staged_impl(staged, Some(fail_before_publish), Some(fail_restore))
}

fn commit_staged_impl(
    mut staged: Vec<StagedParquet>,
    fail_before_publish: Option<usize>,
    fail_restore: Option<usize>,
) -> Result<Vec<WriteSummary>, InterchangeError> {
    let mut seen = HashSet::with_capacity(staged.len());
    for output in &staged {
        if !seen.insert(output.final_path.clone()) {
            return Err(transaction_error(
                &output.final_path,
                "Duplicate final path in Parquet output transaction",
            ));
        }
    }
    let _directory_locks = lock_output_directories(&staged)?;

    let summaries = staged
        .iter()
        .map(|output| output.summary.clone())
        .collect::<Vec<_>>();
    let mut backups = BackupPaths(vec![None; staged.len()]);
    for (index, output) in staged.iter().enumerate() {
        match fs::symlink_metadata(&output.final_path) {
            Ok(metadata) if metadata.is_file() => {
                match create_unique_backup_link(&output.final_path) {
                    Ok(path) => backups.0[index] = Some(path),
                    Err(error) => {
                        let cleanup_error = cleanup_backups(&mut backups.0);
                        return Err(combine_transaction_errors(
                            InterchangeError::io(&output.final_path, error),
                            cleanup_error,
                        ));
                    }
                }
            }
            Ok(_) => {
                let cleanup_error = cleanup_backups(&mut backups.0);
                return Err(combine_transaction_errors(
                    transaction_error(
                        &output.final_path,
                        "Parquet output target exists and is not a regular file",
                    ),
                    cleanup_error,
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                let cleanup_error = cleanup_backups(&mut backups.0);
                return Err(combine_transaction_errors(
                    InterchangeError::io(&output.final_path, error),
                    cleanup_error,
                ));
            }
        }
    }

    let mut published = vec![false; staged.len()];
    for index in 0..staged.len() {
        let publish_result = if fail_before_publish == Some(index) {
            Err(io::Error::other(format!(
                "Injected transaction failure before output {index}"
            )))
        } else {
            let tmp_path = staged[index]
                .tmp_path
                .as_ref()
                .expect("staged output path")
                .clone();
            fs::rename(&tmp_path, &staged[index].final_path)
        };

        if let Err(error) = publish_result {
            let primary = InterchangeError::io(&staged[index].final_path, error);
            let rollback_error =
                rollback_published(&staged, &mut backups.0, &published, fail_restore);
            return Err(combine_transaction_errors(primary, rollback_error));
        }
        staged[index].tmp_path.take();
        published[index] = true;
    }

    // This is failure-atomic rollback, not simultaneous multi-path visibility:
    // publication uses sequential same-directory renames after every stage closes.
    // Backup links are removed only after all paths contain the new generation.
    // Backup removal happens after the commit point. A cleanup problem cannot be
    // reported as a failed generation without violating the public atomicity
    // contract; BackupPaths retries any residual removals from Drop.
    let _ = cleanup_backups(&mut backups.0);
    Ok(summaries)
}

fn rollback_published(
    staged: &[StagedParquet],
    backups: &mut [Option<PathBuf>],
    published: &[bool],
    fail_restore: Option<usize>,
) -> Option<InterchangeError> {
    let mut first_error = None;
    let mut preserve_backup = vec![false; backups.len()];
    for index in (0..staged.len()).rev() {
        if published[index] {
            let had_backup = backups[index].is_some();
            let result = if fail_restore == Some(index) && had_backup {
                Err(io::Error::other(format!(
                    "Injected rollback restore failure for output {index}"
                )))
            } else if let Some(backup_path) = backups[index].as_ref() {
                fs::rename(backup_path, &staged[index].final_path)
            } else {
                fs::remove_file(&staged[index].final_path)
            };
            match result {
                Ok(()) if had_backup => {
                    backups[index].take();
                }
                Ok(()) => {}
                Err(error) => {
                    if let Some(backup_path) = backups[index].as_ref() {
                        preserve_backup[index] = true;
                        first_error.get_or_insert_with(|| {
                            transaction_error(
                                &staged[index].final_path,
                                format!(
                                    "Failed to restore prior output: {error}; recovery backup \
                                     preserved at {}",
                                    backup_path.display()
                                ),
                            )
                        });
                    } else {
                        first_error.get_or_insert_with(|| {
                            InterchangeError::io(&staged[index].final_path, error)
                        });
                    }
                }
            }
        }
    }
    let mut cleanup_error = None;
    for (index, backup) in backups.iter_mut().enumerate() {
        if preserve_backup[index] {
            // Disarm BackupPaths cleanup: this is the only retained recovery copy.
            backup.take();
            continue;
        }
        if let Some(error) = cleanup_backups(std::slice::from_mut(backup)) {
            cleanup_error.get_or_insert(error);
        }
    }
    if let Some(cleanup_error) = cleanup_error {
        first_error.get_or_insert(cleanup_error);
    }
    first_error
}

fn cleanup_backups(backups: &mut [Option<PathBuf>]) -> Option<InterchangeError> {
    let mut first_error = None;
    for backup in backups {
        let Some(path) = backup.as_ref() else {
            continue;
        };
        if let Err(error) = fs::remove_file(path) {
            first_error.get_or_insert_with(|| InterchangeError::io(&path, error));
        } else {
            backup.take();
        }
    }
    first_error
}

fn combine_transaction_errors(
    primary: InterchangeError,
    cleanup: Option<InterchangeError>,
) -> InterchangeError {
    let Some(cleanup) = cleanup else {
        return primary;
    };
    transaction_error(
        Path::new("<parquet-transaction>"),
        format!(
            "{}; rollback/cleanup also failed: {}",
            primary.display_message(),
            cleanup.display_message()
        ),
    )
}

fn transaction_error(path: &Path, message: impl Into<String>) -> InterchangeError {
    InterchangeError::io(path, io::Error::other(message.into()))
}

fn lock_output_directories(staged: &[StagedParquet]) -> Result<Vec<File>, InterchangeError> {
    let mut directories = staged
        .iter()
        .map(|output| {
            output
                .final_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
        })
        .map(|path| fs::canonicalize(path).map_err(|error| InterchangeError::io(path, error)))
        .collect::<Result<Vec<_>, _>>()?;
    directories.sort();
    directories.dedup();

    let mut locks = Vec::with_capacity(directories.len());
    for directory in directories {
        let handle =
            File::open(&directory).map_err(|error| InterchangeError::io(&directory, error))?;
        handle
            .lock()
            .map_err(|error| InterchangeError::io(&directory, error))?;
        locks.push(handle);
    }
    Ok(locks)
}

fn create_unique_temp_file(path: &Path) -> Result<(PathBuf, File), InterchangeError> {
    for _ in 0..4096 {
        let candidate = unique_sibling_path(path, "stage");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(InterchangeError::io(&candidate, error)),
        }
    }
    Err(transaction_error(
        path,
        "Unable to allocate a unique Parquet staging file",
    ))
}

fn create_unique_backup_link(path: &Path) -> Result<PathBuf, io::Error> {
    for _ in 0..4096 {
        let candidate = unique_sibling_path(path, "backup");
        match fs::hard_link(path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Unable to allocate a unique Parquet backup link",
    ))
}

fn unique_sibling_path(path: &Path, kind: &str) -> PathBuf {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "parquet".to_string());
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{filename}.wepp-{kind}-{}-{sequence}",
        std::process::id()
    ))
}

pub fn write_single_chunk(
    path: &Path,
    schema: Schema,
    chunk: Chunk<Box<dyn Array>>,
) -> Result<WriteSummary, InterchangeError> {
    let mut sink = ParquetSink::try_new(path, schema)?;
    sink.write_chunk(chunk)?;
    sink.finish()
}

pub fn empty_chunk(schema: &Schema) -> Chunk<Box<dyn Array>> {
    let arrays = schema
        .fields()
        .iter()
        .map(|field| new_empty_array(field.data_type()).boxed())
        .collect::<Vec<_>>();
    Chunk::new(arrays)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int32Array, StringArray};
    use arrow_schema::{DataType, Field};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::collections::HashMap;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(stem: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        path.push(format!("wepp_interchange_{stem}_{nanos}.parquet"));
        path
    }

    fn one_col_chunk(values: Vec<Option<i32>>) -> Chunk<Box<dyn Array>> {
        Chunk::new(vec![Int32Array::from(values).boxed()])
    }

    fn temporary_artifacts(path: &Path) -> Vec<PathBuf> {
        let parent = path.parent().expect("target parent");
        let filename = path.file_name().expect("target filename").to_string_lossy();
        let prefix = format!(".{filename}.wepp-");
        fs::read_dir(parent)
            .expect("read target parent")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate
                    .file_name()
                    .map(|name| name.to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
            })
            .collect()
    }

    #[test]
    fn arrow01_writer_schema_metadata_roundtrip_wepp_interchange() {
        let mut field_meta = HashMap::new();
        field_meta.insert("units".to_string(), "mm".to_string());
        field_meta.insert("description".to_string(), "test field".to_string());
        let field = Field::new("value", DataType::Int32, true).with_metadata(field_meta);

        let mut dataset_meta = HashMap::new();
        dataset_meta.insert("dataset_version".to_string(), "9.9".to_string());
        dataset_meta.insert("schema_version".to_string(), "9".to_string());
        let schema = Schema::new_with_metadata(vec![field], dataset_meta);

        let path = temp_path("schema_roundtrip");
        let summary = write_single_chunk(&path, schema, one_col_chunk(vec![Some(7), None]))
            .expect("write parquet");
        assert_eq!(summary.rows_written, 2);
        assert_eq!(summary.row_groups, 1);

        let file = File::open(&path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
        let read_schema = builder.schema().as_ref().clone();
        assert_eq!(
            read_schema
                .metadata()
                .get("dataset_version")
                .map(String::as_str),
            Some("9.9")
        );
        assert_eq!(
            read_schema
                .metadata()
                .get("schema_version")
                .map(String::as_str),
            Some("9")
        );
        let read_field = &read_schema.fields()[0];
        assert_eq!(read_field.name(), "value");
        assert_eq!(
            read_field.metadata().get("units").map(String::as_str),
            Some("mm")
        );
        assert_eq!(
            read_field.metadata().get("description").map(String::as_str),
            Some("test field")
        );

        std::fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn arrow01_writer_empty_chunk_contract_wepp_interchange() {
        let schema = Schema::new(vec![
            Field::new("label", DataType::Utf8, true),
            Field::new("large_label", DataType::LargeUtf8, true),
            Field::new("value", DataType::Int32, true),
        ]);
        let path = temp_path("empty_chunk");
        let summary =
            write_single_chunk(&path, schema.clone(), empty_chunk(&schema)).expect("write parquet");
        assert_eq!(summary.rows_written, 0);
        assert_eq!(summary.row_groups, 0);

        let file = File::open(&path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
        assert_eq!(builder.metadata().num_row_groups(), 0);
        let read_schema = builder.schema().as_ref().clone();
        assert_eq!(read_schema.fields().len(), 3);
        assert_eq!(read_schema.fields()[0].name(), "label");
        assert_eq!(read_schema.fields()[1].name(), "large_label");
        assert_eq!(read_schema.fields()[2].name(), "value");
        let mut reader = builder.with_batch_size(1024).build().expect("batch reader");
        let mut total_rows = 0usize;
        for batch in &mut reader {
            total_rows += batch.expect("batch").num_rows();
        }
        assert_eq!(total_rows, 0);

        std::fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn arrow01_writer_row_group_and_row_count_accounting_wepp_interchange() {
        let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
        let path = temp_path("row_group_accounting");
        let mut sink = ParquetSink::try_new(&path, schema).expect("create sink");
        sink.write_chunk(one_col_chunk(vec![Some(1), Some(2)]))
            .expect("first chunk");
        sink.write_chunk(one_col_chunk(vec![Some(3)]))
            .expect("second chunk");
        let summary = sink.finish().expect("finish sink");
        assert_eq!(summary.rows_written, 3);
        assert_eq!(summary.row_groups, 2);

        let file = File::open(&path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
        assert_eq!(builder.metadata().num_row_groups(), 2);

        std::fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn arrow01_writer_accepts_nonempty_utf8_batches_wepp_interchange() {
        let schema = Schema::new(vec![Field::new("label", DataType::Utf8, true)]);
        let path = temp_path("utf8_batch");
        let chunk = Chunk::new(vec![StringArray::from(vec![
            Some("EVENT"),
            Some("NO EVENT"),
        ])
        .boxed()]);
        let summary = write_single_chunk(&path, schema, chunk).expect("write utf8 parquet");
        assert_eq!(summary.rows_written, 2);

        let file = File::open(&path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
        assert_eq!(builder.schema().field(0).data_type(), &DataType::Utf8);

        std::fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn arrow01_error_mapping_typed_contract_wepp_interchange() {
        let schema = Schema::new(vec![
            Field::new("value", DataType::Int32, true),
            Field::new("other", DataType::Int32, true),
        ]);
        let path = temp_path("typed_error");
        let mut sink = ParquetSink::try_new(&path, schema).expect("create sink");
        let err = sink
            .write_chunk(one_col_chunk(vec![Some(1)]))
            .expect_err("expected Arrow mapping error");
        match err {
            InterchangeError::Arrow(message) => {
                assert!(
                    !message.is_empty(),
                    "arrow error message should not be empty"
                );
            }
            other => panic!("expected InterchangeError::Arrow, got {other:?}"),
        }
        drop(sink);
        assert!(!path.exists());
        assert!(temporary_artifacts(&path).is_empty());
    }

    #[test]
    fn commit_preflight_error_cleans_stage_and_preserves_nonfile_target() {
        let path = temp_path("nonfile_target");
        fs::create_dir(&path).expect("create target directory");
        let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
        let mut sink = ParquetSink::try_new(&path, schema).expect("create staged sink");
        sink.write_chunk(one_col_chunk(vec![Some(1)]))
            .expect("write staged row");

        sink.finish().expect_err("directory target must fail");
        assert!(path.is_dir());
        assert!(temporary_artifacts(&path).is_empty());
        fs::remove_dir(path).expect("cleanup target directory");
    }

    #[test]
    fn concurrent_sinks_use_unique_stages_and_clean_up_on_drop() {
        const THREADS: usize = 16;
        let path = temp_path("concurrent_stages");
        let barrier = Arc::new(Barrier::new(THREADS));
        let mut handles = Vec::new();
        for _ in 0..THREADS {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
                let sink = ParquetSink::try_new(&path, schema).expect("create concurrent sink");
                let stage = sink.tmp_path.as_ref().expect("stage path").clone();
                barrier.wait();
                drop(sink);
                stage
            }));
        }

        let stages = handles
            .into_iter()
            .map(|handle| handle.join().expect("join concurrent sink"))
            .collect::<HashSet<_>>();
        assert_eq!(stages.len(), THREADS);
        assert!(temporary_artifacts(&path).is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn concurrent_publishers_never_share_or_expose_incomplete_stages() {
        const THREADS: usize = 8;
        let path = temp_path("concurrent_publishers");
        let barrier = Arc::new(Barrier::new(THREADS));
        let mut handles = Vec::new();
        for value in 0..THREADS as i32 {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
                let mut sink = ParquetSink::try_new(&path, schema).expect("create publisher");
                let stage = sink.tmp_path.as_ref().expect("stage path").clone();
                sink.write_chunk(one_col_chunk(vec![Some(value)]))
                    .expect("write publisher batch");
                barrier.wait();
                let summary = sink.finish().expect("publish parquet");
                assert_eq!(summary.rows_written, 1);
                stage
            }));
        }

        let stages = handles
            .into_iter()
            .map(|handle| handle.join().expect("join publisher"))
            .collect::<HashSet<_>>();
        assert_eq!(stages.len(), THREADS);
        assert!(temporary_artifacts(&path).is_empty());

        let file = File::open(&path).expect("open published parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
        assert_eq!(builder.metadata().num_row_groups(), 1);
        let batches = builder
            .build()
            .expect("build batch reader")
            .collect::<Result<Vec<_>, _>>()
            .expect("read final parquet");
        assert_eq!(
            batches.iter().map(|batch| batch.num_rows()).sum::<usize>(),
            1
        );
        fs::remove_file(path).expect("cleanup parquet");
    }

    #[test]
    fn coordinated_commit_restores_prior_generation_on_later_failure() {
        let first = temp_path("transaction_first");
        let second = temp_path("transaction_second");
        fs::write(&first, b"old-first").expect("write prior first");
        fs::write(&second, b"old-second").expect("write prior second");
        let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);

        let mut first_sink = ParquetSink::try_new(&first, schema.clone()).expect("stage first");
        first_sink
            .write_chunk(one_col_chunk(vec![Some(1)]))
            .expect("write first");
        let mut second_sink = ParquetSink::try_new(&second, schema).expect("stage second");
        second_sink
            .write_chunk(one_col_chunk(vec![Some(2)]))
            .expect("write second");
        let staged = vec![
            first_sink.finish_staged().expect("close first stage"),
            second_sink.finish_staged().expect("close second stage"),
        ];

        commit_staged_with_failure(staged, 1).expect_err("later commit must fail");
        assert_eq!(fs::read(&first).expect("read restored first"), b"old-first");
        assert_eq!(
            fs::read(&second).expect("read restored second"),
            b"old-second"
        );
        assert!(temporary_artifacts(&first).is_empty());
        assert!(temporary_artifacts(&second).is_empty());
        fs::remove_file(first).expect("cleanup first");
        fs::remove_file(second).expect("cleanup second");
    }

    #[test]
    fn coordinated_commit_leaves_no_public_set_on_later_failure_without_prior_outputs() {
        let first = temp_path("transaction_new_first");
        let second = temp_path("transaction_new_second");
        let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
        let mut first_sink = ParquetSink::try_new(&first, schema.clone()).expect("stage first");
        first_sink
            .write_chunk(one_col_chunk(vec![Some(1)]))
            .expect("write first");
        let mut second_sink = ParquetSink::try_new(&second, schema).expect("stage second");
        second_sink
            .write_chunk(one_col_chunk(vec![Some(2)]))
            .expect("write second");
        let staged = vec![
            first_sink.finish_staged().expect("close first stage"),
            second_sink.finish_staged().expect("close second stage"),
        ];

        commit_staged_with_failure(staged, 1).expect_err("later commit must fail");
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(temporary_artifacts(&first).is_empty());
        assert!(temporary_artifacts(&second).is_empty());
    }

    #[test]
    fn failed_rollback_restore_preserves_and_reports_recovery_backup() {
        let first = temp_path("transaction_restore_first");
        let second = temp_path("transaction_restore_second");
        fs::write(&first, b"old-first").expect("write prior first");
        fs::write(&second, b"old-second").expect("write prior second");
        let schema = Schema::new(vec![Field::new("value", DataType::Int32, true)]);
        let mut first_sink = ParquetSink::try_new(&first, schema.clone()).expect("stage first");
        first_sink
            .write_chunk(one_col_chunk(vec![Some(1)]))
            .expect("write first");
        let mut second_sink = ParquetSink::try_new(&second, schema).expect("stage second");
        second_sink
            .write_chunk(one_col_chunk(vec![Some(2)]))
            .expect("write second");
        let staged = vec![
            first_sink.finish_staged().expect("close first stage"),
            second_sink.finish_staged().expect("close second stage"),
        ];

        let error = commit_staged_with_rollback_failure(staged, 1, 0)
            .expect_err("publish and restore must fail");
        let backups = temporary_artifacts(&first)
            .into_iter()
            .filter(|path| path.to_string_lossy().contains(".wepp-backup-"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert!(error
            .display_message()
            .contains(&backups[0].display().to_string()));
        assert_eq!(
            fs::read(&backups[0]).expect("read recovery backup"),
            b"old-first"
        );
        assert_eq!(
            fs::read(&second).expect("read untouched second"),
            b"old-second"
        );

        fs::remove_file(&backups[0]).expect("cleanup recovery backup");
        fs::remove_file(first).expect("cleanup first");
        fs::remove_file(second).expect("cleanup second");
    }
}

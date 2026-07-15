use std::fs::File;
use std::path::{Path, PathBuf};
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
    writer: ArrowWriter<File>,
    schema: Arc<Schema>,
    tmp_path: PathBuf,
    final_path: PathBuf,
    rows_written: usize,
    row_groups: usize,
}

impl ParquetSink {
    pub fn try_new(path: &Path, schema: Schema) -> Result<Self, InterchangeError> {
        let tmp_path = temp_path_for(path);
        if tmp_path.exists() {
            std::fs::remove_file(&tmp_path).map_err(|err| InterchangeError::io(&tmp_path, err))?;
        }
        let file = File::create(&tmp_path).map_err(|err| InterchangeError::io(&tmp_path, err))?;

        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .set_dictionary_enabled(true)
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_statistics_enabled(EnabledStatistics::Chunk)
            .build();

        let schema = Arc::new(schema);
        let writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props))?;

        Ok(Self {
            writer,
            schema,
            tmp_path,
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
            self.writer.write(&batch)?;
        }

        self.row_groups += 1;
        self.rows_written += rows;
        Ok(())
    }

    pub fn finish(self) -> Result<WriteSummary, InterchangeError> {
        self.writer.close()?;
        rename_atomic(&self.tmp_path, &self.final_path)?;
        Ok(WriteSummary {
            rows_written: self.rows_written,
            row_groups: self.row_groups,
        })
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "parquet.tmp".to_string());
    let tmp_name = format!("{filename}.tmp");
    path.with_file_name(tmp_name)
}

fn rename_atomic(tmp_path: &Path, final_path: &Path) -> Result<(), InterchangeError> {
    match std::fs::rename(tmp_path, final_path) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(18) => {
            std::fs::copy(tmp_path, final_path)
                .map_err(|copy_err| InterchangeError::io(final_path, copy_err))?;
            std::fs::remove_file(tmp_path)
                .map_err(|remove_err| InterchangeError::io(tmp_path, remove_err))?;
            Ok(())
        }
        Err(err) => Err(InterchangeError::io(tmp_path, err)),
    }
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
            read_schema.metadata().get("dataset_version").map(String::as_str),
            Some("9.9")
        );
        assert_eq!(
            read_schema.metadata().get("schema_version").map(String::as_str),
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
        assert_eq!(summary.row_groups, 1);

        let file = File::open(&path).expect("open parquet");
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).expect("build reader");
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
                assert!(!message.is_empty(), "arrow error message should not be empty");
            }
            other => panic!("expected InterchangeError::Arrow, got {other:?}"),
        }

        let _ = std::fs::remove_file(path);
    }
}

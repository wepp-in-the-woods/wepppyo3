use std::fs::File;
use std::path::{Path, PathBuf};

use arrow2::array::Array;
use arrow2::chunk::Chunk;
use arrow2::datatypes::{DataType, Schema};
use arrow2::io::parquet::write::{
    row_group_iter, transverse, CompressionOptions, Encoding, FileWriter, Version, WriteOptions,
};

use crate::errors::SwatError;

#[derive(Debug, Clone)]
pub struct WriteSummary {
    pub rows_written: usize,
    pub row_groups: usize,
}

pub struct ParquetSink {
    writer: FileWriter<File>,
    options: WriteOptions,
    encodings: Vec<Vec<Encoding>>,
    fields: Vec<parquet2::schema::types::ParquetType>,
    tmp_path: PathBuf,
    final_path: PathBuf,
    rows_written: usize,
    row_groups: usize,
}

impl ParquetSink {
    pub fn try_new(
        path: &Path,
        schema: Schema,
        compression: CompressionOptions,
    ) -> Result<Self, SwatError> {
        let tmp_path = temp_path_for(path);
        if tmp_path.exists() {
            std::fs::remove_file(&tmp_path).map_err(|err| SwatError::io(&tmp_path, err))?;
        }
        let file = File::create(&tmp_path).map_err(|err| SwatError::io(&tmp_path, err))?;

        let options = WriteOptions {
            write_statistics: true,
            compression,
            version: Version::V2,
            data_pagesize_limit: None,
        };

        let encodings = schema
            .fields
            .iter()
            .map(|field| transverse(&field.data_type, |_| encoding_for_type(&field.data_type)))
            .collect::<Vec<_>>();

        let writer = FileWriter::try_new(file, schema.clone(), options)?;
        let fields = writer.parquet_schema().fields().to_vec();

        Ok(Self {
            writer,
            options,
            encodings,
            fields,
            tmp_path,
            final_path: path.to_path_buf(),
            rows_written: 0,
            row_groups: 0,
        })
    }

    pub fn write_chunk(&mut self, chunk: Chunk<Box<dyn Array>>) -> Result<(), SwatError> {
        let rows = chunk.len();
        let row_group = row_group_iter(
            chunk,
            self.encodings.clone(),
            self.fields.clone(),
            self.options,
        );
        self.writer.write(row_group)?;
        self.row_groups += 1;
        self.rows_written += rows;
        Ok(())
    }

    pub fn finish(mut self) -> Result<WriteSummary, SwatError> {
        self.writer.end(None)?;
        rename_atomic(&self.tmp_path, &self.final_path)?;
        Ok(WriteSummary {
            rows_written: self.rows_written,
            row_groups: self.row_groups,
        })
    }
}

pub fn write_single_chunk(
    path: &Path,
    schema: Schema,
    chunk: Chunk<Box<dyn Array>>,
    compression: CompressionOptions,
) -> Result<WriteSummary, SwatError> {
    let mut sink = ParquetSink::try_new(path, schema, compression)?;
    sink.write_chunk(chunk)?;
    sink.finish()
}

pub fn empty_chunk(schema: &Schema) -> Chunk<Box<dyn Array>> {
    let arrays = schema
        .fields
        .iter()
        .map(|field| arrow2::array::new_empty_array(field.data_type.clone()))
        .collect::<Vec<_>>();
    Chunk::new(arrays)
}

fn temp_path_for(path: &Path) -> PathBuf {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "parquet.tmp".to_string());
    let tmp_name = format!("{filename}.tmp");
    path.with_file_name(tmp_name)
}

fn rename_atomic(tmp_path: &Path, final_path: &Path) -> Result<(), SwatError> {
    match std::fs::rename(tmp_path, final_path) {
        Ok(()) => Ok(()),
        Err(err) if err.raw_os_error() == Some(18) => {
            std::fs::copy(tmp_path, final_path)
                .map_err(|copy_err| SwatError::io(final_path, copy_err))?;
            std::fs::remove_file(tmp_path)
                .map_err(|remove_err| SwatError::io(tmp_path, remove_err))?;
            Ok(())
        }
        Err(err) => Err(SwatError::io(tmp_path, err)),
    }
}

fn encoding_for_type(data_type: &DataType) -> Encoding {
    match data_type.to_logical_type() {
        DataType::Utf8 | DataType::LargeUtf8 => Encoding::Plain,
        _ => Encoding::Plain,
    }
}

// Empty chunk uses plain arrays to avoid dictionary-encoding requirements.

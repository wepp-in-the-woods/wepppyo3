use std::collections::HashSet;
use std::path::{Path, PathBuf};

use arrow_array::{Array, Int32Array};
use arrow_schema::{DataType, Field, Schema};

use crate::arrow_support::{BoxedArray, Chunk};
use crate::errors::InterchangeError;
use crate::parquet::{empty_chunk, ParquetSink, WriteSummary};

pub const DATASET_KIND: &str = "ag_fields_hillslope";
pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub path: PathBuf,
    pub field_id: i32,
    pub sub_field_id: i32,
}

impl Source {
    pub fn new(path: PathBuf, field_id: i32, sub_field_id: i32) -> Self {
        Self {
            path,
            field_id,
            sub_field_id,
        }
    }
}

pub fn schema_from_hillslope(ordinary: Schema) -> Schema {
    let mut fields = Vec::with_capacity(ordinary.fields().len() + 1);
    fields.push(Field::new("field_id", DataType::Int32, false));
    fields.push(Field::new("sub_field_id", DataType::Int32, false));
    fields.extend(
        ordinary
            .fields()
            .iter()
            .skip(1)
            .map(|field| field.as_ref().clone()),
    );

    let mut metadata = ordinary.metadata().clone();
    metadata.insert("dataset_kind".to_string(), DATASET_KIND.to_string());
    metadata.insert(
        "ag_fields_schema_version".to_string(),
        SCHEMA_VERSION.to_string(),
    );
    Schema::new(fields).with_metadata(metadata)
}

pub fn write_sources<F>(
    sources: &[Source],
    output_path: &Path,
    schema: Schema,
    mut parse: F,
) -> Result<WriteSummary, InterchangeError>
where
    F: FnMut(&Path) -> Result<Chunk<Box<dyn Array>>, InterchangeError>,
{
    validate_sources(sources)?;

    let mut sink = ParquetSink::try_new(output_path, schema.clone())?;
    if sources.is_empty() {
        sink.write_chunk(empty_chunk(&schema))?;
    } else {
        for source in sources {
            let ordinary = parse(&source.path)?;
            sink.write_chunk(with_identity(
                ordinary,
                source.field_id,
                source.sub_field_id,
            )?)?;
        }
    }
    sink.finish()
}

pub fn validate_sources(sources: &[Source]) -> Result<(), InterchangeError> {
    let mut seen = HashSet::with_capacity(sources.len());
    for source in sources {
        if source.field_id <= 0 {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                format!("field_id must be positive, got {}", source.field_id),
                None,
            ));
        }
        if source.sub_field_id <= 0 {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                format!("sub_field_id must be positive, got {}", source.sub_field_id),
                None,
            ));
        }
        if !seen.insert(source.sub_field_id) {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                format!("duplicate sub_field_id {}", source.sub_field_id),
                None,
            ));
        }

        let filename_id = extract_filename_id(&source.path)?;
        if filename_id != source.sub_field_id {
            return Err(InterchangeError::parse(
                &source.path,
                None,
                format!(
                    "filename sub-field id {filename_id} does not match supplied sub_field_id {}",
                    source.sub_field_id
                ),
                None,
            ));
        }
    }
    Ok(())
}

fn extract_filename_id(path: &Path) -> Result<i32, InterchangeError> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| InterchangeError::parse(path, None, "Invalid UTF-8 filename", None))?;
    let remainder = name.strip_prefix('H').ok_or_else(|| {
        InterchangeError::parse(
            path,
            None,
            "Expected filename beginning with H<sub_field_id>.",
            None,
        )
    })?;
    let digits = remainder
        .split_once('.')
        .map(|(value, _)| value)
        .unwrap_or(remainder);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(InterchangeError::parse(
            path,
            None,
            "Expected filename beginning with H<sub_field_id>.",
            None,
        ));
    }
    digits.parse::<i32>().map_err(|_| {
        InterchangeError::parse(
            path,
            None,
            format!("Filename sub-field id is outside int32 range: {digits}"),
            None,
        )
    })
}

fn with_identity(
    ordinary: Chunk<Box<dyn Array>>,
    field_id: i32,
    sub_field_id: i32,
) -> Result<Chunk<Box<dyn Array>>, InterchangeError> {
    let rows = ordinary.len();
    let mut ordinary_arrays = ordinary.into_arrays().into_iter();
    if ordinary_arrays.next().is_none() {
        return Err(InterchangeError::Arrow(
            "Ordinary hillslope chunk has no identity column".to_string(),
        ));
    }

    let mut arrays = Vec::with_capacity(ordinary_arrays.len() + 2);
    arrays.push(Int32Array::from(vec![field_id; rows]).boxed());
    arrays.push(Int32Array::from(vec![sub_field_id; rows]).boxed());
    arrays.extend(ordinary_arrays);
    Ok(Chunk::new(arrays))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        hill_ebe_schema, hill_element_schema, hill_loss_schema, hill_pass_schema, hill_soil_schema,
        hill_wat_schema, VersionInfo,
    };
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use std::fs::{self, File};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "wepp_interchange_ag_fields_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn ag_fields_schema_replaces_only_identity_and_adds_kind_metadata() {
        let ordinary = hill_pass_schema(&VersionInfo::new(1, 2));
        let ordinary_tail = ordinary.fields()[1..].to_vec();
        let schema = schema_from_hillslope(ordinary);

        assert_eq!(schema.field(0).name(), "field_id");
        assert_eq!(schema.field(0).data_type(), &DataType::Int32);
        assert!(!schema.field(0).is_nullable());
        assert_eq!(schema.field(1).name(), "sub_field_id");
        assert_eq!(schema.field(1).data_type(), &DataType::Int32);
        assert!(!schema.field(1).is_nullable());
        assert_eq!(schema.fields()[2..], ordinary_tail);
        assert_eq!(schema.metadata().get("dataset_kind").unwrap(), DATASET_KIND);
        assert_eq!(
            schema.metadata().get("ag_fields_schema_version").unwrap(),
            SCHEMA_VERSION
        );
        assert_eq!(schema.metadata().get("dataset_version").unwrap(), "1.2");
    }

    #[test]
    fn source_validation_accepts_shared_field_and_rejects_bad_identity() {
        let sources = vec![
            Source::new(PathBuf::from("H10.pass.dat"), 7, 10),
            Source::new(PathBuf::from("H11.pass.dat"), 7, 11),
            Source::new(PathBuf::from("H12.pass.dat"), 8, 12),
        ];
        validate_sources(&sources).unwrap();

        let cases = [
            (Source::new(PathBuf::from("H1.pass.dat"), 0, 1), "field_id"),
            (
                Source::new(PathBuf::from("H1.pass.dat"), 1, 0),
                "sub_field_id",
            ),
            (
                Source::new(PathBuf::from("H2.pass.dat"), 1, 1),
                "does not match",
            ),
            (
                Source::new(PathBuf::from("bad.pass.dat"), 1, 1),
                "Expected filename",
            ),
        ];
        for (source, expected) in cases {
            let error = validate_sources(&[source]).unwrap_err();
            assert!(error.display_message().contains(expected));
        }

        let error = validate_sources(&[
            Source::new(PathBuf::from("H1.pass.dat"), 1, 1),
            Source::new(PathBuf::from("H1.hbp"), 2, 1),
        ])
        .unwrap_err();
        assert!(error.display_message().contains("duplicate sub_field_id"));
    }

    #[test]
    fn all_ag_fields_writers_emit_exact_versioned_empty_schemas() {
        let dir = temp_dir();
        let version = VersionInfo::new(1, 2);
        let cases = [
            (
                dir.join("H.pass.parquet"),
                schema_from_hillslope(hill_pass_schema(&version)),
            ),
            (
                dir.join("H.ebe.parquet"),
                schema_from_hillslope(hill_ebe_schema(&version)),
            ),
            (
                dir.join("H.element.parquet"),
                schema_from_hillslope(hill_element_schema(&version)),
            ),
            (
                dir.join("H.loss.parquet"),
                schema_from_hillslope(hill_loss_schema(&version)),
            ),
            (
                dir.join("H.soil.parquet"),
                schema_from_hillslope(hill_soil_schema(&version)),
            ),
            (
                dir.join("H.wat.parquet"),
                schema_from_hillslope(hill_wat_schema(&version)),
            ),
        ];

        crate::hill_pass::ag_fields_hillslope_pass_files_to_parquet(
            &[],
            &cases[0].0,
            None,
            &version,
            Some("legacy_ascii"),
        )
        .expect("write empty PASS");
        crate::hill_ebe::ag_fields_hillslope_ebe_files_to_parquet(
            &[],
            &cases[1].0,
            None,
            &version,
            Some(2000),
        )
        .expect("write empty EBE");
        crate::hill_element::ag_fields_hillslope_element_files_to_parquet(
            &[],
            &cases[2].0,
            &version,
            Some(2000),
        )
        .expect("write empty ELEMENT");
        crate::hill_loss::ag_fields_hillslope_loss_files_to_parquet(&[], &cases[3].0, &version)
            .expect("write empty LOSS");
        crate::hill_soil::ag_fields_hillslope_soil_files_to_parquet(
            &[],
            &cases[4].0,
            None,
            &version,
            Some(2000),
        )
        .expect("write empty SOIL");
        crate::hill_wat::ag_fields_hillslope_wat_files_to_parquet(&[], &cases[5].0, None, &version)
            .expect("write empty WAT");

        for (path, expected_schema) in cases {
            let builder = ParquetRecordBatchReaderBuilder::try_new(
                File::open(&path).expect("open empty AgFields parquet"),
            )
            .expect("build empty AgFields reader");
            assert_eq!(builder.schema().as_ref(), &expected_schema);
            assert_eq!(builder.metadata().file_metadata().num_rows(), 0);
            assert_eq!(builder.metadata().num_row_groups(), 0);
        }
    }

    #[test]
    fn every_ag_fields_writer_rejects_filename_mismatch_before_publication() {
        let dir = temp_dir();
        let version = VersionInfo::new(1, 2);
        let cases = [
            ("pass", "H2.pass.dat"),
            ("ebe", "H2.ebe.dat"),
            ("element", "H2.element.dat"),
            ("loss", "H2.loss.dat"),
            ("soil", "H2.soil.dat"),
            ("wat", "H2.wat.dat"),
        ];
        for (family, filename) in cases {
            let output = dir.join(format!("{family}.parquet"));
            let sources = [Source::new(dir.join(filename), 9, 1)];
            let error = match family {
                "pass" => crate::hill_pass::ag_fields_hillslope_pass_files_to_parquet(
                    &sources,
                    &output,
                    None,
                    &version,
                    Some("legacy_ascii"),
                ),
                "ebe" => crate::hill_ebe::ag_fields_hillslope_ebe_files_to_parquet(
                    &sources,
                    &output,
                    None,
                    &version,
                    Some(2000),
                ),
                "element" => crate::hill_element::ag_fields_hillslope_element_files_to_parquet(
                    &sources,
                    &output,
                    &version,
                    Some(2000),
                ),
                "loss" => crate::hill_loss::ag_fields_hillslope_loss_files_to_parquet(
                    &sources, &output, &version,
                ),
                "soil" => crate::hill_soil::ag_fields_hillslope_soil_files_to_parquet(
                    &sources,
                    &output,
                    None,
                    &version,
                    Some(2000),
                ),
                "wat" => crate::hill_wat::ag_fields_hillslope_wat_files_to_parquet(
                    &sources, &output, None, &version,
                ),
                _ => unreachable!(),
            }
            .expect_err("filename mismatch must fail");
            assert!(error.display_message().contains("does not match"));
            assert!(!output.exists());
        }
    }
}

#[cfg(test)]
pub fn assert_parquet_parity(ordinary_path: &Path, ag_fields_path: &Path, sources: &[Source]) {
    use std::fs::File;

    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

    let ordinary_builder = ParquetRecordBatchReaderBuilder::try_new(
        File::open(ordinary_path).expect("open ordinary parquet"),
    )
    .expect("build ordinary parquet reader");
    let ag_fields_builder = ParquetRecordBatchReaderBuilder::try_new(
        File::open(ag_fields_path).expect("open AgFields parquet"),
    )
    .expect("build AgFields parquet reader");
    let ordinary_schema = ordinary_builder.schema();
    let ag_fields_schema = ag_fields_builder.schema();
    assert_eq!(
        &ordinary_schema.fields()[1..],
        &ag_fields_schema.fields()[2..],
        "all non-identity fields must remain exact"
    );
    assert_eq!(ag_fields_schema.field(0).name(), "field_id");
    assert_eq!(ag_fields_schema.field(1).name(), "sub_field_id");
    assert_eq!(
        ag_fields_schema
            .metadata()
            .get("dataset_kind")
            .map(String::as_str),
        Some(DATASET_KIND)
    );
    assert_eq!(
        ag_fields_schema
            .metadata()
            .get("ag_fields_schema_version")
            .map(String::as_str),
        Some(SCHEMA_VERSION)
    );
    for (key, value) in ordinary_schema.metadata() {
        assert_eq!(ag_fields_schema.metadata().get(key), Some(value));
    }

    assert_eq!(ordinary_builder.metadata().num_row_groups(), sources.len());
    assert_eq!(ag_fields_builder.metadata().num_row_groups(), sources.len());
    for (index, source) in sources.iter().enumerate() {
        let ordinary_rows = ordinary_builder.metadata().row_group(index).num_rows();
        let ag_fields_rows = ag_fields_builder.metadata().row_group(index).num_rows();
        assert_eq!(ordinary_rows, ag_fields_rows);

        let ordinary_batches = ParquetRecordBatchReaderBuilder::try_new(
            File::open(ordinary_path).expect("reopen ordinary parquet"),
        )
        .expect("rebuild ordinary reader")
        .with_row_groups(vec![index])
        .build()
        .expect("build ordinary row-group reader")
        .collect::<Result<Vec<_>, _>>()
        .expect("read ordinary row group");
        let ag_fields_batches = ParquetRecordBatchReaderBuilder::try_new(
            File::open(ag_fields_path).expect("reopen AgFields parquet"),
        )
        .expect("rebuild AgFields reader")
        .with_row_groups(vec![index])
        .build()
        .expect("build AgFields row-group reader")
        .collect::<Result<Vec<_>, _>>()
        .expect("read AgFields row group");
        assert_eq!(ordinary_batches.len(), ag_fields_batches.len());

        for (ordinary, ag_fields) in ordinary_batches.iter().zip(&ag_fields_batches) {
            assert_eq!(ordinary.num_rows(), ag_fields.num_rows());
            assert_eq!(ordinary.num_columns() + 1, ag_fields.num_columns());
            let ordinary_id = ordinary
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("ordinary wepp_id Int32");
            let field_id = ag_fields
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("AgFields field_id Int32");
            let sub_field_id = ag_fields
                .column(1)
                .as_any()
                .downcast_ref::<Int32Array>()
                .expect("AgFields sub_field_id Int32");
            assert!(ordinary_id
                .values()
                .iter()
                .all(|id| *id == source.sub_field_id));
            assert!(field_id.values().iter().all(|id| *id == source.field_id));
            assert!(sub_field_id
                .values()
                .iter()
                .all(|id| *id == source.sub_field_id));

            for column in 1..ordinary.num_columns() {
                assert_eq!(
                    ordinary.column(column).to_data(),
                    ag_fields.column(column + 1).to_data(),
                    "column {} differs for source {}",
                    ordinary.schema().field(column).name(),
                    source.path.display()
                );
            }
        }
    }
}

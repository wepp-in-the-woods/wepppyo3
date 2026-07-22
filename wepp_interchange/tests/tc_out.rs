#![allow(dead_code)]

#[path = "../src/arrow_support.rs"]
mod arrow_support;
#[path = "../src/calendar.rs"]
mod calendar;
#[path = "../src/errors.rs"]
mod errors;
#[path = "../src/floats.rs"]
mod floats;
#[path = "../src/parquet.rs"]
mod parquet;
#[path = "../src/schema.rs"]
mod schema;
#[path = "../src/tc_out.rs"]
mod tc_out;

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use arrow_array::{Array, Float64Array, Int16Array, Int32Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use parquet::arrow::ArrowWriter;

    use crate::errors::InterchangeError;
    use crate::schema::{watershed_tc_out_schema, VersionInfo};
    use crate::tc_out::watershed_tc_out_to_parquet;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(stem: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock before epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "wepp_interchange_tc_out_{stem}_{}_{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn read_batches(path: &Path) -> (Schema, Vec<RecordBatch>) {
        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path).expect("open"))
            .expect("build reader");
        let schema = builder.schema().as_ref().clone();
        let batches = builder
            .build()
            .expect("reader")
            .map(|batch| batch.expect("batch"))
            .collect();
        (schema, batches)
    }

    fn write_calendar(path: &Path) {
        let schema = Arc::new(Schema::new(vec![
            Field::new("year", DataType::Int16, true),
            Field::new("month", DataType::Int8, true),
            Field::new("day_of_month", DataType::Int8, true),
        ]));
        let batch = RecordBatch::try_new(
            Arc::clone(&schema),
            vec![
                Arc::new(Int16Array::from(vec![2001, 2001, 2002, 2002])),
                Arc::new(arrow_array::Int8Array::from(vec![1, 1, 1, 1])),
                Arc::new(arrow_array::Int8Array::from(vec![1, 2, 1, 2])),
            ],
        )
        .expect("calendar batch");
        let mut writer =
            ArrowWriter::try_new(File::create(path).expect("calendar file"), schema, None)
                .expect("calendar writer");
        writer.write(&batch).expect("write calendar");
        writer.close().expect("close calendar");
    }

    #[test]
    fn schema_matches_wepppy_contract() {
        let schema = watershed_tc_out_schema(&VersionInfo::new(1, 2));
        let expected = [
            ("day", DataType::Int16, None, "Julian day from tc_out.txt"),
            ("year", DataType::Int16, None, "Calendar year"),
            (
                "sim_day_index",
                DataType::Int32,
                None,
                "1-indexed simulation day",
            ),
            (
                "julian",
                DataType::Int16,
                None,
                "Julian day from tc_out.txt (alias of day)",
            ),
            (
                "Time of Conc (hr)",
                DataType::Float64,
                Some("hr"),
                "Event time of concentration at the outlet channel",
            ),
            (
                "Storm Duration (hr)",
                DataType::Float64,
                Some("hr"),
                "Storm duration for the event",
            ),
            (
                "Storm Peak (hr)",
                DataType::Float64,
                Some("hr"),
                "Time to storm peak for the event",
            ),
        ];

        assert_eq!(schema.fields().len(), expected.len());
        for (field, (name, data_type, units, description)) in schema.fields().iter().zip(expected) {
            assert_eq!(field.name(), name);
            assert_eq!(field.data_type(), &data_type);
            assert_eq!(field.metadata().get("units").map(String::as_str), units);
            assert_eq!(
                field.metadata().get("description").map(String::as_str),
                Some(description)
            );
        }
        assert_eq!(
            schema.metadata().get("dataset_version").map(String::as_str),
            Some("1.2")
        );
        assert_eq!(
            schema.metadata().get("schema_version").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn selects_max_channel_and_preserves_selected_source_order() {
        let dir = TestDir::new("order");
        let source = dir.path().join("tc_out.txt");
        let target = dir.path().join("tc_out.parquet");
        fs::write(
            &source,
            "Element header\n\
             1 C 2 50 1 0 2.0 3.0 4.0\n\
             2 C 10 100 1 0 .5 1.5 2.5\n\
             3 H 99 101 1 0 9.0 9.0 9.0\n\
             4 C 10 1 2 0 6.5 7.5 8.5\n\
             5 C 3 2 2 0 3.0 3.0 3.0\n",
        )
        .expect("write source");

        let summary = watershed_tc_out_to_parquet(
            &source,
            &target,
            None,
            &VersionInfo::new(1, 2),
            Some(2020),
            Some(1),
        )
        .expect("convert");

        assert_eq!(summary.rows_written, 2);
        assert_eq!(summary.row_groups, 2);
        assert_eq!(summary.outlet_channel, Some(10));
        assert_eq!(summary.output_paths, vec![target.display().to_string()]);
        assert!(target.exists());
        assert!(!dir.path().join("tc_out.parquet.tmp").exists());

        let (schema, batches) = read_batches(&target);
        assert_eq!(schema.fields().len(), 7);
        let days: Vec<i16> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<Int16Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        let years: Vec<i16> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<Int16Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        let sim_days: Vec<i32> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(2)
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        let times: Vec<f64> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(4)
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        assert_eq!(days, vec![100, 1]);
        assert_eq!(years, vec![2020, 2021]);
        assert_eq!(sim_days, vec![100, 367]);
        assert_eq!(times, vec![0.5, 6.5]);
    }

    #[test]
    fn uses_cli_calendar_start_year_and_day_counts() {
        let dir = TestDir::new("calendar");
        let source = dir.path().join("tc_out.txt");
        let target = dir.path().join("tc_out.parquet");
        let calendar = dir.path().join("wepp_cli.parquet");
        write_calendar(&calendar);
        fs::write(
            &source,
            "1 C 4 2 1 0 1.0 2.0 3.0\n2 C 4 1 2 0 4.0 5.0 6.0\n",
        )
        .expect("write source");

        watershed_tc_out_to_parquet(
            &source,
            &target,
            Some(&calendar),
            &VersionInfo::new(1, 2),
            None,
            None,
        )
        .expect("convert");

        let (_, batches) = read_batches(&target);
        let years: Vec<i16> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<Int16Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        let sim_days: Vec<i32> = batches
            .iter()
            .flat_map(|batch| {
                batch
                    .column(2)
                    .as_any()
                    .downcast_ref::<Int32Array>()
                    .unwrap()
                    .values()
                    .to_vec()
            })
            .collect();
        assert_eq!(years, vec![2001, 2002]);
        assert_eq!(sim_days, vec![2, 3]);
    }

    #[test]
    fn no_channel_rows_returns_empty_summary_without_output() {
        let dir = TestDir::new("empty");
        let source = dir.path().join("tc_out.txt");
        let target = dir.path().join("tc_out.parquet");
        fs::write(&source, "Element header\n1 H 99 1 1 0 1.0 2.0 3.0\n").expect("write source");

        let summary = watershed_tc_out_to_parquet(
            &source,
            &target,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
        )
        .expect("convert");

        assert_eq!(summary.rows_written, 0);
        assert_eq!(summary.row_groups, 0);
        assert!(summary.output_paths.is_empty());
        assert_eq!(summary.outlet_channel, None);
        assert!(!target.exists());
    }

    #[test]
    fn malformed_channel_record_is_a_contextual_parse_error() {
        let dir = TestDir::new("bad_channel_record");
        let source = dir.path().join("tc_out.txt");
        let target = dir.path().join("tc_out.parquet");
        fs::write(&source, "1 C bad 1 1 0 1.0 2.0 3.0\n").expect("write source");

        let error = watershed_tc_out_to_parquet(
            &source,
            &target,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
        )
        .expect_err("malformed channel record must fail");

        assert!(matches!(error, InterchangeError::Parse { .. }));
        assert!(!target.exists());
    }

    #[test]
    fn missing_input_is_typed_io_error_and_parse_failure_is_atomic() {
        let dir = TestDir::new("errors");
        let missing = dir.path().join("missing.txt");
        let target = dir.path().join("tc_out.parquet");
        let error = watershed_tc_out_to_parquet(
            &missing,
            &target,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
        )
        .expect_err("missing input must fail");
        assert!(matches!(error, InterchangeError::Io { path, .. } if path == missing));

        let source = dir.path().join("tc_out.txt");
        fs::write(&source, "1 C 7 1 1 0 invalid 2.0 3.0\n").expect("write source");
        fs::write(&target, b"existing target").expect("write sentinel target");
        let error = watershed_tc_out_to_parquet(
            &source,
            &target,
            None,
            &VersionInfo::new(1, 2),
            None,
            None,
        )
        .expect_err("invalid measurement must fail");
        assert!(matches!(error, InterchangeError::Parse { .. }));
        assert_eq!(
            fs::read(&target).expect("read sentinel"),
            b"existing target"
        );
    }
}

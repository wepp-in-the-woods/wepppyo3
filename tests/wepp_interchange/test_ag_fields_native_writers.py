from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pyarrow.parquet as pq
import pytest

from wepppyo3.wepp_interchange import (
    ag_fields_hillslope_ebe_files_to_parquet,
    ag_fields_hillslope_element_files_to_parquet,
    ag_fields_hillslope_loss_files_to_parquet,
    ag_fields_hillslope_pass_files_to_parquet,
    ag_fields_hillslope_soil_files_to_parquet,
    ag_fields_hillslope_wat_files_to_parquet,
    hillslope_ebe_files_to_parquet,
    hillslope_element_files_to_parquet,
    hillslope_loss_files_to_parquet,
    hillslope_pass_files_to_parquet,
    hillslope_soil_files_to_parquet,
    hillslope_wat_files_to_parquet,
)


Writer = Callable[..., dict[str, object]]


WRITERS: dict[str, tuple[Writer, Writer, dict[str, object]]] = {
    "pass": (
        ag_fields_hillslope_pass_files_to_parquet,
        hillslope_pass_files_to_parquet,
        {"pass_family": "legacy_ascii"},
    ),
    "ebe": (
        ag_fields_hillslope_ebe_files_to_parquet,
        hillslope_ebe_files_to_parquet,
        {"start_year": 2000},
    ),
    "element": (
        ag_fields_hillslope_element_files_to_parquet,
        hillslope_element_files_to_parquet,
        {"start_year": 2000},
    ),
    "loss": (
        ag_fields_hillslope_loss_files_to_parquet,
        hillslope_loss_files_to_parquet,
        {},
    ),
    "soil": (
        ag_fields_hillslope_soil_files_to_parquet,
        hillslope_soil_files_to_parquet,
        {"start_year": 2000},
    ),
    "wat": (
        ag_fields_hillslope_wat_files_to_parquet,
        hillslope_wat_files_to_parquet,
        {},
    ),
}


@pytest.mark.parametrize("family", WRITERS)
def test_ag_fields_empty_schema_is_additive_and_isolated(
    family: str, tmp_path: Path
) -> None:
    ag_writer, ordinary_writer, kwargs = WRITERS[family]
    ordinary_path = tmp_path / f"ordinary.{family}.parquet"
    ag_path = tmp_path / f"ag_fields.{family}.parquet"

    ordinary_summary = ordinary_writer([], str(ordinary_path), 1, 2, **kwargs)
    ag_summary = ag_writer([], str(ag_path), 1, 2, **kwargs)

    assert ordinary_summary["rows_written"] == ag_summary["rows_written"] == 0
    assert ordinary_summary["row_groups"] == ag_summary["row_groups"] == 0
    ordinary_schema = pq.read_schema(ordinary_path)
    ag_schema = pq.read_schema(ag_path)
    assert ag_schema.names[:2] == ["field_id", "sub_field_id"]
    assert "wepp_id" not in ag_schema.names
    assert "topaz_id" not in ag_schema.names
    assert ag_schema.field("field_id").nullable is False
    assert ag_schema.field("sub_field_id").nullable is False
    assert ag_schema.field("field_id").type == ordinary_schema.field("wepp_id").type
    assert ag_schema.field("sub_field_id").type == ordinary_schema.field("wepp_id").type
    assert list(ag_schema)[2:] == list(ordinary_schema)[1:]
    assert ordinary_schema.metadata == {
        b"dataset_version": b"1.2",
        b"dataset_version_major": b"1",
        b"dataset_version_minor": b"2",
        b"schema_version": b"1",
    }
    assert ag_schema.metadata == {
        **ordinary_schema.metadata,
        b"dataset_kind": b"ag_fields_hillslope",
        b"ag_fields_schema_version": b"1",
    }


@pytest.mark.parametrize("family", WRITERS)
def test_ag_fields_rejects_filename_identity_mismatch_without_publication(
    family: str, tmp_path: Path
) -> None:
    writer, _, kwargs = WRITERS[family]
    output = tmp_path / f"H.{family}.parquet"

    with pytest.raises(ValueError, match="does not match supplied sub_field_id"):
        writer(
            [(str(tmp_path / f"H2.{family}.dat"), 10, 1)],
            str(output),
            1,
            2,
            **kwargs,
        )

    assert not output.exists()


@pytest.mark.parametrize("family", WRITERS)
def test_ag_fields_rejects_duplicate_sub_field_before_reading_sources(
    family: str, tmp_path: Path
) -> None:
    writer, _, kwargs = WRITERS[family]
    output = tmp_path / f"H.{family}.parquet"
    suffix = "hbp" if family == "pass" else f"{family}.dat"

    with pytest.raises(ValueError, match="duplicate sub_field_id 1"):
        writer(
            [
                (str(tmp_path / f"H1.{suffix}"), 10, 1),
                (str(tmp_path / f"H1.{suffix}"), 11, 1),
            ],
            str(output),
            1,
            2,
            **kwargs,
        )

    assert not output.exists()


@pytest.mark.parametrize(
    ("sources", "error_type"),
    [
        ([('H1.pass.dat', "field", 1)], TypeError),
        ([('H1.pass.dat', 1, "sub-field")], TypeError),
        ([('H1.pass.dat', 2**31, 1)], OverflowError),
        ([('H1.pass.dat', 1, 2**31)], OverflowError),
        ([('H1.pass.dat', 1)], ValueError),
    ],
)
def test_ag_fields_python_boundary_rejects_uncoupled_or_non_int32_sources(
    sources: object, error_type: type[BaseException], tmp_path: Path
) -> None:
    with pytest.raises(error_type):
        ag_fields_hillslope_pass_files_to_parquet(
            sources,
            str(tmp_path / "H.pass.parquet"),
            1,
            2,
            pass_family="legacy_ascii",
        )


def test_ag_fields_rejects_non_positive_ids_and_unsupported_compression(
    tmp_path: Path,
) -> None:
    writer = ag_fields_hillslope_pass_files_to_parquet
    target = tmp_path / "H.pass.parquet"
    with pytest.raises(ValueError, match="field_id must be positive"):
        writer(
            [(str(tmp_path / "H1.pass.dat"), 0, 1)],
            str(target),
            1,
            2,
            pass_family="legacy_ascii",
        )
    with pytest.raises(ValueError, match="sub_field_id must be positive"):
        writer(
            [(str(tmp_path / "H1.pass.dat"), 1, 0)],
            str(target),
            1,
            2,
            pass_family="legacy_ascii",
        )
    with pytest.raises(ValueError, match="only 'snappy' is supported"):
        writer([], str(target), 1, 2, compression="gzip")
    assert not target.exists()

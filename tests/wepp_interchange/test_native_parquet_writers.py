from __future__ import annotations

from pathlib import Path

import pyarrow.parquet as pq
import pytest

from wepppyo3.wepp_interchange import (
    hillslope_ebe_files_to_parquet,
    hillslope_element_files_to_parquet,
    hillslope_loss_files_to_parquet,
    hillslope_pass_files_to_parquet,
    hillslope_soil_files_to_parquet,
    watershed_ebe_to_parquet,
    watershed_pass_cli_hint,
    watershed_tc_out_to_parquet,
)


HILLSLOPE_SCHEMAS = {
    "pass": [
        "wepp_id",
        "event",
        "year",
        "sim_day_index",
        "julian",
        "month",
        "day_of_month",
        "water_year",
        "dur",
        "tcs",
        "oalpha",
        "runoff",
        "runvol",
        "sbrunf",
        "sbrunv",
        "drainq",
        "drrunv",
        "peakro",
        "tdet",
        "tdep",
        "sedcon_1",
        "sedcon_2",
        "sedcon_3",
        "sedcon_4",
        "sedcon_5",
        "clot",
        "slot",
        "saot",
        "laot",
        "sdot",
        "gwbfv",
        "gwdsv",
    ],
    "ebe": [
        "wepp_id",
        "year",
        "sim_day_index",
        "month",
        "day_of_month",
        "julian",
        "water_year",
        "Precip",
        "Runoff",
        "IR-det",
        "Av-det",
        "Mx-det",
        "Det-point",
        "Av-dep",
        "Max-dep",
        "Dep-point",
        "Sed.Del",
        "ER",
        "Det-Len",
        "Dep-Len",
    ],
    "element": [
        "wepp_id",
        "ofe_id",
        "year",
        "julian",
        "month",
        "day_of_month",
        "water_year",
        "OFE",
        "Precip",
        "Runoff",
        "EffInt",
        "PeakRO",
        "EffDur",
        "Enrich",
        "Keff",
        "Sm",
        "LeafArea",
        "CanHgt",
        "Cancov",
        "IntCov",
        "RilCov",
        "LivBio",
        "DeadBio",
        "Ki",
        "Kr",
        "Tcrit",
        "RilWid",
        "SedLeave",
        "QRain",
        "QSnow",
    ],
    "loss": [
        "wepp_id",
        "class_id",
        "Class",
        "Diameter",
        "Specific Gravity",
        "% Sand",
        "% Silt",
        "% Clay",
        "% O.M.",
        "Sediment Fraction",
        "In Flow Exiting",
    ],
    "soil": [
        "wepp_id",
        "ofe_id",
        "year",
        "sim_day_index",
        "julian",
        "month",
        "day_of_month",
        "water_year",
        "OFE",
        "Poros",
        "Keff",
        "Suct",
        "FC",
        "WP",
        "Rough",
        "Ki",
        "Kr",
        "Tauc",
        "Saturation",
        "TSW",
        "TSMF",
    ],
}


def _assert_versioned_empty_parquet(path: Path, expected_names: list[str]) -> None:
    table = pq.read_table(path)
    assert table.num_rows == 0
    assert table.schema.names == expected_names
    assert table.schema.metadata == {
        b"dataset_version": b"1.2",
        b"dataset_version_major": b"1",
        b"dataset_version_minor": b"2",
        b"schema_version": b"1",
    }


def test_hillslope_bulk_writers_publish_versioned_empty_parquet(tmp_path: Path) -> None:
    calls = [
        (
            "pass",
            hillslope_pass_files_to_parquet,
            {"pass_family": "legacy_ascii"},
        ),
        ("ebe", hillslope_ebe_files_to_parquet, {}),
        ("element", hillslope_element_files_to_parquet, {}),
        ("loss", hillslope_loss_files_to_parquet, {}),
        ("soil", hillslope_soil_files_to_parquet, {}),
    ]

    for label, writer, kwargs in calls:
        target = tmp_path / f"H.{label}.parquet"
        summary = writer([], str(target), 1, 2, **kwargs)

        assert summary["rows_written"] == 0
        assert summary["row_groups"] == 0
        assert summary["output_paths"] == [str(target)]
        _assert_versioned_empty_parquet(target, HILLSLOPE_SCHEMAS[label])

    pass_schema = pq.read_schema(tmp_path / "H.pass.parquet")
    assert pass_schema.field("clot").metadata == {
        b"units": b"m^3/s",
        b"description": b"Friction flow 1",
    }
    assert pass_schema.field("slot").metadata == {
        b"units": b"%",
        b"description": b"% of exiting sediment in the silt size class",
    }
    assert pass_schema.field("gwbfv").metadata == {
        b"description": b"Groundwater baseflow"
    }
    soil_schema = pq.read_schema(tmp_path / "H.soil.parquet")
    assert soil_schema.field("TSMF").metadata == {
        b"units": b"frac",
        b"description": b"True soil moisture fraction (full profile)",
    }


def test_watershed_tc_out_selects_max_channel_and_preserves_order(tmp_path: Path) -> None:
    source = tmp_path / "tc_out.txt"
    target = tmp_path / "tc_out.parquet"
    source.write_text(
        "\n".join(
            [
                "Element Type ID Day Year Skip TConc Duration Peak",
                "1 C 2 10 1 0 1.00 2.00 3.00",
                "2 C 9 20 1 0 4.00 5.00 6.00",
                "3 C 4 30 1 0 7.00 8.00 9.00",
                "4 C 9 21 1 0 10.00 11.00 12.00",
            ]
        )
        + "\n",
        encoding="ascii",
    )

    summary = watershed_tc_out_to_parquet(
        str(source),
        str(target),
        1,
        2,
        start_year=2000,
    )

    assert summary["rows_written"] == 2
    assert summary["outlet_channel"] == 9
    assert summary["output_paths"] == [str(target)]
    table = pq.read_table(target)
    assert table.schema.names == [
        "day",
        "year",
        "sim_day_index",
        "julian",
        "Time of Conc (hr)",
        "Storm Duration (hr)",
        "Storm Peak (hr)",
    ]
    assert table.column("day").to_pylist() == [20, 21]
    assert table.column("year").to_pylist() == [2000, 2000]
    assert table.column("sim_day_index").to_pylist() == [20, 21]
    assert table.column("Time of Conc (hr)").to_pylist() == [4.0, 10.0]


def test_watershed_tc_out_without_channel_rows_does_not_publish(tmp_path: Path) -> None:
    source = tmp_path / "tc_out.txt"
    target = tmp_path / "tc_out.parquet"
    source.write_text("1 H 2 10 1 0 1.00 2.00 3.00\n", encoding="ascii")

    summary = watershed_tc_out_to_parquet(str(source), str(target), 1, 2)

    assert summary["rows_written"] == 0
    assert summary["row_groups"] == 0
    assert summary["outlet_channel"] is None
    assert summary["output_paths"] == []
    assert not target.exists()


def test_watershed_pass_cli_hint_reads_plain_and_returns_none_for_bad_input(
    tmp_path: Path,
) -> None:
    pass_path = tmp_path / "pass_pw0.txt"
    pass_path.write_text(
        "1.0 --> VERSION NUMBER\n"
        "1 NUMBER OF UNIQUE HILLSLOPES IN WATERSHED\n"
        "1 WATERSHED MAXIMUM SIMULATION TIME (YEARS)\n"
        "2000 BEGINNING YEAR OF WATERSHED CLIMATE FILE\n"
        "HILLSLOPE 1 climate/example.cli 0.1 0.2 10.0 1.0 2.0 3.0 4.0\n"
        "BEGIN HILLSLOPE HYDROLOGY AND SEDIMENT INFORMATION\n",
        encoding="ascii",
    )
    assert watershed_pass_cli_hint(str(pass_path)) == "climate/example.cli"

    pass_path.write_text("not a PASS file\n", encoding="ascii")
    assert watershed_pass_cli_hint(str(pass_path)) is None


def test_watershed_ebe_native_outlet_inference_and_peak_audit(tmp_path: Path) -> None:
    ebe_path = tmp_path / "ebe_pw0.txt"
    chan_path = tmp_path / "chan.out"
    target = tmp_path / "watershed_ebe.parquet"
    (tmp_path / "H7.ebe.dat").write_text("", encoding="ascii")
    chan_path.write_text(
        "Year J Elmt_ID Chan_ID Time Peak\n2000 1 41 1 0 2.0\n",
        encoding="ascii",
    )
    ebe_path.write_text(
        "1 1 2000 1.0 2.0 0.5 4.0 5.0 6.0 7.0\n",
        encoding="ascii",
    )

    summary = watershed_ebe_to_parquet(
        str(ebe_path),
        str(target),
        1,
        2,
        chan_path=str(chan_path),
    )
    assert summary["rows_written"] == 1
    assert pq.read_table(target).column("element_id").to_pylist() == [8]

    target.unlink()
    ebe_path.write_text(
        "1 1 2000 1.0 2.0 0.0 4.0 5.0 6.0 7.0\n",
        encoding="ascii",
    )
    with pytest.raises(ValueError, match="all-zero"):
        watershed_ebe_to_parquet(
            str(ebe_path),
            str(target),
            1,
            2,
            chan_path=str(chan_path),
        )
    assert not target.exists()

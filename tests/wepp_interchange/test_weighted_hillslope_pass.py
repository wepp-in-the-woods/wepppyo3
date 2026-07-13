from __future__ import annotations

from pathlib import Path

import pytest

from wepppyo3.wepp_interchange import combine_weighted_hillslope_pass_files
from wepppyo3.wepp_interchange import hillslope_pass_to_columns


def _write_pass(path: Path, *, climate_token: str, runvol: float) -> None:
    values = [
        3600.0,
        1.0,
        0.72,
        runvol / 4440.9,
        runvol,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0,
        3.0,
        1.0,
        0.01,
        0.02,
        0.03,
        0.04,
        0.05,
        1.0 / 15.0,
        2.0 / 15.0,
        3.0 / 15.0,
        4.0 / 15.0,
        5.0 / 15.0,
        4.0,
        6.0,
    ]
    event = "EVENT      2000     1     " + " ".join(f"{value:.5E}" for value in values)
    path.write_text(
        "\n".join(
            [
                climate_token,
                "   1      2000",
                ".44409E+04",
                "  5    0.20000E-05 0.10000E-04 0.30000E-04 0.30600E-03 0.20000E-03",
                "    0.00     0.00     0.00     0.00",
                event,
            ]
        )
        + "\n",
        encoding="ascii",
    )


def test_weighted_pass_python_api_returns_closure_diagnostics(tmp_path: Path) -> None:
    background = tmp_path / "H71.pass.dat"
    field = tmp_path / "H971.pass.dat"
    output = tmp_path / "H71.weighted.pass.dat"
    _write_pass(background, climate_token="parent.cli", runvol=100.0)
    _write_pass(field, climate_token="field.cli", runvol=200.0)

    result = combine_weighted_hillslope_pass_files(
        [
            ("background", str(background), 2220.45),
            ("field:971", str(field), 2220.45),
        ],
        str(output),
        4440.9,
        "../runs/p71.cli",
    )

    assert result["algorithm"] == "ag_fields_v1"
    assert result["semantic_contract"] == "ag_fields_pass_semantics_v1"
    assert result["source_count"] == 2
    assert result["row_count"] == 1
    assert result["events"][0]["weighted_input"]["runvol_m3"] == pytest.approx(150.0)
    assert abs(result["events"][0]["residuals"]["runvol_m3"]) <= result["events"][0][
        "budgets"
    ]["runvol_m3"]

    columns = hillslope_pass_to_columns(str(output), 1, 0, pass_family="legacy_ascii")
    assert columns["runvol"] == pytest.approx([150.0])
    assert output.read_text(encoding="ascii").splitlines()[0] == "../runs/p71.cli"

from __future__ import annotations

from pathlib import Path

import pyarrow.parquet as pq

from wepppyo3.wepp_interchange import hillslope_wat_files_to_parquet


HEADER = """ ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
  OFE    J    Y      P      RM     Q                Ep      Es      Er     Dp       UpStrmQ   SubRIn    latqcc Total-Soil frozwt Snow-Water QOFE            Tile    Irr        Area
  #      -    -      mm     mm     mm               mm      mm      mm       mm      mm           mm      mm   Water(mm)   mm        mm      mm             mm      mm         m^2
 ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

"""


def _write_wat(path: Path, wepp_id: int) -> None:
    assert path.name == f"H{wepp_id}.wat.dat"
    rows = """     1    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00
     2    1 2000   10.00   10.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      75.00
     1    2 2000   11.00   11.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      50.00
     2    2 2000   11.00   11.00   0.0000000E+00    0.10    0.20    0.30    0.40   0.0000000E+00    0.00    0.50  100.00    1.25    0.00    0.0000000E+00    0.00    0.00      75.00
"""
    path.write_text(HEADER + rows, encoding="utf-8")


def test_hillslope_wat_files_write_direct_parquet(tmp_path: Path) -> None:
    first = tmp_path / "H1.wat.dat"
    second = tmp_path / "H2.wat.dat"
    target = tmp_path / "H.wat.parquet"
    _write_wat(first, 1)
    _write_wat(second, 2)

    summary = hillslope_wat_files_to_parquet(
        [str(first), str(second)],
        str(target),
        1,
        0,
    )

    assert summary["rows_written"] == 8
    assert summary["row_groups"] == 2
    table = pq.read_table(target)
    assert table.column("wepp_id").to_pylist() == [1, 1, 1, 1, 2, 2, 2, 2]
    assert table.column("sim_day_index").to_pylist() == [1, 1, 2, 2, 1, 1, 2, 2]

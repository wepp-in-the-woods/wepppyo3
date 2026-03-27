import json
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest
import rasterio
from rasterio.transform import from_origin

REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_ROOT = REPO_ROOT / "release" / "linux" / "py312"

if str(RELEASE_ROOT) not in sys.path:
    sys.path.insert(0, str(RELEASE_ROOT))

from wepppyo3.roads_flowpath import trace_downslope_flowpath


EXPECTED_KEYS = {
    "seed_row",
    "seed_col",
    "seed_topaz_id",
    "reaches_channel",
    "channel_row",
    "channel_col",
    "channel_topaz_id",
    "termination_reason",
    "rows",
    "cols",
    "indices",
    "distance_m",
    "elevation_m",
    "segment_slope",
    "path_length_m",
    "drop_m",
    "mean_slope",
    "max_slope",
}


def _write_raster(path: Path, data: np.ndarray, dtype: str) -> None:
    height, width = data.shape
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        height=height,
        width=width,
        count=1,
        dtype=dtype,
        crs="EPSG:4326",
        transform=from_origin(0.0, 0.0, 1.0, 1.0),
    ) as dst:
        dst.write(data.astype(dtype), 1)


def _build_fixture_rasters(tmp_path: Path) -> tuple[Path, Path, Path, Path]:
    subwta = np.array([[11, 11, 11, 11]], dtype=np.int32)
    flovec = np.array([[6, 6, 6, 0]], dtype=np.uint8)
    relief = np.array([[100.0, 99.0, 98.0, 97.0]], dtype=np.float32)
    channel = np.array([[0, 0, 1, 1]], dtype=np.int32)

    subwta_path = tmp_path / "subwta.tif"
    flovec_path = tmp_path / "flovec.tif"
    relief_path = tmp_path / "relief.tif"
    channel_path = tmp_path / "channel.tif"

    _write_raster(subwta_path, subwta, "int32")
    _write_raster(flovec_path, flovec, "uint8")
    _write_raster(relief_path, relief, "float32")
    _write_raster(channel_path, channel, "int32")

    return subwta_path, flovec_path, relief_path, channel_path


def test_trace_downslope_flowpath_contract_and_values(tmp_path: Path) -> None:
    subwta_path, flovec_path, relief_path, channel_path = _build_fixture_rasters(tmp_path)

    result = trace_downslope_flowpath(
        str(subwta_path),
        str(flovec_path),
        str(relief_path),
        seed_row=0,
        seed_col=0,
        channel_path=str(channel_path),
        max_steps=100,
    )

    assert set(result.keys()) == EXPECTED_KEYS
    assert result["termination_reason"] == "hit_channel"
    assert result["reaches_channel"] is True
    assert result["channel_row"] == 0
    assert result["channel_col"] == 2
    assert result["rows"] == [0, 0, 0]
    assert result["cols"] == [0, 1, 2]
    assert result["indices"] == [0, 1, 2]
    assert result["distance_m"] == pytest.approx([0.0, 1.0, 2.0])
    assert result["elevation_m"] == pytest.approx([100.0, 99.0, 98.0])
    assert result["segment_slope"] == pytest.approx([1.0, 1.0])
    assert result["path_length_m"] == pytest.approx(2.0)
    assert result["drop_m"] == pytest.approx(2.0)
    assert result["mean_slope"] == pytest.approx(1.0)
    assert result["max_slope"] == pytest.approx(1.0)


def test_trace_downslope_flowpath_matches_peridot_cli_selected_fields(tmp_path: Path) -> None:
    subwta_path, flovec_path, relief_path, channel_path = _build_fixture_rasters(tmp_path)
    cli_json_path = tmp_path / "trace_cli.json"

    command = [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        "/workdir/peridot/Cargo.toml",
        "--bin",
        "trace_downslope_flowpath",
        "--",
        "--subwta",
        str(subwta_path),
        "--flovec",
        str(flovec_path),
        "--relief",
        str(relief_path),
        "--channel",
        str(channel_path),
        "--seed-row",
        "0",
        "--seed-col",
        "0",
        "--max-steps",
        "100",
        "--out-json",
        str(cli_json_path),
    ]
    subprocess.run(command, check=True)

    with cli_json_path.open("r", encoding="utf-8") as handle:
        cli_result = json.load(handle)

    py_result = trace_downslope_flowpath(
        str(subwta_path),
        str(flovec_path),
        str(relief_path),
        seed_row=0,
        seed_col=0,
        channel_path=str(channel_path),
        max_steps=100,
    )

    for key in [
        "termination_reason",
        "reaches_channel",
        "channel_row",
        "channel_col",
        "channel_topaz_id",
        "rows",
        "cols",
        "indices",
    ]:
        assert py_result[key] == cli_result[key]

    for key in ["path_length_m", "drop_m", "mean_slope", "max_slope"]:
        assert py_result[key] == pytest.approx(cli_result[key])


def test_trace_downslope_flowpath_rejects_negative_seed_row(tmp_path: Path) -> None:
    subwta_path, flovec_path, relief_path, channel_path = _build_fixture_rasters(tmp_path)

    with pytest.raises(ValueError, match="seed_row must be >= 0"):
        trace_downslope_flowpath(
            str(subwta_path),
            str(flovec_path),
            str(relief_path),
            seed_row=-1,
            seed_col=0,
            channel_path=str(channel_path),
            max_steps=100,
        )

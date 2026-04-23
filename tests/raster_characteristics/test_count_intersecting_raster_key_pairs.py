from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest
import rasterio
from rasterio.transform import from_origin

from wepppyo3.raster_characteristics import count_intersecting_raster_key_pairs


def _write_i32_raster(path: Path, data: np.ndarray, nodata: int = -9999) -> None:
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        height=data.shape[0],
        width=data.shape[1],
        count=1,
        dtype=np.int32,
        crs="EPSG:4326",
        transform=from_origin(0.0, 0.0, 30.0, 30.0),
        nodata=nodata,
    ) as dst:
        dst.write(data.astype(np.int32), 1)


def test_count_intersecting_raster_key_pairs_counts_expected_values(tmp_path: Path) -> None:
    key_fn = tmp_path / "subwta.tif"
    key2_fn = tmp_path / "mofe.tif"

    _write_i32_raster(
        key_fn,
        np.array(
            [
                [11, 11, 12],
                [12, 14, -9999],
            ],
            dtype=np.int32,
        ),
    )
    _write_i32_raster(
        key2_fn,
        np.array(
            [
                [1, 2, 1],
                [1, 2, 1],
            ],
            dtype=np.int32,
        ),
    )

    result = count_intersecting_raster_key_pairs(
        key_fn=str(key_fn),
        key2_fn=str(key2_fn),
        ignore_channels=False,
    )

    assert result == {
        "11": {"1": 1, "2": 1},
        "12": {"1": 2},
        "14": {"2": 1},
    }


def test_count_intersecting_raster_key_pairs_respects_ignore_filters(tmp_path: Path) -> None:
    key_fn = tmp_path / "subwta.tif"
    key2_fn = tmp_path / "mofe.tif"

    _write_i32_raster(
        key_fn,
        np.array(
            [
                [11, 11, 12],
                [12, 14, 14],
            ],
            dtype=np.int32,
        ),
    )
    _write_i32_raster(
        key2_fn,
        np.array(
            [
                [1, 2, 1],
                [2, 1, 2],
            ],
            dtype=np.int32,
        ),
    )

    result = count_intersecting_raster_key_pairs(
        key_fn=str(key_fn),
        key2_fn=str(key2_fn),
        ignore_channels=True,
        ignore_keys={12},
        ignore_keys2={2},
    )

    assert result == {"11": {"1": 1}}


def test_count_intersecting_raster_key_pairs_shape_mismatch_raises(tmp_path: Path) -> None:
    key_fn = tmp_path / "subwta.tif"
    key2_fn = tmp_path / "mofe.tif"

    _write_i32_raster(key_fn, np.array([[11, 12], [13, 14]], dtype=np.int32))
    _write_i32_raster(key2_fn, np.array([[1, 2, 3]], dtype=np.int32))

    with pytest.raises(ValueError, match="Raster shape mismatch"):
        count_intersecting_raster_key_pairs(
            key_fn=str(key_fn),
            key2_fn=str(key2_fn),
        )


def test_count_intersecting_raster_key_pairs_read_error_raises(tmp_path: Path) -> None:
    key2_fn = tmp_path / "mofe.tif"
    _write_i32_raster(key2_fn, np.array([[1, 2], [3, 4]], dtype=np.int32))

    with pytest.raises(OSError, match="Failed to read raster"):
        count_intersecting_raster_key_pairs(
            key_fn=str(tmp_path / "missing.tif"),
            key2_fn=str(key2_fn),
        )

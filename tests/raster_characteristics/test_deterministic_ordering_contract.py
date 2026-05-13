from __future__ import annotations

from pathlib import Path
from typing import Dict, Tuple

import numpy as np
import pytest
import rasterio
from rasterio.transform import from_origin

from wepppyo3.raster_characteristics import (
    count_intersecting_raster_key_pairs,
    identify_median_intersecting_raster_keys,
    identify_median_single_raster_key,
    identify_mode_intersecting_raster_keys,
    identify_mode_single_raster_key,
)


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


def _write_f64_raster(path: Path, data: np.ndarray, nodata: float = -9999.0) -> None:
    with rasterio.open(
        path,
        "w",
        driver="GTiff",
        height=data.shape[0],
        width=data.shape[1],
        count=1,
        dtype=np.float64,
        crs="EPSG:4326",
        transform=from_origin(0.0, 0.0, 30.0, 30.0),
        nodata=nodata,
    ) as dst:
        dst.write(data.astype(np.float64), 1)


def _build_test_rasters(tmp_path: Path) -> Dict[str, str]:
    key_data = np.array(
        [
            [11, 11, 12, 12],
            [11, 11, 12, 12],
            [14, 14, 14, 14],
        ],
        dtype=np.int32,
    )
    key2_data = np.array(
        [
            [1, 1, 1, 2],
            [2, 2, 2, 2],
            [1, 1, 2, 2],
        ],
        dtype=np.int32,
    )
    param_i32_data = np.array(
        [
            [7, 7, 9, 9],
            [8, 8, 9, 10],
            [10, 11, 12, 12],
        ],
        dtype=np.int32,
    )
    param_f64_data = np.array(
        [
            [1.0, 3.0, 2.0, 4.0],
            [5.0, 7.0, 6.0, 8.0],
            [9.0, 11.0, 10.0, 12.0],
        ],
        dtype=np.float64,
    )

    key_fn = tmp_path / "key.tif"
    key2_fn = tmp_path / "key2.tif"
    param_i32_fn = tmp_path / "param_i32.tif"
    param_f64_fn = tmp_path / "param_f64.tif"

    _write_i32_raster(key_fn, key_data)
    _write_i32_raster(key2_fn, key2_data)
    _write_i32_raster(param_i32_fn, param_i32_data)
    _write_f64_raster(param_f64_fn, param_f64_data)

    return {
        "key_fn": str(key_fn),
        "key2_fn": str(key2_fn),
        "param_i32_fn": str(param_i32_fn),
        "param_f64_fn": str(param_f64_fn),
    }


def _assert_order_stable(
    result: Dict[str, int] | Dict[str, float] | Dict[str, Dict[str, int]] | Dict[str, Dict[str, float]],
    expected_outer_order: Tuple[str, ...],
    expected_inner_order: Dict[str, Tuple[str, ...]] | None = None,
) -> None:
    assert tuple(result.keys()) == expected_outer_order
    if expected_inner_order is None:
        return

    for outer_key, inner_order in expected_inner_order.items():
        inner_map = result[outer_key]
        assert isinstance(inner_map, dict)
        assert tuple(inner_map.keys()) == inner_order


def test_count_intersecting_raster_key_pairs_deterministic_order_and_values(tmp_path: Path) -> None:
    rasters = _build_test_rasters(tmp_path)

    expected = {
        "11": {"1": 2, "2": 2},
        "12": {"1": 1, "2": 3},
        "14": {"1": 2, "2": 2},
    }
    expected_outer_order = ("11", "12", "14")
    expected_inner_order = {"11": ("1", "2"), "12": ("1", "2"), "14": ("1", "2")}

    for _ in range(40):
        result = count_intersecting_raster_key_pairs(
            key_fn=rasters["key_fn"],
            key2_fn=rasters["key2_fn"],
            ignore_channels=False,
            ignore_keys=set(),
            ignore_keys2=set(),
        )
        assert result == expected
        _assert_order_stable(result, expected_outer_order, expected_inner_order)


def test_identify_mode_single_raster_key_deterministic_order_and_values(tmp_path: Path) -> None:
    rasters = _build_test_rasters(tmp_path)

    expected = {"11": 8, "12": 9, "14": 12}
    expected_outer_order = ("11", "12", "14")

    for _ in range(40):
        result = identify_mode_single_raster_key(
            key_fn=rasters["key_fn"],
            parameter_fn=rasters["param_i32_fn"],
            ignore_channels=False,
            ignore_keys=set(),
            band_indx=1,
        )
        assert result == expected
        _assert_order_stable(result, expected_outer_order)


def test_identify_mode_intersecting_raster_keys_deterministic_order_and_values(
    tmp_path: Path,
) -> None:
    rasters = _build_test_rasters(tmp_path)

    expected = {
        "11": {"1": 7, "2": 8},
        "12": {"1": 9, "2": 9},
        "14": {"1": 10, "2": 12},
    }
    expected_outer_order = ("11", "12", "14")
    expected_inner_order = {"11": ("1", "2"), "12": ("1", "2"), "14": ("1", "2")}

    for _ in range(40):
        result = identify_mode_intersecting_raster_keys(
            key_fn=rasters["key_fn"],
            key2_fn=rasters["key2_fn"],
            parameter_fn=rasters["param_i32_fn"],
            ignore_channels=False,
            ignore_keys=set(),
            ignore_keys2=set(),
            band_indx=1,
        )
        assert result == expected
        _assert_order_stable(result, expected_outer_order, expected_inner_order)


def test_identify_median_single_raster_key_deterministic_order_and_values(tmp_path: Path) -> None:
    rasters = _build_test_rasters(tmp_path)

    expected = {"11": 4.0, "12": 5.0, "14": 10.5}
    expected_outer_order = ("11", "12", "14")

    for _ in range(40):
        result = identify_median_single_raster_key(
            key_fn=rasters["key_fn"],
            parameter_fn=rasters["param_f64_fn"],
            ignore_channels=False,
            ignore_keys=set(),
            band_indx=1,
        )
        assert result == expected
        _assert_order_stable(result, expected_outer_order)


def test_identify_median_intersecting_raster_keys_deterministic_order_and_values(
    tmp_path: Path,
) -> None:
    rasters = _build_test_rasters(tmp_path)

    expected = {
        "11": {"1": 2.0, "2": 6.0},
        "12": {"1": 2.0, "2": 6.0},
        "14": {"1": 10.0, "2": 11.0},
    }
    expected_outer_order = ("11", "12", "14")
    expected_inner_order = {"11": ("1", "2"), "12": ("1", "2"), "14": ("1", "2")}

    for _ in range(40):
        result = identify_median_intersecting_raster_keys(
            key_fn=rasters["key_fn"],
            key2_fn=rasters["key2_fn"],
            parameter_fn=rasters["param_f64_fn"],
            ignore_channels=False,
            ignore_keys=set(),
            ignore_keys2=set(),
            band_indx=1,
        )
        assert result == expected
        _assert_order_stable(result, expected_outer_order, expected_inner_order)


@pytest.mark.parametrize(
    ("api_name", "call"),
    [
        (
            "identify_mode_single_raster_key",
            lambda: identify_mode_single_raster_key(
                key_fn="/tmp/missing_key.tif",
                parameter_fn="/tmp/missing_parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                band_indx=1,
            ),
        ),
        (
            "identify_mode_intersecting_raster_keys",
            lambda: identify_mode_intersecting_raster_keys(
                key_fn="/tmp/missing_key.tif",
                key2_fn="/tmp/missing_key2.tif",
                parameter_fn="/tmp/missing_parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                ignore_keys2=set(),
                band_indx=1,
            ),
        ),
        (
            "identify_median_single_raster_key",
            lambda: identify_median_single_raster_key(
                key_fn="/tmp/missing_key.tif",
                parameter_fn="/tmp/missing_parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                band_indx=1,
            ),
        ),
        (
            "identify_median_intersecting_raster_keys",
            lambda: identify_median_intersecting_raster_keys(
                key_fn="/tmp/missing_key.tif",
                key2_fn="/tmp/missing_key2.tif",
                parameter_fn="/tmp/missing_parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                ignore_keys2=set(),
                band_indx=1,
            ),
        ),
    ],
)
def test_identify_apis_missing_raster_paths_raise_existing_panic_contract(
    api_name: str,
    call,
) -> None:
    with pytest.raises(BaseException) as exc_info:
        call()

    assert exc_info.type.__name__ == "PanicException", api_name
    assert "called `Result::unwrap()` on an `Err` value" in str(exc_info.value), api_name


@pytest.mark.parametrize(
    ("api_name", "call"),
    [
        (
            "identify_mode_single_raster_key",
            lambda: identify_mode_single_raster_key(
                key_fn="/tmp/key.tif",
                parameter_fn="/tmp/parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                band_indx=0,
            ),
        ),
        (
            "identify_mode_intersecting_raster_keys",
            lambda: identify_mode_intersecting_raster_keys(
                key_fn="/tmp/key.tif",
                key2_fn="/tmp/key2.tif",
                parameter_fn="/tmp/parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                ignore_keys2=set(),
                band_indx=0,
            ),
        ),
        (
            "identify_median_single_raster_key",
            lambda: identify_median_single_raster_key(
                key_fn="/tmp/key.tif",
                parameter_fn="/tmp/parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                band_indx=0,
            ),
        ),
        (
            "identify_median_intersecting_raster_keys",
            lambda: identify_median_intersecting_raster_keys(
                key_fn="/tmp/key.tif",
                key2_fn="/tmp/key2.tif",
                parameter_fn="/tmp/parameter.tif",
                ignore_channels=False,
                ignore_keys=set(),
                ignore_keys2=set(),
                band_indx=0,
            ),
        ),
    ],
)
def test_identify_apis_invalid_band_index_raises_value_error_contract(
    api_name: str,
    call,
) -> None:
    with pytest.raises(ValueError, match="band_indx must be >= 1. Got 0 instead."):
        call()

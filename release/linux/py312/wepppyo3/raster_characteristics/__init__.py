from typing import Set, Dict, Optional

from .raster_characteristics_rust import (
    count_intersecting_raster_key_pairs as _count_intersecting_raster_key_pairs,
    identify_mode_intersecting_raster_keys as _identify_mode_intersecting_raster_keys,
    identify_mode_single_raster_key as _identify_mode_single_raster_key,
    identify_median_intersecting_raster_keys as _identify_median_intersecting_raster_keys,
    identify_median_single_raster_key as _identify_median_single_raster_key,
    local_mukey_candidates as _local_mukey_candidates,
    local_mukey_geometry as _local_mukey_geometry,
    categorical_support_within_bounds as _categorical_support_within_bounds,
    crop_categorical_raster_to_padded_reference as _crop_categorical_raster_to_padded_reference,
    categorical_support_within_wgs84_radius as _categorical_support_within_wgs84_radius,
    categorical_value_centroid_wgs84 as _categorical_value_centroid_wgs84,
    intersecting_categorical_value_centroids_wgs84 as _intersecting_categorical_value_centroids_wgs84,
    categorical_raster_metadata as _categorical_raster_metadata,
)


def _handle_common_args(ignore_keys: Optional[Set[int]], band_indx: int) -> Set[int]:
    if band_indx < 1:
        raise ValueError(f"band_indx must be >= 1. Got {band_indx} instead.")
    
    return set() if ignore_keys is None else ignore_keys


def identify_mode_single_raster_key(
    key_fn: str,
    parameter_fn: str,
    ignore_channels: bool = True,
    ignore_keys: Optional[Set[int]] = None,
    band_indx: int = 1
) -> Dict[str, int]:
    ignore_keys = _handle_common_args(ignore_keys, band_indx)

    return _identify_mode_single_raster_key(
        key_fn=key_fn, 
        parameter_fn=parameter_fn,
        ignore_channels=ignore_channels,
        ignore_keys=ignore_keys,
        band_indx=band_indx
    )
    
identify_mode_single_raster_key.__doc__ = _identify_mode_single_raster_key.__doc__


def count_intersecting_raster_key_pairs(
    key_fn: str,
    key2_fn: str,
    ignore_channels: bool = True,
    ignore_keys: Optional[Set[int]] = None,
    ignore_keys2: Optional[Set[int]] = None,
) -> Dict[str, Dict[str, int]]:
    ignore_keys = set() if ignore_keys is None else ignore_keys
    ignore_keys2 = set() if ignore_keys2 is None else ignore_keys2

    return _count_intersecting_raster_key_pairs(
        key_fn=key_fn,
        key2_fn=key2_fn,
        ignore_channels=ignore_channels,
        ignore_keys=ignore_keys,
        ignore_keys2=ignore_keys2,
    )


count_intersecting_raster_key_pairs.__doc__ = _count_intersecting_raster_key_pairs.__doc__


def local_mukey_candidates(
    raster_path: str,
    clusters,
    valid_mukeys: Set[int],
    initial_radius_m: float = 250.0,
    max_radius_m: float = 2000.0,
    min_candidates: int = 1,
    workers: Optional[int] = None,
):
    return _local_mukey_candidates(
        raster_path, clusters, set(valid_mukeys), initial_radius_m,
        max_radius_m, min_candidates, workers,
    )


def local_mukey_geometry(
    raster_path: str,
    sources,
    valid_mukeys: Set[int],
    initial_radius_m: float = 250.0,
    max_radius_m: float = 2000.0,
    min_candidates: int = 1,
    workers: Optional[int] = None,
):
    return _local_mukey_geometry(
        raster_path, sources, set(valid_mukeys), initial_radius_m,
        max_radius_m, min_candidates, workers,
    )


def categorical_support_within_bounds(
    raster_path: str, bounds, radius_m: float, excluded_values: Optional[Set[int]] = None, band_index: int = 1,
):
    return _categorical_support_within_bounds(raster_path, bounds, radius_m, set(excluded_values or ()), band_index)


def crop_categorical_raster_to_padded_reference(
    source_path: str,
    reference_path: str,
    destination_path: str,
    padding_m: float = 2000.0,
    band_index: int = 1,
):
    """Crop a categorical source raster to a padded reference extent."""
    return _crop_categorical_raster_to_padded_reference(
        source_path, reference_path, destination_path, padding_m, band_index,
    )


def categorical_support_within_wgs84_radius(
    raster_path: str,
    longitude_wgs84: float,
    latitude_wgs84: float,
    radius_m: float,
    excluded_values: Optional[Set[int]] = None,
    band_index: int = 1,
):
    return _categorical_support_within_wgs84_radius(
        raster_path, longitude_wgs84, latitude_wgs84, radius_m,
        set(excluded_values or ()), band_index,
    )


def categorical_value_centroid_wgs84(raster_path: str, value: int, band_index: int = 1):
    return _categorical_value_centroid_wgs84(raster_path, value, band_index)


def intersecting_categorical_value_centroids_wgs84(
    key_raster_path: str,
    categorical_raster_path: str,
    pairs,
    key_band_index: int = 1,
    categorical_band_index: int = 1,
):
    return _intersecting_categorical_value_centroids_wgs84(
        key_raster_path, categorical_raster_path, pairs, key_band_index, categorical_band_index,
    )


def categorical_raster_metadata(raster_path: str):
    return _categorical_raster_metadata(raster_path)


def identify_median_single_raster_key(
    key_fn: str,
    parameter_fn: str,
    ignore_channels: bool = True,
    ignore_keys: Optional[Set[int]] = None,
    band_indx: int = 1
) -> Dict[str, float]:
    ignore_keys = _handle_common_args(ignore_keys, band_indx)

    return _identify_median_single_raster_key(
        key_fn=key_fn, 
        parameter_fn=parameter_fn,
        ignore_channels=ignore_channels,
        ignore_keys=ignore_keys,
        band_indx=band_indx
    )
    
identify_median_single_raster_key.__doc__ = _identify_median_single_raster_key.__doc__


def identify_mode_intersecting_raster_keys(
    key_fn: str,
    key2_fn: str,
    parameter_fn: str,
    ignore_channels: bool = True,
    ignore_keys: Optional[Set[int]] = None,
    ignore_keys2: Optional[Set[int]] = None,
    band_indx: int = 1
) -> Dict[str, Dict[str, int]]:
    ignore_keys = _handle_common_args(ignore_keys, band_indx)

    ignore_keys2 = set() if ignore_keys2 is None else ignore_keys2

    return _identify_mode_intersecting_raster_keys(
        key_fn=key_fn, 
        key2_fn=key2_fn, 
        parameter_fn=parameter_fn,
        ignore_channels=ignore_channels,
        ignore_keys=ignore_keys,
        ignore_keys2=ignore_keys2,
        band_indx=band_indx
    )
    
identify_mode_intersecting_raster_keys.__doc__ = _identify_mode_intersecting_raster_keys.__doc__


def identify_median_intersecting_raster_keys(
    key_fn: str,
    key2_fn: str,
    parameter_fn: str,
    ignore_channels: bool = True,
    ignore_keys: Optional[Set[int]] = None,
    ignore_keys2: Optional[Set[int]] = None,
    band_indx: int = 1
) -> Dict[str, Dict[str, float]]:
    ignore_keys = _handle_common_args(ignore_keys, band_indx)

    ignore_keys2 = set() if ignore_keys2 is None else ignore_keys2

    return _identify_median_intersecting_raster_keys(
        key_fn=key_fn, 
        key2_fn=key2_fn, 
        parameter_fn=parameter_fn,
        ignore_channels=ignore_channels,
        ignore_keys=ignore_keys,
        ignore_keys2=ignore_keys2,
        band_indx=band_indx
    )
    
identify_median_intersecting_raster_keys.__doc__ = _identify_median_intersecting_raster_keys.__doc__

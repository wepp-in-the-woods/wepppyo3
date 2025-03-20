from typing import Set, Dict, Optional

from .raster_characteristics_rust import (
    identify_mode_intersecting_raster_keys as _identify_mode_intersecting_raster_keys,
    identify_mode_single_raster_key as _identify_mode_single_raster_key,
    identify_median_intersecting_raster_keys as _identify_median_intersecting_raster_keys,
    identify_median_single_raster_key as _identify_median_single_raster_key,
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
) -> Dict[str, int]:
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
) -> Dict[str, float]:
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


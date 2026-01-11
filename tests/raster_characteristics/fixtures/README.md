# Test Fixtures for `identify_mode_single_raster_key`

## Source

Cropped from `/wc1/culverts/55b28bb9-2d61-43f3-9f45-10779e93c501/runs/7/`:
- `subwta.tif` - WhiteboxTools subcatchment raster
- `nlcd.vrt` - NLCD landcover raster (cropped VRT from batch)

## Fixture Files

### `subwta_nodata_edge.tif`
Subcatchment/hillslope key raster.

- **Shape**: 500 rows x 200 cols
- **CRS**: EPSG:32617 (UTM zone 17N)
- **Nodata**: -inf
- **Hillslope IDs**: 1312, 1323, 1442, 1443
- **Channel IDs**: 1444

### `nlcd_nodata_edge.tif`
NLCD landcover parameter raster with partial nodata coverage.

- **Shape**: 500 rows x 200 cols
- **CRS**: EPSG:32617 (UTM zone 17N)
- **Nodata**: 250
- **Valid pixels**: 32,000 (32%)
- **Nodata pixels**: 68,000 (68%)

## Test Scenario

This fixture tests edge cases where the landcover raster has nodata values
overlapping with valid hillslope regions. The nodata occurs because the
NLCD was retrieved via wmesque2 using a WGS84 bounding box, which when
transformed back to UTM results in a smaller extent than the DEM.

### Expected Behavior

When calling `identify_mode_single_raster_key(key_fn, parameter_fn, ignore_channels=True)`:

| Hillslope ID | Expected Result | Notes |
|--------------|-----------------|-------|
| 1312 | Valid NLCD class (e.g., 41, 42, 43) | Entirely within NLCD valid coverage |
| 1323 | Valid NLCD class | Entirely within NLCD valid coverage |
| 1442 | **Must be returned** | Partial or full nodata - requires fallback/default |
| 1443 | **Must be returned** | Partial or full nodata - requires fallback/default |

### Bug Being Fixed

Previously, `identify_mode_single_raster_key` would silently skip hillslopes
where all overlapping pixels in the parameter raster were nodata. This caused
`KeyError` exceptions downstream in `wepp.run_hillslopes()` when trying to
look up landcover for hillslopes not present in `domlc_d`.

The fix ensures all key values from the key raster are returned, even if
the parameter raster has 100% nodata for that key. In such cases, a sentinel
value (e.g., nodata value or configurable default) should be returned.

## Reproduction

```python
from wepppyo3.raster_characteristics import identify_mode_single_raster_key

result = identify_mode_single_raster_key(
    key_fn='subwta_nodata_edge.tif',
    parameter_fn='nlcd_nodata_edge.tif',
    ignore_channels=True,
    ignore_keys=set()
)

# All hillslopes must be present in result
assert '1312' in result or 1312 in result
assert '1323' in result or 1323 in result
assert '1442' in result or 1442 in result  # This was missing before fix
assert '1443' in result or 1443 in result  # This was missing before fix
```

## Related Files

- `wepppy/nodb/core/landuse.py:_build_NLCD()` - Caller that builds `domlc_d`
- `wepppy/nodb/core/wepp.py:run_hillslopes()` - Consumer that fails with KeyError
- `wepppy/rq/culvert_rq.py:_process_culvert_run()` - Culvert batch processing pipeline

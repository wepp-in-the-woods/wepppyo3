# Area-weighted single-key raster mean

Accepted 2026-09-09. Domain-neutral API in `wepppyo3.raster_characteristics`:

```python
identify_area_weighted_mean_single_raster_key(
    key_fn, parameter_fn, ignore_channels=True, ignore_keys=None,
    band_indx=1, default_value=None,
)
```

Read aligned projected rasters; validate dimensions, data lengths, equivalent
CRS and all six affine coefficients. Reject missing/unusable/geographic CRS,
nonfinite or singular transforms, invalid bands and nonfinite defaults with
ValueError. Read failures raise OSError. No panics or truncating mismatched scans.
Keys are integral signed 32-bit identifiers; key masks/nodata and explicit
ignore_keys are excluded. Channel exclusion uses key modulo 10 equal to 4.

Parameter masks, declared nodata (including NaN), and nonfinite values are
missing. Other finite values, including zero and negatives, are valid. Optional
finite default_value contributes only for missing cells to the entire key area.
Without a default, any missing coverage raises a bounded ValueError describing
affected keys and counts. All-missing keys with a default return that default;
an empty eligible grid returns an empty dict.

Projected affine cells have constant area, so cell-count weights give the area
mean. Accumulate stably in float64 and reject nonfinite results. Return a
deterministic string-keyed dict of dict records containing mean (float),
valid_cell_count, missing_cell_count and total_cell_count (integers).
No conductivity positivity or background-key policy belongs in this API.

Validation must exercise real release-tree exports with masks, finite/NaN
nodata, uncovered/default area, negative/zero generic values, exclusions,
invalid alignment/bands/CRS and extreme finite values. WEPPpy owns stacking,
conductivity normalization, default policy and atomic run artifact publication.

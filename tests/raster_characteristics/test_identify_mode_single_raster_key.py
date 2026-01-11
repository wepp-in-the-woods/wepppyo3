import tempfile
import unittest
from pathlib import Path

import numpy as np
import rasterio

from wepppyo3.raster_characteristics import identify_mode_single_raster_key


class TestIdentifyModeSingleRasterKey(unittest.TestCase):
    def setUp(self):
        fixture_dir = Path(__file__).resolve().parent / "fixtures"
        self.key_fn = fixture_dir / "subwta_nodata_edge.tif"
        self.param_fn = fixture_dir / "nlcd_nodata_edge.tif"

    def _global_mode(self, data: np.ndarray, nodata):
        if nodata is None:
            valid = data.ravel()
        else:
            valid = data[data != nodata]
        self.assertGreater(valid.size, 0, "Expected at least one valid parameter value.")
        vals, counts = np.unique(valid, return_counts=True)
        return int(vals[counts.argmax()])

    def test_fallback_for_all_nodata_key(self):
        with rasterio.open(self.key_fn) as key_src:
            key_data = key_src.read(1)

        with rasterio.open(self.param_fn) as param_src:
            param_data = param_src.read(1)
            meta = param_src.meta.copy()
            nodata = param_src.nodata

        if nodata is None:
            self.skipTest("Parameter raster has no nodata value set.")

        target_key = 1442
        if not np.any(key_data == target_key):
            self.skipTest(f"Expected key {target_key} not present in key raster.")

        modified_data = param_data.copy()
        modified_data[key_data == target_key] = nodata
        global_mode = self._global_mode(modified_data, nodata)

        with tempfile.TemporaryDirectory() as tmpdir:
            param_tmp = Path(tmpdir) / "nlcd_key1442_all_nodata.tif"
            with rasterio.open(param_tmp, "w", **meta) as dst:
                dst.write(modified_data, 1)

            result = identify_mode_single_raster_key(
                key_fn=str(self.key_fn),
                parameter_fn=str(param_tmp),
                ignore_channels=True,
                ignore_keys=set(),
                band_indx=1,
            )

        self.assertIn("1442", result)
        self.assertEqual(result["1442"], global_mode)
        self.assertIn("1312", result)
        self.assertIn("1323", result)
        self.assertIn("1443", result)


if __name__ == "__main__":
    unittest.main()

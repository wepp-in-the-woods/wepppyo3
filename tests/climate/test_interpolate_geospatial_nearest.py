import unittest
import numpy as np
from wepppyo3.climate import interpolate_geospatial

class TestInterpolateGeospatialNearest(unittest.TestCase):
    def setUp(self):
        # Define the 1D grid arrays for eastings and northings.
        self.eastings = np.array([0.0, 1.0])
        self.northings = np.array([0.0, 1.0])

    def test_nearest_lower_corner(self):
        # Data shape: (2, 2, 1)
        # Grid:
        #  (0,0)=1.0, (0,1)=2.0, (1,0)=3.0, (1,1)=4.0
        # Target (0.2, 0.2) is closest to grid point (0,0).
        data = np.array([[[1.0], [2.0]],
                         [[3.0], [4.0]]])
        result = interpolate_geospatial(0.2, 0.2, self.eastings, self.northings, data, "nearest")
        self.assertAlmostEqual(result[0], 1.0, places=5)

    def test_nearest_upper_corner(self):
        # Target (0.8, 0.8) is closest to grid point (1,1).
        data = np.array([[[1.0], [2.0]],
                         [[3.0], [4.0]]])
        result = interpolate_geospatial(0.8, 0.8, self.eastings, self.northings, data, "nearest")
        self.assertAlmostEqual(result[0], 4.0, places=5)

    def test_nearest_exact_match(self):
        # When the target is exactly on a grid point, that value is returned.
        # Target (1.0, 0.0) matches grid point (1,0) with value 3.0.
        data = np.array([[[1.0], [2.0]],
                         [[3.0], [4.0]]])
        result = interpolate_geospatial(1.0, 0.0, self.eastings, self.northings, data, "nearest")
        self.assertAlmostEqual(result[0], 3.0, places=5)

    def test_nearest_multiple_dates(self):
        # Data shape: (2, 2, 2)
        # Date 0 grid: [[1.0, 2.0], [3.0, 4.0]]
        # Date 1 grid: [[5.0, 6.0], [7.0, 8.0]]
        data = np.array([
            [[1.0, 5.0], [2.0, 6.0]],
            [[3.0, 7.0], [4.0, 8.0]]
        ])
        # For target (0.2, 0.2), nearest grid point is (0,0).
        result = interpolate_geospatial(0.2, 0.2, self.eastings, self.northings, data, "nearest")
        self.assertAlmostEqual(result[0], 1.0, places=5)
        self.assertAlmostEqual(result[1], 5.0, places=5)

        # For target (0.8, 0.8), nearest grid point is (1,1).
        result = interpolate_geospatial(0.8, 0.8, self.eastings, self.northings, data, "nearest")
        self.assertAlmostEqual(result[0], 4.0, places=5)
        self.assertAlmostEqual(result[1], 8.0, places=5)

if __name__ == "__main__":
    unittest.main()


import unittest
import numpy as np
from wepppyo3.climate import interpolate_geospatial

class TestInterpolateGeospatialCubic(unittest.TestCase):
    def setUp(self):
        # Create a grid with 4 points along each axis.
        self.eastings = np.linspace(0, 3, 4)   # [0, 1, 2, 3]
        self.northings = np.linspace(0, 3, 4)   # [0, 1, 2, 3]

    def generate_data_single_date(self, func):
        """Generate a 3D data array (nx, ny, 1) where each grid value is given by func(x, y)."""
        nx = self.eastings.size
        ny = self.northings.size
        data = np.empty((nx, ny, 1))
        for i, x in enumerate(self.eastings):
            for j, y in enumerate(self.northings):
                data[i, j, 0] = func(x, y)
        return data

    def generate_data_multiple_dates(self, func0, func1):
        """Generate a 3D data array (nx, ny, 2) for two dates."""
        nx = self.eastings.size
        ny = self.northings.size
        data = np.empty((nx, ny, 2))
        for i, x in enumerate(self.eastings):
            for j, y in enumerate(self.northings):
                data[i, j, 0] = func0(x, y)
                data[i, j, 1] = func1(x, y)
        return data

    def test_cubic_single_date(self):
        # Define a cubic function: f(x,y)= x^3 + y^3.
        func = lambda x, y: x**3 + y**3
        data = self.generate_data_single_date(func)
        target_easting = 1.5
        target_northing = 1.5
        result = interpolate_geospatial(target_easting, target_northing,
                                        self.eastings, self.northings, data, "cubic")
        expected = 1.5**3 + 1.5**3  # 3.375 + 3.375 = 6.75
        self.assertAlmostEqual(result[0], expected, places=5)

    def test_cubic_multiple_dates(self):
        # Date 0: f(x,y)= x^3 + y^3.
        # Date 1: g(x,y)= (x+1)^3 + (y+1)^3.
        func0 = lambda x, y: x**3 + y**3
        func1 = lambda x, y: (x+1)**3 + (y+1)**3
        data = self.generate_data_multiple_dates(func0, func1)
        target_easting = 1.5
        target_northing = 1.5
        result = interpolate_geospatial(target_easting, target_northing,
                                        self.eastings, self.northings, data, "cubic")
        expected0 = 1.5**3 + 1.5**3           # 6.75
        expected1 = (1.5+1)**3 + (1.5+1)**3     # 2.5^3 + 2.5^3 = 15.625 + 15.625 = 31.25
        self.assertAlmostEqual(result[0], expected0, places=5)
        self.assertAlmostEqual(result[1], expected1, places=5)

    def test_cubic_a_min_clipping(self):
        # With f(x,y)= x^3 + y^3, the target (1.5,1.5) yields 6.75.
        # Setting a_min=7.0 should clip the result to 7.0.
        func = lambda x, y: x**3 + y**3
        data = self.generate_data_single_date(func)
        target_easting = 1.5
        target_northing = 1.5
        result = interpolate_geospatial(target_easting, target_northing,
                                        self.eastings, self.northings, data, "cubic", a_min=7.0)
        self.assertAlmostEqual(result[0], 7.0, places=5)

    def test_cubic_a_max_clipping(self):
        # With f(x,y)= x^3 + y^3, the target (1.5,1.5) yields 6.75.
        # Setting a_max=6.0 should clip the result to 6.0.
        func = lambda x, y: x**3 + y**3
        data = self.generate_data_single_date(func)
        target_easting = 1.5
        target_northing = 1.5
        result = interpolate_geospatial(target_easting, target_northing,
                                        self.eastings, self.northings, data, "cubic", a_max=6.0)
        self.assertAlmostEqual(result[0], 6.0, places=5)

if __name__ == "__main__":
    unittest.main()


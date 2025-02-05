import unittest
import numpy as np
from wepppyo3.climate import interpolate_geospatial

class TestInterpolateGeospatial(unittest.TestCase):
    def setUp(self):
        # Define a simple grid.
        self.eastings = np.array([0.0, 1.0])
        self.northings = np.array([0.0, 1.0])

    def test_bilinear_single_date(self):
        # Data shape: (2, 2, 1)
        # Grid values:
        #   (0,0)=1.0, (0,1)=3.0, (1,0)=2.0, (1,1)=4.0
        # Bilinear interpolation at (0.5, 0.5) should yield 2.5.
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])
        result = interpolate_geospatial(0.5, 0.5, self.eastings, self.northings, data, "linear")
        self.assertAlmostEqual(result[0], 2.5, places=5)

    def test_bilinear_multiple_dates(self):
        # Data shape: (2, 2, 2)
        # Date 0: same as above.
        # Date 1: values incremented by 1 so that the expected value becomes 3.5.
        data = np.array([
            [[1.0, 2.0], [3.0, 4.0]],
            [[2.0, 3.0], [4.0, 5.0]]
        ])
        result = interpolate_geospatial(0.5, 0.5, self.eastings, self.northings, data, "linear")
        self.assertAlmostEqual(result[0], 2.5, places=5)
        self.assertAlmostEqual(result[1], 3.5, places=5)

    def test_target_outside_domain(self):
        # Target easting is outside the [0, 1] grid.
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])
        with self.assertRaises(ValueError) as cm:
            interpolate_geospatial(-0.1, 0.5, self.eastings, self.northings, data, "linear")
        self.assertIn("Target easting/northing is outside the grid domain", str(cm.exception))

    def test_invalid_data_dimensions(self):
        # Data array is 2D rather than 3D.
        data = np.array([[1.0, 3.0],
                         [2.0, 4.0]])
        with self.assertRaises(TypeError) as cm:
            interpolate_geospatial(0.5, 0.5, self.eastings, self.northings, data, "linear")

    def test_a_min_clipping(self):
        # Without clipping, interpolation at (0.5, 0.5) yields 2.5.
        # With a_min set higher than 2.5, the result should be clipped.
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])
        result = interpolate_geospatial(0.5, 0.5, self.eastings, self.northings, data, "linear", a_min=3.0)
        self.assertAlmostEqual(result[0], 3.0, places=5)

    def test_a_max_clipping(self):
        # With a_max set lower than 2.5, the result should be clipped.
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])
        result = interpolate_geospatial(0.5, 0.5, self.eastings, self.northings, data, "linear", a_max=2.0)
        self.assertAlmostEqual(result[0], 2.0, places=5)
        
    def test_bilinear_reversed_eastings(self):
        """
        Same 2x2 single-date data, but eastings are reversed: [1.0, 0.0].
        We still expect the interpolated value at (0.5, 0.5) to be 2.5.
        """
        reversed_eastings = np.array([1.0, 0.0])
        northings = np.array([0.0, 1.0])
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])  # shape (2,2,1)
        result = interpolate_geospatial(0.5, 0.5, reversed_eastings, northings, data, "linear")
        self.assertAlmostEqual(result[0], 2.5, places=5)

    def test_bilinear_reversed_northings(self):
        """
        Same data, but northings are reversed: [1.0, 0.0].
        We still expect 2.5 at (0.5, 0.5).
        """
        eastings = np.array([0.0, 1.0])
        reversed_northings = np.array([1.0, 0.0])
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])  # shape (2,2,1)
        result = interpolate_geospatial(0.5, 0.5, eastings, reversed_northings, data, "linear")
        self.assertAlmostEqual(result[0], 2.5, places=5)

    def test_bilinear_reversed_both_axes(self):
        """
        Reverse both eastings and northings and verify interpolation is still correct.
        """
        reversed_eastings = np.array([1.0, 0.0])
        reversed_northings = np.array([1.0, 0.0])
        data = np.array([[[1.0], [3.0]],
                         [[2.0], [4.0]]])  # shape (2,2,1)
        result = interpolate_geospatial(0.5, 0.5, reversed_eastings, reversed_northings, data, "linear")
        self.assertAlmostEqual(result[0], 2.5, places=5)

if __name__ == "__main__":
    unittest.main()


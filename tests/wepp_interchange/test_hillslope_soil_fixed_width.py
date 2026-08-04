import tempfile
import unittest
from pathlib import Path

from wepppyo3.wepp_interchange import hillslope_soil_to_columns


class TestHillslopeSoilFixedWidth(unittest.TestCase):
    def _write_soil_file(self, tmpdir: str, name: str, header_tokens: list[str], units_tokens: list[str], rows: list[str]) -> Path:
        path = Path(tmpdir) / name
        lines: list[str] = []
        lines.append(" ".join(header_tokens))
        lines.append(" ".join(units_tokens))
        lines.append("-" * 60)
        lines.extend(rows)
        path.write_text("\n".join(lines) + "\n")
        return path

    def test_recovers_when_fixed_width_values_run_together_modern_header(self):
        header = [
            "OFE",
            "Day",
            "Y",
            "Poros",
            "Keff",
            "Suct",
            "FC",
            "WP",
            "Rough",
            "Ki",
            "Kr",
            "Tauc",
            "Saturation",
            "TSW",
        ]
        units = [
            "%",
            "mm/hr",
            "mm",
            "mm/mm",
            "mm/mm",
            "mm",
            "adjsmt",
            "adjsmt",
            "adjsmt",
            "frac",
            "mm",
        ]
        # Real failing row captured from wepp1 via run_metadata.json error payload.
        row = "  1  166     28   37.761218.58   8.25   0.20   0.11  20.00   0.03   0.03   1.11    0.92   34.73"

        with tempfile.TemporaryDirectory() as tmpdir:
            soil_path = self._write_soil_file(tmpdir, "H265.soil.dat", header, units, [row])

            columns = hillslope_soil_to_columns(str(soil_path), 0, 0)

        self.assertEqual(columns["wepp_id"], [265])
        self.assertEqual(columns["ofe_id"], [1])
        self.assertEqual(columns["OFE"], [1])

        self.assertAlmostEqual(columns["Poros"][0], 37.76, places=2)
        self.assertAlmostEqual(columns["Keff"][0], 1218.58, places=2)
        self.assertAlmostEqual(columns["Tauc"][0], 1.11, places=2)
        self.assertAlmostEqual(columns["Saturation"][0], 0.92, places=2)
        self.assertAlmostEqual(columns["TSW"][0], 34.73, places=2)

        # Column always exists; missing in file layouts should be populated as null.
        self.assertIn("TSMF", columns)
        self.assertIsNone(columns["TSMF"][0])

    def test_parses_tsmf_header(self):
        header = [
            "OFE",
            "Day",
            "Y",
            "Poros",
            "Keff",
            "Suct",
            "FC",
            "WP",
            "Rough",
            "Ki",
            "Kr",
            "Tauc",
            "Saturation",
            "TSW",
            "TSMF",
        ]
        units = [
            "%",
            "mm/hr",
            "mm",
            "mm/mm",
            "mm/mm",
            "mm",
            "adjsmt",
            "adjsmt",
            "adjsmt",
            "frac",
            "mm",
            "frac",
        ]
        row = (
            "  1  166     28   37.761218.58   8.25   0.20   0.11  20.00   0.03   0.03   1.11    0.92   34.73"
            + " "
            + " 0.1234"
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            soil_path = self._write_soil_file(tmpdir, "H265.soil.dat", header, units, [row])

            columns = hillslope_soil_to_columns(str(soil_path), 0, 0)

        self.assertIn("TSMF", columns)
        self.assertAlmostEqual(columns["TSMF"][0], 0.1234, places=4)

    def test_recovers_widened_ofe_with_touching_values_and_tsmf(self):
        header = [
            "OFE",
            "Day",
            "Y",
            "Poros",
            "Keff",
            "Suct",
            "FC",
            "WP",
            "Rough",
            "Ki",
            "Kr",
            "Tauc",
            "Saturation",
            "TSW",
            "TSMF",
        ]
        units = [
            "%",
            "mm/hr",
            "mm",
            "mm/mm",
            "mm/mm",
            "mm",
            "adjsmt",
            "adjsmt",
            "adjsmt",
            "frac",
            "mm",
            "frac",
        ]
        # Exact failure from manual-rustle under the widened i5 OFE contract
        # used by wepp_260727 and wepp_260803.
        row = "     1  244     14   47.831119.01   7.99   0.14   0.07  20.00   0.03   0.27   1.48    0.99   47.13  0.8597"

        with tempfile.TemporaryDirectory() as tmpdir:
            soil_path = self._write_soil_file(tmpdir, "H111.soil.dat", header, units, [row])

            columns = hillslope_soil_to_columns(str(soil_path), 0, 0)

        self.assertEqual(columns["wepp_id"], [111])
        self.assertEqual(columns["OFE"], [1])
        self.assertEqual(columns["julian"], [244])
        self.assertEqual(columns["year"], [14])
        self.assertAlmostEqual(columns["Poros"][0], 47.83, places=2)
        self.assertAlmostEqual(columns["Keff"][0], 1119.01, places=2)
        self.assertAlmostEqual(columns["TSMF"][0], 0.8597, places=4)


if __name__ == "__main__":
    unittest.main()

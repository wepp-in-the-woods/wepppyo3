import tempfile
import unittest
from pathlib import Path

from wepppyo3.wepp_interchange import hillslope_element_to_columns


HEADER = (
    " OFE DD MM YYYY  Precip   Runoff   EffInt PeakRO  EffDur Enrich    Keff   Sm  LeafArea"
    "  CanHgt  Cancov IntCov  RilCov  LivBio DeadBio  Ki    Kr     Tcrit RilWid   SedLeave"
)
UNITS = (
    " na  na na  na     mm       mm     mm/h    mm/h      h    Ratio    mm/h   mm    Index"
    "    m       %       %       %     Kg/m^2  Kg/m^2  na    na      na     m       kg/m"
)
BASE_ROW = (
    "  1  1  1 2000    0.000    0.000   0.000   0.000   0.000 0.000  40.000 566.696"
    "  13.990 15.448   99.900   99.900   99.900    0.493  1.382  0.041  0.007  3.000"
    "  0.150    0.000"
)


def _write_element_file(tmpdir: str, name: str, rows: list[str]) -> Path:
    path = Path(tmpdir) / name
    path.write_text("\n".join([HEADER, UNITS, *rows]) + "\n")
    return path


class TestHillslopeElementQRainQSnow(unittest.TestCase):
    def test_legacy_element_rows_emit_null_qrain_qsnow(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            element_path = _write_element_file(tmpdir, "H12.element.dat", [BASE_ROW])
            columns = hillslope_element_to_columns(str(element_path), 0, 0)

        self.assertEqual(columns["wepp_id"], [12])
        self.assertEqual(columns["OFE"], [1])
        self.assertEqual(columns["year"], [2000])
        self.assertAlmostEqual(columns["SedLeave"][0], 0.0, places=6)
        self.assertEqual(columns["QRain"], [None])
        self.assertEqual(columns["QSnow"], [None])

    def test_extended_element_rows_parse_qrain_qsnow(self):
        extended_row = f"{BASE_ROW}{1.234:9.3f}{2.345:9.3f}"
        with tempfile.TemporaryDirectory() as tmpdir:
            element_path = _write_element_file(tmpdir, "H77.element.dat", [extended_row])
            columns = hillslope_element_to_columns(str(element_path), 0, 0)

        self.assertEqual(columns["wepp_id"], [77])
        self.assertEqual(columns["QRain"], [1.234])
        self.assertEqual(columns["QSnow"], [2.345])

    def test_mixed_rows_keep_legacy_and_extended_behavior(self):
        extended_row = f"{BASE_ROW}{3.210:9.3f}{0.450:9.3f}"
        with tempfile.TemporaryDirectory() as tmpdir:
            element_path = _write_element_file(tmpdir, "H99.element.dat", [BASE_ROW, extended_row])
            columns = hillslope_element_to_columns(str(element_path), 0, 0)

        self.assertEqual(columns["wepp_id"], [99, 99])
        self.assertEqual(columns["QRain"][0], None)
        self.assertEqual(columns["QSnow"][0], None)
        self.assertAlmostEqual(columns["QRain"][1], 3.210, places=6)
        self.assertAlmostEqual(columns["QSnow"][1], 0.450, places=6)


if __name__ == "__main__":
    unittest.main()

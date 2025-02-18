import unittest
import numpy as np
from wepppyo3.climate import (
    cli_revision, 
    interpolate_geospatial, 
    rust_cli_p_scale_monthlies, 
    rust_cli_p_scale, 
    calculate_monthlies, 
    rust_cli_calculate_p_annual_monthlies, 
    rust_cli_p_scale_annual_monthlies,
    calculate_p_annual_monthlies
)

from wepppy.climates.cligen import ClimateFile

class TestInterpolateGeospatialNearest(unittest.TestCase):
    def setUp(self):
        pass

    def test_calculate_p_annual_monthlies(self):
        src_fn = 'stochastic.cli'
        p_annual_monthlies = calculate_p_annual_monthlies(src_fn)
        
        cli = ClimateFile(src_fn)
        cli_df = cli.as_dataframe()
        p_annual_monthlies2 = calculate_p_annual_monthlies(months=cli_df['mo'], ppts=cli_df['prcp'])
        
        self.assertTrue(np.allclose(p_annual_monthlies, p_annual_monthlies2, rtol=1e-3))

    def test_scalar(self):
        src_fn = 'stochastic.cli'
        p_scale = 1.0
        
        out_fn = 'out/scaled.cli'
        rust_cli_p_scale(src_fn, out_fn, p_scale)
        
        og_monthlies = calculate_monthlies(src_fn)
        out_monthlies = calculate_monthlies(out_fn)
        
        for parameter in og_monthlies:
            self.assertTrue(np.allclose(og_monthlies[parameter], out_monthlies[parameter], rtol=1e-3))
        
    def test_monthlies_scale(self):
        src_fn = 'stochastic.cli'
        p_scale = [2.0, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 3.0, 3.1]
        
        out_fn = 'out/scaled2.cli'
        rust_cli_p_scale_monthlies(src_fn, out_fn, p_scale)
        
        og_monthlies = calculate_monthlies(src_fn)
        scaled_og_monthlies = []
        
        for i, p in enumerate(og_monthlies['ppts']):
            scaled_og_monthlies.append(p * p_scale[i])
            
        out_monthlies = calculate_monthlies(out_fn)
        
        self.assertTrue(np.allclose(scaled_og_monthlies, out_monthlies['ppts'], rtol=1e-3))
        
    def test_annual_monthlies_scale(self):
        src_fn = 'stochastic.cli'
        
        src_fn = 'stochastic.cli'
        p_annual_monthlies = calculate_p_annual_monthlies(src_fn)
        
        cli = ClimateFile(src_fn)
        cli_df = cli.as_dataframe()
        p_annual_monthlies2 = calculate_p_annual_monthlies(months=cli_df['mo'], ppts=cli_df['prcp'])
        p_scale = []
        
        for i, p in enumerate(p_annual_monthlies):
            if p_annual_monthlies2[i] == 0:
                p_scale.append(1.0)
            else:
                p_scale.append(p / p_annual_monthlies2[i])
            
        out_fn = 'out/scaled2.cli'
        rust_cli_p_scale_annual_monthlies(src_fn, out_fn, p_scale)
        
        og_monthlies = calculate_monthlies(src_fn)
        out_monthlies = calculate_monthlies(out_fn)
        
        self.assertTrue(np.allclose(og_monthlies['ppts'], out_monthlies['ppts'], rtol=1e-3))
        


if __name__ == "__main__":
    unittest.main()


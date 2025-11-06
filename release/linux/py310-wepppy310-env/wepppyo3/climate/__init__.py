import numpy as np

from .cli_revision_rust import (
    cli_revision, 
    interpolate_geospatial, 
    rust_cli_p_scale_monthlies, 
    rust_cli_p_scale, 
    rust_cli_calculate_monthlies, 
    rust_cli_calculate_p_annual_monthlies_from_lists,
    rust_cli_calculate_p_annual_monthlies, 
    rust_cli_p_scale_annual_monthlies,
    make_rhem_storm_file,
)

def calculate_p_annual_monthlies(src_fn=None, months=None, ppts=None) -> list:
    """Calculates annual precipitation statistics from a CLIGEN climate file or months and ppts array like

    Args:
        src_fn (str): Path to the CLIGEN climate file.
        months (list): A list of month numbers (1-12) to calculate annual precipitation statistics for.
        ppts (list): A list of monthly precipitation values in inches.

    Returns:
        np.ndarray: A numpy array of annual precipitation statistics in inches.
    """
    if src_fn is not None:
        return np.array(rust_cli_calculate_p_annual_monthlies(src_fn))
    else:
        months = np.asarray(months, dtype=np.int32)
        ppts = np.asarray(ppts, dtype=np.float64)
        return np.array(rust_cli_calculate_p_annual_monthlies_from_lists(months, ppts))


def calculate_monthlies(src_fn) -> dict:
    """Calculates monthly climate statistics from a CLIGEN climate file.

    Args:
        src_fn (str): Path to the CLIGEN climate file.

    Returns:
        dict: A dictionary containing monthly climate statistics with the following keys:
            - ppts: Monthly precipitation averages in inches
            - tmaxs: Monthly maximum temperature averages in Fahrenheit
            - tmins: Monthly minimum temperature averages in Fahrenheit
            - nwds: Monthly number of wet days
    """
    monthlies =  rust_cli_calculate_monthlies(src_fn)
    return dict(ppts=monthlies[0], tmaxs=monthlies[1], tmins=monthlies[2], nwds=monthlies[3])

__all__ = [
    'calculate_p_annual_monthlies',
    'calculate_monthlies',
    'make_rhem_storm_file',
]

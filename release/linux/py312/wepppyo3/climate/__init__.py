import numpy as np

from .cli_revision_rust import (
    build_hyetograph_breakpoint as rust_build_hyetograph_breakpoint,
    build_hyetograph_non_breakpoint as rust_build_hyetograph_non_breakpoint,
    cli_revision,
    compute_peak_intensities_breakpoint as rust_compute_peak_intensities_breakpoint,
    compute_peak_intensities_from_hyetograph as rust_compute_peak_intensities_from_hyetograph,
    compute_peak_intensities_non_breakpoint as rust_compute_peak_intensities_non_breakpoint,
    compute_static_r_from_cli as rust_compute_static_r_from_cli,
    interpolate_geospatial,
    make_rhem_storm_file,
    rust_cli_calculate_monthlies,
    rust_cli_calculate_p_annual_monthlies,
    rust_cli_calculate_p_annual_monthlies_from_lists,
    rust_cli_p_scale,
    rust_cli_p_scale_annual_monthlies,
    rust_cli_p_scale_monthlies,
)

_DEFAULT_WINDOWS = (10, 15, 30, 60)


def _normalize_windows(windows_minutes):
    if windows_minutes is None:
        return list(_DEFAULT_WINDOWS)
    return [int(window) for window in windows_minutes]


def _peak_intensity_dict(windows_minutes, values):
    keys = [f"peak_intensity_{int(window)}" for window in windows_minutes]
    return {key: float(value) for key, value in zip(keys, values)}


def calculate_p_annual_monthlies(src_fn=None, months=None, ppts=None) -> np.ndarray:
    """Calculate annual monthly precipitation means from CLI or month/depth arrays."""
    if src_fn is not None:
        return np.array(rust_cli_calculate_p_annual_monthlies(src_fn))

    months_arr = np.asarray(months, dtype=np.int32)
    ppts_arr = np.asarray(ppts, dtype=np.float64)
    return np.array(rust_cli_calculate_p_annual_monthlies_from_lists(months_arr, ppts_arr))


def calculate_monthlies(src_fn) -> dict:
    """Calculate monthly climate summaries from a CLIGEN CLI file."""
    monthlies = rust_cli_calculate_monthlies(src_fn)
    return dict(
        ppts=monthlies[0],
        tmaxs=monthlies[1],
        tmins=monthlies[2],
        nwds=monthlies[3],
    )


def build_hyetograph_non_breakpoint(
    *,
    prcp_mm,
    dur_hr,
    tp,
    ip,
    ip_correction=0.70,
    min_step_minutes=5.0,
) -> dict:
    """Build non-breakpoint WEPP hyetograph segments."""
    segments = rust_build_hyetograph_non_breakpoint(
        float(prcp_mm),
        float(dur_hr),
        float(tp),
        float(ip),
        float(ip_correction),
        float(min_step_minutes),
    )
    return {
        "source_mode": "non_breakpoint",
        "storm_depth_mm": float(max(prcp_mm, 0.0)),
        "storm_duration_hours": float(max(dur_hr, 0.0)),
        "tp": float(tp),
        "ip": float(ip),
        "segments": segments,
    }


def build_hyetograph_breakpoint(
    *,
    breakpoint_times_hr,
    breakpoint_cum_depth_mm,
) -> dict:
    """Build breakpoint WEPP hyetograph segments from cumulative rows."""
    times = [float(v) for v in breakpoint_times_hr]
    depths = [float(v) for v in breakpoint_cum_depth_mm]
    segments = rust_build_hyetograph_breakpoint(times, depths)
    return {
        "source_mode": "breakpoint",
        "storm_depth_mm": float(depths[-1]) if depths else 0.0,
        "storm_duration_hours": float(times[-1]) if times else 0.0,
        "tp": None,
        "ip": None,
        "segments": segments,
    }


def compute_peak_intensities_from_hyetograph(
    *,
    segments,
    storm_depth_mm,
    storm_duration_hours,
    windows_minutes=_DEFAULT_WINDOWS,
    time_step_minutes=5.0,
) -> dict:
    """Compute peak intensities from pre-built hyetograph segments."""
    windows = _normalize_windows(windows_minutes)
    values = rust_compute_peak_intensities_from_hyetograph(
        [(float(start), float(end), float(intensity)) for start, end, intensity in segments],
        float(storm_depth_mm),
        float(storm_duration_hours),
        windows,
        float(time_step_minutes),
    )
    return _peak_intensity_dict(windows, values)


def compute_peak_intensities_non_breakpoint(
    *,
    prcp_mm,
    dur_hr,
    tp,
    ip,
    windows_minutes=_DEFAULT_WINDOWS,
    ip_correction=0.70,
    time_step_minutes=5.0,
) -> dict:
    """Compute peak intensities for non-breakpoint storms."""
    windows = _normalize_windows(windows_minutes)
    values = rust_compute_peak_intensities_non_breakpoint(
        float(prcp_mm),
        float(dur_hr),
        float(tp),
        float(ip),
        windows,
        float(ip_correction),
        float(time_step_minutes),
    )
    return _peak_intensity_dict(windows, values)


def compute_peak_intensities_breakpoint(
    *,
    breakpoint_times_hr,
    breakpoint_cum_depth_mm,
    windows_minutes=_DEFAULT_WINDOWS,
    time_step_minutes=5.0,
) -> dict:
    """Compute peak intensities for breakpoint storms."""
    windows = _normalize_windows(windows_minutes)
    values = rust_compute_peak_intensities_breakpoint(
        [float(v) for v in breakpoint_times_hr],
        [float(v) for v in breakpoint_cum_depth_mm],
        windows,
        float(time_step_minutes),
    )
    return _peak_intensity_dict(windows, values)


def compute_static_r_from_cli(
    src_fn,
    *,
    ip_correction=0.70,
    time_step_minutes=5.0,
    storm_depth_threshold_mm=12.5,
) -> dict:
    """Compute static annual R erosivity metrics from a WEPP CLI file."""
    return rust_compute_static_r_from_cli(
        str(src_fn),
        float(ip_correction),
        float(time_step_minutes),
        float(storm_depth_threshold_mm),
    )


__all__ = [
    "build_hyetograph_breakpoint",
    "build_hyetograph_non_breakpoint",
    "calculate_monthlies",
    "calculate_p_annual_monthlies",
    "cli_revision",
    "compute_peak_intensities_breakpoint",
    "compute_peak_intensities_from_hyetograph",
    "compute_peak_intensities_non_breakpoint",
    "compute_static_r_from_cli",
    "interpolate_geospatial",
    "make_rhem_storm_file",
    "rust_cli_p_scale",
    "rust_cli_p_scale_annual_monthlies",
    "rust_cli_p_scale_monthlies",
]

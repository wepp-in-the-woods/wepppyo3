from __future__ import annotations

from pathlib import Path

import pytest

from wepppyo3.wepp_interchange import segment_single_ofe_slope


def _write_single_ofe_slope(
    path: Path,
    *,
    aspect: float = 311.995,
    width: float = 82.4,
    length: float = 100.0,
    points: list[tuple[float, float]],
) -> None:
    npts = len(points)
    row = " ".join(f"{d:.5f}, {s:.4f}" for d, s in points)
    path.write_text(
        "\n".join(["97.5", "1", f"{aspect} {width}", f"{npts} {length}", row]) + "\n",
        encoding="utf-8",
    )


def _parse_profiles(path: Path) -> list[list[tuple[float, float]]]:
    lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    n_ofes = int(lines[1])
    cursor = 3
    ofe_profiles: list[list[tuple[float, float]]] = []
    for _ in range(n_ofes):
        npts = int(lines[cursor].split()[0])
        cursor += 1
        row = lines[cursor].replace(",", "").split()
        cursor += 1
        assert len(row) == npts * 2
        profile = [(float(row[i * 2]), float(row[i * 2 + 1])) for i in range(npts)]
        ofe_profiles.append(profile)
    return ofe_profiles


def test_segment_single_ofe_slope_drops_duplicate_rounded_distances(tmp_path: Path) -> None:
    src = tmp_path / "hill_11.slp"
    dst = tmp_path / "hill_11.mofe.slp"
    _write_single_ofe_slope(
        src,
        points=[
            (0.0, 0.9),
            (0.25, 0.8),
            (0.25001, 0.1),
            (0.5, 0.6),
            (0.75, 0.5),
            (1.0, 0.4),
        ],
    )

    n_mofes = segment_single_ofe_slope(
        str(src),
        dst_fn=str(dst),
        target_length=25.0,
        apply_buffer=False,
    )

    assert n_mofes == 4
    profiles = _parse_profiles(dst)
    assert profiles[1][0] == (0.0, 0.1)
    for profile in profiles:
        distances = [distance for distance, _ in profile]
        assert distances[0] == 0.0
        assert distances[-1] == 1.0
        assert all(curr > prev for prev, curr in zip(distances, distances[1:]))


def test_segment_single_ofe_slope_respects_max_ofes_and_default_output_path(tmp_path: Path) -> None:
    src = tmp_path / "hill_15.slp"
    _write_single_ofe_slope(
        src,
        length=1000.0,
        points=[
            (0.0, 0.9),
            (0.25, 0.8),
            (0.5, 0.6),
            (0.75, 0.5),
            (1.0, 0.4),
        ],
    )

    n_mofes = segment_single_ofe_slope(
        str(src),
        target_length=50.0,
        apply_buffer=False,
        max_ofes=3,
    )

    dst = tmp_path / "hill_15.mofe.slp"
    assert n_mofes == 3
    assert dst.exists()
    lines = [line.strip() for line in dst.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert int(lines[1]) == 3


def test_segment_single_ofe_slope_buffer_mode_keeps_monotonic_distances(tmp_path: Path) -> None:
    src = tmp_path / "hill_12.slp"
    dst = tmp_path / "hill_12.mofe.slp"
    _write_single_ofe_slope(
        src,
        points=[
            (0.0, 0.9),
            (0.15, 0.8),
            (0.15001, 0.2),
            (0.5, 0.6),
            (0.75, 0.5),
            (1.0, 0.4),
        ],
    )

    n_mofes = segment_single_ofe_slope(
        str(src),
        dst_fn=str(dst),
        target_length=25.0,
        apply_buffer=True,
        buffer_length=15.0,
    )

    profiles = _parse_profiles(dst)
    assert len(profiles) == n_mofes
    assert n_mofes >= 2
    for profile in profiles:
        distances = [distance for distance, _ in profile]
        assert distances[0] == pytest.approx(0.0)
        assert distances[-1] == pytest.approx(1.0)
        assert all(curr > prev for prev, curr in zip(distances, distances[1:]))

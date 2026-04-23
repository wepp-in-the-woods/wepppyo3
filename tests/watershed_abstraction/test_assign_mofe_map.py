from __future__ import annotations

import numpy as np
import pytest

from wepppyo3.watershed_abstraction import assign_mofe_map


def _compute_mofe_segment_cell_counts(d_fractions: np.ndarray, n_cells: int) -> np.ndarray:
    if n_cells <= 0:
        raise ValueError(f"n_cells must be positive; received {n_cells}")

    if len(d_fractions) < 2:
        raise ValueError(f"d_fractions must include at least two points; received {d_fractions}")

    n_ofe = len(d_fractions) - 1
    if n_cells < n_ofe:
        raise ValueError(f"cannot assign {n_ofe} OFE ids across only {n_cells} cells")

    segment_lengths = np.diff(np.asarray(d_fractions, dtype=np.float64))
    if np.any(segment_lengths < 0):
        raise ValueError(f"d_fractions must be non-decreasing; received {d_fractions}")

    raw_counts = segment_lengths * float(n_cells)
    counts = np.floor(raw_counts).astype(np.int32)
    remainders = raw_counts - counts

    counts = np.maximum(counts, 1)

    diff = int(n_cells - int(np.sum(counts)))
    if diff > 0:
        order = np.argsort(-remainders)
        idx = 0
        while diff > 0:
            counts[order[idx % len(order)]] += 1
            diff -= 1
            idx += 1
    elif diff < 0:
        order = np.argsort(remainders)
        for index in order:
            while diff < 0 and counts[index] > 1:
                counts[index] -= 1
                diff += 1
            if diff == 0:
                break

    if diff != 0:
        raise ValueError(
            f"unable to reconcile MOFE segment counts for {n_cells} cells: "
            f"counts={counts.tolist()}, d_fractions={d_fractions}"
        )

    return counts


def _assign_mofe_ids_by_discharge_rank(discha_vals: np.ndarray, d_fractions: np.ndarray) -> np.ndarray:
    flat_discha_vals = np.asarray(discha_vals).reshape(-1)
    n_cells = int(flat_discha_vals.size)

    counts = _compute_mofe_segment_cell_counts(np.asarray(d_fractions, dtype=np.float64), n_cells)
    order = np.argsort(flat_discha_vals, kind="stable")[::-1]

    labels = np.empty(n_cells, dtype=np.int32)
    start = 0
    for ofe_id, count in enumerate(counts, start=1):
        end = start + int(count)
        labels[order[start:end]] = ofe_id
        start = end

    if start != n_cells:
        raise ValueError(f"MOFE rank assignment mismatch: assigned={start}, n_cells={n_cells}")

    return labels


def _legacy_assign_mofe_map(
    subwta: np.ndarray,
    discha: np.ndarray,
    topaz_ids: list[int],
    d_fractions_by_topaz: dict[int, np.ndarray],
) -> np.ndarray:
    mofe_map = np.zeros(subwta.shape, np.int32)

    for topaz_id in topaz_ids:
        indices = np.where(subwta == int(topaz_id))
        if len(indices[0]) == 0:
            raise ValueError(f"No subwta cells found for topaz_id={topaz_id} while building MOFE map")

        _discha_vals = discha[indices]
        max_discha = np.max(_discha_vals)

        d_fractions = np.asarray(d_fractions_by_topaz[topaz_id], dtype=np.float64)
        n_ofe = len(d_fractions) - 1
        if n_ofe == 1:
            mofe_indices = np.where(subwta == int(topaz_id))
            mofe_map[mofe_indices] = 1
        else:
            j = 1
            for i in range(n_ofe):
                _max_pct = (1.0 - d_fractions[i]) * 100
                _min_pct = (1.0 - d_fractions[i + 1]) * 100
                _min = np.percentile(_discha_vals, _min_pct)
                _max = np.percentile(_discha_vals, _max_pct)

                mofe_indices = np.where(
                    (subwta == int(topaz_id))
                    & (mofe_map == 0)
                    & (discha >= _min)
                    & (discha <= _max)
                )
                if len(mofe_indices[0]) == 0:
                    available_indices = np.where((subwta == int(topaz_id)) & (mofe_map == 0))
                    candidate_indices = available_indices if len(available_indices[0]) > 0 else indices
                    candidate_discha_vals = discha[candidate_indices]
                    target_value = (1.0 - d_fractions[i]) * max_discha
                    diff = np.abs(target_value - candidate_discha_vals)
                    closest_index = np.argmin(diff)
                    mofe_indices = (
                        candidate_indices[0][closest_index],
                        candidate_indices[1][closest_index],
                    )

                mofe_map[mofe_indices] = j
                j += 1

        mofe_ids = set(mofe_map[indices])
        if 0 in mofe_ids:
            mofe_ids.remove(0)

        if len(mofe_ids) != n_ofe:
            repaired_labels = _assign_mofe_ids_by_discharge_rank(_discha_vals, d_fractions)
            mofe_map[indices] = repaired_labels
            mofe_ids = set(mofe_map[indices])
            mofe_ids.discard(0)

        if len(mofe_ids) != n_ofe:
            expected = set(range(1, n_ofe + 1))
            missing = sorted(expected.difference(mofe_ids))
            raise ValueError(
                f"Unable to assign contiguous MOFE ids for topaz_id={topaz_id}: "
                f"expected=1..{n_ofe} present={sorted(mofe_ids)} missing={missing} "
                f"cells={len(indices[0])}"
            )

    return mofe_map


def test_assign_mofe_map_repairs_non_contiguous_flat_discharge() -> None:
    subwta = np.array([[171, 171, 171, 171]], dtype=np.int32)
    discha = np.array([[5, 5, 5, 5]], dtype=np.int32)
    topaz_ids = [171]
    d_fractions = {171: [0.0, 0.34, 0.67, 1.0]}

    result = assign_mofe_map(subwta, discha, topaz_ids, d_fractions)

    assert result.shape == subwta.shape
    assert set(np.unique(result[subwta == 171]).tolist()) == {1, 2, 3}


def test_assign_mofe_map_matches_python_legacy_oracle() -> None:
    subwta = np.array(
        [
            [171, 171, 172, 172],
            [171, 171, 172, 172],
            [171, 171, 172, 172],
        ],
        dtype=np.int32,
    )
    discha = np.array(
        [
            [9, 8, 7, 6],
            [5, 4, 9, 3],
            [2, 1, 8, 1],
        ],
        dtype=np.int32,
    )

    topaz_ids = [171, 172]
    d_fractions = {
        171: np.array([0.0, 0.34, 0.67, 1.0], dtype=np.float64),
        172: np.array([0.0, 0.5, 1.0], dtype=np.float64),
    }

    expected = _legacy_assign_mofe_map(subwta, discha, topaz_ids, d_fractions)
    result = assign_mofe_map(subwta, discha, topaz_ids, d_fractions)

    assert np.array_equal(result, expected)


def test_assign_mofe_map_raises_for_missing_topaz_cells() -> None:
    subwta = np.array([[171, 171]], dtype=np.int32)
    discha = np.array([[1, 2]], dtype=np.int32)

    with pytest.raises(ValueError, match="No subwta cells found for topaz_id=172"):
        assign_mofe_map(
            subwta,
            discha,
            [172],
            {172: [0.0, 1.0]},
        )


def test_assign_mofe_map_raises_for_shape_mismatch() -> None:
    subwta = np.array([[171, 171]], dtype=np.int32)
    discha = np.array([[1], [2]], dtype=np.int32)

    with pytest.raises(ValueError, match="subwta/discha shape mismatch"):
        assign_mofe_map(
            subwta,
            discha,
            [171],
            {171: [0.0, 1.0]},
        )

"""Scratch characterization; no production module is imported or changed."""
from __future__ import annotations

import ast
import hashlib
import inspect
import json
import math
from pathlib import Path
import subprocess

import numpy as np
import pandas as pd
from pandas.core.sorting import nargsort


def less(a, b):
    return a < b or (math.isnan(b) and not math.isnan(a))


def ascending_scalar_argsort(values, depth_override=None):
    """Behavioral replica of scalar NumPy 1.26 numeric indirect introsort."""
    order = list(range(len(values)))
    heap_count = 0

    def heap(lo, hi):
        nonlocal heap_count
        heap_count += 1
        a = [None] + order[lo:hi + 1]
        n = len(a) - 1
        for root in range(n // 2, 0, -1):
            saved = a[root]
            i, j = root, root * 2
            while j <= n:
                if j < n and less(values[a[j]], values[a[j + 1]]):
                    j += 1
                if not less(values[saved], values[a[j]]):
                    break
                a[i], i, j = a[j], j, j * 2
            a[i] = saved
        while n > 1:
            saved = a[n]
            a[n] = a[1]
            n -= 1
            i, j = 1, 2
            while j <= n:
                if j < n and less(values[a[j]], values[a[j + 1]]):
                    j += 1
                if not less(values[saved], values[a[j]]):
                    break
                a[i], i, j = a[j], j, j * 2
            a[i] = saved
        order[lo:hi + 1] = a[1:]

    if len(order) < 2:
        return order, heap_count
    lo, hi = 0, len(order) - 1
    depth = 2 * (len(order).bit_length() - 1)
    if depth_override is not None:
        depth = depth_override
    stack = []
    while True:
        if depth < 0:
            heap(lo, hi)
        else:
            while hi - lo > 15:
                mid = lo + (hi - lo) // 2
                if less(values[order[mid]], values[order[lo]]):
                    order[mid], order[lo] = order[lo], order[mid]
                if less(values[order[hi]], values[order[mid]]):
                    order[hi], order[mid] = order[mid], order[hi]
                if less(values[order[mid]], values[order[lo]]):
                    order[mid], order[lo] = order[lo], order[mid]
                pivot = values[order[mid]]
                i, j = lo, hi - 1
                order[mid], order[j] = order[j], order[mid]
                while True:
                    i += 1
                    while less(values[order[i]], pivot):
                        i += 1
                    j -= 1
                    while less(pivot, values[order[j]]):
                        j -= 1
                    if i >= j:
                        break
                    order[i], order[j] = order[j], order[i]
                order[i], order[hi - 1] = order[hi - 1], order[i]
                depth -= 1
                if i - lo < hi - i:
                    stack.append((i + 1, hi, depth))
                    hi = i - 1
                else:
                    stack.append((lo, i - 1, depth))
                    lo = i + 1
            for i in range(lo + 1, hi + 1):
                saved = order[i]
                j = i
                while j > lo and less(values[saved], values[order[j - 1]]):
                    order[j] = order[j - 1]
                    j -= 1
                order[j] = saved
        if not stack:
            return order, heap_count
        lo, hi, depth = stack.pop()


def descending_positions(values):
    not_nan = [i for i, v in enumerate(values) if not math.isnan(v)]
    nan = [i for i, v in enumerate(values) if math.isnan(v)]
    reverse_ids = not_nan[::-1]
    sorted_positions, heap_count = ascending_scalar_argsort([values[i] for i in reverse_ids])
    return [reverse_ids[i] for i in sorted_positions][::-1] + nan, heap_count


def clean(value):
    if isinstance(value, dict):
        return {str(k): clean(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [clean(v) for v in value]
    if isinstance(value, (float, np.floating)) and not math.isfinite(value):
        return 'NaN' if math.isnan(value) else ('+Infinity' if value > 0 else '-Infinity')
    return value


def frozen_function(path, function_names, namespace):
    source = subprocess.check_output(['git', '-C', '/workdir/wepppy', 'show', 'abebd09239f398af7627924c4c000ff3229a01ee:' + path], text=True)
    module = ast.parse(source)
    functions = [node for node in module.body if isinstance(node, ast.FunctionDef) and node.name in function_names]
    exec(compile(ast.fix_missing_locations(ast.Module(body=[ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)] + functions, type_ignores=[])), path, 'exec'), namespace)
    return hashlib.sha256(source.encode()).hexdigest()


def main():
    # Include partition threshold, long equal plateaus, signed zeros, null placement,
    # unequal groups, and sequential sorts of the same records.
    cases = [(f'equal_{n}', [0.] * n) for n in [0, 1, 2, 15, 16, 17, 20, 31, 32, 33, 100]]
    cases += [
        ('mixed_20', [3., 0., 3., 2., 1., 0., 2., 3., 1., 0., 2., 3., 0., 1., 3., 2., 0., 1., 2., 3.]),
        ('nan_signed_zero_20', [float('nan'), 2., -0., 0., 2., 1., float('nan'), -0., 1., 2., 0., 1., 2., -0., 1., 2., 0., 1., float('nan'), 0.]),
        ('infinity_20', [float('inf'), float('-inf'), 0., 1., float('inf')] * 4),
        ('organ_pipe_101', list(map(float, list(range(51)) + list(range(49, -1, -1))))),
    ]
    results = []
    for name, values in cases:
        actual = nargsort(np.asarray(values), ascending=False).tolist()
        replica, heap_count = descending_positions(values)
        assert replica == actual, name
        results.append({'name': name, 'values': values, 'descending_positions': actual, 'replica_heap_fallbacks': heap_count})

    rng = np.random.RandomState(20260908)
    randomized_count = 0
    heap_fallbacks = 0
    for n in list(range(0, 70)) + [100, 127, 128, 129, 256, 1000, 10000]:
        for iteration in range(20):
            values = rng.randint(-7, 8, size=n).astype(float)
            if n and iteration % 4 == 0:
                values[rng.randint(0, n, size=max(1, n // 10))] = np.nan
            replica, heap_count = ascending_scalar_argsort(values)
            assert replica == values.argsort(kind='quicksort').tolist(), ('ascending', n, iteration)
            replica, heap_count_desc = descending_positions(values)
            assert replica == nargsort(values, ascending=False).tolist(), ('descending', n, iteration)
            forced_heap, _ = ascending_scalar_argsort(values, depth_override=-1)
            assert forced_heap == values.argsort(kind='heapsort').tolist(), ('heapsort', n, iteration)
            heap_fallbacks += heap_count + heap_count_desc
            randomized_count += 1

    adversarial = []
    for n in [1000, 4096, 10000]:
        inputs = {
            'organ_pipe': np.r_[np.arange(n // 2), np.arange(n // 2 - 1, -1, -1)].astype(float),
            'sorted': np.arange(n, dtype=float),
            'reversed': np.arange(n, dtype=float)[::-1],
            'sawtooth': (np.arange(n) % 13).astype(float),
        }
        for name, values in inputs.items():
            replica, heap_count = descending_positions(values)
            assert replica == nargsort(values, ascending=False).tolist(), ('adversarial', name, n)
            adversarial.append({'name': name, 'n': n, 'heap_fallbacks': heap_count, 'permutation_sha256': hashlib.sha256(json.dumps(replica).encode()).hexdigest()})

    measure_names = ['wind_transport (tonne)', 'water_transport (tonne)', 'ash_transport (tonne)']
    frame = pd.DataFrame({'row_id': list(range(20)), measure_names[0]: cases[11][1], measure_names[1]: [float((i * 7) % 4) for i in range(20)]})
    frame[measure_names[2]] = frame[measure_names[0]] + frame[measure_names[1]]
    sequential = {'input_rows': frame.to_dict('records'), 'sorts': []}
    for measure in measure_names:
        frame.sort_values(by=measure, ascending=False, inplace=True)
        sequential['sorts'].append({'measure': measure, 'row_ids': frame.row_id.tolist(), 'average_ranks': frame[measure].rank(ascending=False).tolist()})

    namespace = {'np': np, 'pd': pd}
    source_hashes = {}
    source_hashes['statistics'] = frozen_function('wepppy/all_your_base/stats/stats.py', ['probability_of_occurrence', 'weibull_series'], namespace)
    source_hashes['isfloat'] = frozen_function('wepppy/all_your_base/all_your_base.py', ['isfloat'], namespace)
    source_hashes['ashpost'] = frozen_function('wepppy/nodb/mods/ash_transport/ashpost.py', ['calculate_return_periods'], namespace)
    recurrence = [1000, 500, 200, 100, 50, 25, 20, 10, 5, 2]
    return_period_cases = []
    for name, years, values in [
        ('all_zero_20', 20., [0.] * 20),
        ('all_equal_positive_20', 20., [1.] * 20),
        ('two_positive_20', 20., [2., 2.] + [0.] * 18),
        ('mixed_20', 20., cases[11][1]),
        ('less_than_two_years', 20. / 365.25, cases[11][1]),
    ]:
        df = pd.DataFrame({measure_names[0]: values, 'days_from_fire (days)': np.arange(1, 21, dtype=np.uint16), 'year0': np.arange(2000, 2020, dtype=np.uint16), 'year': np.arange(2000, 2020, dtype=np.uint16), 'julian': np.ones(20, dtype=np.uint16)})
        result = namespace['calculate_return_periods'](df, measure_names[0], recurrence, years, ['days_from_fire (days)', 'year0', 'year', 'julian'])
        return_period_cases.append({'name': name, 'num_fire_years': years, 'values': values, 'result': result})

    output = {
        'numpy': np.__version__, 'pandas': pd.__version__,
        'numpy_avx512_skx': bool(np.core._multiarray_umath.__cpu_features__['AVX512_SKX']),
        'pandas_nargsort_sha256': hashlib.sha256(inspect.getsource(nargsort).encode()).hexdigest(),
        'frozen_baseline': 'abebd09239f398af7627924c4c000ff3229a01ee',
        'frozen_source_hashes': source_hashes,
        'sort_cases': results, 'sequential_sort_case': sequential,
        'return_period_cases': return_period_cases,
        'randomized_arrays': randomized_count, 'randomized_heap_fallbacks': heap_fallbacks,
        'adversarial_results': adversarial,
    }
    Path('/tmp/ashpost_argsort_oracles.json').write_text(json.dumps(clean(output), indent=2, allow_nan=False) + '\n')
    print(json.dumps({'output': '/tmp/ashpost_argsort_oracles.json', 'sort_cases': len(results), 'randomized_arrays': randomized_count, 'randomized_heap_fallbacks': heap_fallbacks, 'adversarial_results': adversarial}, indent=2))


if __name__ == '__main__':
    main()

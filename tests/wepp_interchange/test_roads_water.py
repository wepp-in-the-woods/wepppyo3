"""Modeled Roads profiles retain native topology until outlet reduction."""
import importlib.util
import os
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
import pytest

FIXTURE = Path(__file__).resolve().parents[1] / 'fixtures/totalwatsed3/contracts/normal'


@pytest.fixture
def native():
    binary = os.environ.get('WEPP_INTERCHANGE_TEST_BINARY')
    if binary:
        spec = importlib.util.spec_from_file_location('wepp_interchange_rust', binary)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    from wepppyo3.wepp_interchange import wepp_interchange_rust
    return wepp_interchange_rust


def sources(path, weights=(1., 1.)):
    return [(str(path), h, h, area * weight, None, None)
            for h, area, weight in zip((1, 2), (3000., 6000.), weights)]


def produce(native, tmp_path, inputs, pass_path=None):
    out, accounting = tmp_path / 'total.parquet', tmp_path / 'accounting.parquet'
    native.roads_water_to_parquet(str(pass_path or FIXTURE / 'H.pass.parquet'), inputs,
                                 str(out), str(accounting), 0., .04, 0., 1, 2)
    return pq.read_table(out), pq.read_table(accounting)


def test_zero_roads_matches_original_reducer(native, tmp_path):
    inputs = sources(FIXTURE / 'H.wat.parquet')
    for i, row in enumerate(inputs):
        inputs[i] = (*row[:4], str(FIXTURE / 'H.soil.parquet'), str(FIXTURE / 'H.element.parquet'))
    out, accounting = produce(native, tmp_path, inputs)
    legacy = tmp_path / 'legacy.parquet'
    native.totalwatsed3_to_parquet(str(FIXTURE / 'H.pass.parquet'), str(FIXTURE / 'H.wat.parquet'),
                                  str(legacy), 0., .04, 0., 1, 2,
                                  soil_path=str(FIXTURE / 'H.soil.parquet'),
                                  element_path=str(FIXTURE / 'H.element.parquet'))
    original = pq.read_table(legacy)
    for col in out.column_names:
        np.testing.assert_allclose(out[col].to_numpy(), original[col].to_numpy(), equal_nan=True)
    assert accounting.schema.metadata[b'derived_accounting'] == b'true'
    assert set(accounting['ofe_id'].to_pylist()) == {1}


def test_independent_outlets_and_literal_diagnostics(native, tmp_path):
    rows = pq.read_table(FIXTURE / 'H.wat.parquet').to_pylist()
    for row in rows:
        row['Q'] = 100. if row['ofe_id'] == 1 else float(row['wepp_id'])
    wat = tmp_path / 'raw.parquet'
    pq.write_table(pa.Table.from_pylist(rows), wat)
    # Both native profiles map to one target; neither terminal may replace the other.
    inputs = [(str(wat), 1, 1, 1500., None, None), (str(wat), 2, 1, 6000., None, None)]
    out, accounting = produce(native, tmp_path, inputs)
    np.testing.assert_allclose(out['Area'].to_numpy(), 7500.)
    np.testing.assert_allclose(out['Q'].to_numpy(), 259.)  # 50 + 1 + 200 + 8 m3
    np.testing.assert_allclose(accounting['Q'].to_numpy(), 13.5 / 7500. * 1000.)
    np.testing.assert_allclose(accounting['latqcc'].to_numpy(), 20. / 7500. * 1000.)


@pytest.mark.parametrize('defect', ['duplicate', 'missing_ofe', 'area_drift', 'outlet_null', 'missing_day'])
def test_invalid_profiles_fail_without_publication(native, tmp_path, defect):
    rows = pq.read_table(FIXTURE / 'H.wat.parquet').to_pylist()
    if defect == 'duplicate': rows.append(dict(rows[0]))
    elif defect == 'missing_ofe': rows.pop(0)
    elif defect == 'area_drift': rows[0]['Area'] += 1
    elif defect == 'outlet_null': rows[1]['latqcc'] = None
    else: rows = [r for r in rows if not (r['wepp_id'] == 1 and r['sim_day_index'] == 1)]
    wat = tmp_path / 'raw.parquet'
    pq.write_table(pa.Table.from_pylist(rows), wat)
    with pytest.raises(RuntimeError):
        produce(native, tmp_path, sources(wat))
    assert not (tmp_path / 'total.parquet').exists()
    assert not (tmp_path / 'accounting.parquet').exists()


def test_output_alias_rejected(native, tmp_path):
    path = tmp_path / 'raw.parquet'
    path.write_bytes((FIXTURE / 'H.wat.parquet').read_bytes())
    original = path.read_bytes()
    with pytest.raises(RuntimeError, match='alias'):
        native.roads_water_to_parquet(str(FIXTURE / 'H.pass.parquet'), sources(path),
                                     str(path), str(tmp_path / 'other.parquet'), 0., .04, 0., 1, 2)
    assert path.read_bytes() == original


@pytest.mark.parametrize('defect', ['infinity', 'overflow', 'optional_calendar'])
def test_nonfinite_and_optional_calendar_errors(native, tmp_path, defect):
    wat_rows = pq.read_table(FIXTURE / 'H.wat.parquet').to_pylist()
    soil_rows = pq.read_table(FIXTURE / 'H.soil.parquet').to_pylist()
    if defect == 'infinity': wat_rows[0]['P'] = float('inf')
    elif defect == 'overflow': wat_rows[1]['P'] = 1e308
    else: soil_rows[0]['julian'] += 10
    wat, soil = tmp_path / 'raw.parquet', tmp_path / 'soil.parquet'
    pq.write_table(pa.Table.from_pylist(wat_rows), wat)
    pq.write_table(pa.Table.from_pylist(soil_rows), soil)
    inputs = [(str(wat), h, h, area, str(soil), None) for h, area in ((1, 3000.), (2, 6000.))]
    with pytest.raises(RuntimeError):
        produce(native, tmp_path, inputs)
    assert not (tmp_path / 'total.parquet').exists()
    assert not (tmp_path / 'accounting.parquet').exists()


def test_nonexistent_outputs_with_symlink_parent_cannot_alias(native, tmp_path):
    real, link = tmp_path / 'real', tmp_path / 'link'
    real.mkdir()
    link.symlink_to(real, target_is_directory=True)
    with pytest.raises(RuntimeError, match='alias'):
        native.roads_water_to_parquet(str(FIXTURE / 'H.pass.parquet'), sources(FIXTURE / 'H.wat.parquet'),
                                     str(real / 'out.parquet'), str(link / 'out.parquet'), 0., .04, 0., 1, 2)
    assert not (real / 'out.parquet').exists()


@pytest.mark.parametrize('defect', ['null_pass', 'large_sim_day', 'malformed_optional', 'area_overflow'])
def test_required_input_boundaries(native, tmp_path, defect):
    wat = pq.read_table(FIXTURE / 'H.wat.parquet').to_pylist()
    ps = pq.read_table(FIXTURE / 'H.pass.parquet').to_pylist()
    if defect == 'null_pass': ps[0]['runvol'] = None
    elif defect == 'large_sim_day':
        for row in wat + ps: row['sim_day_index'] += 2**31
    elif defect == 'area_overflow':
        for row in wat: row['Area'] = 1e308
    wp, pp = tmp_path / 'wat.parquet', tmp_path / 'pass.parquet'
    pq.write_table(pa.Table.from_pylist(wat), wp)
    pq.write_table(pa.Table.from_pylist(ps), pp)
    inputs = sources(wp)
    if defect == 'malformed_optional':
        soil = tmp_path / 'soil.parquet'
        pq.write_table(pa.table({'wepp_id': [1]}), soil)
        inputs[0] = (*inputs[0][:4], str(soil), None)
    with pytest.raises(RuntimeError):
        produce(native, tmp_path, inputs, pp)
    assert not (tmp_path / 'total.parquet').exists()
    assert not (tmp_path / 'accounting.parquet').exists()

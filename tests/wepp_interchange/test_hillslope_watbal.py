"""Hillslope summary contracts: compact output, malformed input, atomic publication."""
import importlib.util
import os
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
import pytest


@pytest.fixture(scope='module')
def native():
    binary = os.environ.get('WEPP_INTERCHANGE_TEST_BINARY')
    if binary:
        spec = importlib.util.spec_from_file_location('wepp_interchange_rust', binary)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        from wepppyo3.wepp_interchange import wepp_interchange_rust as module
    return module


def source(tmp_path, **replace):
    data = dict(wepp_id=[2, 1, 1, 2], ofe_id=[1, 1, 2, 1], water_year=[2001, 2000, 2000, 2001],
                sim_day_index=[1, 1, 1, 2], Area=[20., 10., 5., 20.], P=[1., 2., None, 4.], Dp=[.1]*4, Q=[.2]*4, QOFE=[.2]*4,
                latqcc=[.3]*4, Ep=[.4]*4, Es=[.5]*4, Er=[.6]*4)
    data.update(replace)
    path = tmp_path/'H.wat.parquet'
    pq.write_table(pa.table(data), path, row_group_size=1)
    return path


def invoke(native, path, output, mapping=None):
    return native.hillslope_watbal_to_parquet(str(path), str(output), mapping or {1: 101, 2: 101})


def test_merged_topaz_first_ofe_area_and_separate_flux_sums(native, tmp_path):
    path = source(tmp_path)
    assert native.hillslope_watbal_wepp_ids(str(path)) == [1, 2]
    out = tmp_path/'summary.parquet'
    assert invoke(native, path, out) == dict(input_rows=4, rows_written=2, ofe_keys=3)
    table = pq.read_table(out)
    assert table.column_names == ['TopazID', 'WaterYear', 'Area_m2', 'Precipitation (mm)',
        'Percolation (mm)', 'Surface Runoff (mm)', 'Lateral Flow (mm)', 'Transpiration + Evaporation (mm)']
    assert table['WaterYear'].to_pylist() == [2000, 2001]
    assert table['Area_m2'].to_pylist() == [35., 35.]
    np.testing.assert_allclose(table['Precipitation (mm)'].to_numpy(), [4/3, 5.])
    np.testing.assert_allclose(table['Transpiration + Evaporation (mm)'].to_numpy(), [1.5, 3.])
    assert all(f.nullable for f in table.schema)


@pytest.mark.parametrize('missing', [None, float('nan')])
def test_null_ofe_fails_without_publishing(native, tmp_path, missing):
    path = source(tmp_path, ofe_id=[1., missing, 1., 2.])
    out = tmp_path/'summary.parquet'
    with pytest.raises(ValueError):
        invoke(native, path, out)
    assert not out.exists()


@pytest.mark.parametrize('area', [None, float('nan'), 0., -1., float('inf')])
def test_invalid_area_fails_without_publishing(native, tmp_path, area):
    path = source(tmp_path, Area=[area, 10., 99., 5.])
    out = tmp_path/'summary.parquet'
    with pytest.raises(ValueError):
        invoke(native, path, out)
    assert not out.exists()


def test_empty_null_schema(native, tmp_path):
    path = source(tmp_path)
    schema = pq.read_schema(path)
    pq.write_table(pa.table({name: pa.nulls(0) for name in schema.names}), path)
    out = tmp_path/'summary.parquet'
    assert native.hillslope_watbal_wepp_ids(str(path)) == []
    assert invoke(native, path, out)['rows_written'] == 0
    assert all(f.type == pa.null() for f in pq.read_schema(out))


@pytest.mark.parametrize('column,values', [
    ('wepp_id', [1., 1.5, 1., 2.]), ('water_year', [None, 2000, 2000, 2001]),
    ('ofe_id', [1., 1.5, 1., 2.]), ('P', [float('inf'), 1., 1., 1.]),
    ('P', ['a']*4), ('wepp_id', pa.array([2**63]*4, type=pa.uint64())),
])
def test_malformed_values_preserve_existing_cache(native, tmp_path, column, values):
    path = source(tmp_path, **{column: values})
    out = tmp_path/'summary.parquet'; out.write_bytes(b'prior cache')
    with pytest.raises(ValueError):
        invoke(native, path, out)
    assert out.read_bytes() == b'prior cache'
    assert sorted(p.name for p in tmp_path.iterdir()) == ['H.wat.parquet', 'summary.parquet']


def test_missing_mapping_and_column(native, tmp_path):
    path = source(tmp_path);out = tmp_path/'summary.parquet'
    with pytest.raises(KeyError): invoke(native, path, out, {1: 101})
    table = pq.read_table(path).drop(['Dp']);pq.write_table(table, path)
    with pytest.raises(ValueError): invoke(native, path, out)
    assert not out.exists()


def test_output_alias_symlink_directory_and_missing_parent(native, tmp_path):
    path = source(tmp_path);prior=path.read_bytes()
    alias=tmp_path/'alias';alias.symlink_to(path)
    with pytest.raises(ValueError): invoke(native, alias, path)
    with pytest.raises(ValueError): invoke(native, path, alias)
    target=tmp_path/'old';target.write_bytes(b'old')
    link=tmp_path/'link';link.symlink_to(target)
    with pytest.raises(OSError): invoke(native, path, link)
    with pytest.raises(OSError): invoke(native, path, tmp_path)
    with pytest.raises(OSError): invoke(native, path, tmp_path/'absent'/'summary.parquet')
    assert path.read_bytes() == prior
    assert target.read_bytes() == b'old'
    assert not list(tmp_path.glob('.*.wepp-*'))


def test_late_evaporation_overflow_removes_stage(native, tmp_path):
    path = source(tmp_path, Ep=[8e307, 8e307, 0., 0.], Es=[8e307, 8e307, 0., 0.], Er=[8e307, 8e307, 0., 0.])
    out = tmp_path/'summary.parquet';out.write_bytes(b'prior cache')
    with pytest.raises(ValueError, match='overflow'):
        invoke(native, path, out)
    assert out.read_bytes() == b'prior cache'
    assert sorted(p.name for p in tmp_path.iterdir()) == ['H.wat.parquet', 'summary.parquet']


def test_os_write_failure_preserves_cache_and_removes_stage(native, tmp_path):
    import subprocess
    import sys
    path=source(tmp_path);out=tmp_path/'summary.parquet';out.write_bytes(b'prior cache')
    code = '''
import importlib.util, resource, signal, sys
spec=importlib.util.spec_from_file_location('wepp_interchange_rust', sys.argv[1])
native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
resource.setrlimit(resource.RLIMIT_FSIZE, (128,128))
try:
    native.hillslope_watbal_to_parquet(sys.argv[2],sys.argv[3],{1:101,2:101})
except OSError:
    print('expected OSError')
else:
    raise AssertionError('writer unexpectedly succeeded')
'''
    result=subprocess.run([sys.executable,'-c',code,native.__file__,str(path),str(out)],capture_output=True,text=True)
    assert result.returncode == 0, result.stderr
    assert 'expected OSError' in result.stdout
    assert out.read_bytes() == b'prior cache'
    assert sorted(p.name for p in tmp_path.iterdir()) == ['H.wat.parquet', 'summary.parquet']


@pytest.mark.parametrize('mode', [0o600, 0o640, 0o660])
def test_existing_cache_permissions_preserved(native, tmp_path, mode):
    import stat
    path=source(tmp_path);out=tmp_path/'summary.parquet'
    out.write_bytes(b'old');out.chmod(mode)
    invoke(native,path,out)
    assert stat.S_IMODE(out.stat().st_mode) == mode


def test_unequal_area_daily_oracle_and_merged_ids(native, tmp_path):
    path = source(tmp_path, wepp_id=[1, 2, 1, 2], ofe_id=[1]*4,
        water_year=[2001]*4, sim_day_index=[1, 1, 2, 2],
        Area=[250., 750.]*2, Ep=[1., 4.]*2, Es=[.5, 1.]*2,
        Er=[.5, 1.]*2, Dp=[4., 8.]*2)
    out = tmp_path/'summary.parquet'
    invoke(native, path, out)
    row = pq.read_table(out).to_pylist()[0]
    assert row['Area_m2'] == 1000.
    assert row['Transpiration + Evaporation (mm)'] == 10.
    assert row['Percolation (mm)'] == 14.
    assert pq.read_schema(out).metadata[b'hillslope_watbal_aggregation'] == b'mofe-water-v2'


def test_single_ofe_fast_path_matches_equal_state_subdivision(native, tmp_path):
    path = source(tmp_path, wepp_id=[1,1,2,2],ofe_id=[1]*4,water_year=[2001]*4,
        sim_day_index=[1,2,1,2],Area=[1000.,1000.,2000.,2000.],
        Ep=[1.,4.]*2,Es=[.5,1.]*2,Er=[.5,1.]*2,Dp=[4.,8.]*2)
    out = tmp_path/'summary.parquet'
    invoke(native,path,out,{1:101,2:202})
    single = pq.read_table(out).to_pandas()
    np.testing.assert_allclose(single['Transpiration + Evaporation (mm)'],[8.,8.])
    np.testing.assert_allclose(single['Percolation (mm)'],[12.,12.])
    frame = pq.read_table(path).to_pandas(); frame.Area *= .5
    part = frame.copy(); part.ofe_id = 2
    import pandas as pd
    pd.concat([frame,part],ignore_index=True).to_parquet(path,index=False)
    invoke(native,path,out,{1:101,2:202})
    split = pq.read_table(out).to_pandas()
    for column in ['Area_m2','Transpiration + Evaporation (mm)','Percolation (mm)']:
        np.testing.assert_allclose(single[column],split[column])


def test_sparse_day_is_normalized_before_annual_sum(native,tmp_path):
    path = source(tmp_path,wepp_id=[1]*4,ofe_id=[1,2,2,2],water_year=[2001]*4,
        sim_day_index=[1,1,2,3],Area=[250.,750.,750.,750.],
        Ep=[1.,4.,1.,1.],Es=[.5,1.,.5,.5],Er=[.5,1.,.5,.5],Dp=[4.,8.,4.,4.])
    out = tmp_path/'summary.parquet';invoke(native,path,out)
    row = pq.read_table(out).to_pylist()[0]
    assert row['Transpiration + Evaporation (mm)'] == 9.
    assert row['Percolation (mm)'] == 15.


@pytest.mark.parametrize('statistics', [True, False])
@pytest.mark.parametrize('mapping', [{1:101,2:202}, {1:101,2:101}])
def test_row_group_geometry_proof_matches_scan(native,tmp_path,statistics,mapping):
    path=source(tmp_path,wepp_id=[1,2,1,2],ofe_id=[1]*4,water_year=[2001]*4,
        sim_day_index=[1,1,2,2],Area=[250.,750.]*2,
        Ep=[1.,4.]*2,Es=[.5,1.]*2,Er=[.5,1.]*2,Dp=[4.,8.]*2)
    table=pq.read_table(path)
    out=tmp_path/'summary.parquet'
    invoke(native,path,out,mapping)
    expected=pq.read_table(out)
    pq.write_table(table,path,row_group_size=1,write_statistics=statistics)
    invoke(native,path,out,mapping)
    assert pq.read_table(out).equals(expected)


def test_physical_water_outlets_not_internal_transfers(native, tmp_path):
    data = dict(wepp_id=[1,1], ofe_id=[1,2], water_year=[2000,2000],
                sim_day_index=[1,1], Area=[250.,750.], P=[10.,10.],
                Q=[4.,8.], QOFE=[4.,8.*1000/750], latqcc=[4.,6.],
                Ep=[1.,4.], Es=[.5,1.], Er=[.5,1.], Dp=[4.,8.])
    path=tmp_path/'H.wat.parquet'; pq.write_table(pa.table(data),path)
    out=tmp_path/'summary.parquet'; invoke(native,path,out,{1:101})
    row=pq.read_table(out).to_pylist()[0]
    assert row['Precipitation (mm)']==10.
    assert row['Surface Runoff (mm)']==8.
    assert row['Lateral Flow (mm)']==4.5
    assert row['Transpiration + Evaporation (mm)']==5.
    assert row['Percolation (mm)']==7.


def test_missing_known_terminal_rejects_partial_publication(native,tmp_path):
    data=dict(wepp_id=[1,1,1],ofe_id=[1,2,1],water_year=[2000]*3,
              sim_day_index=[1,1,2],Area=[250.,750.,250.],P=[10.]*3,
              Q=[4.,8.,4.],QOFE=[4.,8.,4.],latqcc=[4.,6.,4.],
              Ep=[1.]*3,Es=[1.]*3,Er=[1.]*3,Dp=[1.]*3)
    path=tmp_path/'H.wat.parquet';pq.write_table(pa.table(data),path)
    out=tmp_path/'summary.parquet';out.write_bytes(b'prior')
    with pytest.raises(ValueError,match='terminal'):
        invoke(native,path,out,{1:101})
    assert out.read_bytes()==b'prior'


@pytest.mark.parametrize('native_id,ofe_id', [(2, 1), (1, 1), (1, 2)])
def test_duplicate_daily_observation_preserves_cache(native, tmp_path, native_id, ofe_id):
    path = source(tmp_path)
    table = pq.read_table(path)
    rows = table.to_pylist()
    duplicate = next(row.copy() for row in rows if row['wepp_id'] == native_id and row['ofe_id'] == ofe_id)
    duplicate['P'] = 99.
    pq.write_table(pa.Table.from_pylist(rows + [duplicate], schema=table.schema), path)
    out = tmp_path / 'summary.parquet'
    out.write_bytes(b'prior')
    with pytest.raises(ValueError, match='duplicate'):
        invoke(native, path, out, {1: 101, 2: 202})
    assert out.read_bytes() == b'prior'

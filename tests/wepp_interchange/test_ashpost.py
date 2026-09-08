"""Native AshPost extraction contracts with small generated parquet inputs."""
import importlib.util
import os

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


def source(tmp_path, **changes):
    data = {'year0': [2000]*4, 'year': [2000, 2000, 2001, 2001],
            'julian': [300, 300, 1, 301], 'days_from_fire (days)': [1, 1, 67, 367]}
    for name in ('wind', 'water', 'ash'):
        data[f'{name}_transport (tonne/ha)'] = [1., 2., 4., 8.]
        data[f'cum_{name}_transport (tonne/ha)'] = [10., 20., 40., 80.]
    data.update(changes)
    path = tmp_path / 'hill.parquet'
    pq.write_table(pa.table(data), path, row_group_size=1)
    return path


def invoke(native, root, manifest=None, **kwargs):
    return native.ashpost_to_parquet(str(root), str(root/'post'),
        [('hill.parquet', 101, 2., 2)] if manifest is None else manifest, [2, 5], **kwargs)


def test_daily_coalescing_first_year_and_cumulative_selection(native, tmp_path):
    source(tmp_path)
    result = invoke(native, tmp_path)
    assert result['input_rows'] == 4
    assert len(result['rows_written']) == 5
    hill = pq.read_table(tmp_path/'post/hillslope_annuals.parquet').to_pydict()
    assert hill['wind_transport (tonne/ha)'] == [3.5]
    annual = pq.read_table(tmp_path/'post/watershed_annuals.parquet').to_pydict()
    assert annual['year'] == [2000, 2001]
    assert annual['area (ha)'] == [2., 2.]
    assert annual['wind_transport (tonne)'] == [6., 8.]
    daily = pq.read_table(tmp_path/'post/watershed_daily.parquet')
    assert daily['wind_transport (tonne)'].to_pylist() == [6., 8.]
    assert daily['Streamflow_orig (mm)'].to_pylist() == [0., 0.]
    assert daily['tot_seddep+ash (tonne)'].to_pylist() == [0., 0.]
    assert daily.schema.field('area (ha)').type == pa.float32()
    assert daily.schema.field('year').type == pa.uint16()
    assert daily.schema.metadata[b'dataset_version'] == b'1.0'
    cumulative = pq.read_table(tmp_path/'post/watershed_cumulatives.parquet')
    assert cumulative['cum_wind_transport (tonne)'].to_pylist() == [160.]
    assert cumulative['days_from_fire (days)'].to_pylist() == [367]


def test_empty_manifest_writes_nothing(native, tmp_path):
    result = invoke(native, tmp_path, [])
    assert result['return_periods'] is None
    assert result['rows_written'] == {}
    assert not (tmp_path/'post').exists()


@pytest.mark.parametrize('area', [0., 1.25])
def test_null_and_nan_metrics_sum_as_zero(native, tmp_path, area):
    source(tmp_path, **{'wind_transport (tonne/ha)': [None, float('nan'), 0., 8.]})
    invoke(native, tmp_path, [('hill.parquet', 101, area, 2)])
    table = pq.read_table(tmp_path/'post/watershed_daily.parquet')
    assert table['wind_transport (tonne)'].to_pylist() == [0., 0.]
    assert table['wind_transport (tonne/ha)'].to_pylist() == [0., 0.]


@pytest.mark.parametrize('changes', [
    {'year': [2000, None, 2001, 2001]},
    {'julian': [0, 300, 1, 301]},
    {'days_from_fire (days)': [1., 1.5, 67., 367.]},
    {'wind_transport (tonne/ha)': [1., float('inf'), 2., 3.]},
    {'wind_transport (tonne/ha)': ['a']*4},
])
def test_invalid_rows_fail_explicitly(native, tmp_path, changes):
    source(tmp_path, **changes)
    with pytest.raises(ValueError):
        invoke(native, tmp_path)


@pytest.mark.parametrize('entry', [
    ('hill.parquet', 0, 1., 2), ('hill.parquet', 65536, 1., 2),
    ('hill.parquet', 101, -1., 2), ('hill.parquet', 101, float('nan'), 2),
    ('hill.parquet', 101, 1., 5), ('../hill.parquet', 101, 1., 2),
])
def test_invalid_manifest(native, tmp_path, entry):
    source(tmp_path)
    with pytest.raises(ValueError):
        invoke(native, tmp_path, [entry])


def test_missing_empty_malformed_and_missing_column(native, tmp_path):
    with pytest.raises(FileNotFoundError):
        invoke(native, tmp_path)
    path = source(tmp_path)
    table = pq.read_table(path)
    for invalid in [table.slice(0, 0), table.drop(['year'])]:
        pq.write_table(invalid, path)
        with pytest.raises(ValueError):
            invoke(native, tmp_path)
    path.write_bytes(b'not parquet')
    with pytest.raises(ValueError):
        invoke(native, tmp_path)


def test_duplicate_and_mixed_inputs(native, tmp_path):
    path = source(tmp_path)
    os.link(path, tmp_path/'alias.parquet')
    with pytest.raises(ValueError, match='duplicate'):
        invoke(native, tmp_path, [('hill.parquet', 101, 1., 1), ('alias.parquet', 102, 1., 2)])
    pq.write_table(pq.read_table(path).append_column('tau', pa.array([1.]*4)), tmp_path/'other.parquet')
    with pytest.raises(ValueError, match='mixed'):
        invoke(native, tmp_path, [('hill.parquet', 101, 1., 1), ('other.parquet', 102, 1., 2)])


def test_symlink_and_output_alias_rejected(native, tmp_path):
    path = source(tmp_path)
    (tmp_path/'link.parquet').symlink_to(path)
    with pytest.raises(ValueError, match='symlink'):
        invoke(native, tmp_path, [('link.parquet', 101, 1., 1)])
    (tmp_path/'post').mkdir()
    os.link(path, tmp_path/'post/hillslope_annuals.parquet')
    original = path.read_bytes()
    with pytest.raises(ValueError, match='aliases'):
        invoke(native, tmp_path)
    assert path.read_bytes() == original


def test_individual_writer_failure_and_rerun(native, tmp_path):
    source(tmp_path)
    blocked = tmp_path/'post/watershed_daily.parquet'
    blocked.mkdir(parents=True)
    with pytest.raises(ValueError, match='regular file'):
        invoke(native, tmp_path)
    assert (tmp_path/'post/hillslope_annuals.parquet').is_file()
    assert not (tmp_path/'post/watershed_cumulatives.parquet').exists()
    blocked.rmdir()
    invoke(native, tmp_path)
    assert len(list((tmp_path/'post').iterdir())) == 5
    output = tmp_path/'post/hillslope_annuals.parquet'
    output.chmod(0o640)
    invoke(native, tmp_path)
    assert output.stat().st_mode & 0o777 == 0o640


def hydrology(tmp_path, qofe=None, flow=1.):
    pq.write_table(pa.table({'year': [2000, 2001], 'julian': [300, 1],
        'Streamflow': [flow]*2, 'Runoff': [1.]*2, 'Lateral Flow': [0.]*2,
        'Baseflow': [0.]*2, 'Area': [10000.]*2}), tmp_path/'hydro.parquet')
    pq.write_table(pa.table({'wepp_id': [1]*2, 'year': [2000, 2001], 'julian': [300, 1],
        'QOFE': pa.array([qofe]*2, type=pa.float64()), 'Area': [10000.]*2}), tmp_path/'wat.parquet')
    return dict(hydrology_path='hydro.parquet', wat_path='wat.parquet', ash_wepp_ids=[1])


def test_null_runoff_and_empty_hydrology(native, tmp_path):
    source(tmp_path)
    kwargs = hydrology(tmp_path)
    invoke(native, tmp_path, **kwargs)
    assert pq.read_table(tmp_path/'post/watershed_daily.parquet')['Streamflow_ash_corr (mm)'].to_pylist() == [None, None]
    pq.write_table(pa.table({'year': pa.array([], type=pa.uint16())}), tmp_path/'hydro.parquet')
    invoke(native, tmp_path, **kwargs)
    assert pq.read_table(tmp_path/'post/watershed_daily.parquet')['Streamflow_ash_corr (mm)'].to_pylist() == [0., 0.]
    kwargs['hydrology_path'] = None
    (tmp_path/'wat.parquet').write_bytes(b'bad parquet')
    with pytest.raises(ValueError):
        invoke(native, tmp_path, **kwargs)


@pytest.mark.parametrize('case', ['mass_overflow', 'runoff_overflow', 'infinite_flow', 'float32', 'null_string'])
def test_unsupported_metrics_and_arithmetic_overflow(native, tmp_path, case):
    path = source(tmp_path)
    kwargs = {}
    if case == 'mass_overflow':
        source(tmp_path, **{'wind_transport (tonne/ha)': [1e308, 0., 0., 0.]})
    elif case in ('runoff_overflow', 'infinite_flow'):
        kwargs = hydrology(tmp_path, qofe=1e308 if case == 'runoff_overflow' else 0.,
                           flow=float('inf') if case == 'infinite_flow' else 1.)
    else:
        table = pq.read_table(path)
        values = pa.array([None]*4, type=pa.string()) if case == 'null_string' else pa.array([1.]*4, type=pa.float32())
        pq.write_table(table.set_column(table.schema.get_field_index('wind_transport (tonne/ha)'), 'wind_transport (tonne/ha)', values), path)
    with pytest.raises(ValueError):
        invoke(native, tmp_path, **kwargs)


def test_valid_nan_runoff_propagates_beside_finite_row(native, tmp_path):
    source(tmp_path)
    kwargs = hydrology(tmp_path, qofe=0.1)
    table = pa.table({'wepp_id': [1, 1, 1], 'year': [2000, 2000, 2001],
        'julian': [300, 300, 1], 'QOFE': [float('nan'), 0.1, 0.1], 'Area': [10000.]*3})
    pq.write_table(table, tmp_path/'wat.parquet')
    invoke(native, tmp_path, **kwargs)
    values = pq.read_table(tmp_path/'post/watershed_daily.parquet')['Streamflow_ash_corr (mm)'].to_pylist()
    assert values[0] is None
    assert values[1] == pytest.approx(0.9)


@pytest.mark.parametrize('input_name', ['hydro.parquet', 'wat.parquet'])
def test_optional_input_output_alias_rejected(native, tmp_path, input_name):
    source(tmp_path)
    kwargs = hydrology(tmp_path, qofe=0.1)
    (tmp_path/'post').mkdir()
    os.link(tmp_path/input_name, tmp_path/'post/watershed_daily.parquet')
    before = (tmp_path/input_name).read_bytes()
    with pytest.raises(ValueError, match='aliases'):
        invoke(native, tmp_path, **kwargs)
    assert (tmp_path/input_name).read_bytes() == before


@pytest.mark.parametrize('case_index', range(4))
def test_populated_cumulative_return_periods_match_frozen_python(native, tmp_path, case_index):
    import json
    from pathlib import Path
    oracle = json.loads((Path(__file__).parents[2]/'docs/work-packages/20260908-ashpost-native-001/artifacts/argsort-oracles.json').read_text())['return_period_cases'][case_index]
    values = oracle['values']; count = len(values)
    data = {'year0': list(range(2000,2000+count)), 'year': list(range(2000,2000+count)),
            'julian': [1]*count, 'days_from_fire (days)': list(range(1,count+1))}
    for name in ('wind','water','ash'):
        data[f'{name}_transport (tonne/ha)'] = values
        data[f'cum_{name}_transport (tonne/ha)'] = values
    pq.write_table(pa.table(data),tmp_path/'hill.parquet')
    intervals = [1000,500,200,100,50,25,20,10,5,2]
    result = native.ashpost_to_parquet(str(tmp_path),str(tmp_path/'post'),
        [('hill.parquet',101,1.,2)],intervals)
    actual = result['cum_return_periods']['cum_wind_transport (tonne)']
    for interval, expected in oracle['result'].items():
        expected = {key.replace('wind_transport','cum_wind_transport'): value
                    for key,value in expected.items() if key not in ('year','julian','days_from_fire (days)')}
        row = actual[int(interval)]
        assert row.keys() == expected.keys()
        for key,value in expected.items():
            assert type(row[key]) is type(value),(key,row[key],value)
            assert row[key] == pytest.approx(value,rel=1e-10,abs=1e-12)

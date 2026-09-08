"""Frozen oracle and publication contracts for the required native producer."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
import pytest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'tests/fixtures/totalwatsed3'
CASES = sorted(p.name for p in (FIXTURES / 'contracts').iterdir() if p.is_dir())


@pytest.fixture(scope='module')
def native():
    # Explicit development binary override; the default exercises the release import.
    binary = os.environ.get('WEPP_INTERCHANGE_TEST_BINARY')
    if binary:
        spec = importlib.util.spec_from_file_location('wepp_interchange_rust', binary)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        from wepppyo3.wepp_interchange import wepp_interchange_rust as module
    assert callable(module.totalwatsed3_to_parquet)
    return module.totalwatsed3_to_parquet


def assert_parity(oracle_path, actual_path):
    oracle, actual = pq.read_table(oracle_path), pq.read_table(actual_path)
    assert actual.schema.equals(oracle.schema, check_metadata=True)
    assert actual.num_rows == oracle.num_rows
    for name in oracle.column_names:
        a, o = actual[name], oracle[name]
        np.testing.assert_array_equal(a.is_null().to_numpy(), o.is_null().to_numpy(), err_msg=name)
        valid = ~o.is_null().to_numpy()
        if pa.types.is_floating(o.type):
            np.testing.assert_allclose(a.to_numpy()[valid], o.to_numpy()[valid], rtol=1e-10, atol=1e-12, equal_nan=True, err_msg=name)
        else:
            np.testing.assert_array_equal(a.to_numpy()[valid], o.to_numpy()[valid], err_msg=name)


def produce(native, source, output, oracle=None, **options):
    schema = pq.read_schema(oracle or source / 'totalwatsed3.parquet')
    return native(str(source / 'H.pass.parquet'), str(source / 'H.wat.parquet'), str(output),
                  options.pop('gwstorage', 0.), options.pop('bfcoeff', .04), options.pop('dscoeff', 0.), 1, 2,
                  soil_path=str(source / 'H.soil.parquet') if (source / 'H.soil.parquet').exists() else None,
                  element_path=str(source / 'H.element.parquet') if (source / 'H.element.parquet').exists() else None,
                  pandas_metadata=(schema.metadata.get(b'pandas') or b'').decode() or None, **options)


@pytest.mark.parametrize('case', CASES)
def test_frozen_contract(native, tmp_path, case):
    source = FIXTURES / 'contracts' / case
    options = json.loads((source / 'case.json').read_text())
    options['ash_inputs'] = [(str(source / path), *metadata) for path, *metadata in options['ash_inputs']]
    out = tmp_path / 'totalwatsed3.parquet'
    summary = produce(native, source, out, **options)
    assert_parity(source / 'totalwatsed3.parquet', out)
    assert summary['rows_written'] == pq.read_metadata(out).num_rows
    assert sorted(p.name for p in tmp_path.iterdir()) == ['totalwatsed3.parquet']


@pytest.mark.parametrize('run', ['decimal-pleasing', 'incommensurate-stickball', 'throbbing-sylvan'])
def test_real_fixture_oracle(native, tmp_path, run):
    source = FIXTURES / run
    oracle = source / 'totalwatsed3.parquet'
    if run == 'decimal-pleasing':
        source = Path(os.environ.get('WEPPPY_SOURCE_ROOT', '/workdir/wepppy')) / 'tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange'
        if not source.exists():
            pytest.skip('decimal-pleasing input fixture requires the adjacent WEPPpy checkout')
    output = tmp_path / 'totalwatsed3.parquet'
    produce(native, source, output, oracle=oracle)
    assert_parity(oracle, output)


def test_mofe_lateral_flow_excludes_internal_ofes(native, tmp_path):
    source = FIXTURES / 'contracts/normal'
    out = tmp_path / 'totalwatsed3.parquet'
    produce(native, source, out)
    # Per day: outlet OFE 2 contributes 4 mm over areas 2000 + 4000 m2.
    # All OFEs contribute 9000 m2 to the watershed denominator.
    table = pq.read_table(out)
    np.testing.assert_allclose(table['latqcc'].to_numpy(), [24.] * 3)
    np.testing.assert_allclose(table['Lateral Flow'].to_numpy(), [24. / 9000 * 1000] * 3)


def test_failed_publication_cleans_stage_and_preserves_target(native, tmp_path):
    source = FIXTURES / 'contracts/normal'
    output = tmp_path / 'totalwatsed3.parquet'
    output.mkdir()
    marker = output / 'prior-generation'
    marker.write_bytes(b'unchanged')
    with pytest.raises((OSError, RuntimeError, ValueError)):
        produce(native, source, output)
    assert marker.read_bytes() == b'unchanged'
    assert list(tmp_path.iterdir()) == [output]


def test_missing_input_preserves_existing_output(native, tmp_path):
    output = tmp_path / 'totalwatsed3.parquet'
    output.write_bytes(b'prior generation')
    with pytest.raises((OSError, RuntimeError, ValueError)):
        native(str(tmp_path / 'missing.pass'), str(tmp_path / 'missing.wat'), str(output), 0., .04, 0., 1, 2)
    assert output.read_bytes() == b'prior generation'
    assert list(tmp_path.iterdir()) == [output]


def test_input_alias_rejected(native, tmp_path):
    source = FIXTURES / 'contracts/normal'
    output = tmp_path / 'output.parquet'
    output.symlink_to(source / 'H.pass.parquet')
    before = (source / 'H.pass.parquet').read_bytes()
    with pytest.raises((OSError, RuntimeError, ValueError), match='alias'):
        produce(native, source, output)
    assert output.is_symlink()
    assert (source / 'H.pass.parquet').read_bytes() == before


@pytest.mark.parametrize('codec,dictionary', [('gzip', False), ('zstd', True)])
def test_metadata_compaction_preserves_physical_reads(native, tmp_path, codec, dictionary):
    source = FIXTURES / 'contracts/normal'
    oracle = source / 'totalwatsed3.parquet'
    for path in source.glob('H.*.parquet'):
        table = pq.read_table(path)
        # Unused nested columns shift leaf indices relative to root indices.
        table = table.add_column(0, 'unused_nested', pa.array([{'left': [1, 2], 'right': 3}] * table.num_rows))
        with pq.ParquetWriter(tmp_path / path.name, table.schema, compression=codec,
                              use_dictionary=dictionary, write_statistics=False) as writer:
            # Cross the bounded reader group window, including empty groups.
            for _ in range(20):
                writer.write_table(table.slice(0, 0))
            writer.write_table(table, row_group_size=1)
    output = tmp_path / 'totalwatsed3.parquet'
    options = json.loads((source / 'case.json').read_text())
    produce(native, tmp_path, output, oracle=oracle, **options)
    assert_parity(oracle, output)


def _mutate_footer_integer(path, field_path, replacement):
    """Change one known compact-Thrift integer in a freshly written test file."""
    data = path.read_bytes()
    footer_size = int.from_bytes(data[-8:-4], 'little')
    footer_start = len(data) - footer_size - 8
    footer = data[footer_start:-8]
    position = 0
    matches = []

    def varint():
        nonlocal position
        value, shift = 0, 0
        while True:
            byte = footer[position]
            position += 1
            value |= (byte & 127) << shift
            if not byte & 128:
                return value
            shift += 7

    def visit(kind, trail):
        nonlocal position
        if kind in (1, 2):  # Boolean values are encoded in the field header.
            return
        if kind == 3:
            position += 1
        elif kind in (4, 5, 6):  # i16, i32, i64 share zigzag varint encoding.
            start = position
            varint()
            if trail == field_path:
                matches.append((start, position))
        elif kind == 7:
            position += 8
        elif kind == 8:
            length = varint()
            position += length
        elif kind in (9, 10):
            header = footer[position]
            position += 1
            size = header >> 4
            if size == 15:
                size = varint()
            for index in range(size):
                visit(header & 15, (*trail, index))
        elif kind == 12:
            previous = 0
            while True:
                header = footer[position]
                position += 1
                if header == 0:
                    return
                delta = header >> 4
                if delta:
                    field = previous + delta
                else:
                    encoded = varint()
                    field = (encoded >> 1) ^ -(encoded & 1)
                previous = field
                visit(header & 15, (*trail, field))
        else:
            raise AssertionError(f'unexpected compact-Thrift test field type {kind}')

    visit(12, ())
    assert position == len(footer)
    assert len(matches) == 1, (field_path, matches)
    encoded = (replacement << 1) ^ (replacement >> 63)
    value = bytearray()
    while encoded >= 128:
        value.append((encoded & 127) | 128)
        encoded >>= 7
    value.append(encoded)
    start, end = matches[0]
    footer = footer[:start] + value + footer[end:]
    path.write_bytes(data[:footer_start] + footer + len(footer).to_bytes(4, 'little') + b'PAR1')


@pytest.mark.parametrize('field_path,replacement', [
    pytest.param((3,), -1, id='negative-file-rows'),
    pytest.param((4, 0, 3), -1, id='negative-group-rows'),
    pytest.param((4, 0, 2), -1, id='negative-group-bytes'),
    pytest.param((4, 0, 1, 0, 3, 5), -1, id='negative-column-values'),
    pytest.param((4, 0, 1, 0, 3, 6), -1, id='negative-column-uncompressed-size'),
    pytest.param((4, 0, 1, 0, 3, 7), -1, id='negative-column-compressed-size'),
    pytest.param((4, 0, 1, 0, 3, 9), -1, id='negative-data-offset'),
    pytest.param((4, 0, 1, 0, 3, 11), -1, id='negative-dictionary-offset'),
    pytest.param((4, 0, 1, 0, 3, 9), 10**9, id='data-offset-beyond-eof'),
    pytest.param((4, 0, 1, 0, 3, 11), 10**9, id='dictionary-offset-beyond-eof'),
    pytest.param((4, 0, 1, 0, 3, 7), 10**9, id='column-size-beyond-eof'),
])
def test_malformed_metadata_fails_without_panic_or_publication(native, tmp_path, field_path, replacement):
    source = FIXTURES / 'contracts/normal'
    oracle = source / 'totalwatsed3.parquet'
    for path in source.glob('H.*.parquet'):
        pq.write_table(pq.read_table(path), tmp_path / path.name, write_statistics=False,
                       row_group_size=1, use_dictionary=True)
    _mutate_footer_integer(tmp_path / 'H.wat.parquet', field_path, replacement)
    inputs_before = {path: path.read_bytes() for path in tmp_path.glob('H.*.parquet')}
    output = tmp_path / 'totalwatsed3.parquet'
    output.write_bytes(b'prior generation')

    # PyO3 PanicException inherits BaseException and must fail this ordinary-error gate.
    with pytest.raises(RuntimeError, match='(?i)(parquet|metadata|row|column|offset)'):
        produce(native, tmp_path, output, oracle=oracle)

    assert output.read_bytes() == b'prior generation'
    assert {path: path.read_bytes() for path in inputs_before} == inputs_before
    assert not list(tmp_path.glob('.*.wepp-*'))

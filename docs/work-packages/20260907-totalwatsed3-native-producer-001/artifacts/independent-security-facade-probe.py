"""Direct public-facade malformed-metadata probe; uses disposable copied inputs."""
from pathlib import Path
import hashlib
import importlib.util
import json
import tempfile

import pyarrow as pa
import pyarrow.parquet as pq
from wepppyo3.wepp_interchange import wepp_interchange_rust
from wepppy.nodb.core.wepp import BaseflowOpts
from wepppy.wepp.interchange.totalwatsed3 import run_totalwatsed3
from wepppy.wepp.interchange._rust_interchange import WeppInterchangeExecutionError

root = Path('/workdir/wepppyo3')
spec = importlib.util.spec_from_file_location('security_footer_fixture_helper', root / 'tests/wepp_interchange/test_totalwatsed3.py')
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
with tempfile.TemporaryDirectory(prefix='totalwatsed-facade-security-') as td:
    folder = Path(td)
    for path in (root / 'tests/fixtures/totalwatsed3/contracts/normal').glob('H.*.parquet'):
        pq.write_table(pq.read_table(path), folder / path.name, write_statistics=False,
                       row_group_size=1, use_dictionary=True)
    helper._mutate_footer_integer(folder / 'H.wat.parquet', (4, 0, 1, 0, 3, 7), -1)
    inputs_before = {path: path.read_bytes() for path in folder.glob('H.*.parquet')}
    output = folder / 'totalwatsed3.parquet'
    output.write_bytes(b'prior generation')
    try:
        run_totalwatsed3(folder, BaseflowOpts(gwstorage=0., bfcoeff=.04, dscoeff=0.))
    except WeppInterchangeExecutionError as exc:
        assert type(exc.__cause__) is RuntimeError, repr(exc.__cause__)
        assert 'negative totalwatsed3 Parquet column' in str(exc.__cause__)
        result = {'exception_class': type(exc).__name__, 'cause_class': type(exc.__cause__).__name__,
                  'cause_message': str(exc.__cause__)}
    else:
        raise AssertionError('malformed metadata did not fail through required native boundary')
    assert output.read_bytes() == b'prior generation'
    assert {path: path.read_bytes() for path in inputs_before} == inputs_before
    assert not list(folder.glob('.*.wepp-*'))
    result.update({'release_sha256': hashlib.sha256(Path(wepp_interchange_rust.__file__).read_bytes()).hexdigest(),
                   'pyarrow_version': pa.__version__, 'input_bytes_preserved': True,
                   'prior_output_preserved': True, 'staging_leaks': False,
                   'facade': str(Path(run_totalwatsed3.__code__.co_filename).resolve())})
receipt = root / 'target/totalwatsed3-evidence/independent-security-facade-probe.json'
receipt.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))

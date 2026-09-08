"""Compare native wrapper products and facades with frozen Python captures."""
import copy
import importlib.util
import json
import logging
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np
import pandas as pd
import pyarrow.parquet as pq
from wepppy.nodb.mods.ash_transport import Ash, ashpost

ROOT=Path('/workdir/wepppyo3/target/ashpost-evidence')
ART=Path(__file__).resolve().parent


def compare(expected, actual, path=''):
    if isinstance(expected,dict):
        assert expected.keys()==actual.keys(),path
        for key in expected: compare(expected[key],actual[key],path+'/'+str(key))
    elif isinstance(expected,list):
        assert len(expected)==len(actual),path
        for i,(left,right) in enumerate(zip(expected,actual)):compare(left,right,path+'/'+str(i))
    elif isinstance(expected,(int,float)):
        assert type(expected) is type(actual),(path,type(expected),type(actual))
        np.testing.assert_allclose(actual,expected,rtol=1e-10,atol=1e-12,err_msg=path)
    else: assert expected==actual,(path,expected,actual)


def verify(fixture, oracle, candidate):
    records={}
    for path in (oracle/'ash/post').glob('*.parquet'):
        left=pq.read_table(path);right=pq.read_table(candidate/'ash/post'/path.name)
        assert left.schema.equals(right.schema,check_metadata=True),path.name
        pd.testing.assert_frame_equal(left.to_pandas(),right.to_pandas(),check_exact=False,rtol=1e-10,atol=1e-12)
        records[path.name]=left.num_rows
    original=json.loads((oracle/'return-periods.json').read_text())
    actual=json.loads((candidate/'return-periods.json').read_text())
    compare(original,actual)
    if fixture!='OR-202':
        ashpost.write_version_manifest(candidate/'ash/post')
        ashpost.generate_ashpost_documentation(str(candidate/'ash/post'))
        for name in ('ashpost_version.json','README.md'):
            assert (oracle/'ash/post'/name).read_bytes()==(candidate/'ash/post'/name).read_bytes(),name
        state=json.loads((ROOT/'source'/fixture/'ash.nodb').read_text())['py/state']
        fake=SimpleNamespace(meta=state['meta'],fire_date=state['fire_date'])
        facade=object.__new__(ashpost.AshPost);facade.wd=str(candidate)
        facade._return_periods,facade._cum_return_periods,facade._burn_class_return_periods=actual
        with patch.object(Ash,'getInstance',return_value=fake):
            facades={name:copy.deepcopy(getattr(facade,name)) for name in (
                'return_periods','cum_return_periods','burn_class_return_periods','hillslope_annuals',
                'watershed_annuals','pw0_stats','ash_out')}
        compare(json.loads((oracle/'facades.json').read_text()),json.loads(json.dumps(facades)))
    return records


if __name__=='__main__':
    results={}
    for fixture,label in [('canine-liar','python-1'),('assisted-weakness','python-3')]:
        candidate=ROOT/'benchmarks'/fixture/'native-dcc822-1'
        results[fixture]=verify(fixture,ROOT/'oracles'/fixture/label,candidate)
    native=ROOT/'benchmarks/OR-202/native-native-1'
    if (native/'measurement.json').exists():
        results['OR-202']=verify('OR-202',ROOT/'benchmarks/OR-202/python-baseline-streamed-1',native)
    (ART/'wrapper-parity.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(results),flush=True)

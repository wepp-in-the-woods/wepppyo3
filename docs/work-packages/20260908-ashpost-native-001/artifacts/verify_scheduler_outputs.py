"""Compare old/new schedulers on identical Forest copies of both small models."""
from pathlib import Path
import hashlib
import importlib.util
import json
import shutil
import sys
import uuid
import pandas as pd
import pyarrow.parquet as pq
from wepppy.nodb.mods.ash_transport import Ash, ash as candidate

ROOT=Path('/workdir/wepppyo3/target/ashpost-evidence')
ART=Path('/workdir/wepppyo3/docs/work-packages/20260908-ashpost-native-001/artifacts')
spec=importlib.util.spec_from_file_location('wepppy.nodb.mods.ash_transport._scheduler_frozen',ROOT/'python-baseline-ash.py')
baseline=importlib.util.module_from_spec(spec);sys.modules[spec.name]=baseline;spec.loader.exec_module(baseline)
def main():
    results={}
    for fixture in ('canine-liar','assisted-weakness'):
        outputs=[]
        for mode,module in [('python',baseline),('native',candidate)]:
            source=ROOT/'source'/fixture
            target=ROOT/'rq-runs'/f'parity-{fixture}-{mode}-{uuid.uuid4().hex[:8]}'
            shutil.copytree(source,target)
            for path in target.glob('*.nodb'):
                data=json.loads(path.read_text());s=data['py/state']
                s['wd']=str(target);s['_run_group']='profile';s['_group_name']='tmp'
                path.write_text(json.dumps(data))
            ash=Ash.getInstance(str(target));state=json.loads((target/'ash.nodb').read_text())['py/state']
            fire=state['fire_date']
            module.Ash.run_ash(ash,f"{fire['month']}/{fire['day']}",state['ini_white_ash_depth_mm'],state['ini_black_ash_depth_mm'])
            outputs.append(target)
        first,second=outputs
        paths=sorted((first/'ash').glob('H*_ash.parquet'))
        assert [p.name for p in paths]==sorted(p.name for p in (second/'ash').glob('H*_ash.parquet'))
        records=[]
        for path in paths:
            other=second/'ash'/path.name
            left=pq.read_table(path);right=pq.read_table(other)
            assert left.schema.equals(right.schema,check_metadata=True),path.name
            pd.testing.assert_frame_equal(left.to_pandas(),right.to_pandas(),check_exact=False,rtol=1e-10,atol=1e-12)
            records.append(dict(name=path.name,rows=left.num_rows,exact_values=left.equals(right),
                python_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),native_sha256=hashlib.sha256(other.read_bytes()).hexdigest()))
        results[fixture]=dict(paths=list(map(str,outputs)),files=records)
    (ART/'scheduler-output-parity.json').write_text(json.dumps(results,indent=2)+'\n')
    print('SCHEDULER_PARITY',json.dumps(results),flush=True)

if __name__ == '__main__': main()

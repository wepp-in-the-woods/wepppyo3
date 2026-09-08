"""Copy frozen runs into disposable real-RQ destinations without source writes."""
import json
from pathlib import Path
import shutil
import uuid

ROOT = Path('/workdir/wepppyo3/target/ashpost-evidence')
token = uuid.uuid4().hex[:10]
runs = []
for fixture in ('OR-202','canine-liar'):
    source = ROOT/'source'/fixture
    name = f'ashpost-{fixture.lower()}-{token}'
    target = ROOT/'rq-runs'/name
    shutil.copytree(source,target)
    for file in target.glob('*.nodb'):
        data=json.loads(file.read_text());state=data['py/state']
        state['wd']=str(target);state['_run_group']='profile';state['_group_name']='tmp'
        file.write_text(json.dumps(data))
    ash=json.loads((target/'ash.nodb').read_text())['py/state']
    fire=ash['fire_date']
    runs.append(dict(fixture=fixture,wd=str(target),runid=f'profile;;tmp;;{name}',
        fire_date=f"{fire['month']}/{fire['day']}",
        white=ash['ini_white_ash_depth_mm'],black=ash['ini_black_ash_depth_mm']))
(ROOT/'rq-runs/selection.json').write_text(json.dumps(runs,indent=2)+'\n')
print(json.dumps(runs),flush=True)

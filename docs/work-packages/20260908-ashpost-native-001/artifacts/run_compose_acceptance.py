"""Start the canonical Compose worker service with a bounded acceptance override."""
from pathlib import Path
import json
import os
import subprocess
import time
import uuid

ART=Path('/workdir/wepppyo3/docs/work-packages/20260908-ashpost-native-001/artifacts')
ROOT=Path('/workdir/wepppyo3/target/ashpost-evidence')
name='ashpost-rq-acceptance-'+uuid.uuid4().hex[:10]
env=dict(os.environ,WCTL_COMPOSE_FILE_EXTRAS=str(ART/'compose-acceptance.yml'))
command=['wctl','run','--no-deps','--name',name,'rq-worker','/opt/venv/bin/python',str(ART/'ashpost_acceptance.py')]
(ART/'compose-command.json').write_text(json.dumps(command,indent=2)+'\n')
samples=[];cg=None;stopped=False
with (ART/'compose-rq.log').open('w') as log:
    process=subprocess.Popen(command,cwd='/workdir/wepppy',env=env,stdout=log,stderr=subprocess.STDOUT)
    while process.poll() is None:
        if cg is None:
            inspect=subprocess.run(['docker','inspect','--format','{{json .State}}',name],capture_output=True,text=True)
            if inspect.returncode==0:
                state=json.loads(inspect.stdout)
                if state['Pid']:
                    path=Path(f"/proc/{state['Pid']}/cgroup").read_text().strip().split(':',2)[2]
                    cg=Path('/sys/fs/cgroup')/path.lstrip('/')
        if cg is not None:
            try:
                current=int((cg/'memory.current').read_text())
                stats=dict(line.split() for line in (cg/'memory.stat').read_text().splitlines())
                samples.append(dict(time=time.time(),memory=current,anon=int(stats['anon']),file=int(stats['file']),pids=int((cg/'pids.current').read_text())))
                if current>=10*2**30 and not stopped:
                    stopped=True;subprocess.run(['docker','kill',name],check=True)
            except FileNotFoundError:
                pass
        time.sleep(.1)
state=json.loads(subprocess.check_output(['docker','inspect','--format','{{json .State}}',name]))
record=dict(name=name,state=state,stopped=stopped,exit=process.returncode,
    restarts=int(subprocess.check_output(['docker','inspect','--format','{{.RestartCount}}',name])),
    external_observed_peak=max((s['memory'] for s in samples),default=0))
(ROOT/'rq-runs/cgroup-samples.json').write_text(json.dumps(samples)+'\n')
(ART/'compose-state.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
if process.returncode:raise SystemExit(process.returncode)

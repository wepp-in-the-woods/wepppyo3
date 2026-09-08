"""Run isolated containers and enforce the candidate 10 GiB stop threshold."""
from pathlib import Path
import json
import subprocess
import sys
import time
import uuid

ART = Path('/workdir/wepppyo3/docs/work-packages/20260908-ashpost-native-001/artifacts')
ROOT = Path('/workdir/wepppyo3/target/ashpost-evidence')
IMAGE = 'sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75'


def run(mode, fixture, label):
    name = 'ashpost-bench-'+uuid.uuid4().hex[:12]
    command = ['docker','create','--name',name,'--memory','12g','--memory-swap','12g',
        '--cpuset-cpus','0-11','--user','1000:993','--network','none',
        '--entrypoint','/opt/venv/bin/python','-e','PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312',
        '-e','PYTHONDONTWRITEBYTECODE=1','-e','WEPPPY_NCPU=12','-e','ASHPOST_REPEAT_CALLS='+str(__import__('os').environ.get('ASHPOST_REPEAT_CALLS','3')),'-v','/workdir/wepppy:/workdir/wepppy:ro',
        '-v','/workdir/wepppyo3:/workdir/wepppyo3:ro','-v',str(ROOT)+':/evidence',IMAGE,
        str(ART/'benchmark.py'),mode,fixture,label]
    evidence = ART/f'benchmark-{fixture}-{mode}-{label}'
    evidence.with_suffix('.command.json').write_text(json.dumps(command,indent=2)+'\n')
    subprocess.run(command,check=True,stdout=subprocess.DEVNULL)
    maximum = 0; stopped = False
    with evidence.with_suffix('.log').open('w') as log:
        process = subprocess.Popen(['docker','start','-a',name],stdout=log,stderr=subprocess.STDOUT)
        cg = None
        while process.poll() is None:
            if cg is None:
                state = json.loads(subprocess.check_output(['docker','inspect','--format','{{json .State}}',name]))
                if state['Pid']:
                    cgroup = Path(f"/proc/{state['Pid']}/cgroup").read_text().strip().split(':',2)[2]
                    cg = Path('/sys/fs/cgroup')/cgroup.lstrip('/')/'memory.current'
            if cg is not None:
                try: current = int(cg.read_text())
                except FileNotFoundError: current = 0
                maximum = max(maximum,current)
                if mode.startswith('native') and current >= 10*2**30:
                    stopped = True
                    subprocess.run(['docker','kill',name],check=True,stdout=subprocess.DEVNULL)
            time.sleep(.01)
        status = json.loads(subprocess.check_output(['docker','inspect','--format','{{json .State}}',name]))
        restarts = int(subprocess.check_output(['docker','inspect','--format','{{.RestartCount}}',name]))
    record = {'mode':mode,'fixture':fixture,'label':label,'state':status,'restarts':restarts,
              'external_observed_peak':maximum,'threshold_stop':stopped,'command_exit':process.returncode}
    evidence.with_suffix('.state.json').write_text(json.dumps(record,indent=2)+'\n')
    subprocess.run(['docker','rm',name],check=True,stdout=subprocess.DEVNULL)
    print(json.dumps(record),flush=True)
    if process.returncode: raise SystemExit(process.returncode)


if __name__ == '__main__':
    if len(sys.argv)>1:
        run(*sys.argv[1:])
    else:
        token = uuid.uuid4().hex[:6]
        for fixture in ('canine-liar','assisted-weakness'):
            for repetition in range(1,6):
                for mode in ('python','native'):
                    run(mode,fixture,f'{token}-{repetition}')

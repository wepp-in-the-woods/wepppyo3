from pathlib import Path
import json,subprocess,time
root=Path('/workdir/wepppyo3');out=root/'target/totalwatsed3-evidence'
records=[];commands=[]
for run in ('incommensurate-stickball','throbbing-sylvan'):
 for repetition in range(1,6):
  for mode in ('python','native'):
   cmd=['docker','run','--rm','--memory','12g','--memory-swap','12g','--cpuset-cpus','0-11','--user','1000:993','--network','none','--entrypoint','/opt/venv/bin/python','-e','PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312','-e','WEPPPY_NCPU=12','-e','PYTHONDONTWRITEBYTECODE=1','-v','/workdir/wepppy:/workdir/wepppy:ro','-v','/workdir/wepppyo3:/workdir/wepppyo3:ro','-v',str(out)+':/evidence','sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75','/evidence/review_benchmark.py',mode,run,str(repetition)]
   commands.append(cmd);(out/'review-benchmark-commands.json').write_text(json.dumps(commands,indent=2)+'\n')
   done=subprocess.run(cmd,capture_output=True,text=True)
   if done.returncode:
    (out/'benchmark-failure.log').write_text(done.stdout+'\n'+done.stderr);raise RuntimeError(f'benchmark failed: {mode} {run} {repetition}')
   record=json.loads(done.stdout.strip().splitlines()[-1]);records.append(record)
   (out/'review-benchmark-measurements.json').write_text(json.dumps(records,indent=2)+'\n')
   print(mode,run,repetition,round(record['wall_seconds'],3),round(record['memory_peak_bytes']/2**20,1),'MiB',flush=True)
   if mode=='native' and record['memory_peak_bytes']>10*2**30:raise RuntimeError('native memory stop condition')

from pathlib import Path
import json,subprocess,time
root=Path('/workdir/wepppyo3');out=root/'target/totalwatsed3-evidence';records=[];commands=[]
for count in (73,146,293,586,2344,5860):
 while not (out/'scales'/str(count)/'manifest.json').exists():time.sleep(2)
 cmd=['docker','run','--rm','--memory','12g','--memory-swap','12g','--cpuset-cpus','0-11','--user','1000:993','--network','none','--entrypoint','/opt/venv/bin/python','-e','PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312','-e','WEPPPY_NCPU=12','-e','PYTHONDONTWRITEBYTECODE=1','-v','/workdir/wepppy:/workdir/wepppy:ro','-v','/workdir/wepppyo3:/workdir/wepppyo3:ro','-v',str(out)+':/evidence','sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75','/evidence/measure_scale.py',str(count)]
 commands.append(cmd);(out/'scale-commands.json').write_text(json.dumps(commands,indent=2)+'\n')
 done=subprocess.run(cmd,capture_output=True,text=True)
 if done.returncode:
  (out/f'scale-failure-{count}.log').write_text(done.stdout+'\n'+done.stderr);raise RuntimeError(f'scale {count} failed')
 r=json.loads(done.stdout.strip().splitlines()[-1]);records.append(r);(out/'scale-measurements.json').write_text(json.dumps(records,indent=2)+'\n');print(count,round(r['observations'][0]['wall_seconds'],3),r['memory_peak_bytes'],flush=True)
 if count==5860:
  reference=next(v for v in records if v['hillslopes']==586)
  ratio=r['memory_peak_bytes']/reference['memory_peak_bytes'];print('PEAK_RATIO',ratio,flush=True)
  assert ratio<=1.5, 'package scaling stop condition'

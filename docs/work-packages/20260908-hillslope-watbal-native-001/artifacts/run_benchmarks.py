from pathlib import Path
import json,subprocess,statistics
art=Path('/workdir/wepppyo3/docs/work-packages/20260908-hillslope-watbal-native-001/artifacts')
base=json.loads((art/'python-oracle-commands.json').read_text())[0][:-2]
records=[];commands=[]
for repetition in range(1,6):
 for mode in ('python','native'):
  cmd=base+[str(art/'benchmark.py'),mode,str(repetition)];commands.append(cmd)
  (art/'benchmark-commands.json').write_text(json.dumps(commands,indent=2)+'\n')
  result=subprocess.run(cmd,capture_output=True,text=True)
  (art/f'benchmark-{mode}-{repetition}.log').write_text(result.stdout+result.stderr)
  if result.returncode:raise SystemExit(f'benchmark failed: {mode} {repetition} {result.returncode}')
  record=json.loads(result.stdout.strip().splitlines()[-1]);records.append(record)
  (art/'benchmark-measurements.json').write_text(json.dumps(records,indent=2)+'\n')
  print(mode,repetition,record['wall_seconds'],record['memory_peak'],flush=True)
  if mode=='native' and record['memory_peak']>=10*2**30:raise SystemExit('STOP: native memory threshold')
p=statistics.median(r['wall_seconds'] for r in records if r['mode']=='python');n=statistics.median(r['wall_seconds'] for r in records if r['mode']=='native')
result={'python_median':p,'native_median':n,'ratio':n/p,'timing_passed':n<=1.05*p and max(r['wall_seconds'] for r in records if r['mode']=='native')<=1.15*p}
(art/'benchmark-summary.json').write_text(json.dumps(result,indent=2)+'\n');print(result,flush=True)
if not result['timing_passed']:raise SystemExit('STOP: native timing gate exceeded')

"""One isolated warm-cache baseline/acceptance observation."""
from pathlib import Path
from types import SimpleNamespace
import importlib.util,json,os,resource,sys,tempfile,time
import pyarrow.parquet as pq
# Both modes import the exact historical producer for comparable baseline overhead.
import types
old=types.ModuleType('wepppy.wepp.interchange._benchmark_previous')
sys.modules[old.__name__]=old
exec(compile(Path('/evidence/review-previous-producer.py').read_text(), 'previous_totalwatsed3.py', 'exec'), old.__dict__)
run_totalwatsed3=old.run_totalwatsed3
root=Path('/workdir/wepppyo3');mode,run,number=sys.argv[1:]
from wepppyo3.wepp_interchange import wepp_interchange_rust as native
source=root/'tests/fixtures/totalwatsed3'/run
oracle=source/'totalwatsed3.parquet';pandas_metadata=pq.read_schema(oracle).metadata[b'pandas'].decode()
folder=Path(tempfile.mkdtemp(prefix=f'bench-{mode}-{run}-{number}-',dir='/evidence'))
for p in source.glob('H.*.parquet'):(folder/p.name).symlink_to(p)
opts=SimpleNamespace(gwstorage=0.,bfcoeff=.04,dscoeff=0.)
def invoke(output):
 if mode=='python':
  run_totalwatsed3(folder,opts,ash_dir=folder/'no-ash')
  (folder/'totalwatsed3.parquet').rename(output)
 else:
  native.totalwatsed3_to_parquet(str(folder/'H.pass.parquet'),str(folder/'H.wat.parquet'),str(output),0.,.04,0.,1,2,soil_path=str(folder/'H.soil.parquet'),element_path=str(folder/'H.element.parquet'),pandas_metadata=pandas_metadata)
initial=int(Path('/sys/fs/cgroup/memory.current').read_text())
invoke(folder/'warmup.parquet')
baseline=int(Path('/sys/fs/cgroup/memory.current').read_text())
usage0=resource.getrusage(resource.RUSAGE_SELF);start=time.perf_counter()
output=folder/'measured.parquet';invoke(output)
wall=time.perf_counter()-start;usage1=resource.getrusage(resource.RUSAGE_SELF)
peak=int(Path('/sys/fs/cgroup/memory.peak').read_text());after=int(Path('/sys/fs/cgroup/memory.current').read_text())
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py')
compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
parity=compare.compare(oracle,output)
record=dict(mode=mode,run=run,repetition=int(number),output=str(output),wall_seconds=wall,user_seconds=usage1.ru_utime-usage0.ru_utime,system_seconds=usage1.ru_stime-usage0.ru_stime,initial_baseline_bytes=initial,precall_baseline_bytes=baseline,postcall_baseline_bytes=after,memory_peak_bytes=peak,incremental_peak_upper_bound_bytes=peak-initial,peak_includes_warmup=True,peak_before_parity=True,cpu_affinity=sorted(os.sched_getaffinity(0)),memory_max=Path('/sys/fs/cgroup/memory.max').read_text().strip(),memory_events=Path('/sys/fs/cgroup/memory.events').read_text(),page_cache='warm: one untimed execution in each isolated container; shared host cache not dropped',input_bytes=sum(p.stat().st_size for p in source.glob('H.*.parquet')),output_bytes=output.stat().st_size,parity=parity)
(folder/'measurement.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
assert parity['passed']

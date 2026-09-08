from pathlib import Path
from types import SimpleNamespace
import importlib.util,json,os,resource,sys,tempfile,time
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq
from wepppy.wepp.interchange.totalwatsed3 import run_totalwatsed3
root=Path('/workdir/wepppyo3');count=int(sys.argv[1]);source=Path('/evidence/scales')/str(count)
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so');native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
reference=root/'tests/fixtures/totalwatsed3/throbbing-sylvan/totalwatsed3.parquet';metadata=pq.read_schema(reference).metadata[b'pandas'].decode()
folder=Path(tempfile.mkdtemp(prefix=f'scale-{count}-',dir='/evidence'))
for p in source.glob('H.*.parquet'):(folder/p.name).symlink_to(p)
def memstat():return {k:int(v) for k,v in (line.split() for line in Path('/sys/fs/cgroup/memory.stat').read_text().splitlines()) if k in ('anon','file','kernel')}
def call(output):
 return native.totalwatsed3_to_parquet(str(folder/'H.pass.parquet'),str(folder/'H.wat.parquet'),str(output),0.,.04,0.,1,2,soil_path=str(folder/'H.soil.parquet'),element_path=str(folder/'H.element.parquet'),pandas_metadata=metadata)
initial=int(Path('/sys/fs/cgroup/memory.current').read_text());call(folder/'warmup.parquet');records=[]
for i in range(3 if count==586 else 1):
 before=int(Path('/sys/fs/cgroup/memory.current').read_text());ms=memstat();r0=resource.getrusage(resource.RUSAGE_SELF);start=time.perf_counter();output=folder/f'native-{i}.parquet';summary=call(output);wall=time.perf_counter()-start;r1=resource.getrusage(resource.RUSAGE_SELF)
 records.append(dict(execution=i,wall_seconds=wall,user_seconds=r1.ru_utime-r0.ru_utime,system_seconds=r1.ru_stime-r0.ru_stime,baseline_before_bytes=before,baseline_after_bytes=int(Path('/sys/fs/cgroup/memory.current').read_text()),stat_before=ms,stat_after=memstat(),summary=summary))
peak=int(Path('/sys/fs/cgroup/memory.peak').read_text())
if count<586:
 run_totalwatsed3(folder,SimpleNamespace(gwstorage=0.,bfcoeff=.04,dscoeff=0.),ash_dir=folder/'no-ash');oracle=folder/'totalwatsed3.parquet';kind='Python producer on exact subset inputs'
elif count==586:oracle=reference;kind='approved original oracle'
else:
 assert count%586==0;factor=count//586;t=pq.read_table(reference)
 volumes=['runvol','sbrunv','tdet','tdep',*[f'seddep_{i}' for i in range(1,6)],'sed_del','Area','P','RM','Q','Dp','latqcc','QOFE','Ep','Es','Er']
 for name in volumes:
  col=t.schema.get_field_index(name);t=t.set_column(col,t.schema.field(col),pc.multiply(t[name],pa.scalar(float(factor))))
 oracle=folder/'scaled-oracle.parquet';pq.write_table(t,oracle);kind=f'approved oracle with volume/mass/area fields multiplied by {factor}; depths/concentrations/dates unchanged'
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py');compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
parities=[compare.compare(oracle,folder/f'native-{i}.parquet') for i in range(len(records))]
result=dict(hillslopes=count,source=str(source),folder=str(folder),input_bytes=sum(p.stat().st_size for p in source.glob('H.*.parquet')),initial_baseline_bytes=initial,memory_peak_bytes=peak,incremental_peak_upper_bound_bytes=peak-initial,peak_includes_warmup=True,peak_before_python_or_comparison=True,observations=records,oracle_method=kind,parity=parities,memory_events=Path('/sys/fs/cgroup/memory.events').read_text(),cpu_affinity=sorted(os.sched_getaffinity(0)),memory_max=Path('/sys/fs/cgroup/memory.max').read_text().strip(),temporary_files=[p.name for p in folder.iterdir() if p.name.endswith('.tmp') or '.tmp.' in p.name])
(folder/'measurement.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
assert all(r['passed'] for r in parities)
assert peak<9*2**30

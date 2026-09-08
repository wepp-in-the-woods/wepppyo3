"""Reuse predecessor scale inputs; verify repeated and sequential native outputs."""
from pathlib import Path
import importlib.util,json,os,time
import numpy as np
import pyarrow.parquet as pq
root=Path('/workdir/wepppyo3');source=root/'target/totalwatsed3-evidence/scales/5860';out=Path('/evidence/scale');out.mkdir(exist_ok=True)
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so');native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
reference=pq.read_table('/evidence/throbbing-sylvan/summary.parquet');metadata=reference.schema.metadata[b'pandas'].decode()
ids=native.hillslope_watbal_wepp_ids(str(source/'H.wat.parquet'));mapping={i:i*10+1 for i in ids};assert len(ids)==5860
records=[]
def mem():
 return {'current':int(Path('/sys/fs/cgroup/memory.current').read_text()),'peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'stat':{k:int(v) for k,v in (line.split() for line in Path('/sys/fs/cgroup/memory.stat').read_text().splitlines()) if k in ('anon','file','kernel')}}
def call(label):
 before=mem();start=time.perf_counter();output=out/(label+'.parquet')
 summary=native.hillslope_watbal_to_parquet(str(source/'H.wat.parquet'),str(output),mapping,pandas_metadata=metadata)
 result={'label':label,'before':before,'after':mem(),'wall_seconds':time.perf_counter()-start,'summary':summary}
 assert result['after']['peak']<9*2**30
 actual=pq.read_table(output)
 assert actual.schema.equals(reference.schema,check_metadata=True)
 base=reference.to_pandas();frame=actual.to_pandas()
 assert len(frame)==10*len(base)
 for replica in range(10):
  subset=frame.iloc[replica*len(base):(replica+1)*len(base)].copy()
  subset['TopazID']-=replica*587*10
  np.testing.assert_allclose(subset.to_numpy(),base.to_numpy(),rtol=1e-10,atol=1e-12)
 result['parity']=True;records.append(result)
 (out/'results.json').write_text(json.dumps(records,indent=2)+'\n');print(json.dumps(result),flush=True)
for i in range(3):call('repeat-'+str(i))
assert records[-1]['after']['peak']<4*2**30
# The same process retains previous allocations, then executes both native producers consecutively.
total_metadata=pq.read_schema(root/'tests/fixtures/totalwatsed3/throbbing-sylvan/totalwatsed3.parquet').metadata[b'pandas'].decode()
start=time.perf_counter()
native.totalwatsed3_to_parquet(str(source/'H.pass.parquet'),str(source/'H.wat.parquet'),str(out/'totalwatsed3.parquet'),0.,.04,0.,1,2,soil_path=str(source/'H.soil.parquet'),element_path=str(source/'H.element.parquet'),pandas_metadata=total_metadata)
print('totalwatsed3_seconds',time.perf_counter()-start,flush=True)
call('sequential')
assert not [p for p in out.iterdir() if '.stage.' in p.name or '.backup.' in p.name]
(out/'memory-events.txt').write_text(Path('/sys/fs/cgroup/memory.events').read_text())

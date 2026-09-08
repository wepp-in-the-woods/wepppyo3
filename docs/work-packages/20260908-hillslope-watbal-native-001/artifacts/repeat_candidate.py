"""Measure post-call baseline without comparator allocations or distinct output files."""
from pathlib import Path
import json,time
from wepppyo3.wepp_interchange import hillslope_watbal_to_parquet,hillslope_watbal_wepp_ids
source=Path('/workdir/wepppyo3/target/totalwatsed3-evidence/scales/5860/H.wat.parquet');out=Path('/evidence/repeat-fixed.parquet')
mapping={i:i*10+1 for i in hillslope_watbal_wepp_ids(str(source))}
def memory():
 return {'current':int(Path('/sys/fs/cgroup/memory.current').read_text()),'peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'stat':{k:int(v) for k,v in (line.split() for line in Path('/sys/fs/cgroup/memory.stat').read_text().splitlines()) if k in ('anon','file','kernel')}}
records=[]
for i in range(4):
 before=memory();start=time.perf_counter();hillslope_watbal_to_parquet(str(source),str(out),mapping)
 row={'iteration':i,'warmup':i==0,'before':before,'after':memory(),'seconds':time.perf_counter()-start};records.append(row)
 Path('/evidence/repeat-fixed.json').write_text(json.dumps(records,indent=2)+'\n');print(json.dumps(row),flush=True)
 assert row['after']['peak']<4*2**30
assert not list(out.parent.glob('.repeat-fixed.parquet.wepp-*'))
# A bounded allowance covers allocator/page-cache settling; raw anon data remains evidence.
assert records[-1]['after']['stat']['anon']-records[1]['after']['stat']['anon']<32*2**20

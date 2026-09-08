"""Compare a built candidate with immutable preimplementation oracles."""
from pathlib import Path
import importlib.util,json,time,resource
import numpy as np
import pyarrow.parquet as pq
root=Path('/workdir/wepppyo3');out=Path('/evidence')
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so')
native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
results=[]
for run in ('decimal-pleasing','incommensurate-stickball','throbbing-sylvan'):
 folder=out/run; source=folder/'wepp/output/interchange/H.wat.parquet'
 info=json.loads((folder/'oracle.json').read_text());mapping={int(k):v for k,v in info['mapping'].items()}
 oracle=pq.read_table(folder/'summary.parquet');output=folder/'candidate.parquet'
 assert native.hillslope_watbal_wepp_ids(str(source))==sorted(mapping)
 start=time.perf_counter()
 summary=native.hillslope_watbal_to_parquet(str(source),str(output),mapping,pandas_metadata=oracle.schema.metadata[b'pandas'].decode())
 elapsed=time.perf_counter()-start
 actual=pq.read_table(output)
 errors=[]
 if not actual.schema.equals(oracle.schema,check_metadata=True):errors.append('schema/metadata')
 if actual.num_rows!=oracle.num_rows:errors.append('rows')
 if not errors:
  for c in oracle.column_names:
   a=actual[c].to_numpy();b=oracle[c].to_numpy()
   if not np.allclose(a,b,rtol=1e-10,atol=1e-12,equal_nan=True):errors.append({'column':c,'max_abs':float(np.max(np.abs(a-b)))})
 result={'run':run,'summary':summary,'wall_seconds':elapsed,'memory_peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'errors':errors}
 results.append(result); print(json.dumps(result),flush=True)
 (out/'candidate-parity.json').write_text(json.dumps(results,indent=2)+'\n')
 if errors:raise SystemExit('STOP: output semantics differ')

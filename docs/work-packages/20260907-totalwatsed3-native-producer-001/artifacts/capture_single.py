from pathlib import Path
from types import SimpleNamespace
import json, shutil, tempfile, time
import numpy as np
import pyarrow.parquet as pq
from wepppy.wepp.interchange.totalwatsed3 import run_totalwatsed3
results=[]
for run in ("decimal-pleasing",):
 src=Path("/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange")
 out=Path(tempfile.mkdtemp(prefix=run+"-",dir="/evidence"))
 for p in src.glob("H.*.parquet"): (out/p.name).symlink_to(p)
 oracle=pq.read_table(src/"totalwatsed3.parquet")
 r=oracle["Reservoir Volume"].to_numpy(); b=oracle["Baseflow"].to_numpy(); l=oracle["Aquifer losses"].to_numpy()
 opts=SimpleNamespace(gwstorage=float(r[0]),bfcoeff=float(b[1]/r[1]),dscoeff=float(l[np.flatnonzero(r[:-1])[0]]/r[np.flatnonzero(r[:-1])[0]]))
 print(run,vars(opts),flush=True)
 before=int(Path("/sys/fs/cgroup/memory.current").read_text())
 started=time.monotonic()
 run_totalwatsed3(out,opts,ash_dir=out/"no-ash")
 elapsed=time.monotonic()-started
 actual=pq.read_table(out/"totalwatsed3.parquet")
 diff=[]
 for col in oracle.column_names:
  a=actual[col]; o=oracle[col]
  if a.type!=o.type: diff.append({"column":col,"type_mismatch":True});continue
  x=a.to_numpy(); y=o.to_numpy()
  if not np.array_equal(a.is_null().to_numpy(),o.is_null().to_numpy()): diff.append({"column":col,"null_mismatch":True})
  ok=np.isclose(x,y,rtol=1e-10,atol=1e-12,equal_nan=True)
  if not ok.all():
   idx=int(np.flatnonzero(~ok)[0])
   diff.append({"column":col,"mismatches":int((~ok).sum()),"max_abs":float(np.nanmax(np.abs(x-y))),"first":idx,"actual":float(x[idx]),"oracle":float(y[idx])})
 result=dict(missing_from_frozen_oracle=[c for c in actual.column_names if c not in oracle.column_names],extra_in_frozen_oracle=[c for c in oracle.column_names if c not in actual.column_names],run=run,baseflow=vars(opts),output=str(out),seconds=elapsed,baseline=before,memory_peak=int(Path("/sys/fs/cgroup/memory.peak").read_text()),schema_equal=actual.schema.equals(oracle.schema,check_metadata=True),rows=actual.num_rows,differences=diff)
 results.append(result)
 Path("/evidence/single-ofe-baseline.json").write_text(json.dumps(results,indent=2)+"\n")
 print(json.dumps(result),flush=True)
 if diff: break

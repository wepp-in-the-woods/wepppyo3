"""Freeze unchanged report outputs in a disposable directory; source mounts read-only."""
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import hashlib,json,os,resource,sys,time
import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppy.wepp.reports.hillslope_watbal import HillslopeWatbalReport
os.umask(0o022)
run=sys.argv[1]
root=Path('/workdir/wepppyo3')
source=(Path('/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange') if run=='decimal-pleasing' else root/'tests/fixtures/totalwatsed3'/run)/'H.wat.parquet'
folder=Path('/evidence')/run
folder.mkdir()
dest=folder/'wepp/output/interchange/H.wat.parquet';dest.parent.mkdir(parents=True);dest.symlink_to(source)
ids=set()
for batch in pq.ParquetFile(source).iter_batches(columns=['wepp_id'],batch_size=8192): ids.update(batch.column(0).to_pylist())
# Deterministic injective translation exercises the report facade without source NoDb mutation.
mapping={int(i):int(i)*10+1 for i in ids}
translator=SimpleNamespace(top=lambda wepp:mapping[wepp])
before=int(Path('/sys/fs/cgroup/memory.current').read_text()); usage=resource.getrusage(resource.RUSAGE_SELF);start=time.perf_counter()
with patch.object(Watershed,'getInstance',return_value=SimpleNamespace(translator_factory=lambda:translator)):
 report=HillslopeWatbalReport(folder)
elapsed=time.perf_counter()-start
report._per_hill_year.to_parquet(folder/'summary.parquet',index=False)
result={'evidence':'confirmed','run':run,'source_sha256':hashlib.file_digest(source.open('rb'),'sha256').hexdigest(),'mapping':mapping,'header':report.header,'units':report.units_d,'avg_annual_header':report.avg_annual_header,'avg_annual_units':report.avg_annual_units,'yearly_header':report.yearly_header,'yearly_units':report.yearly_units,'avg_annual':[dict(r.row) for r in report.avg_annual_iter()],'yearly':[dict(r.row) for r in report.yearly_iter()],'summary_schema':str(pq.read_schema(folder/'summary.parquet')),'summary_sha256':hashlib.file_digest((folder/'summary.parquet').open('rb'),'sha256').hexdigest(),'wall_seconds':elapsed,'memory_before':before,'memory_peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'memory_events':Path('/sys/fs/cgroup/memory.events').read_text(),'uid':os.getuid(),'gid':os.getgid(),'cpu_affinity':sorted(os.sched_getaffinity(0))}
(folder/'oracle.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['run','source_sha256','wall_seconds','memory_peak']}),flush=True)

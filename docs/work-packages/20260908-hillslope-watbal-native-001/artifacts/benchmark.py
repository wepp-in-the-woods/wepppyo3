"""Isolated warm-cache standalone measurement against the unchanged Python producer."""
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import importlib.util,json,os,resource,sys,time
import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppy.wepp.reports.hillslope_watbal import HillslopeWatbalReport
os.umask(0o022)
root=Path('/workdir/wepppyo3'); mode, repetition=sys.argv[1:]
folder=Path('/evidence/throbbing-sylvan');source=folder/'wepp/output/interchange/H.wat.parquet'
info=json.loads((folder/'oracle.json').read_text());mapping={int(k):v for k,v in info['mapping'].items()}
metadata=pq.read_schema(folder/'summary.parquet').metadata[b'pandas'].decode()
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so');native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
baseline_spec=importlib.util.spec_from_file_location('wepppy.wepp.reports._hillslope_watbal_baseline',root/'target/hillslope-watbal-evidence/python-baseline-hillslope_watbal.py')
baseline=importlib.util.module_from_spec(baseline_spec);baseline_spec.loader.exec_module(baseline)
report=baseline.HillslopeWatbalReport.__new__(baseline.HillslopeWatbalReport);report.wd=folder;report._output_scope='baseline'
translator=SimpleNamespace(top=lambda wepp:mapping[wepp])
def invoke(label):
 output=folder/f'bench-{mode}-{repetition}-{label}.parquet'
 if mode=='python':
  with patch.object(Watershed,'getInstance',return_value=SimpleNamespace(translator_factory=lambda:translator)):
   frame=report._build_summary()
  frame.to_parquet(output,index=False)
 else:
  selected={i:mapping[i] for i in native.hillslope_watbal_wepp_ids(str(source))}
  native.hillslope_watbal_to_parquet(str(source),str(output),selected,pandas_metadata=metadata)
 return output
initial=int(Path('/sys/fs/cgroup/memory.current').read_text());invoke('warmup')
before=int(Path('/sys/fs/cgroup/memory.current').read_text());u=resource.getrusage(resource.RUSAGE_SELF);start=time.perf_counter()
output=invoke('measured');wall=time.perf_counter()-start;v=resource.getrusage(resource.RUSAGE_SELF)
result={'mode':mode,'repetition':int(repetition),'wall_seconds':wall,'user_seconds':v.ru_utime-u.ru_utime,'system_seconds':v.ru_stime-u.ru_stime,'initial_baseline':initial,'precall_baseline':before,'postcall_baseline':int(Path('/sys/fs/cgroup/memory.current').read_text()),'memory_peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'memory_max':Path('/sys/fs/cgroup/memory.max').read_text(),'memory_events':Path('/sys/fs/cgroup/memory.events').read_text(),'cpu_affinity':sorted(os.sched_getaffinity(0)),'input_bytes':source.stat().st_size,'output_bytes':output.stat().st_size,'cache_condition':'one untimed warmup per isolated container; shared host cache not dropped','native_includes_id_discovery':True}
print(json.dumps(result),flush=True)

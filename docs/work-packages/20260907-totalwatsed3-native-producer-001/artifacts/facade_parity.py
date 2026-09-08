from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import importlib, importlib.util, json, tempfile
import pyarrow.parquet as pq
root=Path('/workdir/wepppyo3'); fixtures=root/'tests/fixtures/totalwatsed3'
facade=importlib.import_module('wepppy.wepp.interchange.totalwatsed3')
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py');compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
output=Path(tempfile.mkdtemp(prefix='facade-',dir=root/'target/totalwatsed3-evidence'));results=[]
cases=[(p.name,p,p/'totalwatsed3.parquet',json.loads((p/'case.json').read_text())) for p in sorted((fixtures/'contracts').iterdir())]
for name in ('decimal-pleasing','incommensurate-stickball','throbbing-sylvan'):
 source=fixtures/name
 if name=='decimal-pleasing':source=Path('/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange')
 cases.append((name,source,fixtures/name/'totalwatsed3.parquet',dict(gwstorage=0.,bfcoeff=.04,dscoeff=0.,wepp_ids=None,ash_inputs=[])))
for name,source,oracle,options in cases:
 folder=output/name;folder.mkdir()
 for file in source.glob('H.*.parquet'):(folder/file.name).symlink_to(file)
 areas={};types={};densities={}
 for path,area,kind,density in options['ash_inputs']:
  id=int(Path(path).stem[1:].split('_')[0]);areas[id]=area;types[id]=kind;densities[id]=density
 with patch.object(facade,'_build_ash_type_and_density_lookup',return_value=(types,densities)):
  actual=facade.run_totalwatsed3(folder,SimpleNamespace(**{k:options[k] for k in ('gwstorage','bfcoeff','dscoeff')}),wepp_ids=options['wepp_ids'],ash_dir=source/'ash',ash_area_lookup=areas)
 result=compare.compare(oracle,actual);results.append(dict(case=name,**result));assert result['passed'],(name,result)
(root/'target/totalwatsed3-evidence/facade-parity-results.json').write_text(json.dumps(results,indent=2)+'\n');print('Public facade parity passed:',len(results))

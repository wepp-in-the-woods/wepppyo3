# Run from the published image without source mounts.
import hashlib, runpy
preflight=runpy.run_path('/workdir/wepppy/docker/wepppyo3-interchange-preflight.py')
so, digest=preflight['validate_wepppyo3_interchange']()
print('Verified native release:', so, digest)
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import importlib, importlib.util, json, tempfile
import pyarrow.parquet as pq
root=Path('/workdir/wepppyo3'); fixtures=root/'tests/fixtures/totalwatsed3'
manifest=json.loads((root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/release-integration-manifest.json').read_text())
for expected in manifest['files']:
    actual_path=Path(expected['path'])
    assert hashlib.sha256(actual_path.read_bytes()).hexdigest()==expected['sha256'],actual_path
print('Verified all paired implementation file hashes from the native release manifest.')
facade=importlib.import_module('wepppy.wepp.interchange.totalwatsed3')
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py');compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
output=Path(tempfile.mkdtemp(prefix='facade-'));results=[]
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
print(json.dumps({'parity_cases': results, 'passed': len(results)}, indent=2))

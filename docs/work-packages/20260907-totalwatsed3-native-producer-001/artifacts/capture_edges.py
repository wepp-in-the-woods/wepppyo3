from pathlib import Path
from types import SimpleNamespace
import importlib.util, json, shutil, tempfile
import pyarrow as pa
import pyarrow.parquet as pq
from wepppy.wepp.interchange.totalwatsed3 import run_totalwatsed3
root=Path('/workdir/wepppyo3');base=Path(tempfile.mkdtemp(prefix='edge-cases-',dir='/evidence'))
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so');native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py');compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
results=[]
for case in ('null_pass_class','infinite_interception','absent_soil_element','null_soil_element','null_single_ofe_lateral','empty_pass'):
 f=base/case;f.mkdir()
 for p in (root/'tests/fixtures/totalwatsed3/contracts/normal').glob('H.*.parquet'):shutil.copyfile(p,f/p.name)
 def rewrite(name,fn):
  path=f/name;t=pq.read_table(path);pq.write_table(fn(t),path,row_group_size=5)
 if case=='null_pass_class':rewrite('H.pass.parquet',lambda t:t.set_column(t.schema.get_field_index('sedcon_1'),'sedcon_1',pa.nulls(t.num_rows,pa.float64())))
 if case=='infinite_interception':rewrite('H.wat.parquet',lambda t:t.set_column(t.schema.get_field_index('Interception'),'Interception',pa.array([float('inf') if d==1 else -float('inf') if d==2 else float('nan') for d in t['julian'].to_pylist()])))
 if case=='absent_soil_element':
  (f/'H.soil.parquet').unlink();(f/'H.element.parquet').unlink()
 if case=='null_soil_element':
  rewrite('H.soil.parquet',lambda t:t.set_column(t.schema.get_field_index('TSMF'),'TSMF',pa.nulls(t.num_rows,pa.float64())))
  for col in ('QRain','QSnow'):rewrite('H.element.parquet',lambda t:t.set_column(t.schema.get_field_index(col),col,pa.nulls(t.num_rows,pa.float64())))
 if case=='null_single_ofe_lateral':
  rewrite('H.wat.parquet',lambda t:t.set_column(t.schema.get_field_index('ofe_id'),'ofe_id',pa.array([1]*t.num_rows,pa.int32())))
  rewrite('H.wat.parquet',lambda t:t.set_column(t.schema.get_field_index('latqcc'),'latqcc',pa.nulls(t.num_rows,pa.float64())))
 if case=='empty_pass':rewrite('H.pass.parquet',lambda t:t.slice(0,0))
 run_totalwatsed3(f,SimpleNamespace(gwstorage=1.,bfcoeff=.04,dscoeff=.02),ash_dir=f/'no-ash')
 oracle=f/'totalwatsed3.parquet';schema=pq.read_schema(oracle)
 native.totalwatsed3_to_parquet(str(f/'H.pass.parquet'),str(f/'H.wat.parquet'),str(f/'native.parquet'),1.,.04,.02,1,2,soil_path=str(f/'H.soil.parquet') if (f/'H.soil.parquet').exists() else None,element_path=str(f/'H.element.parquet') if (f/'H.element.parquet').exists() else None,pandas_metadata=schema.metadata[b'pandas'].decode())
 result=compare.compare(oracle,f/'native.parquet');result['case']=case;results.append(result);print(json.dumps(result),flush=True)
 (f/'case.json').write_text(json.dumps(dict(wepp_ids=None,ash_inputs=[],gwstorage=1.,bfcoeff=.04,dscoeff=.02),indent=2)+'\n')
(base/'results.json').write_text(json.dumps(results,indent=2)+'\n');print('EDGE_ROOT',base,flush=True)
assert all(r['passed'] for r in results)

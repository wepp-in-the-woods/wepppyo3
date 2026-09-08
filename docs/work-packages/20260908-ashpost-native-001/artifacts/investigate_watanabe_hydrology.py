"""Compare saved AshPost hydrology with isolated upstream recomputations."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace

import numpy as np
import pyarrow.parquet as pq

ROOT=Path('/workdir/wepppyo3/target/ashpost-evidence')
SOURCE=ROOT/'source/assisted-weakness'
OUT=ROOT/'hydrology-investigation'
OUT.mkdir(exist_ok=False)
for name in ['H.pass.parquet','H.wat.parquet','H.soil.parquet','H.element.parquet']:
    os.link(SOURCE/'wepp/output/interchange'/name,OUT/name)
name='wepppy.wepp.interchange._hydrology_investigation'
spec=importlib.util.spec_from_file_location(name,ROOT/'python-pre-native-totalwatsed3.py')
module=importlib.util.module_from_spec(spec);sys.modules[name]=module;spec.loader.exec_module(module)
opts=json.loads((SOURCE/'wepp.nodb').read_text())['py/state']['baseflow_opts']
baseflow=SimpleNamespace(**{key:opts[key] for key in ['gwstorage','bfcoeff','dscoeff']})
module.run_totalwatsed3(OUT,baseflow,ash_dir=OUT/'absent-ash')
os.rename(OUT/'totalwatsed3.parquet',OUT/'python-totalwatsed3.parquet')
sys.path.insert(0,'/workdir/wepppyo3/release/linux/py312')
from wepppyo3.wepp_interchange import totalwatsed3_to_parquet
result=totalwatsed3_to_parquet(str(OUT/'H.pass.parquet'),str(OUT/'H.wat.parquet'),str(OUT/'native-totalwatsed3.parquet'),baseflow.gwstorage,baseflow.bfcoeff,baseflow.dscoeff,1,2,soil_path=str(OUT/'H.soil.parquet'),element_path=str(OUT/'H.element.parquet'),ash_inputs=[])
saved=pq.read_table(SOURCE/'ash/post/watershed_daily.parquet').to_pandas()
current=pq.read_table(SOURCE/'wepp/output/interchange/totalwatsed3.parquet').to_pandas()
records={}
for label,path in [('pre_native_python',OUT/'python-totalwatsed3.parquet'),('current_native',OUT/'native-totalwatsed3.parquet'),('saved_upstream',SOURCE/'wepp/output/interchange/totalwatsed3.parquet')]:
    data=pq.read_table(path).to_pandas()
    joined=saved[['year','julian','Streamflow_orig (mm)']].merge(data[['year','julian','Streamflow']],on=['year','julian'],how='left',validate='many_to_one')
    x=joined['Streamflow_orig (mm)'].to_numpy();y=joined['Streamflow'].to_numpy()
    bad=~np.isclose(x,y,rtol=1e-10,atol=1e-12,equal_nan=True)
    z=current[['year','julian','Streamflow']].merge(data[['year','julian','Streamflow']],on=['year','julian'],suffixes=('_saved','_recomputed'),validate='one_to_one')
    records[label]={'vs_saved_ashpost':{'bad_rows':int(bad.sum()),'max_absolute_difference':float(np.nanmax(np.abs(x-y)))},'vs_current_upstream':{'bad_rows':int((~np.isclose(z.Streamflow_saved,z.Streamflow_recomputed,rtol=1e-10,atol=1e-12,equal_nan=True)).sum()),'max_absolute_difference':float(np.nanmax(np.abs(z.Streamflow_saved-z.Streamflow_recomputed)))}}
records['mtime_ns']={str(path.relative_to(SOURCE)):path.stat().st_mtime_ns for path in [SOURCE/'ash/post/watershed_daily.parquet',SOURCE/'wepp/output/interchange/totalwatsed3.parquet']}
records['baseflow_options']=vars(baseflow)
records['ash_omitted_for_hydrology_isolation']=True
records['upstream_python_source_sha256']=hashlib.sha256((ROOT/'python-pre-native-totalwatsed3.py').read_bytes()).hexdigest()
print(json.dumps(records,indent=2))
(OUT/'result.json').write_text(json.dumps(records,indent=2)+'\n')

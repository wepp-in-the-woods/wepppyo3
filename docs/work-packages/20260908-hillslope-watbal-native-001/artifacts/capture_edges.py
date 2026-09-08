"""Freeze edge outputs from the immutable preimplementation source revision."""
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import importlib.util,json
import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppyo3.wepp_interchange import hillslope_watbal_to_parquet
root=Path('/workdir/wepppyo3');ev=Path('/evidence')
spec=importlib.util.spec_from_file_location('wepppy.wepp.reports._hillslope_watbal_baseline',ev/'python-baseline-hillslope_watbal.py');baseline=importlib.util.module_from_spec(spec);spec.loader.exec_module(baseline)
base=dict(wepp_id=[2,1,1,1],ofe_id=[1.,1.,1.,2.],water_year=[2001,2000,2000,2001],Area=[20.,10.,99.,5.],P=[1.,2.,None,4.],Dp=[.1]*4,QOFE=[.2]*4,latqcc=[.3]*4,Ep=[.4]*4,Es=[.5]*4,Er=[.6]*4)
results=[]
for case,changes in [('normal',{}),('null_area',{'Area':[None,float('nan'),99.,5.]}),('null_ofe',{'ofe_id':[1.,None,1.,2.]}),('nan_ofe',{'ofe_id':[1.,float('nan'),1.,2.]}),('empty',{k:[] for k in base})]:
 folder=ev/('edge-'+case);folder.mkdir();source=folder/'wepp/output/interchange/H.wat.parquet';source.parent.mkdir(parents=True);pq.write_table(pa.table({**base,**changes}),source,row_group_size=1)
 report=baseline.HillslopeWatbalReport.__new__(baseline.HillslopeWatbalReport);report.wd=folder;report._output_scope='baseline'
 mapping={1:101,2:101}
 with patch.object(Watershed,'getInstance',return_value=SimpleNamespace(translator_factory=lambda:SimpleNamespace(top=lambda wepp:mapping[wepp]))):frame=report._build_summary()
 oracle=folder/'oracle.parquet';frame.to_parquet(oracle,index=False);schema=pq.read_schema(oracle)
 out=folder/'native.parquet';hillslope_watbal_to_parquet(str(source),str(out),mapping,pandas_metadata=schema.metadata[b'pandas'].decode())
 actual=pq.read_table(out);assert actual.schema.equals(schema,check_metadata=True)
 if len(frame):np.testing.assert_allclose(actual.to_pandas().to_numpy(),frame.to_numpy(),rtol=1e-10,atol=1e-12)
 results.append({'case':case,'columns':list(frame.columns),'data':frame.to_dict('list'),'pandas_metadata':schema.metadata[b'pandas'].decode(),'parity':True})
(ev/'edge-oracles.json').write_text(json.dumps(results,indent=2)+'\n');print([r['case'] for r in results])

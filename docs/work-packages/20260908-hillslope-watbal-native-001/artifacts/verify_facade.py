"""Compare installed report facade and cache against preimplementation captures."""
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import json
import numpy as np
import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppy.wepp.reports.hillslope_watbal import HillslopeWatbalReport
results=[]
for run in ('decimal-pleasing','incommensurate-stickball','throbbing-sylvan'):
 oracle=Path('/evidence')/run;folder=Path('/evidence')/('integrated-'+run);folder.mkdir()
 source=folder/'wepp/output/interchange/H.wat.parquet';source.parent.mkdir(parents=True);source.symlink_to(oracle/'wepp/output/interchange/H.wat.parquet')
 expected=json.loads((oracle/'oracle.json').read_text());mapping={int(k):v for k,v in expected['mapping'].items()}
 with patch.object(Watershed,'getInstance',return_value=SimpleNamespace(translator_factory=lambda:SimpleNamespace(top=lambda wepp:mapping[wepp]))):
  report=HillslopeWatbalReport(folder)
 actual=pq.read_table(folder/'wepp/reports/cache/hillslope_watbal_summary.parquet');reference=pq.read_table(oracle/'summary.parquet')
 assert actual.schema.equals(reference.schema,check_metadata=True)
 np.testing.assert_allclose(actual.to_pandas().to_numpy(),reference.to_pandas().to_numpy(),rtol=1e-10,atol=1e-12)
 for field,rows in [('avg_annual',report.avg_annual_iter()),('yearly',report.yearly_iter())]:
  got=[dict(r.row) for r in rows];want=expected[field]
  assert len(got)==len(want)
  for a,b in zip(got,want):
   assert list(a)==list(b)
   np.testing.assert_allclose(list(a.values()),list(b.values()),rtol=1e-10,atol=1e-12)
 assert report.header==expected['header'] and report.units_d==expected['units']
 assert report.avg_annual_header==expected['avg_annual_header']
 assert report.yearly_header==expected['yearly_header']
 assert json.loads((folder/'wepp/reports/cache/hillslope_watbal_summary.meta.json').read_text())=={'version':'1'}
 results.append({'run':run,'rows':actual.num_rows,'parity':True})
(Path('/evidence')/'facade-parity.json').write_text(json.dumps(results,indent=2)+'\n');print(results)

from pathlib import Path
import json, subprocess, sys, types, tempfile
from types import SimpleNamespace
import pyarrow as pa
import pyarrow.parquet as pq
from wepppyo3.wepp_interchange import totalwatsed3_to_parquet
root=Path('/workdir/wepppyo3')
old=types.ModuleType('wepppy.wepp.interchange._review_previous')
sys.modules[old.__name__]=old
source=subprocess.check_output(['git','show','3c51780f50e2599ef72b03214c35bf20538e55c4:wepppy/wepp/interchange/totalwatsed3.py'],cwd='/workdir/wepppy',text=True)
exec(compile(source,'previous_totalwatsed3.py','exec'),old.__dict__)
folder=Path(tempfile.mkdtemp(prefix='independent-review-ash-nan-',dir=root/'target/totalwatsed3-evidence'))
fixture=root/'tests/fixtures/totalwatsed3/contracts/ash_types'
for p in fixture.glob('H.*.parquet'):
    (folder/p.name).symlink_to(p)
(folder/'ash').mkdir()
for p in (fixture/'ash').glob('*.parquet'):
    table=pq.read_table(p)
    values=table['year0'].to_pylist()
    values[0]=float('nan')
    table=table.set_column(table.schema.get_field_index('year0'),'year0',pa.array(values,type=pa.float64()))
    pq.write_table(table,folder/'ash'/p.name)
old._build_ash_type_and_density_lookup=lambda *a:({1:'black',2:'white'},{1:220.,2:310.})
opts=SimpleNamespace(gwstorage=1.,bfcoeff=.04,dscoeff=.02)
old_output=old.run_totalwatsed3(folder,opts,ash_dir=folder/'ash',ash_area_lookup={1:1.,2:2.})
old_output.rename(folder/'oracle.parquet')
ash=[(str(folder/'ash'/f'H{h}_ash.parquet'),float(h),'black' if h==1 else 'white',220. if h==1 else 310.) for h in (1,2)]
totalwatsed3_to_parquet(str(folder/'H.pass.parquet'),str(folder/'H.wat.parquet'),str(folder/'native.parquet'),1.,.04,.02,1,2,soil_path=str(folder/'H.soil.parquet'),element_path=str(folder/'H.element.parquet'),ash_inputs=ash,pandas_metadata=pq.read_schema(folder/'oracle.parquet').metadata[b'pandas'].decode())
for name in ('oracle.parquet','native.parquet'):
 print(name,pq.read_table(folder/name,columns=['julian','ash_transport']).to_pydict())
print('EVIDENCE',folder)

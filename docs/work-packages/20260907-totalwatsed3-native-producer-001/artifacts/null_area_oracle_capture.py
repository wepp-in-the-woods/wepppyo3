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
folder=Path(tempfile.mkdtemp(prefix='independent-review-null-area-',dir=root/'target/totalwatsed3-evidence'))
for p in (root/'tests/fixtures/totalwatsed3/contracts/normal').glob('H.*.parquet'):
    table=pq.read_table(p)
    if p.name=='H.wat.parquet':
        rows=table.to_pylist()
        for row in rows:
            if row['wepp_id']==1: row['Area']=None
        table=pa.Table.from_pylist(rows,schema=table.schema)
    pq.write_table(table,folder/p.name)
opts=SimpleNamespace(gwstorage=1.,bfcoeff=.04,dscoeff=.02)
old_output=old.run_totalwatsed3(folder,opts,ash_dir=folder/'absent')
old_output.rename(folder/'oracle.parquet')
totalwatsed3_to_parquet(str(folder/'H.pass.parquet'),str(folder/'H.wat.parquet'),str(folder/'native.parquet'),1.,.04,.02,1,2,soil_path=str(folder/'H.soil.parquet'),element_path=str(folder/'H.element.parquet'),pandas_metadata=pq.read_schema(folder/'oracle.parquet').metadata[b'pandas'].decode())
for name in ('oracle.parquet','native.parquet'):
 print(name,pq.read_table(folder/name,columns=['Area','TSMF','QRain','QSnow']).to_pydict())
print('EVIDENCE',folder)

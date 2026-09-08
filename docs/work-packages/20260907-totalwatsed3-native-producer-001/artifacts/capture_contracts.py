"""Freeze synthetic oracle cases from the pre-retirement producer."""
import importlib.util,json,shutil,tempfile
from pathlib import Path
from types import SimpleNamespace
import pyarrow as pa
import pyarrow.parquet as pq
import wepppy.wepp.interchange.totalwatsed3 as old
root=Path('/workdir/wepppyo3')
spec=importlib.util.spec_from_file_location('wepp_interchange_rust',root/'target/release/libwepp_interchange_rust.so')
native=importlib.util.module_from_spec(spec);spec.loader.exec_module(native)
spec=importlib.util.spec_from_file_location('compare',root/'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py')
compare=importlib.util.module_from_spec(spec);spec.loader.exec_module(compare)
base=Path(tempfile.mkdtemp(prefix='contracts-',dir='/evidence'))
wat_names=['P','RM','Q','Dp','latqcc','QOFE','Ep','Es','Er','UpStrmQ','SubRIn','Total-Soil Water','frozwt','Snow-Water','Tile','Irr',*old.WAT_OPTIONAL_COLUMNS]
def key(hill,ofe,day):return dict(wepp_id=hill,ofe_id=ofe,year=2020,sim_day_index=day,julian=day,month=1,day_of_month=day,water_year=2020)
def write(path,rows,schema_rows=None):
    sample=(schema_rows or rows)[0]
    schema=pa.schema([(n,pa.int32() if n in ('wepp_id','ofe_id','OFE','year','sim_day_index','day','julian','month','day_of_month','water_year','year0','days_from_fire (days)') else pa.float64()) for n in sample])
    pq.write_table(pa.Table.from_pylist(rows,schema=schema),path,row_group_size=5)
results=[]
for case in ('normal','optional_missing','optional_null','no_ofe','legacy','mixed_groups','duplicate_joins','subset','empty_subset','empty_inputs','null_lateral','ash_types','ash_no_year0'):
    folder=base/case;folder.mkdir();wat=[];soil=[];element=[];pas=[]
    for h in (1,2):
        for day in (1,2,3):
            pk=key(h,1,day);pk.pop('ofe_id');pk.update(runvol=1.25*h*day,sbrunv=.4,tdet=2.,tdep=.1,**{f'sedcon_{i}':.01*i for i in range(1,6)});pas.append(pk)
            for ofe in (1,2):
                k=key(h,ofe,day);w=dict(k,Area=float(1000*h*ofe),**{n:float(i+1)*.1 for i,n in enumerate(wat_names)});w['latqcc']=2.*ofe
                if case=='null_lateral':w['latqcc']=None
                if case=='optional_missing':
                    for n in old.WAT_OPTIONAL_COLUMNS:w.pop(n)
                if case=='optional_null':
                    for n in old.WAT_OPTIONAL_COLUMNS:w[n]=None
                wat.append(w);soil.append(dict(k,TSMF=None if ofe==1 else .3))
                if day!=2:
                    e=dict(k,QRain=.4,QSnow=None if ofe==1 else .2);e.pop('sim_day_index');element.append(e)
    if case=='duplicate_joins':wat.append(dict(wat[0]));soil.append(dict(soil[0]));element.append(dict(element[0]))
    if case=='mixed_groups':wat=wat[::2]+wat[1::2];soil=list(reversed(soil));element=list(reversed(element))
    if case in ('no_ofe','legacy'):
        for rows in (wat,soil,element):
            for r in rows:
                if case=='no_ofe':r.pop('ofe_id')
                else:r['OFE']=r.pop('ofe_id')
        if case=='legacy':
            for rows in (pas,wat,soil):
                for r in rows:r['day']=r.pop('sim_day_index')
    for name,rows in [('H.pass.parquet',pas),('H.wat.parquet',wat),('H.soil.parquet',soil),('H.element.parquet',element)]:write(folder/name,[] if case=='empty_inputs' else rows,rows)
    ash=[];ash_dir=folder/'ash';lookup={1:1.,2:2.}
    if case.startswith('ash'):
        ash_dir.mkdir()
        for h in (1,2):
            rows=[]
            for day in (1,1,2,3):
                r={'year':2020,'julian':day,'year0':2020 if day<3 else 2019,'days_from_fire (days)':day if day<3 else 400}
                r.update({f'{n} (tonne/ha)':.1*(i+1) for i,n in enumerate(old.ASH_METRIC_BASES)})
                if case=='ash_no_year0':r.pop('year0')
                rows.append(r)
            path=ash_dir/f'H{h}_ash.parquet';write(path,rows);ash.append((str(path),lookup[h],'black' if h==1 else 'white',220. if h==1 else 310.))
    old._build_ash_type_and_density_lookup=lambda *a:({1:'black',2:'white'},{1:220.,2:310.})
    opts=SimpleNamespace(gwstorage=1.,bfcoeff=.04,dscoeff=.02)
    ids=[2,2] if case=='subset' else [] if case=='empty_subset' else None
    oracle=old.run_totalwatsed3(folder,opts,wepp_ids=ids,ash_dir=ash_dir,ash_area_lookup=lookup)
    schema=pq.read_schema(oracle);out=folder/'native.parquet'
    native.totalwatsed3_to_parquet(str(folder/'H.pass.parquet'),str(folder/'H.wat.parquet'),str(out),1.,.04,.02,1,2,soil_path=str(folder/'H.soil.parquet'),element_path=str(folder/'H.element.parquet'),wepp_ids=ids,ash_inputs=ash,pandas_metadata=(schema.metadata.get(b'pandas') or b'').decode() or None)
    result=compare.compare(oracle,out);result['case']=case;results.append(result);print(json.dumps(result),flush=True)
    (folder/'case.json').write_text(json.dumps(dict(wepp_ids=ids,ash_inputs=[(str(Path(x[0]).relative_to(folder)),*x[1:]) for x in ash],gwstorage=1.,bfcoeff=.04,dscoeff=.02),indent=2)+'\n')
(base/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print('CONTRACT_ROOT',base,flush=True)
assert all(r['passed'] for r in results)

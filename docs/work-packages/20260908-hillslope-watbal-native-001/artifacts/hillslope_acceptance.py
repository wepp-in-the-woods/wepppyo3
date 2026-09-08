"""Isolated real Forest Compose RQ sequence; source runs are read-only."""
from pathlib import Path
import json,os,uuid
ROOT=Path('/workdir/wepppyo3')
EVIDENCE=ROOT/'target/hillslope-watbal-evidence'


def sequential(runid):
    from wepppy.rq.wepp_rq_stage_post import _build_totalwatsed3_rq, _run_hillslope_watbal_rq
    _build_totalwatsed3_rq(runid)
    _run_hillslope_watbal_rq(runid)
    return {'sequential':True}


def downstream(runid):
    import numpy as np
    import pyarrow.parquet as pq
    from wepppy.weppcloud.utils.helpers import get_wd
    from wepppy.wepp.reports.hillslope_watbal import HillslopeWatbalReport
    from wepppy.query_engine.activate import activate_query_engine
    wd=Path(get_wd(runid)); report=HillslopeWatbalReport(wd)
    reference=pq.read_table(EVIDENCE/'throbbing-sylvan/summary.parquet').to_pandas()
    actual=pq.read_table(wd/'wepp/reports/cache/hillslope_watbal_summary.parquet').to_pandas()
    assert len(actual)==len(reference)*10
    for replica in range(10):
        block=actual.iloc[replica*len(reference):(replica+1)*len(reference)].copy()
        block['TopazID']-=replica*587*10
        np.testing.assert_allclose(block.to_numpy(),reference.to_numpy(),rtol=1e-10,atol=1e-12)
    oracle=json.loads((EVIDENCE/'throbbing-sylvan/oracle.json').read_text())
    rows=[dict(r.row) for r in report.yearly_iter()]
    for a,b in zip(rows,oracle['yearly']):np.testing.assert_allclose(list(a.values()),list(b.values()),rtol=1e-10,atol=1e-12)
    activate_query_engine(wd,run_interchange=False,force_refresh=True)
    assert (wd/'_query_engine/catalog.json').exists()
    assert 'totalwatsed3.parquet' in (wd/'wepp/output/interchange/README.md').read_text()
    return {'summary_rows':len(actual),'yearly_rows':len(rows),'query_catalog':True,'parity':True}


def main():
    from redis import Redis
    from rq import Queue
    from wepppy.config.redis_settings import RedisDB,redis_connection_kwargs
    from wepppy.nodb.core import Climate,Wepp,Watershed
    from wepppy.nodb.core.wepp import BaseflowOpts
    from wepppy.rq import WepppyRqWorker
    import hillslope_acceptance
    token=uuid.uuid4().hex[:12];queue_name='hillslope-acceptance-'+token
    parent=Path(os.environ['PROFILE_PLAYBACK_RUN_ROOT']);parent.mkdir(parents=True,exist_ok=True)
    runs=[]
    for count in (5860,586):
        name=f'hillslope-{count}-{token}';wd=parent/name;wd.mkdir()
        Climate(str(wd),'disturbed9002.cfg',run_group='profile',group_name='tmp')
        wepp=Wepp(str(wd),'disturbed9002.cfg',run_group='profile',group_name='tmp')
        watershed=Watershed(str(wd),'disturbed9002.cfg',run_group='profile',group_name='tmp')
        with wepp.locked():wepp.baseflow_opts=BaseflowOpts(gwstorage=0.,bfcoeff=.04,dscoeff=0.)
        with watershed.locked():
            # The predecessor replicas leave one WEPP ID gap per block. Fill the
            # translator's ID domain with unused synthetic hillslopes, no source rows.
            watershed._subs_summary={str(i*10+1):None for i in range(1, (5869 if count==5860 else 586)+1)}
            watershed._chns_summary={}
        dest=wd/'wepp/output/interchange';dest.mkdir(parents=True)
        for source in (ROOT/'target/totalwatsed3-evidence/scales'/str(count)).glob('H.*.parquet'):(dest/source.name).symlink_to(source)
        runs.append((f'profile;;tmp;;{name}',wd))
    conn=Redis(**redis_connection_kwargs(RedisDB.RQ));queue=Queue(queue_name,connection=conn,default_timeout=600)
    first=queue.enqueue(hillslope_acceptance.sequential,runs[0][0],result_ttl=3600,failure_ttl=3600)
    second=queue.enqueue(hillslope_acceptance.downstream,runs[0][0],depends_on=first,result_ttl=3600,failure_ttl=3600)
    third=queue.enqueue(hillslope_acceptance.sequential,runs[1][0],depends_on=second,result_ttl=3600,failure_ttl=3600)
    worker=WepppyRqWorker([queue],connection=conn,name=queue_name);worker.work(burst=True,with_scheduler=False)
    mask=os.umask(0);os.umask(mask)
    result={'queue':queue_name,'jobs':[{'id':j.id,'status':str(j.get_status(refresh=True)),'exception':j.exc_info,'dependencies':[v.decode() if isinstance(v,bytes) else v for v in j.dependency_ids]} for j in (first,second,third)],'runids':[r for r,_ in runs],'run_paths':[str(w) for _,w in runs],'uid':os.getuid(),'gid':os.getgid(),'groups':os.getgroups(),'umask':oct(mask),'cpu_affinity':sorted(os.sched_getaffinity(0)),'memory_max':Path('/sys/fs/cgroup/memory.max').read_text(),'memory_peak':int(Path('/sys/fs/cgroup/memory.peak').read_text()),'memory_events':Path('/sys/fs/cgroup/memory.events').read_text()}
    (EVIDENCE/'compose-rq-result.json').write_text(json.dumps(result,indent=2)+'\n')
    assert all(j.is_finished for j in (first,second,third)),result
    assert result['memory_peak']<9*2**30
    print('COMPOSE_ACCEPTANCE',json.dumps(result),flush=True)

if __name__=='__main__':main()

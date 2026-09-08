"""Real sequential ash RQ jobs on disposable Forest fixture copies."""
from pathlib import Path
import copy
import hashlib
import json
import os
import resource
import time
import uuid

ROOT = Path('/workdir/wepppyo3/target/ashpost-evidence')


def phase(name, wd):
    usage = resource.getrusage(resource.RUSAGE_SELF)
    children = resource.getrusage(resource.RUSAGE_CHILDREN)
    record = dict(phase=name, time=time.time(), wd=wd,
        memory_current=int(Path('/sys/fs/cgroup/memory.current').read_text()),
        memory_peak=int(Path('/sys/fs/cgroup/memory.peak').read_text()),
        self_cpu=usage.ru_utime+usage.ru_stime, child_cpu=children.ru_utime+children.ru_stime,
        process=os.getpid())
    with (ROOT/'rq-runs/phases.jsonl').open('a') as stream:
        stream.write(json.dumps(record)+'\n')
    print('ASHPOST_PHASE',json.dumps(record),flush=True)


def execute(run):
    from wepppy.nodb.mods.ash_transport import ash as ash_module
    from wepppy.nodb.mods.ash_transport import AshPost
    from wepppy.rq.project_rq import run_ash_rq
    # Instrument phase boundaries; execute the unchanged real RQ/model path.
    original_work=ash_module._run_hillslope_work
    original_post=AshPost.run_post
    def work(*args,**kwargs):
        phase('model_start',run['wd'])
        result=original_work(*args,**kwargs)
        phase('executor_closed',run['wd'])
        return result
    def post(self,*args,**kwargs):
        phase('post_start',self.wd)
        result=original_post(self,*args,**kwargs)
        phase('post_complete',self.wd)
        return result
    ash_module._run_hillslope_work=work
    AshPost.run_post=post
    try:
        run_ash_rq(run['runid'],run['fire_date'],run['white'],run['black'])
    finally:
        ash_module._run_hillslope_work=original_work
        AshPost.run_post=original_post
    phase('rq_pipeline_complete',run['wd'])
    return validate(run)


def validate(run):
    import pyarrow.parquet as pq
    from wepppy.nodb.mods.ash_transport import AshPost
    from wepppy.nodb.mods.ash_transport.ashpost import ASH_POST_FILES
    wd=Path(run['wd']);post=AshPost.getInstance(str(wd))
    facades={name:copy.deepcopy(getattr(post,name)) for name in (
        'return_periods','cum_return_periods','burn_class_return_periods',
        'hillslope_annuals','watershed_annuals','pw0_stats','ash_out')}
    outputs={}
    for name in ASH_POST_FILES.values():
        path=wd/'ash/post'/name;table=pq.ParquetFile(path)
        assert table.metadata.num_rows>0,name
        outputs[name]=dict(rows=table.metadata.num_rows,sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    for name in ['ashpost_version.json','README.md']:
        assert (wd/'ash/post'/name).is_file(),name
    assert (wd/'_query_engine/catalog.json').is_file()
    (wd/'acceptance-facades.json').write_text(json.dumps(facades,indent=2)+'\n')
    return dict(runid=run['runid'],outputs=outputs,facades=True,catalog=True)


def main():
    from redis import Redis
    from rq import Queue
    from wepppy.config.redis_settings import RedisDB,redis_connection_kwargs
    from wepppy.rq import WepppyRqWorker
    import ashpost_acceptance
    os.umask(0o022)
    runs=json.loads((ROOT/'rq-runs/selection.json').read_text())
    conn=Redis(**redis_connection_kwargs(RedisDB.RQ))
    name='ashpost-acceptance-'+uuid.uuid4().hex[:12]
    queue=Queue(name,connection=conn,default_timeout=14400)
    first=queue.enqueue(ashpost_acceptance.execute,runs[0],result_ttl=86400,failure_ttl=86400)
    second=queue.enqueue(ashpost_acceptance.execute,runs[1],depends_on=first,result_ttl=86400,failure_ttl=86400)
    (ROOT/'rq-runs/jobs.json').write_text(json.dumps({'queue':name,'jobs':[first.id,second.id]},indent=2)+'\n')
    worker=WepppyRqWorker([queue],connection=conn,name=name)
    worker.work(burst=True,with_scheduler=False)
    record=dict(queue=name,jobs=[dict(id=j.id,status=str(j.get_status(refresh=True)),exception=j.exc_info,result=j.result) for j in (first,second)],
        uid=os.getuid(),gid=os.getgid(),groups=os.getgroups(),cpus=sorted(os.sched_getaffinity(0)),
        memory_peak=int(Path('/sys/fs/cgroup/memory.peak').read_text()),
        memory_events=Path('/sys/fs/cgroup/memory.events').read_text())
    (ROOT/'rq-runs/result.json').write_text(json.dumps(record,indent=2)+'\n')
    assert all(j.is_finished for j in (first,second)),record
    assert record['memory_peak']<9*2**30,record
    print('COMPOSE_ACCEPTANCE',json.dumps(record),flush=True)

if __name__ == '__main__': main()

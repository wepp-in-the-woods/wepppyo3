from pathlib import Path
import json
from redis import Redis
from rq import Queue
from rq.job import Job
from rq.exceptions import NoSuchJobError
from wepppy.config.redis_settings import RedisDB, redis_connection_kwargs
root=Path('/workdir/wepppyo3/target/totalwatsed3-evidence')
conn=Redis(**redis_connection_kwargs(RedisDB.RQ));records=[]
for name in ('compose-rq-result.json',):
    result=json.loads((root/name).read_text())
    assert result['queue'].startswith('totalwatsed3-acceptance-')
    for item in result['jobs']:
        try:
            job=Job.fetch(item['id'],connection=conn)
        except NoSuchJobError:
            records.append({'job':item['id'],'state':'already expired'})
            continue
        assert job.origin==result['queue'] and job.args[0] in result['runids']
        records.append({'job':job.id,'state':str(job.get_status()),'queue':job.origin})
        job.delete()
    Queue(result['queue'],connection=conn).delete(delete_jobs=True)
(root/'acceptance-cleanup.json').write_text(json.dumps(records,indent=2)+'\n')
print('Removed only the three published-image acceptance jobs and their private queue.')

"""Disposable Compose/RQ acceptance harness; never imported by production code."""
from pathlib import Path
import importlib.util, json, os, uuid

ROOT = Path('/workdir/wepppyo3')
EVIDENCE = ROOT / 'target/totalwatsed3-evidence'


def downstream(runid):
    from wepppy.weppcloud.utils.helpers import get_wd
    from wepppy.wepp.reports.total_watbal import TotalWatbalReport
    from wepppy.query_engine.activate import activate_query_engine
    import pyarrow as pa
    import pyarrow.compute as pc
    import pyarrow.parquet as pq
    wd = Path(get_wd(runid))
    output = wd / 'wepp/output/interchange/totalwatsed3.parquet'
    reference = ROOT / 'tests/fixtures/totalwatsed3/throbbing-sylvan/totalwatsed3.parquet'
    table = pq.read_table(reference)
    for name in ['runvol', 'sbrunv', 'tdet', 'tdep', *[f'seddep_{i}' for i in range(1, 6)], 'sed_del', 'Area', 'P', 'RM', 'Q', 'Dp', 'latqcc', 'QOFE', 'Ep', 'Es', 'Er']:
        i = table.schema.get_field_index(name)
        table = table.set_column(i, table.schema.field(i), pc.multiply(table[name], pa.scalar(10.)))
    oracle = wd / 'scaled-oracle.parquet'
    pq.write_table(table, oracle)
    spec = importlib.util.spec_from_file_location('compare', ROOT / 'docs/work-packages/20260907-totalwatsed3-native-producer-001/artifacts/compare_oracle.py')
    comparator = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(comparator)
    parity = comparator.compare(oracle, output)
    assert parity['passed'], parity
    report = TotalWatbalReport(wd)
    assert len(report.years) > 0
    catalog = activate_query_engine(wd, run_interchange=False, force_refresh=True)
    assert (wd / '_query_engine/catalog.json').exists()
    assert 'totalwatsed3.parquet' in (wd / 'wepp/output/interchange/README.md').read_text()
    result = {'parity': parity, 'water_balance_years': len(report.years), 'query_catalog': str(wd / '_query_engine/catalog.json')}
    (wd / 'downstream-result.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


def main():
    from redis import Redis
    from rq import Queue
    from wepppy.config.redis_settings import RedisDB, redis_connection_kwargs
    from wepppy.nodb.core import Climate, Wepp
    from wepppy.nodb.core.wepp import BaseflowOpts
    from wepppy.rq import WepppyRqWorker
    from wepppy.rq.wepp_rq_stage_post import _build_totalwatsed3_rq
    import totalwatsed3_acceptance

    token = uuid.uuid4().hex[:12]
    queue_name = f'totalwatsed3-acceptance-{token}'
    parent = Path(os.environ['PROFILE_PLAYBACK_RUN_ROOT'])
    parent.mkdir(parents=True, exist_ok=True)
    runs = []
    for count in (5860, 586):
        name = f'totalwatsed3-{count}-{token}'
        wd = parent / name
        wd.mkdir()
        Climate(str(wd), 'disturbed9002.cfg', run_group='profile', group_name='tmp')
        wepp = Wepp(str(wd), 'disturbed9002.cfg', run_group='profile', group_name='tmp')
        with wepp.locked():
            wepp.baseflow_opts = BaseflowOpts(gwstorage=0., bfcoeff=.04, dscoeff=0.)
        dest = wd / 'wepp/output/interchange'
        dest.mkdir(parents=True)
        for source in (EVIDENCE / 'scales' / str(count)).glob('H.*.parquet'):
            (dest / source.name).symlink_to(source)
        runs.append((f'profile;;tmp;;{name}', wd))
    conn = Redis(**redis_connection_kwargs(RedisDB.RQ))
    queue = Queue(queue_name, connection=conn, default_timeout=600)
    first = queue.enqueue(_build_totalwatsed3_rq, runs[0][0], result_ttl=3600, failure_ttl=3600)
    second = queue.enqueue(totalwatsed3_acceptance.downstream, runs[0][0], depends_on=first, result_ttl=3600, failure_ttl=3600)
    third = queue.enqueue(_build_totalwatsed3_rq, runs[1][0], depends_on=second, result_ttl=3600, failure_ttl=3600)
    umask = os.umask(0)
    os.umask(umask)
    worker = WepppyRqWorker([queue], connection=conn, name=queue_name)
    worker.work(burst=True, with_scheduler=False)
    jobs = [{'id': job.id, 'status': str(job.get_status(refresh=True)), 'exception': job.exc_info} for job in (first, second, third)]
    result = {'queue': queue_name, 'worker': worker.name, 'jobs': jobs, 'runids': [r for r, _ in runs], 'run_paths': [str(p) for _, p in runs],
              'uid': os.getuid(), 'gid': os.getgid(), 'groups': os.getgroups(), 'umask': oct(umask),
              'cpu_affinity': sorted(os.sched_getaffinity(0)), 'WEPPPY_NCPU': os.environ['WEPPPY_NCPU'],
              'memory_max': Path('/sys/fs/cgroup/memory.max').read_text().strip(),
              'memory_peak_bytes': int(Path('/sys/fs/cgroup/memory.peak').read_text()),
              'memory_events': Path('/sys/fs/cgroup/memory.events').read_text()}
    (EVIDENCE / 'compose-rq-result.json').write_text(json.dumps(result, indent=2) + '\n')
    assert all(job.is_finished for job in (first, second, third)), jobs
    assert result['memory_peak_bytes'] < 9 * 2**30
    assert not any('.tmp' in p.name for _, wd in runs for p in wd.rglob('*'))
    print('COMPOSE_RQ_ACCEPTANCE', json.dumps(result), flush=True)


if __name__ == '__main__':
    main()

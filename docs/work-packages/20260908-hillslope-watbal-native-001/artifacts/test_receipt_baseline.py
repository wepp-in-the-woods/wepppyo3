"""Reproduce receipt contention through real NoDb persistence and a lock stub."""
import os
import pytest
from wepppy.nodb.batch_runner import BatchRunner
import wepppy.nodb.base as base
from tests.nodb.test_base_boundary_characterization import _DummyNoDb, _RedisStub

pytestmark = pytest.mark.unit

def test_receipt_stale_write(tmp_path, monkeypatch):
    monkeypatch.setattr(base, 'redis_lock_client', _RedisStub())
    monkeypatch.setattr(base, 'redis_nodb_cache_client', None)
    runner = _DummyNoDb(str(tmp_path))
    runner._rq_job_ids = {'run_batch_rq': 'root-job'}
    for _ in range(3):
        with runner.locked():
            runner.value = 'before'
    parent = _DummyNoDb._hydrate_instance(str(tmp_path), False, False, False, use_redis_cache=False)
    child = _DummyNoDb._hydrate_instance(str(tmp_path), False, False, False, use_redis_cache=False)
    before = os.stat(child._nodb)
    with child.locked():
        child.value = 'after!'
    after = os.stat(child._nodb)
    assert before.st_size == after.st_size
    with pytest.raises(base.NoDbStaleWriteError) as failure:
        BatchRunner.set_rq_job_id(parent, 'final_batch_complete_rq', 'final-job')
    durable = _DummyNoDb._hydrate_instance(str(tmp_path), False, False, False, use_redis_cache=False)
    assert durable.value == 'after!'
    assert durable._rq_job_ids == {'run_batch_rq': 'root-job'}
    print('CONFIRMED same-size stale receipt:', str(failure.value))

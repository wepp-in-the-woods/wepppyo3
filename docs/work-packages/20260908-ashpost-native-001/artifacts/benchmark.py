"""Isolated frozen-Python/native AshPost measurement; source inputs are read-only."""
import gc
import importlib.util
import json
import logging
import multiprocessing as mp
import os
from pathlib import Path
import resource
import sys
import time
from types import SimpleNamespace
from unittest.mock import patch

import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppy.nodb.mods.ash_transport import Ash
from wepppy.nodb.mods.ash_transport import ashpost as candidate
from wepppy.topo.watershed_abstraction.wepp_top_translator import WeppTopTranslator

ROOT = Path('/evidence')
ART = Path('/workdir/wepppyo3/docs/work-packages/20260908-ashpost-native-001/artifacts')


def sample(stop, ready, path):
    # Stream samples: collection must not grow memory inside the measured cgroup.
    with path.open('w') as stream:
        ready.set()
        while not stop.is_set():
            stats = dict(line.split() for line in Path('/sys/fs/cgroup/memory.stat').read_text().splitlines())
            row = [time.monotonic(), int(Path('/sys/fs/cgroup/memory.current').read_text()),
                   int(stats['anon']), int(stats['file']), int(stats.get('kernel', 0))]
            stream.write(json.dumps(row)+'\n')
            stop.wait(.001)


def prepare(fixture, label):
    source = ROOT/'source'/fixture
    target = ROOT/'benchmarks'/fixture/label
    target.mkdir(parents=True, exist_ok=False)
    ash_dir = target/'ash'; ash_dir.mkdir()
    for path in (source/'ash').glob('H*_ash.parquet'):
        os.link(path, ash_dir/path.name)
    # Native input paths stay within the disposable root.
    (target/'wepp/output/interchange').mkdir(parents=True)
    for name in ('H.wat.parquet', 'totalwatsed3.parquet'):
        path = source/'wepp/output/interchange'/name
        if path.exists(): os.link(path, target/'wepp/output/interchange'/name)
    ws = json.loads((source/'watershed.nodb').read_text())['py/state']
    ash_state = json.loads((source/'ash.nodb').read_text())['py/state']
    translator = WeppTopTranslator(map(int, ws['_subs_summary']), map(int, ws['_chns_summary']))
    areas = {str(r['topaz_id']): r['area'] for r in pq.read_table(source/'watershed/hillslopes.parquet', columns=['topaz_id','area']).to_pylist()}
    watershed = SimpleNamespace(_subs_summary=ws['_subs_summary'], translator_factory=lambda: translator,
                                hillslope_area=lambda key: areas[str(key)])
    ash = SimpleNamespace(meta=ash_state['meta'], ash_dir=str(ash_dir), logger=logging.getLogger('benchmark'))
    return source, target, watershed, ash


def main():
    mode, fixture, label = sys.argv[1:]
    os.umask(0o022)
    spec = importlib.util.spec_from_file_location('wepppy.nodb.mods.ash_transport._benchmark_frozen', ROOT/'python-baseline-ashpost.py')
    baseline = importlib.util.module_from_spec(spec); spec.loader.exec_module(baseline)
    source, target, watershed, ash = prepare(fixture, mode+'-'+label)
    # Warm all source data, including both upstream hydrology files, without tables.
    for path in list((source/'ash').glob('H*_ash.parquet')) + [p for name in ('H.wat.parquet', 'totalwatsed3.parquet') if (p := source/'wepp/output/interchange'/name).exists()]:
        with path.open('rb') as stream:
            while stream.read(8*1024*1024): pass
    gc.collect()
    stop = mp.Event(); ready = mp.Event()
    sampler = mp.Process(target=sample, args=(stop, ready, target/'samples.jsonl'))
    sampler.start(); ready.wait()
    before = int(Path('/sys/fs/cgroup/memory.current').read_text())
    usage = resource.getrusage(resource.RUSAGE_SELF); start = time.perf_counter()
    calls = []
    try:
        with patch.object(Watershed, 'getInstance', return_value=watershed), patch.object(Ash, 'getInstance', return_value=ash):
            for repetition in range(int(os.environ.get('ASHPOST_REPEAT_CALLS', '3')) if mode == 'native-repeat' else 1):
                call_start = time.perf_counter()
                call_before = int(Path('/sys/fs/cgroup/memory.current').read_text())
                result = (baseline if mode == 'python' else candidate).watershed_daily_aggregated(str(target))
                calls.append(dict(repetition=repetition+1,wall_seconds=time.perf_counter()-call_start,
                    before=call_before,after=int(Path('/sys/fs/cgroup/memory.current').read_text()),
                    stat=dict(line.split() for line in Path('/sys/fs/cgroup/memory.stat').read_text().splitlines()),
                    rss=Path('/proc/self/status').read_text().split('VmRSS:')[1].splitlines()[0].strip()))
    finally:
        elapsed = time.perf_counter()-start
        after = int(Path('/sys/fs/cgroup/memory.current').read_text())
        stop.set(); sampler.join()
    final = resource.getrusage(resource.RUSAGE_SELF)
    sample_count = 0
    sample_peak = 0
    with (target/'samples.jsonl').open() as stream:
        for line in stream:
            sample_count += 1
            sample_peak = max(sample_peak, json.loads(line)[1])
    record = dict(mode=mode, fixture=fixture, label=label, wall_seconds=elapsed, calls=calls,
        user_seconds=final.ru_utime-usage.ru_utime, system_seconds=final.ru_stime-usage.ru_stime,
        precall_memory=before, postcall_memory=after, sampled_peak=sample_peak,
        sampled_increment=sample_peak-before,
        memory_peak=int(Path('/sys/fs/cgroup/memory.peak').read_text()),
        memory_events=Path('/sys/fs/cgroup/memory.events').read_text(),
        memory_max=Path('/sys/fs/cgroup/memory.max').read_text(), samples=sample_count,
        uid=os.getuid(), gid=os.getgid(), cpu_affinity=sorted(os.sched_getaffinity(0)),
        cache='streamed source bytes before timed call; fresh container/process per sample',
        sampler='separate process, 1ms target; sampled peaks are lower bounds',
        timing_scope='Python discovery and five-table production plus compact mappings; no docs/facades')
    (target/'measurement.json').write_text(json.dumps(record, indent=2)+'\n')
    (target/'return-periods.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(record), flush=True)

if __name__ == '__main__': main()

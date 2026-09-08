"""Capture immutable baseline aggregation without hydrating writable source NoDb."""
import argparse
import copy
import hashlib
import importlib.util
import json
import logging
import os
from pathlib import Path
import shutil
import time
from types import SimpleNamespace
from unittest.mock import patch

import pyarrow.parquet as pq
from wepppy.nodb.core import Watershed
from wepppy.nodb.mods.ash_transport import Ash
from wepppy.topo.watershed_abstraction.wepp_top_translator import WeppTopTranslator

ROOT = Path('/workdir/wepppyo3/target/ashpost-evidence')

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('fixture')
    parser.add_argument('--label', default='python-1')
    args = parser.parse_args()
    source = ROOT / 'source' / args.fixture
    target = ROOT / 'oracles' / args.fixture / args.label
    target.mkdir(parents=True, exist_ok=False)
    ash_dir = target / 'ash'
    ash_dir.mkdir()
    for file in (source / 'ash').glob('H*_ash.parquet'):
        os.link(file, ash_dir / file.name)
    s = json.loads((source / 'watershed.nodb').read_text())['py/state']
    a = json.loads((source / 'ash.nodb').read_text())['py/state']
    translator = WeppTopTranslator(map(int, s['_subs_summary']), map(int, s['_chns_summary']))
    areas = {str(row['topaz_id']): row['area'] for row in pq.read_table(source / 'watershed/hillslopes.parquet', columns=['topaz_id', 'area']).to_pylist()}
    watershed = SimpleNamespace(_subs_summary=s['_subs_summary'], translator_factory=lambda: translator, hillslope_area=lambda key: areas[str(key)])
    ash = SimpleNamespace(meta=a['meta'], ash_dir=str(ash_dir), logger=logging.getLogger('oracle'), fire_date=a['fire_date'])
    module_name = 'wepppy.nodb.mods.ash_transport._frozen_ashpost'
    spec = importlib.util.spec_from_file_location(module_name, ROOT / 'python-baseline-ashpost.py')
    baseline = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(baseline)
    start = time.perf_counter()
    with patch.object(Watershed, 'getInstance', return_value=watershed), patch.object(Ash, 'getInstance', return_value=ash):
        result = baseline.watershed_daily_aggregated(str(source))
        baseline.write_version_manifest(ash_dir / 'post')
        baseline.generate_ashpost_documentation(str(ash_dir / 'post'))
        facade = object.__new__(baseline.AshPost)
        facade.wd = str(target)
        facade._return_periods, facade._cum_return_periods, facade._burn_class_return_periods = result
        facades = {name: copy.deepcopy(getattr(facade, name)) for name in ('return_periods', 'cum_return_periods', 'burn_class_return_periods', 'hillslope_annuals', 'watershed_annuals', 'pw0_stats', 'ash_out')}
    elapsed = time.perf_counter() - start
    (target / 'return-periods.json').write_text(json.dumps(result, indent=2) + '\n')
    (target / 'facades.json').write_text(json.dumps(facades, indent=2) + '\n')
    manifest=[]
    for file in sorted((source / 'ash').glob('H*_ash.parquet')):
        parquet=pq.ParquetFile(file)
        manifest.append({'path':file.name, 'size':file.stat().st_size,'sha256':sha(file),'rows':parquet.metadata.num_rows,'schema':str(parquet.schema_arrow)})
    outputs={}
    for file in sorted((ash_dir / 'post').iterdir()):
        record={'sha256':sha(file),'size':file.stat().st_size}
        if file.suffix=='.parquet':
            table=pq.read_table(file)
            record.update(rows=table.num_rows,schema=str(table.schema),schema_serialized_hex=table.schema.serialize().to_pybytes().hex())
            original=pq.read_table(source / 'ash/post' / file.name)
            import pandas as pd
            assert original.schema.equals(table.schema, check_metadata=True)

            try:
                pd.testing.assert_frame_equal(original.to_pandas(), table.to_pandas(), check_exact=False, rtol=1e-10, atol=1e-12)
                record['saved_output_parity'] = 'passed'
            except AssertionError as exc:
                record['saved_output_parity'] = 'failed: preserved for disposition'
                (target / (file.stem + '-saved-drift.txt')).write_text(str(exc))
        outputs[file.name]=record
    summary={'fixture':args.fixture,'model':a['_model'],'configuration':a['_config'],'baseline_sha256':sha(ROOT/'python-baseline-ashpost.py'),'wall_seconds':elapsed,'inputs':manifest,'outputs':outputs,'saved_output_drift': any(v.get('saved_output_parity', '').startswith('failed') for v in outputs.values())}
    (target / 'capture.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({'fixture':args.fixture,'wall_seconds':elapsed,'inputs':len(manifest),'saved_output_drift': any(v.get('saved_output_parity', '').startswith('failed') for v in outputs.values())}),flush=True)

if __name__=='__main__':
    main()

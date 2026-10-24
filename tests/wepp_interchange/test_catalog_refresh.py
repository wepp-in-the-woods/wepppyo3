"""Catalog metadata parity, maintenance exclusion, and serving heartbeat regression."""
import threading
import time

import pyarrow as pa
import pyarrow.parquet as pq

from wepppyo3.wepp_interchange import catalog_scan


def test_catalog_parity_and_refresh_exclusion(tmp_path):
    source = tmp_path / "soils.parquet"
    pq.write_table(pa.table({"topaz_id": [11, 12], "mukey": ["a", "b"]}), source)
    refresh = tmp_path / "_query_engine" / "refresh"
    refresh.mkdir(parents=True)
    (refresh / "status.json").write_text('{}')
    (tmp_path / "status-alias.json").symlink_to(refresh / "status.json")
    entries = catalog_scan(str(tmp_path))
    assert [entry["path"] for entry in entries] == ["soils.parquet"]
    assert entries[0]["size_bytes"] == source.stat().st_size
    assert entries[0]["schema"]["fields"] == [
        {"name": "topaz_id", "type": "int64"},
        {"name": "mukey", "type": "string"},
    ]
    assert (refresh / "status.json").read_text() == '{}'


def test_catalog_scan_allows_python_heartbeat(tmp_path):
    # A real directory traversal, not a mocked native call. Enough entries to
    # observe a heartbeat strictly inside the call, excluding boundary races.
    for index in range(12000):
        (tmp_path / f"asset-{index}.json").write_text('{}')
    beats = []
    stop = threading.Event()

    def heartbeat():
        while not stop.wait(0.001):
            beats.append(time.monotonic())

    thread = threading.Thread(target=heartbeat)
    thread.start()
    try:
        started = time.monotonic()
        entries = catalog_scan(str(tmp_path))
        ended = time.monotonic()
    finally:
        stop.set()
        thread.join()
    assert len(entries) == 12000
    assert any(started + 0.005 < beat < ended - 0.005 for beat in beats), (
        f"Python heartbeat did not run inside catalog scan ({ended - started:.3f}s)"
    )

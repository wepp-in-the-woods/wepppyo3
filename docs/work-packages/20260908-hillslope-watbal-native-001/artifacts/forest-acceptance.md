# Forest execution evidence

Ran: the referenced scripts and raw logs record execution. Static: ownership and publication boundaries follow the reviewed package.

## Environment and provenance

`confirmed`: both repositories began clean and current on their existing branches.
All 127 predecessor fixture checksums and Git LFS fsck passed. The reused
5,860-hillslope generator hash and source sizes were verified; no new large
fixture was copied. Sources were mounted read-only; generated outputs live under
`target/hillslope-watbal-evidence`. See baseline-provenance.json and
reused-scale-provenance.json.

Containers use image
`sha256:6ac7e71030467a10e5d73dc18893cbd85c9202976d4b1b561a19dbb0d7ef2b75`,
UID 1000, GID/groups 993, umask 0022, CPUs 0-11, and 12 GiB memory/swap caps.
Raw commands and measurements accompany this document.

## Parity and standalone measurements

`confirmed`: frozen single-OFE, real-MOFE, and 586-hillslope outputs match the
installed report facade, exact schemas/metadata, sort/key order, headers,
version sidecars, and serialized iterators. Numeric tolerance is rtol=1e-10,
atol=1e-12. See facade-parity.json and python-oracle-*.json. Frozen compact
Parquet oracles remain in the disposable evidence tree and were not overwritten.

After warm-up, five paired observations give Python median 3.1755 seconds and
initial native median 2.6953 seconds, including native distinct-ID discovery.
Native peaks were about 223 MiB versus Python about 2.4 GiB. The final binary's
five additional native timings are separately recorded in
final-native-benchmark-summary.json. These are fixture-specific measurements.
The benchmark loads the original Python producer from a disposable extraction
of WEPPpy revision d8fbe9f4ab270b6a7ab119d7cdda55d3cb628986; no production
fallback is retained.

## Scale, repeatability, and sequential execution

`confirmed`: the reused scale has 5,860 hillslopes and 94,176,060 H.wat rows.
Three candidate calls match the deterministic ten-replica oracle. The same
process then executes native totalwatsed3 immediately followed by hillslope
watbal, retaining prior allocator state; peak is 971,087,872 bytes. See
scale-results.json and scale-memory-events.txt.

A separate final-binary run excludes comparator allocations and repeatedly
replaces the same output. After warm-up, three calls grow anonymous memory by
90,112 bytes; peak is 306,360,320 bytes and no stage remains. This separates
native retention from the earlier comparator/page-cache growth. See
repeat-fixed.json and repeat-candidate.log.

## Real Compose RQ acceptance

`confirmed`: use the existing development rq-worker service, entrypoint,
configuration, secrets, mounts, Redis connection, and WepppyRqWorker with the
bounded override in compose-acceptance.yml. A unique queue and disposable
profile/tmp run roots isolate the test. Real Climate, Wepp, and Watershed NoDb
objects are used. Synthetic translator entries fill the predecessor replica
ID gaps; no source rows or source run state are mutated.

The first job calls the real `_build_totalwatsed3_rq` and then
`_run_hillslope_watbal_rq` in one RQ work horse on 5,860 hillslopes. The second
consumes report/cache output, verifies parity, and activates the query catalog.
The third processes 586 hillslopes as a subsequent job. All three finish.
Whole-container peak is 1,165,426,688 bytes; OOM/max events and restart count
are zero. Startup verifies the final native API and SHA. See compose-rq.log,
compose-rq-result.json, and compose-container-state.json.

The first attempt completed all three jobs but its collector could not serialize
byte-valued dependency IDs. Its raw log/state are retained; the corrected
collector and complete second workflow provide acceptance evidence. This was a
collector failure, not a worker failure or OOM.

## Publication boundary

Registry publication and production deployment are not authorized here.
WEPPpy's master push normally triggers image publication, so its integration
commit uses `[skip ci]`. Local required validation remains recorded. This follows
[GitHub's skip-workflow contract](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/skip-workflow-runs).
Publication checks must confirm repository revisions without triggering images.

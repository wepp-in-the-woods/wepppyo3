# Final reviewed release disposition

The canonical native producer and required WEPPpy facade are built and validated.
Release SHA-256:
`bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354`.
Exact source/build/file hashes are in native-build.json and
release-integration-manifest.json. Python owns path/scalar/ash orchestration;
Rust performs table work with the existing failure-atomic sink. The paired
startup hash/API inventory requires this release without fallback.

## Review and validation

Independent correctness, QA and security reviews pass with no open blockers.
Review repaired null-area weighted numerators, first-NaN ash filter metadata,
and malformed-footer error handling while preserving valid empty dictionary
chunks. See review-fixes.md and the three independent review artifacts.

Installed native module: 89 tests pass. Public facade: 25 frozen oracle cases
pass. Final targeted WEPPpy suite: 83 pass, 1 skip; this includes bounded-preview
and correct output-path logging coverage. Independent security additionally
passes 12 path/input probes and an actual Compose facade error-boundary probe.
All 127 fixture Parquet files are frozen with SHA-256 manifests and LFS pointers.
The broad final-source WEPPpy suite is running; the earlier 7,720/72 result remains
historical evidence. Eight original downstream suites passed 148 tests.

## Resources and real workflow

Final five-repeat median: 3.868492 seconds native versus 3.629247 Python
(106.5921%); all twenty benchmark outputs pass parity. Native peak is below half
Python. Controlled symmetric host-prewarm scaling: raw peaks 473,591,808 and
681,893,888 bytes, ratio 1.4398346. The earlier final-source mixed-cache run failed
the ratio at 2.07079, with a large additional file-cache charge. Both receipts
remain recorded; the passing ratio is qualified to the controlled cache condition,
not a cache-independent guarantee. See review-performance.md.

The final rebuilt release passes the real Forest Compose RQ chain for 5,860
hillslopes, dependent strict parity/water balance/query catalog, and a subsequent
586-hillslope job. Peak is 903,184,384 bytes (861.34 MiB) under 12 GiB, with zero
OOM events, restarts or staging leaks. UID 1000, GID 993, umask 0022. No producer
or controller is mocked. The private queue and three jobs were cleaned after
readback; the stopped one-shot container was removed. Source HPC runs and the
historical single-OFE fixture remain unchanged.

## Publication

Reviews and release-specific gates are complete. Native-first commit/push,
remote clean-checkout LFS verification, paired WEPPpy publication and GHCR image
publication are in progress. No production deployment is claimed. Publication
receipts will identify both source revisions and the immutable image digest.

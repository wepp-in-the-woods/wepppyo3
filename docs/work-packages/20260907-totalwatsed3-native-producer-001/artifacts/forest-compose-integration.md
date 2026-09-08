# Forest Compose RQ integration

confirmed: The paired py312 release was built and atomically installed in the
canonical release tree. Existing service startup preflight verified SHA-256
`0e681131bde2790af53e1cda9d7292aba99a1aa1debb5cc71b3ee8c84956ba2f`
and the required API before starting the one-shot worker.

## Real workflow

Use the existing development Compose rq-worker service and its environment,
secrets, mounts, entrypoint, Redis DB 9, and WepppyRqWorker. A disposable override
sets 12 GiB memory/swap limit, CPUs 0-11, and WEPPPY_NCPU=12. No existing worker
was restarted. The test uses a unique queue and real Climate/Wepp NoDb objects
under the supported profile/tmp run root. Source H.* inputs are read-only links
to the already generated repository scale fixtures. No source HPC run is changed.

Command from /workdir/wepppy:

```sh
WCTL_COMPOSE_FILE_EXTRAS=/workdir/wepppyo3/target/totalwatsed3-evidence/compose-acceptance.yml \
  wctl run --no-deps --name totalwatsed3-release-acceptance-v2 rq-worker \
  /opt/venv/bin/python /workdir/wepppyo3/target/totalwatsed3-evidence/totalwatsed3_acceptance.py
```

The worker executes the actual _build_totalwatsed3_rq production stage for
5,860 hillslopes, then a dependent parity/water-balance/query-catalog check, then
the actual producer stage for 586 hillslopes as a subsequent job. Each job forks
through the installed WepppyRqWorker; no producer or controller is mocked.
The supplementary downstream job uses the same 10-times oracle construction
as the approved scale measurement and TotalWatbalReport/activate_query_engine.
DSS, return periods, WATAR, batch, culvert, and migration coverage is provided by
the separate downstream suites, not claimed as part of this minimal run tree.

confirmed: All three jobs finish. Peak whole-container memory is 911,167,488
bytes (868.96 MiB); limit is 12,884,901,888 bytes. OOM/max events are zero,
restart count is zero, no AbandonedJobError or temporary-file leak occurs.
Identity is UID 1000, GID/groups 993, umask 0022, matching this Forest Compose
service. The output README includes totalwatsed3; output parity and water-balance
processing pass and the query catalog is generated. See compose-rq-result.json,
compose-rq.log, compose-container-state.json, and totalwatsed3_acceptance.py.
This establishes Forest acceptance, not a deployment or unverified Kubernetes
runtime-equivalence claim for openwepp.org.

## Failure found and corrected

The first attempt published the native output, then OOM-killed its work horse
while interchange_documentation.py called pq.read_table for a three-row README
preview. Whole-container peak reached the 12 GiB limit. The reader now uses the
file schema plus the first bounded three-row batch. Empty and multi-row-group
preview tests preserve output behavior and prohibit full-table reads. The same
production stage succeeds after this correction. before-preview-fix-* retains
the failed job, container, and cgroup evidence; this is not hidden as a clean
first attempt. This fix changes documentation memory use, not any data formula.

After preserving job states, the six acceptance jobs and their two private queues
were removed; both stopped one-shot containers were removed. Disposable run trees
remain as evidence. Existing application services and source runs were untouched.

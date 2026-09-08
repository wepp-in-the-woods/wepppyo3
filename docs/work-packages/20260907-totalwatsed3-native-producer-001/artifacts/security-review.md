# Security review: native totalwatsed3 integration

## Metadata

Reviewer: implementing Codex agent (first-party review, not independent sign-off).
Scope: native source and failure-atomic writer use, required Python boundary,
paired startup hash, bounded README preview, and disposable Compose RQ workflow.
Source/build identity: release-integration-manifest.json and native-build.json.
Dedicated review is required because native file parsing/publication and a worker
startup artifact pin are changed. The package originally classified impact as
medium; WEPPpy's file/path-handling review standard treats this surface as high.

## Findings

| ID | Severity | Surface | Finding | Evidence | Status |
| --- | --- | --- | --- | --- | --- |
| SEC-01 | High | Post-producer documentation | Whole-table reads for three preview rows exhaust the 12 GiB worker after successful native publication. | before-preview-fix-compose-rq-result.json; preview-fix-tests.log; compose-rq-result.json | Resolved by schema plus bounded preview batch; real RQ workflow rerun passes. |

No additional confirmed finding is recorded by this first-party review.
This is not an exhaustive hostile-Parquet allocation audit or independent approval.

## Boundary evidence

- Existing route authorization, run scoping, Redis secrets and RQ dependencies are
  unchanged. File paths and scalar/controller metadata remain WEPPpy-owned.
- Missing native support fails explicitly with the existing unavailable error;
  native failures retain their cause through the execution-error boundary. Tests
  verify a missing symbol cannot invoke a Python/DuckDB fallback or alter output.
- Installed release/startup checks verify the new API and exact .so hash.
- Native tests directly exercise input/output alias rejection, missing input,
  failure-atomic publication and staging cleanup. Frozen cases cover empty,
  absent optional, populated and legacy states without rejecting those states.
- Source H.* data are opened read-only. The native transform does not invoke
  subprocesses, add dependencies, send network requests, or modify NoDb state.
- Existing ash controller lookup behavior is retained; no new broad catch or
  silent fallback is added. Changed-file broad-exception enforcement passes.
- The passing real WepppyRqWorker uses the existing Compose identity/mounts,
  umask 0022, required startup preflight, and 12 GiB cgroup. Three dependent jobs
  finish with zero OOM/max events, restarts or AbandonedJobError.
- The acceptance harness created only private queues and disposable run trees.
  It reads existing secret files through canonical Redis configuration, never
  places credentials in argv/logs, and preserves source fixtures.

## Verdict and limits

First-party findings are resolved. Independent correctness/security review and
publication remain pending; this artifact does not provide independent release
sign-off. Forest workflow evidence does not establish openwepp.org Kubernetes
identity/mount equivalence or authorize a production rollout. Malicious metadata
and arbitrary schema combinations are not claimed exhaustively covered.

# Batch receipt transaction contract

Static: reviewed transaction design; Ran: baseline-receipt.log and combined-tests.log record the deterministic regression.

Classification: conformance fix for the existing canonical NoDb Writer Ownership
and Mutation Topology contract in WEPPpy's
`docs/schemas/nodb-persistence-concurrency-contract.md`. No RQ graph, finalizer
policy, or compatibility-key change is intended.

Add BatchRunner.set_rq_job_id_fresh(key, job_id). Preserve the existing setter
for other callers. Empty key remains no-op; empty job ID removes the named key.
The fresh method acquires the instance's distributed lock; hydrates directly
from disk using _hydrate_instance(use_redis_cache=False) under that lock;
checks unchanged distributed lock identity; replaces the stale instance state
with durable state; applies only the requested receipt mutation; dumps once;
and unlocks promptly. Lock tokens live in the external weak instance-keyed map
in NoDbBase, so durable attribute refresh does not replace the ownership token.
Follow the existing _derived_build.finalize pattern without its artifact logic.
Invalidate only the instance's cached signature after a failed commit so a
subsequent read cannot return uncommitted fields. Do not retry or clear caches.

Use the new helper at both receipt sites: parent hydration precedes the
active-job lookup even for run_batch_rq, so another writer can commit before
that receipt. Preserve best-effort logged receipt failures for expected lock,
I/O, and Redis exceptions, replacing broad Exception catches with explicit
types (NoDbAlreadyLockedError, NoDbStaleWriteError, OSError, RedisError).
Untyped ownership RuntimeError propagates conservatively; do not introduce a
new exception abstraction or hide unrelated RuntimeError/RecursionError. Unexpected programming/decoding failures propagate. Root RQ metadata
remains authoritative. Save the root finalizer linkage and release a ready
deferred finalizer before publishing its compatibility receipt, so receipt
failure cannot skip required RQ bookkeeping. No enqueue takes place under the
NoDb lock. Inject receipt failure to verify metadata and release still occur.

Regression: hydrate parent, commit a detached child's unrelated same-length
field, call old setter and prove NoDbStaleWriteError, then discard the stale
mutation base and call fresh setter. Read disk and verify child value plus
both receipt IDs; inspect logs for no stale warning. Cover removal, hydration
failure, lock ownership, and absence of temporary files. Retain existing
failure-tolerant and Omni dependency tests, status/deferred recovery consumers,
and live isolated job-tree acceptance. All outputs use disposable state.

The deterministic same-size stale-write baseline passes (baseline-receipt.log).
Independent correctness review accepts the transaction shape. Security review
identified finalizer failure ordering; the ordering above dispositions that
finding. Add explicit mtime separation to the regression, lock-ownership loss,
and failed-dump cache refresh cases. Final security review remains pending.

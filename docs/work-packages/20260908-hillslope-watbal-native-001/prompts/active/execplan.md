# Deliver bounded native hillslope water balance and safe batch receipt persistence

This ExecPlan is a living document and must be maintained according to
`/workdir/wepppy/docs/prompt_templates/codex_exec_plans.md`. Keep `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`
current. A new agent must be able to resume using only this file and the package.

## Purpose / Big Picture

Large batch watersheds currently finish WEPP and native totalwatsed3, then lose
the whole RQ worker when Python materializes `H.wat.parquet` for the hillslope
water-balance report. After this work, that summary is streamed and aggregated
by the owned Rust module, the existing WEPPpy report looks identical, and the
complete post-processing sequence stays safely below the 12 GiB worker cap.
The batch orchestrator also records finalizer IDs without racing child writes to
`batch_runner.nodb`.

## Progress

- [x] (2026-09-08) Scaffolded the execution-ready cross-repository package from
  live OOM and stale-write evidence.
- [ ] Verify repository/LFS state and capture immutable baseline evidence.
- [ ] Freeze Python output, cache, report iterator, and failure-mode oracles.
- [ ] Freeze the native API and NoDb transaction design in reviewed artifacts.
- [ ] Implement the Rust writer and native tests.
- [ ] Integrate native-only WEPPpy behavior and safe receipt persistence.
- [ ] Pass parity, scale, timing, memory, sequential, and real Compose RQ gates.
- [ ] Run broad validation and close independent reviews.
- [ ] Commit, push, and clean-checkout verify both repositories.

## Surprises & Discoveries

- Observation: all seven production failures occurred after native
  totalwatsed3 returned successfully and immediately after Python hillslope
  water-balance processing began.
  Evidence: pod last states were `OOMKilled`/137 and prior logs ended at
  `_run_hillslope_watbal`; source files were 1.15-2.82 GB compressed.
- Observation: the finalizer-ID warning is a separate shared-NoDb race, not the
  cause of the OOM failures.
  Evidence: the root RQ job completed OK after logging a same-size stale-write
  rejection for `batch_runner.nodb`.

## Decision Log

- Decision: perform faithful native extraction with no fallback.
  Rationale: the Python full-source implementation is the demonstrated OOM
  path, and silent fallback would reintroduce it.
  Date/Author: 2026-09-08, Codex for Roger Lew.
- Decision: reuse the prior LFS fixtures and its scale machinery.
  Rationale: they already cover single-OFE, real MOFE, 586 hillslopes, and a
  near-incident 5,860-hillslope case with verified provenance.
  Date/Author: 2026-09-08, Codex for Roger Lew.
- Decision: preserve the NoDb receipt mirror but update it from fresh durable
  state under lock.
  Rationale: status and deferred recovery consume the mirror; a scoped
  transaction fixes contention without a behavior migration.
  Date/Author: 2026-09-08, Codex for Roger Lew.

## Outcomes & Retrospective

Not started. Completion requires both repositories to be published and the
real sequential Compose path to pass; implemented-but-unwired Rust is not an
acceptable outcome.

## Context and Orientation

The Rust workspace is `/workdir/wepppyo3`. Its existing Python extension module
is defined under `wepp_interchange/src/`, exposed as
`wepppyo3.wepp_interchange`, tested under `tests/wepp_interchange/`, and copied
into `release/linux/py312/wepppyo3/wepp_interchange/` for WEPPpy images.
The completed predecessor package is
`docs/work-packages/20260907-totalwatsed3-native-producer-001/`; its fixtures,
measurement tools, evidence conventions, and release process are reusable.

The integration repository is `/workdir/wepppy`. The current producer and
report facade are in `wepppy/wepp/reports/hillslope_watbal.py`.
`HillslopeWatbalReport._build_summary()` reads the entire source through
PyArrow/pandas and returns a compact per-Topaz/per-year dataframe. The report
then derives average-annual and watershed-yearly views from that compact data.
`ReportCacheManager` owns cache paths, version sidecars, and freshness.

The RQ orchestrator is `wepppy/rq/batch_rq.py`. It hydrates `BatchRunner`, starts
children, creates `_final_batch_complete_rq`, and calls
`BatchRunner.set_rq_job_id()`. Children may update the same whole-object
`batch_runner.nodb` before that late call. `wepppy/nodb/base.py` rejects the
stale mutation base correctly. The fix must follow
`docs/schemas/nodb-persistence-concurrency-contract.md` rather than weakening
that rejection.

## Plan of Work

First establish clean Forest baselines. Pull both repositories without
discarding unrelated work, verify the exact remote revisions, run `git lfs
pull`, and validate all predecessor fixture hashes. Copy no additional large
fixture. Record toolchain, image, UID/GID, cgroup limit, CPU, and cache-state
information in artifacts.

Next freeze the current Python contract before editing it. Produce canonical
summary Parquet and serialized `avg_annual_iter()`/`yearly_iter()` results for
single-OFE, real MOFE, and performance inputs. Capture schemas, metadata,
ordering, null behavior, cache/sidecar behavior, Roads mappings, empty and
malformed cases, and errors. Build a comparator that checks exact structural
parity and the package's numeric tolerances. Measure the current Python
standalone path and preserve any OOM as valid baseline evidence.

Then write and independently review a native API contract. The API accepts
source/output paths and bounded translation data, streams the eleven projected
columns, and writes the eight-column compact summary atomically. Decide how
WEPPpy discovers distinct source IDs without materializing rows. Keep Roads
policy and logging in WEPPpy, but return compact mapping telemetry when needed.
Document concrete Rust types, Python signature, exceptions, and memory bounds
before implementation.

Implement the Rust aggregation in small testable units: schema projection and
validation, batch ingestion, first-area tracking per WEPP/OFE, translated
Topaz/year flux accumulation, stable sort, Arrow array construction, and atomic
Parquet publication. Use checked conversions and explicit errors; no panics may
cross the Python boundary. Add malformed-schema, null, nonfinite, duplicate,
empty, mapping, and injected-publication-failure tests. Refresh the installed
py312 release and provenance only after source tests pass.

Integrate WEPPpy by reducing `_build_summary()` to path/mapping orchestration,
the required native call, sidecar publication, and loading only the compact
summary. Delete the executable full-source Python producer and prohibit a
fallback. Preserve cache reads and report-derived views. Update startup
preflight, stubs, developer/operator docs, and all affected tests.

In parallel within the integration milestone, create a deterministic NoDb
interleaving test. It must mutate an unrelated BatchRunner field after parent
hydration and before the receipt update. Implement a short lock-owned fresh
transaction that rehydrates the durable controller and changes only the named
receipt key. Keep errors observable and narrow. Verify both receipt sites,
status snapshot consumers, deferred recovery, root metadata, Omni finalizer
linkage, and the static RQ graph. If preserving behavior is impossible, stop
and create the required contract-decision checkpoint before continuing.

Finally run parity and performance on Forest. Measure standalone Python/native
on `throbbing-sylvan`, the 5,860-hillslope generated scale, three-call leak
behavior, and totalwatsed3 immediately followed by hillslope watbal in the same
process. Exercise the actual Compose RQ post-processing stage at the production
identity and 12 GiB limit, consume the cache through the report and query paths,
and run a subsequent job. Preserve raw cgroup evidence. Run the complete WEPPpy
suite and independent correctness, QA, and security reviews. Fix and re-run all
affected gates before committing.

## Concrete Steps

Work on Forest and begin with:

    ssh forest
    cd /workdir/wepppyo3
    git status --short --branch
    git pull --ff-only
    git lfs pull
    git lfs fsck
    cd /workdir/wepppy
    git status --short --branch
    git pull --ff-only

Read all applicable `AGENTS.md` files. Record exact SHAs before edits. Verify
fixture checksums against the predecessor package's
`artifacts/all-fixture-checksums.json`.

Run initial focused baselines through the repository wrappers. Expected output
is passing current tests; any baseline failure must be recorded before code is
changed:

    cd /workdir/wepppy
    wctl run-pytest tests/wepp/reports/test_hillslope_watbal.py --maxfail=1
    wctl run-pytest tests/rq/test_batch_rq_retry_selection.py tests/rq/test_batch_deferred_recovery.py --maxfail=1

Keep scripts and raw output under this package's `artifacts/`. Use disposable
directories outside fixture trees for generated summaries. After native source
changes:

    cd /workdir/wepppyo3
    cargo fmt --check
    cargo test -p wepp_interchange_rust
    python3 -m pytest tests/wepp_interchange

After WEPPpy integration:

    cd /workdir/wepppy
    wctl run-pytest tests/wepp/reports/test_hillslope_watbal.py --maxfail=1
    wctl run-pytest tests/wepp/interchange --maxfail=1
    wctl run-pytest tests/nodb --maxfail=1
    wctl run-pytest tests/rq/test_batch_rq_retry_selection.py tests/rq/test_batch_deferred_recovery.py --maxfail=1
    wctl check-rq-graph
    wctl check-test-stubs
    python3 tools/check_broad_exceptions.py --enforce-changed --base-ref origin/master
    wctl run-pytest tests --maxfail=1

Use the predecessor's Compose measurement pattern, setting an explicit 12 GiB
limit. Record `/sys/fs/cgroup/memory.peak`, container state, restart count, job
result, generated hashes, and subsequent-job acceptance. Do not run against or
write into the active openwepp.org batch.

Commit/push the reviewed native repository first, then pin its exact revision
and artifact hash in WEPPpy and commit/push WEPPpy. Verify both with clean
checkouts and LFS downloads. Move this ExecPlan to `prompts/completed/` only
after every exit criterion is met.

## Validation and Acceptance

Acceptance requires exact frozen parity on single-OFE and MOFE inputs; explicit
edge/error/cache/Roads coverage; native median at or below 105% of Python;
standalone incremental memory below 2 GiB on 586 hillslopes; generated-scale
whole-container memory below 9 GiB; and the same-process/RQ sequence below 9
GiB with no OOM, restart, abandoned job, leak, or temporary-file residue.

The NoDb regression must fail before the repair with the observed same-size
stale signature and pass afterward while preserving both the concurrent field
and receipt ID. Static and live job-tree evidence must show unchanged
failure-tolerant finalizer and Omni dependencies. A green unit suite without
this real interleaving is insufficient.

The final broad suite, installed-release tests, startup preflight, documentation
lint, stub checks, exception enforcement, LFS verification, independent review,
and clean remote checkouts must pass. Record exact counts and commands; do not
claim tests that were not run.

## Idempotence and Recovery

All fixture sources are read-only. Benchmark outputs use unique disposable
paths and can be deleted after their hashes and measurements are captured.
Native publication writes a sibling temporary file and atomically replaces only
the requested output; injected failure must retain the previous good cache and
remove the temporary file. Do not repair NoDb tests by disabling stale checks or
clearing Redis globally. If a release build fails, retain the prior canonical
shared object until source and release tests pass together.

Do not deploy from this package. If production promotion is later authorized,
use the immutable WEPPpy image and normal deployment repository rollback. The
code-level rollback is the prior paired revisions; fixture and failed evidence
remain immutable.

## Artifacts and Notes

Create at minimum: baseline revisions and fixture verification; Python oracle;
native contract; NoDb transaction contract and interleaving transcript; parity
results; benchmark commands/raw measurements/summary; scaling results;
same-process and Compose RQ evidence; validation summary; release integration
manifest; independent correctness, QA, and security reviews; findings
disposition; and remote/LFS verification.

Use `confirmed`, `inference`, and `hypothesis` labels from
`docs/claim-discipline.md`. Preserve failures, including candidates that exceed
memory or timing gates.

## Interfaces and Dependencies

Extend the existing `wepp_interchange_rust` crate and
`wepppyo3.wepp_interchange` package; do not create a new extension module. The
native function name should be `hillslope_watbal_to_parquet` unless the contract
review identifies a collision. Its conceptual interface is:

    hillslope_watbal_to_parquet(
        wat_path: str,
        output_path: str,
        topaz_by_wepp_id: Mapping[int, int],
    ) -> Mapping[str, object]

The exact mapping preparation and compact telemetry are frozen before coding.
The Rust accumulator key is `(TopazID, WaterYear)`; area tracking must preserve
first area per `(wepp_id, ofe_id)` before translation and summation.

In WEPPpy, `HillslopeWatbalReport` remains the public facade. It may use pandas
only for the compact native output and derived report views. The batch receipt
helper must expose a clear operation such as
`BatchRunner.set_rq_job_id_fresh(key, job_id)` or a more general existing-style
transaction primitive. Its contract is lock, hydrate current durable state,
apply one idempotent key mutation, dump once, unlock, with narrow observable
errors and no stale-object retry.

No new third-party dependency is expected. If existing Arrow/Parquet and NoDb
primitives cannot implement this safely, stop for dependency or contract review
instead of adding an unreviewed library.

Revision note: created 2026-09-08 from the live openwepp.org OOM and NoDb
contention evidence, reusing the completed native-totalwatsed3 fixtures and
Forest validation model.

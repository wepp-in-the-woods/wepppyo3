# 20260908-hillslope-watbal-native-001

## Status

- state: completed; source published, no image publication or deployment
- date: 2026-09-08
- execution host: `forest` via `ssh forest`
- repositories: `/workdir/wepppyo3` and `/workdir/wepppy`

## Objective

Replace the full-table pandas implementation of WEPPpy's hillslope water-balance
summary producer with a required, bounded-memory Rust producer in
`wepppyo3.wepp_interchange`. Preserve the existing report and cache contracts
without a Python fallback. In the same WEPPpy integration, repair the observed
`batch_runner.nodb` stale write when `run_batch_rq()` records its finalizer job
ID. Prove both changes through the real Forest Compose worker path under the
open-wepp.org 12 GiB memory limit.

## Incident basis

The open-wepp.org batch job `f6f5d7d7-90cf-4408-b28e-7ec3c89568ac` for
`nasa-roses-202608-psbs` produced seven failed watershed jobs. Kubernetes
reported `OOMKilled`, exit code 137, for seven distinct batch-worker pods. The
affected runs were `OR-8`, `OR-202`, `WA-77`, `OR-6`, `OR-7`, `WA-78`, and
`OR-160`. Their compressed `H.wat.parquet` inputs ranged from 1,153,268,555 to
2,819,675,605 bytes. In every recoverable log, WEPP and native totalwatsed3
completed, then the process died seconds after entering
`HillslopeWatbalReport._build_summary()`.

The current producer in
`/workdir/wepppy/wepppy/wepp/reports/hillslope_watbal.py` reads eleven complete
Parquet columns into Arrow, converts the entire table to pandas, adds and
converts columns, performs groupbys, and creates further dataframe copies. This
can expand a 1-3 GB compressed source beyond the 12 GiB cgroup limit.

The same orchestrator emitted:

    batch_rq: failed to persist final_batch_complete_rq job id - stale NoDb write rejected

The warning occurred because child jobs can begin mutating the whole
`batch_runner.nodb` file while the parent retains an older mutation base. The
finalizer ID write is currently best-effort, but the compatibility mirror is
used by batch status and deferred-job recovery. The repair must make that
single-key update from freshly hydrated durable state under the NoDb lock; it
must not suppress the warning or retry a stale object.

## Ownership boundary

- WEPPpy owns run paths, the Watershed WEPP-to-Topaz translation, Roads segment
  mapping policy, report objects and iterators, cache freshness/sidecars, NoDb
  state, RQ orchestration, logging, and user-visible behavior.
- `wepppyo3.wepp_interchange` owns the bounded transformation from projected
  `H.wat.parquet` batches to the compact per-Topaz/per-water-year summary.
- Native support is required. Missing or stale native support must fail
  explicitly through the existing required-native startup/runtime contract.
- No WEPP formula, unit, public report field, cache name, output scope, RQ graph,
  retry classification, or finalizer behavior is intentionally changed.

## Reused fixtures and provenance

Do not copy another fixture set. Reuse the Git LFS fixtures and provenance from
`docs/work-packages/20260907-totalwatsed3-native-producer-001/`.

### Single-OFE parity

Use:

`/workdir/wepppyo3/tests/fixtures/totalwatsed3/decimal-pleasing/`

and the corresponding WEPPpy fixture at:

`/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange/`

The source has 67 single-OFE hillslopes and 4,018 output days. Freeze the
current Python hillslope-watbal summary and iterator results before deleting
the producer.

### Real MOFE parity

Use:

`/workdir/wepppyo3/tests/fixtures/totalwatsed3/incommensurate-stickball/H.wat.parquet`

Its source provenance is
`/wc1/runs/in/incommensurate-stickball/wepp/output/interchange/H.wat.parquet`,
28,590,445 bytes, SHA-256
`3f10ff9f094484e82ef806818279a1565b2d9e4325c0a18b93810e2ae7da173b`.
It contains 68 hillslopes, including one through five OFEs, 7,670 days, and
1,840,800 rows.

### Performance, memory, and scaling

Use:

`/workdir/wepppyo3/tests/fixtures/totalwatsed3/throbbing-sylvan/H.wat.parquet`

Its source provenance is
`/wc1/runs/th/throbbing-sylvan/wepp/output/interchange/H.wat.parquet`,
169,477,490 bytes, SHA-256
`303707b3997b0d2a9335e315bba27279f75713a0cdf185c30b8436a74aa5068c`.
It contains 586 single-OFE hillslopes, 16,071 days, and 9,417,606 rows.

Reuse or minimally generalize the prior package's deterministic scale generator
and measurement harness. Do not commit duplicate Parquet payloads. Verify LFS
objects and the recorded checksums before testing. Treat `/wc1` sources as
read-only and write only to disposable output locations.

## Existing output contract to preserve

The compact cache remains `hillslope_watbal_summary.parquet`, with scoped cache
names for Roads and the same metadata sidecar/version behavior. Preserve exact
column order, Arrow types, nullability, sort order, and values for:

1. `TopazID`
2. `WaterYear`
3. `Area_m2`
4. `Precipitation (mm)`
5. `Percolation (mm)`
6. `Surface Runoff (mm)`
7. `Lateral Flow (mm)`
8. `Transpiration + Evaporation (mm)`

Preserve the current semantics exactly: null numeric inputs become zero; OFE
area is the first `Area` for each `(wepp_id, ofe_id)` and is summed per mapped
Topaz; the seven flux columns are summed per `(TopazID, water_year)`;
transpiration plus evaporation is `Ep + Es + Er`; baseline unknown translator
IDs raise; Roads IDs use the segment manifest mapping and then the existing raw
ID fallback with the existing warnings. Preserve empty input behavior,
source-newer-than-cache rebuilding, legacy cache reading, report headers and
units, `avg_annual_iter()`, `yearly_iter()`, and the historical
`max(number_of_years - 1, 1)` average divisor. This package does not correct or
reinterpret scientific/report semantics.

## Native contract

Add a narrow API to the existing `wepppyo3.wepp_interchange` module. The final
signature must be frozen in a preimplementation contract artifact. It must:

- accept source and failure-atomic output paths plus a bounded WEPP-to-Topaz
  mapping prepared by WEPPpy;
- stream only the required projected columns by row group or bounded record
  batch;
- keep aggregation state proportional to distinct Topaz/year/OFE keys, not
  source row count;
- write the compact summary without returning the full source or a full Python
  dataframe;
- return only compact telemetry, including row counts and any Roads mapping or
  fallback IDs needed for WEPPpy's existing logging;
- reject malformed schemas, unsupported values, missing baseline mappings, and
  write failures with stable Python exceptions; and
- publish atomically in the destination directory and remove temporary files
  after success or failure.

WEPPpy may load the compact summary into pandas to preserve the report facade.
It must not retain an executable pandas/PyArrow full-source producer or runtime
fallback. Any distinct-ID discovery used to prepare the mapping must itself be
bounded; returning one ID per hillslope is acceptable, returning source rows is
not.

## Batch NoDb integration contract

Keep the `BatchRunner.rq_job_ids` compatibility mirror unless a separately
ratified contract checkpoint authorizes its removal. Implement the finalizer-ID
update as a bounded read-modify-write transaction:

1. acquire the distributed lock for `batch_runner.nodb`;
2. hydrate current durable state while that lock is held;
3. apply only the named job-ID key while preserving unrelated fields;
4. dump once and release promptly.

Use a reusable, narrowly named helper only if the repository lacks a safe
primitive. Do not hold the lock while enqueueing jobs, do not globally clear
NoDb caches, do not catch `Exception`, do not silently discard a failed write,
and do not retry `dump()` on the stale instance. Apply the same safe primitive
to the `run_batch_rq` receipt write if the deterministic audit proves it has the
same race. Preserve root-job metadata as authoritative RQ evidence and keep the
NoDb mirror synchronized for current UI and recovery consumers.

Add a deterministic regression in which a child commits an unrelated
`batch_runner.nodb` update after parent hydration but before receipt
publication. It must reproduce `NoDbStaleWriteError` before the fix; afterward
both fields must survive, no stale warning may be emitted, and the finalizer
must retain its failure-tolerant dependencies, including Omni additions.

## Included scope

- Freeze Python hillslope-watbal oracles before producer deletion.
- Implement and test the native summary writer in `wepppyo3`.
- Integrate it as required native functionality in WEPPpy with no fallback.
- Preserve baseline and Roads mapping/cache/report behavior.
- Refresh the canonical py312 release artifact and provenance.
- Update the WEPPpy container startup preflight and publication pin.
- Repair and test the finalizer receipt NoDb contention described above.
- Validate affected report, interchange, batch, RQ, UI snapshot, deferred
  recovery, and generated documentation consumers.
- Run independent correctness, QA, and security reviews and disposition all
  medium/high findings before publication.

## Explicitly out of scope

- Raising the 12 GiB worker limit or reducing worker count as the fix.
- Changing report formulas, units, columns, cache keys, or output scope.
- Reworking unrelated water-balance reports or WEPP output producers.
- Weakening NoDb stale-write detection, adding blind retries, or suppressing
  persistence failures.
- Changing queue names, worker topology, dependency failure policy, or batch
  completion semantics.
- Mutating HPC source runs or the currently active production batch.
- Deploying to openwepp.org. Deployment requires later explicit authorization
  after the running batch drains.

## Correctness gates

- Exact column order, Arrow types, nullability, metadata, row count, keys, sort
  order, headers, units, cache behavior, and iterator ordering.
- Exact equality for integer/year keys; floating-point parity at `rtol=1e-10`,
  `atol=1e-12`. Every exception requires quantified evidence and operator
  disposition.
- Single-OFE and real-MOFE parity, including multi-OFE area handling and merged
  Topaz mappings.
- Empty, null, malformed, missing-column, unknown-ID, legacy-cache, stale-cache,
  baseline, Roads-manifest, Roads-fallback, and atomic-write cases.
- No executable full-source Python producer or fallback remains.
- NoDb interleaving regression proves unrelated state and both receipt IDs
  survive without a stale warning or changed job graph.

## Forest performance and memory gates

Use a one-shot container matching the Forest/open-wepp.org worker image, UID
1000, GID 993, umask 0022, and a 12 GiB cgroup limit. Record exact repository
revisions, image identity, commands, fixture checksums, CPU allocation, cache
condition, input/output bytes, wall/user/system time, and cgroup `memory.peak`.
Do not overwrite frozen oracles.

After one warm-up, compare five Python and five native standalone runs on
`throbbing-sylvan`. Native median wall time must be no greater than 105% of the
Python median and no native run may exceed 115% of that median. Native
incremental peak must be below 2 GiB, preferably below 1 GiB, and at least 50%
lower than Python unless Python cannot complete under 12 GiB.

Generate the same noncommitted approximately 5,860-hillslope scale case used by
the prior package. Native standalone peak must remain below 4 GiB and total
container peak below 9 GiB. Peak growth must be explained by compact key state,
not source-row materialization. Three consecutive runs must show no materially
increasing post-call baseline or temporary-file leak.

Most importantly, run native totalwatsed3 followed immediately by native
hillslope watbal in the same process, then exercise the real Forest Compose RQ
post-processing path under 12 GiB. Require whole-container peak below 9 GiB,
zero OOM events/restarts/`AbandonedJobError`, parity output, successful cache and
report consumption, and successful acceptance of a subsequent job. Isolated
native success does not satisfy this gate.

## Intended write set

### wepppyo3

- `Cargo.lock` and `wepp_interchange/Cargo.toml` only if existing dependencies
  are insufficient
- `wepp_interchange/src/**/*.rs`
- `tests/wepp_interchange/**/*.py`
- `release/linux/py312/wepppyo3/wepp_interchange/**`
- `README.md`, `docs/module-registry.md`, `docs/release-provenance.md`
- `docs/work-packages/README.md`
- `docs/work-packages/20260908-hillslope-watbal-native-001/**`

### WEPPpy

- `wepppy/wepp/reports/hillslope_watbal.py` and its stub
- required-native interchange/startup integration and documentation
- `wepppy/nodb/batch_runner.py` and its stub only as required for the safe
  transaction primitive
- `wepppy/rq/batch_rq.py`, its stub, and the RQ dependency catalog if generated
  metadata changes
- focused tests under `tests/wepp/reports/`, `tests/wepp/interchange/`,
  `tests/nodb/`, `tests/rq/`, and batch route snapshot tests
- release and work-package evidence

Do not modify unrelated dirty files in either repository.

## Validation

Minimum commands, augmented by narrower cases discovered during execution:

```sh
cd /workdir/wepppyo3
git lfs pull
cargo fmt --check
cargo test -p wepp_interchange_rust
python3 -m pytest tests/wepp_interchange
git lfs ls-files
git diff --check

cd /workdir/wepppy
wctl run-pytest tests/wepp/reports/test_hillslope_watbal.py --maxfail=1
wctl run-pytest tests/wepp/interchange --maxfail=1
wctl run-pytest tests/nodb --maxfail=1
wctl run-pytest tests/rq/test_batch_rq_retry_selection.py tests/rq/test_batch_deferred_recovery.py --maxfail=1
wctl check-rq-graph
wctl check-test-stubs
python3 tools/check_broad_exceptions.py --enforce-changed --base-ref origin/master
git diff --check
```

Run the complete WEPPpy suite before handoff because the report facade and NoDb
transaction boundary are broadly shared. Validate the installed py312 release,
not only Cargo's test extension.

## Commit and publication sequence

1. Keep both repositories on their existing branches unless explicitly
   authorized otherwise.
2. Commit and push the reviewed wepppyo3 implementation, tests, docs, and
   release artifact first.
3. Record that exact native revision and shared-object hash in WEPPpy.
4. Commit and push the WEPPpy integration and NoDb repair.
5. Verify both remote revisions and LFS objects from a clean Forest checkout.
6. Build and verify the immutable registry image only if the operator has
   authorized publication; do not deploy it from this package.

## Exit criteria

- Required native hillslope-watbal production is wired with no Python fallback.
- All parity, edge, cache, Roads, atomicity, memory, timing, and repeatability
  gates pass on Forest.
- The same-process totalwatsed3-to-watbal and real Compose RQ gates pass below
  9 GiB with zero restart.
- The finalizer receipt interleaving passes without stale warning or lost state.
- Required release artifacts, preflight, docs, dependency catalog, and
  provenance are current.
- Independent correctness, QA, and security reviews have no unresolved
  medium/high findings.
- Both repositories are clean, committed, pushed, and clean-checkout verified.

## Stop conditions

Stop and preserve evidence if output semantics differ, fixture hashes do not
match, an HPC source would be mutated, native or same-process peak reaches 10
GiB, the native median exceeds the timing gate, NoDb repair loses unrelated
state or changes dependency semantics, LFS/release provenance cannot be
verified, or implementation requires a new dependency or public contract
change not authorized here.

## Security impact

- security_impact: high
- dedicated_security_review_required: yes
- rationale: native code parses large shared-run files and publishes cache
  artifacts, while the paired WEPPpy change touches distributed locking and RQ
  finalizer bookkeeping. Review path validation, symlinks, malformed Parquet,
  bounded allocation, panic/error translation, atomic publication, lock-token
  ownership, stale-state preservation, and failure observability.

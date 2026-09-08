# Deliver bounded ash modeling and native AshPost

This ExecPlan is a living document maintained according to
`/workdir/wepppy/docs/prompt_templates/codex_exec_plans.md`. Keep `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`
current so a new agent can resume from this file alone.

## Purpose / Big Picture

Large ash-enabled watersheds currently finish every hillslope model and then
lose the RQ worker while Python gathers the outputs. After this work, each
completed hillslope releases its Python input state, Rust streams the resulting
parquet files into the same AshPost products, and OR-202 completes below the
production 12 GiB limit. Users see the same reports and datasets without
abandoned jobs.

## Progress

- [x] (2026-09-08) Characterized the three production OOM kills and located the
  immediate failure after all hillslope files and before the first post output.
- [x] Selected `canine-liar`, `assisted-weakness`, and OR-202 fixtures and wrote
  the execution package.
- [ ] Verify Forest revisions, clean trees, tools, fixture provenance, and
  disposable output setup.
- [ ] Capture immutable Python oracles, contracts, and baseline measurements.
- [ ] Implement and validate native streaming AshPost.
- [ ] Implement and validate bounded per-hillslope scheduling and integration.
- [ ] Pass Compose, full-suite, independent-review, and remote-verification
  gates; archive this plan under `prompts/completed/`.

## Surprises & Discoveries

- Observation: the immediate OOM boundary is AshPost, but producer inputs remain
  live across that call.
  Evidence: all three logs reached the final task; all hill parquets and empty
  `ash/post/` directories exist; `Ash.run_ash()` retains `args` while calling
  `AshPost.run_post()` in the same function.

## Decision Log

- Decision: change both object lifetime and aggregation implementation.
  Rationale: either fix alone leaves an avoidable watershed-scale allocation
  risk at the phase boundary.
  Date/Author: 2026-09-08, Codex for Roger Lew.
- Decision: require native AshPost without fallback.
  Rationale: production has demonstrated that the whole-watershed Python path
  is unsafe under the supported memory cap.
  Date/Author: 2026-09-08, Codex for Roger Lew.
- Decision: require the real OR-202 sequential RQ workflow.
  Rationale: it captures producer retention, native aggregation, NoDb, catalog,
  and worker lifecycle in one observable test.
  Date/Author: 2026-09-08, Codex for Roger Lew.

## Outcomes & Retrospective

Pending implementation and Forest acceptance.

## Context and Orientation

The native repository is `/workdir/wepppyo3`. Its existing PyO3 extension is
`wepp_interchange/src/`, exposed as `wepppyo3.wepp_interchange`, tested in
`tests/wepp_interchange/`, and copied to
`release/linux/py312/wepppyo3/wepp_interchange/` for WEPPpy images.

The integration repository is `/workdir/wepppy`. Per-hillslope orchestration is
in `wepppy/nodb/mods/ash_transport/ash.py`. It constructs every work dictionary,
including pandas climate and water-balance tables, stores them in `args`, submits
all futures, and retains them until the surrounding function returns. Each task
writes `ash/H<wepp_id>_ash.parquet` plus plots.

`wepppy/nodb/mods/ash_transport/ashpost.py` owns post-processing. Its
`watershed_daily_aggregated()` reads all hill parquets into DataFrames,
concatenates them, makes deep copies for multiple analyses, then rereads the
files for cumulative results. `AshPost.run_post()` stores compact return-period
dictionaries in NoDb, writes five parquets under `ash/post/`, writes a version
manifest and documentation, and activates the query catalog.

The OR-202 source is on host `hpc` at
`/tank/kubernetes/weppcloud/weppcloud-wc1/pvc-d4c5528b-3204-438b-909a-61af286d6fea/batch/nasa-roses-202608-psbs/runs/OR-202/`.
Forest has verified noninteractive SSH access to `hpc`; copy only required
inputs into a disposable Forest directory and never write to the source.

The completed predecessor packages
`docs/work-packages/20260907-totalwatsed3-native-producer-001/` and
`docs/work-packages/20260908-hillslope-watbal-native-001/` demonstrate the
native release, parity, cgroup measurement, Compose, review, and native-first
publication pattern. Reuse their harness structure without copying unrelated
fixtures or claims.

## Plan of Work

First verify both Forest repositories are clean and fast-forwarded, record exact
SHAs and toolchains, and inspect each fixture's configuration to confirm the
stated model. Inventory every per-hillslope schema and AshPost output. Hash the
read-only sources and create disposable destinations outside `/wc1` fixture
trees.

Before changing code, run current Python AshPost on both small fixtures and
capture all five parquet products, schemas, metadata, row order, values, return
period dictionaries, facade properties, version manifest, generated docs, and
catalog behavior. Capture failure/edge behavior. Reproduce and measure OR-202
in a disposable copy under a 12 GiB container; preserve OOM evidence or a
successful high-memory oracle without modifying its source.

Freeze a native API and multi-file transaction contract in artifacts. WEPPpy
should pass a bounded manifest of path, Topaz ID, area, and burn class. Rust
must validate the manifest, stream projected row groups, maintain only bounded
keyed aggregates, calculate existing return-period products, and stage all five
outputs. Publication must never expose a mixed old/new set after failure.

Implement the Rust pieces in testable layers: manifest validation and path
containment, schema projection, row-batch aggregation, output schema/metadata,
return-period calculations, atomic multi-file staging, cleanup, and PyO3 error
translation. Add generated edge fixtures and fault injection. Refresh the
canonical py312 release only after source tests pass.

Then refactor the Python scheduler into a lazy work iterator with a bounded
in-flight future map. Consume, discard, and replace futures one at a time while
preserving failure cancellation and progress. Ensure the executor and all large
work references are gone before invoking the native function. Reduce AshPost
Python to manifest/policy orchestration, compact result persistence,
documentation, and catalog activation. Remove its executable pandas aggregation
path and prohibit fallback.

Run exact parity for both model variants, then memory/timing/repeatability tests.
Finally execute OR-202 through the real Compose RQ path under 12 GiB, inspect
cgroup peak/events and container restart, consume every facade/output, and run a
subsequent small job on the same worker. Run focused and full suites plus
independent reviews, resolve findings, update docs/provenance, and push
native-first.

## Concrete Steps

On Forest begin with:

    ssh forest
    cd /workdir/wepppyo3
    git status --short --branch
    git pull --ff-only
    git lfs pull
    git lfs fsck
    cd /workdir/wepppy
    git status --short --branch
    git pull --ff-only

Read all applicable `AGENTS.md` files. Record fixture manifests and hashes under
this package's `artifacts/`. Do not place generated output in a source run.

After native changes run:

    cd /workdir/wepppyo3
    cargo fmt --check
    cargo test -p wepp_interchange_rust
    python3 -m pytest tests/wepp_interchange --maxfail=1
    python3.12 -c "import wepppyo3.wepp_interchange"

After integration run:

    cd /workdir/wepppy
    wctl run-pytest wepppy/nodb/mods/ash_transport/tests --maxfail=1
    wctl run-pytest tests/wepp/interchange --maxfail=1
    wctl run-pytest tests/nodb --maxfail=1
    wctl run-pytest tests/rq --maxfail=1
    wctl check-rq-graph
    wctl check-test-stubs
    python3 tools/check_broad_exceptions.py --enforce-changed --base-ref origin/master
    wctl run-pytest tests --maxfail=1

Use the predecessor package's Compose measurement structure with a hard 12 GiB
limit. Record raw commands and outputs, not only summaries. Move this plan to
`prompts/completed/execplan.md` only after every exit criterion passes.

## Validation and Acceptance

Acceptance requires exact or tolerance-governed parity for both small fixtures,
all specified malformed/atomic cases, and proof that the installed release is
the tested binary. OR-202 must complete the real sequential worker path below
9 GiB with all five outputs, NoDb and RQ completion, no OOM event or restart,
and a subsequent accepted job. Inspecting code or passing a microbenchmark does
not satisfy wired completion.

Independent correctness review must validate formulas and schemas against the
frozen oracle. QA must audit test coverage and commands. Performance review must
audit cgroup measurement and cache conditions. Security review must cover paths,
symlinks, malformed inputs, bounded allocation, panic translation, multi-file
atomicity, temporary permissions, and cleanup. No medium/high finding may remain.

## Idempotence and Recovery

All captures and benchmarks write to unique disposable directories and may be
rerun. Never delete or overwrite source fixture data. Atomic fault tests must
start from copies and verify the prior complete output set remains readable. If
implementation or validation fails, preserve logs and cgroup evidence, update
this plan and tracker, and leave production untouched. Revert code through
normal Git commits; do not reset shared checkouts destructively.

## Artifacts and Notes

Store an indexed artifact for provenance, frozen contracts, baseline/candidate
commands, hashes, parity, timing, cgroup memory, Compose job results, changed
files, release identity, and each independent review. Label assertions as
`Static:` or `Ran:` according to `docs/work-packages/README.md`.

## Interfaces and Dependencies

The final native function belongs to `wepppyo3.wepp_interchange`. Freeze the
exact signature before implementation; it must accept only paths, simple scalar
metadata, recurrence values, and optional frozen schema metadata. It returns
compact telemetry and return-period mappings, never pandas objects or complete
source rows. Reuse the workspace's Arrow/Parquet stack. WEPPpy retains the
`AshPost` class and public properties but delegates production aggregation to
the required native function.

Revision note: initial plan created 2026-09-08 from the production OOM evidence
and Roger Lew's fixture and ownership decisions.

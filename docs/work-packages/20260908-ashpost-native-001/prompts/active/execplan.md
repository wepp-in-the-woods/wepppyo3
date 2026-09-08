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
- [x] (2026-09-08) Characterized the production failure boundary and froze both model oracles.
- [x] (2026-09-08) Operator confirmed existing individual-file writer scope; added storage proposals are superseded.
- [x] (2026-09-08) Implemented streaming native AshPost and bounded parent input loading. All five tables, mappings, small-model facades, docs and version manifests match frozen Python; OR-202 tables also match.
- [x] (2026-09-08) Five corrected warm measurements per implementation/model pass: 81.8%/82.0% lower incremental memory and 83.1%/83.2% lower runtime for Srivastava/Watanabe.
- [x] (2026-09-08) OR-202 standalone Python completed at 8.40 GiB / 159.4 seconds with the corrected sampler; native completed at 0.32 GiB / 44.4 seconds. Both used a 12 GiB limit.
- [x] (2026-09-08) Eight consecutive native calls show bounded resident memory and no temporary residue. Real 12-worker Compose RQ model -> post -> totalwatsed3 completed OR-202, then canine-liar on the same worker; peak 4.43 GiB, zero OOM/restart events.
- [x] (2026-09-08) Both models' 20 freshly regenerated hillslope Parquet files are byte-identical under old/new schedulers.
- [x] (2026-09-08) Independent correctness, security, QA and performance reviews have no unresolved medium/high finding. Installed native suite: 139 passed; Rust: 98 + 17 passed; focused integration: 23 passed; combined interchange/report: 85 passed, 1 skipped. Both stubtests, stub inventory, RQ graph, broad-exception and scoped isolation checks pass.
- [x] (2026-09-08) Full WEPPpy suite passed: 7,742 passed, 72 skipped in 792.90 seconds.
- [ ] Bind final release provenance, push native-first, verify remotes, and archive this plan.
## Surprises & Discoveries

- Ran/Static: Watanabe saved post hydrology differs from the current upstream
  generation, which RQ rebuilds after AshPost. Both previous Python and current
  native totalwatsed3 match current Streamflow within tolerance. Fresh Python
  AshPost captures on the same inputs match exactly. Evidence:
  `../../artifacts/watanabe-hydrology-investigation.json` and
  `../../artifacts/watanabe-oracle-repeatability.json`.
- Discovery: the initial plan introduced unnecessary set-atomic publication
  requirements. The operator rejected that expansion; NFS directory exchange
  and custom recovery design are not blockers to the established writer pattern.

- Observation: the immediate OOM boundary is AshPost, but producer inputs remain
  live across that call.
  Evidence: all three logs reached the final task; all hill parquets and empty
  `ash/post/` directories exist; `Ash.run_ash()` retains `args` while calling
  `AshPost.run_post()` in the same function.

## Decision Log

- Decision: use the established individual-file producer/aggregate-reader
  pattern, without new multi-file transactions, rollback/recovery, generation
  directories, migrations, or NFS-specific mechanisms. Preserve existing writer
  error handling and rerun behavior. No all-five staging barrier is required.
  Rationale: the operator explicitly confirmed this matches the original intent;
  the existing interchange -> totalwatsed3 workflow is the precedent.
  Date/Author: 2026-09-08, Roger Lew; recorded by Codex.
- Decision: compare fresh Python/native results using identical point-in-time
  inputs and preserve historical saved-output drift as evidence. RQ ordering
  and scientific formulas remain unchanged.
  Date/Author: 2026-09-08, Codex for Roger Lew.

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
Implementation, parity, performance, bounded-memory and real RQ workflow gates
pass. Native-first commit/remote verification remains
active. Production is unchanged. Durable scope is recorded in WEPPpy
`docs/schemas/output-scope-contract.md`, "AshPost file-production scope".

A full-suite run exposed test-only import cleanup that deleted real interchange
modules and left stale module references in report tests. Cleanup now removes
only loader-owned injections and preserves real module identity; the regression,
combined report suite and scoped isolation checks pass. No production fallback
or storage mechanism was added to address this test defect.
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

Freeze the native API in artifacts, including ordered path/Topaz/area/burn
metadata, optional hydrology paths, and independent ash WEPP-ID selection.
Rust validates and streams inputs, maintains bounded keyed aggregates, computes
existing return-period products, and writes outputs individually using the
existing writer behavior. Do not add a multi-file transaction or recovery layer.

Implement the Rust pieces in testable layers: manifest validation and path
containment, schema projection, row-batch aggregation, output schema/metadata,
return-period calculations, existing per-file writing, and PyO3 error
translation. Add relevant generated edge fixtures and write-failure coverage. Refresh the
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
    wctl run-pytest tests/nodb/mods/test_ashpost_no_data.py tests/nodb/mods/test_ash_transport_run_ash.py --maxfail=1
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
all specified malformed-input/write-failure cases, and proof that the installed release is
the tested binary. OR-202 must complete the real sequential worker path below
9 GiB with all five outputs, NoDb and RQ completion, no OOM event or restart,
and a subsequent accepted job. Inspecting code or passing a microbenchmark does
not satisfy wired completion.

Independent correctness review must validate formulas and schemas against the
frozen oracle. QA must audit test coverage and commands. Performance review must
audit cgroup measurement and cache conditions. Security review must cover paths,
symlinks, malformed inputs, bounded allocation, panic translation, existing
writer behavior, temporary permissions, and cleanup. No medium/high finding may remain.

## Idempotence and Recovery

All captures and benchmarks write to unique disposable directories and may be
rerun. Never delete or overwrite source fixture data. Write-failure tests use
disposable outputs and preserve existing error propagation and rerun behavior. If
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

Revision note: operator confirmed the original simple scope; removed assistant-
added transaction/recovery requirements and recorded existing-writer precedent.

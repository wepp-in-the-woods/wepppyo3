# Tracker: bounded ash modeling and native AshPost

- Status: `READY`
- Execution host: `forest`
- Primary repository: `/workdir/wepppyo3`
- Integration repository: `/workdir/wepppy`
- Security impact: `high`; independent review required

## Progress

- [x] Production OOM boundary characterized from Kubernetes, RQ, logs, and
  persisted artifacts.
- [x] Srivastava, Watanabe dynamic, and production-scale fixtures selected.
- [x] Package, active ExecPlan, execution prompt, and evidence directory
  scaffolded.
- [ ] Verify clean Forest checkouts, exact revisions, fixture paths, models,
  schemas, checksums, and writable disposable destinations.
- [ ] Freeze both Python model/output/facade oracles and reproduce the OR-202
  12 GiB failure without mutating source data.
- [ ] Freeze the native API, multi-file transaction, and bounded scheduler
  contracts before implementation.
- [ ] Implement streaming Rust AshPost, tests, and py312 release refresh.
- [ ] Integrate required native AshPost and bounded hillslope scheduling in
  WEPPpy.
- [ ] Pass parity, edge, atomicity, timing, memory, repeatability, and real
  Compose RQ gates.
- [ ] Complete broad validation and independent correctness, QA, performance,
  and security reviews.
- [ ] Commit/push both repositories in native-first order and verify clean
  remotes and LFS objects.

## Incident evidence

- Production job: `8eb973ed-d7e6-4b5d-be24-b371237cea1e`.
- OR-8, OR-202, and WA-77 containers were `OOMKilled` with exit code 137.
- Their logs completed all 2,182, 1,090, and 2,336 hillslope ash tasks.
- Per-hillslope parquet files exist; `ash/post/` exists but contains no files.
- The Python producer retains all work-item DataFrames/futures, then AshPost
  loads, concatenates, deep-copies, and rereads the watershed.

## Decisions

- Decision: fix both the producer lifetime and AshPost aggregation boundaries.
  Rationale: native post-processing must not leave unnecessary watershed-sized
  Python retention or rely on incidental garbage collection.
- Decision: faithful native extraction with no runtime Python fallback.
  Rationale: the current whole-watershed pandas path is the demonstrated OOM
  path and silently retaining it would reintroduce the production failure.
- Decision: accept only the real sequential OR-202 Compose RQ path under 12 GiB.
  Rationale: isolated native benchmarks do not measure retained producer state
  or the actual worker lifecycle.
- Decision: source fixtures are read-only and OR-202 is not committed.
  Rationale: preserve production evidence and avoid duplicating large data.

## Handoff

Dispatch `prompts/execute.md` on Forest. The executing agent must keep
`prompts/active/execplan.md` and this tracker current. No registry publication or
openwepp.org deployment is authorized by this package.

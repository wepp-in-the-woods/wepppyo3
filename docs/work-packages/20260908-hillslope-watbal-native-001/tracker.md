# Tracker: native hillslope water balance and batch receipt contention

- Status: `COMPLETED`
- Execution host: `forest`
- Primary repository: `/workdir/wepppyo3`
- Integration repository: `/workdir/wepppy`
- Security impact: `high`; independent review required

## Progress

- [x] Production failure and seven OOM-killed workers characterized.
- [x] Existing pandas producer, report/cache contract, and finalizer stale-write
  call site inventoried.
- [x] Prior single-OFE, MOFE, performance, and scaling fixtures selected for
  reuse without duplicate LFS payloads.
- [x] Package, active ExecPlan, execution prompt, and evidence directory
  scaffolded.
- [x] Verify clean Forest checkouts, fixture LFS objects, and baseline revisions.
- [x] Freeze Python summary/cache/report oracles and reproduce stale-write
  failure; retain production OOM incident evidence.
- [x] Freeze native API and NoDb transaction contracts before implementation.
- [x] Implement Rust producer, focused tests, and py312 release refresh.
- [x] Integrate native-only WEPPpy report path and safe batch receipt update.
- [x] Pass parity, edge, timing, memory, same-process, and Compose RQ gates.
- [x] Complete broad validation and independent correctness/QA/security review.
- [x] Commit/push both repositories in native-first order and verify remotes.

## Incident evidence

- Production job: `f6f5d7d7-90cf-4408-b28e-7ec3c89568ac`.
- Seven failed leaves correspond exactly to seven pod terminations with
  `OOMKilled`, exit code 137.
- Each log completed native totalwatsed3 and entered `_run_hillslope_watbal`
  before termination.
- Failed `H.wat.parquet` sizes span 1.15-2.82 GB compressed.
- The batch orchestrator separately logged a same-size `NoDbStaleWriteError`
  while persisting `final_batch_complete_rq`, but RQ marked the orchestrator OK.

## Decisions

- Reuse the prior package's fixtures and scale harness; do not commit duplicate
  large Parquet files.
- Acceptance is the real sequential post-processing path in one 12 GiB process,
  not an isolated Rust microbenchmark.
- Preserve the `BatchRunner.rq_job_ids` compatibility mirror through a fresh,
  lock-owned transaction; do not hide or weaken stale-write detection.
- Do not deploy to openwepp.org while the active batch is running or without a
  later explicit deployment request.

## Handoff

Native release: `0ba67a55f35c7193c6313fb5487d6874dc42e198`.
WEPPpy integration: `59d1b94b67e8c35dfcda363b0ae2927727f148c6`.
Both source revisions are pushed and verified from clean Forest checkouts;
LFS integrity and all 127 fixture checksums pass. Clean runtime preflight and
62 WEPPpy plus 18 native tests pass. No image was published or deployed.

Final native median: 2.8502 s versus Python 3.1755 s. Compose peak:
1,165,426,688 bytes; all three jobs finished. Full suite: 7732 passed,
72 skipped. Independent correctness, QA, and security reviews are closed.
Required stub completeness and report stubtest pass; the extra source-only
BatchRunner stubtest remains limited by reproduced baseline typing errors.
See `artifacts/validation-summary.md`, `artifacts/clean-remote-verification.json`,
and `prompts/completed/execplan.md` for final evidence and decisions.

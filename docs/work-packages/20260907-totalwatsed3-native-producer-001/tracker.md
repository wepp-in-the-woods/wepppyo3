# Tracker: native totalwatsed3 producer

- Status: `COMPLETED`
- Execution host: `forest`
- Primary repository: `/workdir/wepppyo3`
- Integration repository: `/workdir/wepppy`

## Progress

- [x] Incident characterized and HPC fixture provenance frozen.
- [x] Copy/checksum ten HPC artifacts; verify staged LFS pointer storage.
- [x] User approves current-producer single-OFE oracle; legacy fixture preserved.
- [x] Capture Python contract, build strict comparator, and freeze synthetic cases.
- [x] Implement source candidate and focused Rust/Python tests.
- [x] Pass the three approved real-oracle comparisons and nineteen synthetic cases.
- [x] Close identified null/nonfinite contract edges; 26 Python tests pass.
- [x] Complete independent correctness, QA and security reviews; no open blockers.
- [x] Record operator acceptance of measured 107.1821% timing.
- [x] Run scaling and three-consecutive-call repeatability.
- [x] Pass scaling gate after metadata compaction: 1.45178 times versus maximum 1.5.
- [x] Integrate required native path and delete Python producer/fallback.
- [x] Refresh release artifact and pass downstream/Compose RQ gates under 12 GiB.
- [x] Commit/push both repositories and verify clean remote LFS checkout.
- [x] Publish and verify immutable GHCR image.

## Evidence

fixture-manifest.md, all-fixture-checksums.json, python-oracle.md,
native-contract.md, parity-results.md, native-build.json,
benchmark-results.md, benchmark-summary.json, benchmark-measurements.json,
native-tests.log, cargo-test.log, validation-summary.json, and
final-disposition.md are under artifacts/.

All independent review gates and paired repository/LFS publication are complete.
GHCR workflow 34192361367 succeeded for d8fbe9f4 with native bb7451ee. The
published image passes 25 parity cases and the real RQ chain at 1.11 GiB. See artifacts/publication.json.

## Decision authority

The approved single-OFE oracle and rationale are recorded in package.md,
Approved single-OFE oracle authority (2026-09-07). The user authorized resumption
after the old-oracle mismatch, not relaxation of any timing or memory gate.

## Current scaling disposition

Metadata projection and compact chunk locations reduced the near-incident peak
from 871.54 MiB to 648.48 MiB. The 5,860/586 ratio is now 1.45178 and passes.
All six scale outputs pass parity. See artifacts/scaling-optimization.md.
Five-repeat timing is 3.813276 seconds native versus 3.541915 Python (107.6614%).
User authorized release/integration after this measurement; the original 105%
target remains historical evidence, not relabeled as a pass.

## Accepted timing disposition (2026-09-07)

The user explicitly accepted the measured 107.18% native/Python median on the
586-hillslope fixture (exact ratio 107.1821%, native 3.784380 s, Python 3.530796 s).
This satisfies the timing disposition for this candidate and authorizes continued
execution. It is a scoped acceptance of the measured result, not a general
relaxation of correctness, memory, scaling, worst-run, or workflow gates.
Remaining contract corrections and integration should not materially regress
this accepted performance. Retain the measurement and original 105% target as
evidence rather than reclassifying the original measurement as a target pass.

## Scaling optimization resumption

User requested reducing the 1.7906-times ratio toward 1.5. Profiling confirms
whole-source column metadata grows with row-group count. Project retained
metadata to consumed columns, then rerun parity, memory scaling, and timing.
Original failed evidence remains preserved.

## Optimization handoff

Scaling optimization is complete: 1.45178 ratio, 28 Python tests, 112 Rust
executions, all six scale parities and twenty benchmark parities pass. Ten-call
repeatability shows no accumulating anonymous state. Remaining package work is
listed above; see artifacts/final-disposition.md for exact timing and limits.

## Release/integration progress

Canonical release and required-native facade are wired. Installed release: 75
tests; public facade: 22 frozen parity cases. All named downstream suites pass
in isolated processes. Real Compose RQ stage, downstream processing and next job
pass at 868.96 MiB after fixing the full-table README preview OOM. Broad suite
passes: 7,720 tests, 72 skips. Independent reviews and publication remain pending.

## Release/integration handoff

Requested release and integration are validated. Broad suite: 7,720 passed,
72 skipped; final targeted integration: 83 passed, 1 skipped; installed native
release: 75 passed; facade oracle comparisons: 22 passed; downstream suites:
148 passed. Actual RQ workflow: 868.96 MiB peak with zero OOM/restarts.
Independent sign-off and commit/push/LFS publication remain pending.

## Review and publication resumption

User authorized review and container publication. Both remote branches still
match the implementation base revisions. The canonical common-image workflow
pins an older native revision, so publication requires updating that one pin
after native review, commit, push, and remote LFS verification.

Independent reviewers found COR-01 (null area), COR-02 (first-NaN ash metadata),
and SEC-02 (malformed footer assertion escape). Repairs and regression evidence
are tracked in artifacts/review-fixes.md; publication waits for repaired release
sign-off. Three extra frozen cases bring fixture coverage to 127 Parquet files.

## Reviewed release publication

All independent reviews pass for bf21f5e5; 89 native-module tests and 25 facade
oracles pass. Final real RQ workflow peaks at 861.34 MiB with no failures. Timing
is 106.5921%; controlled symmetric host-prewarm scaling is 1.4398346. Preserve the
mixed-cache 2.07079 raw-ratio failure and qualify the passing cache condition.
Native-first repository publication is proceeding while final WEPPpy broad
revalidation runs. See artifacts/final-disposition.md and review-performance.md.

## Final closure

All review, parity, accepted timing, controlled-cache scaling, workflow and
publication gates are complete. Native release bb7451ee and WEPPpy d8fbe9f4 are
published and clean-remote verified. GHCR digest 3201f5cd passes packed-code,
25-case parity and three-job RQ validation with no source overlays. Cleanup is
complete. Artifacts/publication.json contains the full immutable identifiers;
container-publication.md explains the validation boundary and retained limits.

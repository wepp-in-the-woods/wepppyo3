# Execute the native totalwatsed3 producer on Forest

This living ExecPlan follows /workdir/wepppy/docs/prompt_templates/codex_exec_plans.md.
The binding scope, exact fixture checksums, numerical tolerances, memory and timing
limits, stop conditions, and publication order are incorporated from ../../package.md.

## Purpose / Big Picture

Replace the existing Python/DuckDB producer with a required native file transform
so totalwatsed3 production completes within a 12 GiB worker. This is a faithful
extraction: preserve schema, metadata, units, recurrence, joins, nulls, ash behavior,
and MOFE outlet flow. Implementation alone is insufficient: closure requires the
rebuilt release module, generated output parity, and the real Forest RQ workflow.

## Progress

- [x] Read both root guides, package, and host requirements; Forest verified.
- [x] Copy ten HPC artifacts and verify their exact checksums.
- [x] User approves current-producer single-OFE oracle; legacy fixture preserved.
- [x] Freeze native contract, comparator, and nineteen synthetic oracle cases.
- [x] Implement and compile source candidate; 112 Rust test executions pass.
- [x] Run 26 targeted native Python tests, including all three real oracles.
- [x] Run five measured executions per implementation for MOFE and performance fixtures.
- [x] User accepts the measured 107.1821% timing result and authorizes resumption.
- [x] Close nullable PASS-class and nonfinite Interception edges; six additional oracle cases pass.
- [x] Run six scales and three-consecutive-call stability; all outputs pass parity.
- [x] Pass scaling gate after metadata compaction (1.45178 observed versus 1.5 maximum).
- [x] Wire required native WEPPpy facade and retire Python producer.
- [x] Refresh release provenance and pass downstream and live Compose RQ gates.
- [x] Complete independent correctness, QA and security reviews; all findings closed.
- [x] Rebuild and validate repaired release, 25 facade oracles, resource and real RQ gates.
- [ ] Publish native then WEPPpy, verify remote LFS and publish/verify GHCR image.

## Surprises & Discoveries

Independent review found nullable-area numerator poisoning, first-NaN ash
metadata mismatch and a malformed-footer Rust assertion escape. Reproduce and
close each before publication; see artifacts/review-fixes.md.

Existing output includes pandas schema metadata in addition to the declared schema.
Native parity must preserve that metadata; field metadata and version metadata match
the current Python schema for both copied oracles.

The single-OFE frozen oracle has 68 columns against 79 currently, and its Runoff
matches WAT Q depth rather than the current PASS runvol depth. Runoff and
Streamflow each fail on 703 rows, with maximum absolute error 0.106790437 mm.
Both outputs carry dataset_version 1.2. Evidence: artifacts/oracle-comparison.json
and artifacts/parity-results.md at the package root.

The serial candidate matched all real oracles but took 18.16 seconds on the
586-hillslope fixture. Bounded parallel scans and compact per-date validity state
reduced this to 3.784380 seconds median. The formal gate still fails against
Python's 3.530796 seconds median. Peak native memory is 504,229,888 bytes;
the 50% reduction margin is only 4,096 bytes. Source inspection also identifies
nullable PASS-class and infinite Interception edges, now closed with frozen tests.
Evidence: artifacts/benchmark-results.md and artifacts/parity-results.md.

## Decision Log

Complete required independent review using the user-authorized reviewer agents.
Repair confirmed parity and metadata-error defects, freeze additional old-producer
oracles, rebuild the paired release and revalidate before publishing. These are
contract restorations; no output formulas or accepted tolerances change.

2026-09-07: Execute exclusively on Forest, using repository fixture copies and
disposable output directories. Source run trees remain read-only.
2026-09-07: Preserve the package's stop conditions without relaxing tolerance or
resource limits. Do not deploy to openwepp.org.

2026-09-07: Stop before native implementation under the package output-semantics
stop condition; preserve all frozen oracles. Recommend a reviewed current-producer
single-OFE oracle instead of silently changing parity expectations.

2026-09-07: After the user's explicit approval, preserve the captured 79-column
single-OFE output as the parity authority under LFS; retain the old fixture.
2026-09-07: Use at most twelve std threads, projected 8192-row batches, and one
hillslope's join state per thread. Compact date aggregates use validity bitmaps;
no additional dependency is introduced.
2026-09-07: Stop at the explicit timing condition after five repetitions; do not
wire the source candidate, refresh the canonical release, or publish either repo.

## Outcomes & Retrospective

Source implementation exists and the approved real fixtures plus thirteen
synthetic cases pass. The implementation is not wired or released. The timing
gate failed and the package remains incomplete. Memory is far below the absolute
limit, but the measured relative-memory threshold has negligible margin.
Two nullable/nonfinite source-inspection edges need correction and coverage;
no claim of full parity closure is made. See artifacts/final-disposition.md.

## Context and Orientation

WEPPpy's wepppy/wepp/interchange/totalwatsed3.py aggregates H.pass and H.wat,
joins optional soil/element metrics, handles ash, and applies baseflow. Rust belongs
in wepppyo3/wepp_interchange/src with Python exposure through src/lib.rs and
release/linux/py312/wepppyo3/wepp_interchange. Existing ParquetSink provides
same-directory staging and failure-atomic publication. Use existing Arrow/Parquet
dependencies; additional query engines are outside scope.

## Plan of Work

First freeze the producer revision and inspect schemas, joins, and optional behavior.
Store fixtures with scoped Git LFS attributes and verify staged pointers. Capture
oracles into disposable directories without overwriting the supplied oracle files.
Record native arguments and the compatibility/regression plan in
artifacts/native-contract.md before source edits.

Implement projected bounded record-batch scans with aggregation by full date key.
Resolve MOFE outlet selection and area joins without whole-run materialization.
Preserve deterministic sorting, all arithmetic, metadata, and failure publication
contracts. Expose compact write telemetry only. Add focused Rust and release API
tests, then compare the generated outputs using the package's strict comparator.

Benchmark on Forest under 12 GiB after warm-up, five runs per implementation.
Measure cgroup peak, baseline, CPU, bytes, cache condition, and timings; record the
73 through 5860-hillslope scale series and three-call stability. Only after parity
and resource gates pass, replace the WEPPpy producer with native orchestration.
Exercise the named downstream suites and the real Compose worker with twelve CPUs.
Complete independent/security reviews, update provenance/docs, and publish native
changes first, then the WEPPpy integration pin. Verify clean remote LFS retrieval.

## Concrete Steps

From Forest /workdir/wepppyo3 run cargo fmt --check,
cargo test -p wepp_interchange_rust, and the installed py312 release API suite
tests/wepp_interchange. From /workdir/wepppy use the exact wctl commands in
package.md and broader tests required by touched contracts. Run git diff --check
in both trees and verify git lfs ls-files plus staged pointer blobs. Record exact
benchmark and Compose commands with measured results as they are established.

## Validation and Acceptance

Every exit criterion in package.md is required. Numerical comparison uses
rtol=1e-10 and atol=1e-12, exact date/integer equality, and identical schemas,
including field and schema metadata. All resource, runtime, scaling, downstream,
release, review, and remote retrieval gates must pass; missing evidence is not a pass.

## Idempotence and Recovery

Never overwrite fixture oracles or source runs. Use unique disposable outputs and
keep starting revisions in evidence. Do not alter branches or unrelated files.
At any specified stop condition preserve evidence and mark the tracker stopped;
do not publish incomplete native integration.

## Artifacts and Notes

artifacts/fixture-checksums.json records all ten verified copies. Required evidence
files are enumerated in tracker.md and use Static:/Ran: sections with confirmed,
inference, or hypothesis labels. Update this plan and tracker at each handoff.

## Interfaces and Dependencies

Reuse Arrow 53.4.1, Parquet 53.4.1, PyO3 0.22, and ParquetSink. Keep
run_totalwatsed3 public arguments stable. The implemented source signature is documented in artifacts/native-contract.md
at the package root. It returns compact telemetry and releases the GIL during
scans. The production facade and packaged native API remain unchanged.

Revision note: created 2026-09-07 after host and checksum preflight.

Revision note: stopped 2026-09-07 after reproducing and quantifying the single-OFE
oracle incompatibility; no acceptance criterion was relaxed.

## Approved single-OFE oracle authority (2026-09-07)

The user approved designating the captured current-producer output as the
single-OFE parity oracle and resuming execution, while preserving the old
WEPPpy fixture. The active oracle is now
`tests/fixtures/totalwatsed3/decimal-pleasing/totalwatsed3.parquet` in wepppyo3,
SHA-256 `e7c6e2f0a37093a59540dff0b67bbb44a25f9abdea6e145a6bd65d4c37809550`.
Input files continue to come from the unchanged WEPPpy decimal-pleasing fixture.
The new oracle was captured from WEPPpy revision
`3c51780f50e2599ef72b03214c35bf20538e55c4`, with gwstorage=0, bfcoeff=0.04,
dscoeff=0, and no ash. It preserves current PASS-runoff semantics and all
79 columns. Reverting to the older WAT-Q runoff formula was rejected because
this package is a faithful extraction of current production behavior.
The old 68-column fixture remains historical evidence, not the native parity
authority. No numerical tolerance or other package gate changes.

Revision note: stopped after the 2026-09-07 five-repeat timing gate; source,
tests, measurements, known gaps, and hashes are preserved without publication.

## Accepted timing disposition (2026-09-07)

The user explicitly accepted the measured 107.18% native/Python median on the
586-hillslope fixture (exact ratio 107.1821%, native 3.784380 s, Python 3.530796 s).
This satisfies the timing disposition for this candidate and authorizes continued
execution. It is a scoped acceptance of the measured result, not a general
relaxation of correctness, memory, scaling, worst-run, or workflow gates.
Remaining contract corrections and integration should not materially regress
this accepted performance. Retain the measurement and original 105% target as
evidence rather than reclassifying the original measurement as a target pass.

## Scaling stop update (2026-09-07)

Progress: 26 Python tests and 112 Rust test executions pass after edge fixes.
Surprise: near-incident peak is 871.54 MiB but 1.7906 times baseline, over 1.5.
Decision: timing acceptance does not relax scaling; stop and preserve evidence.
Outcome: all six scale outputs pass parity and repeated calls show no monotonic
anonymous-memory retention. Integration, release, reviews, and publication remain
incomplete. See ../../artifacts/memory-scaling-results.md and final-disposition.md.

## Scaling optimization resumption

2026-09-07: User requests getting closer to the 1.5-times limit. Resume profiling
and bounded-memory optimization; preserve the failed measurements and unchanged
parity/resource gates. No data or output-schema changes are intended. Validate
with existing frozen contracts, real oracles, and the same scale containers.

## Scaling optimization outcome

Progress: metadata projection alone reached 1.6728; compact physical chunk
locations and reconstruction of at most sixteen groups per scan reached 1.45178.
Surprise: full source metadata accounted for approximately 359 MB at 5,860 hillslopes.
Decision: retain only read-required chunk properties and preserve physical offsets,
Arrow types, codecs, and singleton indexing. No output or numerical contract changes.
Outcome: six scale parities pass, peak decreases 25.59%, 28 targeted tests and
112 Rust executions pass. Evidence: ../../artifacts/scaling-optimization.md.
Five-repeat timing is 3.813276 seconds native versus 3.541915 Python (107.6614%).
All twenty benchmark outputs pass parity. Ten-call repeatability passes without
accumulating anonymous state; the optimization request is complete. Release,
integration, workflow acceptance, reviews, and publication remain pending.

## Release and integration resumption

User explicitly requested release build and WEPPpy integration after reviewing
the optimized measurements. Preserve the public facade, schema and scalar
metadata resolution; remove all Python aggregation and use the existing required
native boundary. Rebuild and atomically refresh the canonical shared object,
paired startup hash and API inventory. Validate through installed release imports,
frozen oracles, downstream suites, and a dedicated Compose worker workflow.

Integration discovery: native publication succeeded at 5,860 hillslopes, but
README generation then used pq.read_table on complete H.* files and OOM-killed
the RQ work horse at 12 GiB. Fix the existing documentation reader to read schema
and at most three preview rows. This is required to complete the authorized
workflow; no output schema/formula or preview semantics change. Preserve failed
worker evidence and rerun the same production stage and downstream dependencies.

Release/integration outcome: 75 installed-release tests and 22 public-facade
oracles pass. All eight isolated downstream suites pass. The real Compose RQ
workflow passes at 911,167,488 bytes with no OOM or restart after the bounded
preview correction. Independent review/publication remain open.

## Release/integration handoff outcome

All authorized build/integration changes and workflow checks are complete. Broad
WEPPpy suite passes 7,720 tests (72 skips); final targeted suite passes 83 (one
skip), including all three new preview regressions absent from broad collection.
The installed release passes 75 tests; facade parity passes 22 cases and eight
isolated downstream suites pass 148 tests. First-party review evidence is recorded;
independent sign-off and publication remain open. Existing services were not
restarted and production deployment is outside the package scope.

## Publication resumption (2026-09-07)

The user requested completing review and container publication. Execute the
existing review and native-first repository publication milestone, then update
WEPPpy `.github/workflows/publish-weppcloud-image.yml` WEPPPYO3_REF to the exact
native commit. This workflow is hand-maintained: it has no forest workflow spec
and is outside the builder script enumeration. The master push automatically
publishes ghcr.io/rogerlew/wepppy:sha-<WEPPpy commit>. Record the workflow receipt
and immutable digest, verify image contents and native execution, and retain
production deployment as out of scope. This revision adds the explicitly
authorized container publication to the remaining milestone.

Review revision: independent review findings require a final rebuild and focused
regression/performance recheck before the authorized native-first publication.

Final reviewed-release milestone: 89 native-module tests, 25 facade oracles,
independent correctness/QA/security sign-off, 106.5921% timing and final RQ chain
at 861.34 MiB pass. Controlled symmetric host-prewarm raw scaling is 1.4398346;
the mixed-cache 2.07079 failure is preserved and no cache-independent guarantee
is made. See artifacts/review-performance.md for unchanged thresholds and methods.
Native-first publication can proceed; final WEPPpy broad revalidation is running.

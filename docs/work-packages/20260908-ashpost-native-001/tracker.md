# Tracker: bounded ash modeling and native AshPost

- Status: `COMPLETED` (operator-confirmed simple scope)
- Execution host: `forest`
- Primary repository: `/workdir/wepppyo3`
- Integration repository: `/workdir/wepppy`
- Security impact: `high`; independent review required

## Progress
- [x] Production failure, repository revisions, fixture models/schemas and hashes recorded; source inputs remain unchanged after acceptance.
- [x] Both Python model/output/facade oracles frozen; OR-202 Python post completed in the controlled 12 GiB baseline.
- [x] Native API and bounded scheduler contracts frozen under the accepted individual-file writer scope.
- [x] Streaming Rust AshPost, edge tests and canonical py312 release implemented.
- [x] Required native AshPost and bounded hillslope scheduling integrated in WEPPpy.
- [x] Both model and OR-202 parity, five-run small benchmarks, eight-call memory audit and real sequential Compose RQ gates pass.
- [x] Independent correctness, QA, performance and security reviews have no unresolved medium/high finding; final release binding verified.
- [x] Full WEPPpy suite passed: 7,742 passed, 72 skipped; documentation/provenance checks pass.
- [x] Native release `2f395bed` and WEPPpy `9e30d217f` pushed native-first; clean remote checkouts, source/binary hashes and LFS objects verified.
## Incident evidence

- Production job: `8eb973ed-d7e6-4b5d-be24-b371237cea1e`.
- OR-8, OR-202, and WA-77 containers were `OOMKilled` with exit code 137.
- Their logs completed all 2,182, 1,090, and 2,336 hillslope ash tasks.
- Per-hillslope parquet files exist; `ash/post/` exists but contains no files.
- The Python producer retains all work-item DataFrames/futures, then AshPost
  loads, concatenates, deep-copies, and rereads the watershed.

## Decisions

- Operator-confirmed scope (2026-09-08): use the existing hillslope interchange
  -> totalwatsed3 individual-file/aggregate-reader pattern. No new multi-file
  transaction, all-five staging barrier, rollback/recovery system, generation
  storage, migration, or NFS-specific mechanism. Earlier assistant proposals
  are superseded; they are not prerequisites or review gates.
- Fresh Python/native parity uses the same frozen inputs. Watanabe historical
  upstream drift is recorded, not treated as an AshPost implementation failure.

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

Completed on 2026-09-08. Native release: `2f395bedda6bd16e0cf8c8cdcaee81d7614347e3`.
WEPPpy release: `9e30d217f830ac73494df241259175ef3e8a2b16`.
Both repositories and their remote verification checkouts are clean.

Full WEPPpy suite: 7,742 passed, 72 skipped. Native suite: 139 passed.
Clean remote verification: 139 native tests and 118 WEPPpy tests passed,
with one WEPPpy skip; startup hash and LFS integrity checks passed.
Real OR-202 then canine-liar RQ acceptance peaked at 4.43 GiB under 12 GiB,
with no OOM or restart. Both models preserve output parity. Reviews are closed.

See [execution results](artifacts/execution-results.md),
[evidence index](artifacts/README.md), and the
[completed plan](prompts/completed/execplan.md). Durable decision: WEPPpy
`docs/schemas/output-scope-contract.md#ashpost-file-production-scope`.
No registry image was published and no deployment was performed. `[skip ci]`
prevented master pushes from triggering the image-publication workflow.

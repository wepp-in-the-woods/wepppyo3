# Independent AshPost QA review

Reviewer: `/root/qa_review`, 2026-09-08.

Static: no unresolved medium/high quality finding remains in the reviewed
implementation. The two regression gaps below were closed during review, and
the executor's passing test logs were inspected. Completing parity,
performance, full-suite, and Compose acceptance remains the executing agent's
separate validation responsibility.

## Scope

Reviewed native aggregation/statistics, `test_ashpost.py`,
`ashpost_sort_tests.rs`, and WEPPpy's `ash.py`, `ashpost.py`, and
`test_ash_transport_run_ash.py`, after the independent correctness review.
Follow-up scope: WEPPpy `tests/wepp/interchange/conftest.py`,
`module_loader.py`, and `test_module_loader.py`, addressing a full-suite
import-isolation failure found during this package.
Authority: [native contract](native-contract.md) and
[accepted scope](accepted-scope.md). Individual existing writers and normal
retry behavior are the intended publication scope. This review imposes no
multi-file transaction, rollback system, or NFS redesign.

The reviewer made no production or test edits. This artifact is the only
reviewer-authored file. Evidence below distinguishes source inspection from
executor logs inspected by this reviewer. This reviewer did not rerun the
executor's validation commands.

## Findings and disposition

| Finding | Severity | Disposition |
| --- | --- | --- |
| The scheduler's failure test covered input loading, but did not reach a failing `future.result()`. | Medium; already identified by executor | Closed in `test_hillslope_queue_propagates_model_failure_and_cancels_pending`. A completed future raises the model error, the error propagates, all three remaining futures are cancelled, and no replacement is submitted. |
| Generated native tests did not assert populated return-period mappings, and frozen sort vectors always began with identity order. | Medium | Closed by four frozen cumulative return-period cases and `sequential_measures_retain_prior_tie_order`. Cases cover zero values, equal positive values, two positive values, and mixed values. Assertions check selected year, value, rank, recurrence, probability, dictionary keys, and Python scalar types. Three successive sorts retain and validate the preceding order. |
| Interchange test loading replaced real modules, and autouse cleanup purged real interchange and parent packages, leaving existing callers attached to different globals. | Medium; found by executor during full-suite validation | Closed in the bounded loader/fixture change. A matching resolved source path reuses the existing module object; cleanup removes registered loader entries without unconditional parent/package purges. The identity regression checks the returned native module and preserved parent modules. The combined interchange/report selection passes. |

## Inspected execution evidence

Ran by the executor; this reviewer read the logs and confirmed their results:

| Selection | Result | Session log |
| --- | --- | --- |
| Native AshPost Python tests, including populated cumulative cases | 32 passed | `/tmp/ashpost-native-tests.log` |
| Native Rust library and writer tests, including sequential ordering | 98 library tests and 17 writer tests passed | `/tmp/ashpost-cargo-test.log` |
| Ash scheduler, AshPost integration, and Alex static tests through `wctl` | 23 passed | `/tmp/ashpost-integration-tests.log` |
| Interchange tests followed by hillslope report tests through `wctl` | 85 passed, 1 skipped | `/tmp/ashpost-combined-tests-fixed.log` |

The combined command was `wctl run-pytest tests/wepp/interchange
tests/wepp/reports/test_hillslope_watbal.py --maxfail=1`; its log confirms the
new loader regression and downstream report selection executed successfully.
The executor separately reports both Ash and AshPost stubtests passing.
Full-suite and actual Compose completion were still in progress at this
follow-up review; no success is inferred for either from focused tests.

## Coverage and implementation assessment

- The scheduler weak-reference test bounds loaded hill inputs at six for three
  workers, checks input release before executor exit, consumes every work item,
  and checks that scalar input dictionaries are not mutated. Load-failure and
  model-failure tests exercise distinct cancellation paths. Real spawned-worker
  memory and shutdown behavior still require Compose evidence.
- Native generated tests cover daily coalescing, first-year filtering,
  cumulative selection, numeric widths, empty manifest, zero area, null/NaN
  metrics, malformed values and schemas, duplicate/aliased paths, input
  symlinks, optional hydrology, arithmetic overflow, and individual writer
  failure followed by regeneration. The partial-write test intentionally
  permits earlier complete files and validates a successful retry.
- Correctness-review fixes for all-NULL runoff, valid NaN runoff, empty optional
  hydrology, selected malformed H.wat, and unsupported metric widths have
  persistent generated regressions. Large fixture parity remains necessary
  for complete model schemas and all facade outputs.
- Python keeps discovery, metadata descriptions, warning presentation, NoDb,
  versioning, documentation, and catalog ownership. The reviewed production
  facade has no whole-watershed dataframe aggregation or executable fallback.
- Native code separates bounded ingestion/aggregation from recurrence sorting
  and uses the existing Parquet writer. The custom ordering algorithm has an
  explicit frozen NumPy/pandas compatibility reason; replacing it casually
  with a standard stable sort would change tied-event selection.

## Non-blocking maintenance notes

- `ashpost.rs`, `ashpost_stats.rs`, and the PyO3 wrapper share positional table
  and row layouts. Keep these invariants explicit when a future schema change
  touches them; named table access would reduce accidental index drift. No
  refactor is needed for this extraction.
- Python retains legacy column/type constants after removing the aggregation
  helpers. Some remain declared in `ashpost.pyi`. Their removal should follow
  a caller audit, not an incidental public-surface change in this package.
- Resolved during follow-up: the ash-transport local `AGENTS.md` now routes
  aggregation, schema changes, and testing to the native implementation and
  current test paths instead of the deleted Python aggregation helpers.
- The test loader's existing cleanup registry tracks module names rather than
  object identities. Audited callers clean their loader state immediately;
  preserving arbitrary replacements or restoring preexisting nonmatching
  modules is outside this targeted repair. Avoid treating this helper as a
  general-purpose import-state transaction.

## Reviewed source identity

SHA-256 values at closure inspection:

| Repository and path | SHA-256 |
| --- | --- |
| wepppyo3 `wepp_interchange/src/ashpost.rs` | `4d437bdd2b0dc99377987c6465f0c5039c428ef468ddb2ac9f17d17d66c60211` |
| wepppyo3 `wepp_interchange/src/ashpost_stats.rs` | `70f1dc57bbfcdec0ba5a057060a73fd307d88e197c38682c100f5e1317d18a11` |
| wepppyo3 `wepp_interchange/src/ashpost_sort_tests.rs` | `331c3d94644a878d9cc9e12acfbf84fe8129a73c201d878b35f7836adba577b5` |
| wepppyo3 `tests/wepp_interchange/test_ashpost.py` | `2efc6fc6e8b366522454b64b9034f076a761056992c95b48fabec7c46a8ebb53` |
| WEPPpy `wepppy/nodb/mods/ash_transport/ash.py` | `17b8ab600483e5e7b983456382693f64c566202362f45f10457127e7bee221f1` |
| WEPPpy `wepppy/nodb/mods/ash_transport/ashpost.py` | `e3237756cfcc78f192e4300f97f3cdd74945121fef6bb077123e0c8971b94801` |
| WEPPpy `tests/nodb/mods/test_ash_transport_run_ash.py` | `5875e27a1f1c90c5b178aefd6297adeddc0e9135dface82ba02fc346a5cf7ccf` |
| WEPPpy `tests/wepp/interchange/conftest.py` | `339d283651c054ab6f18f1cd4ca190b2d43f9e9e7470c073b00888622b34d3fa` |
| WEPPpy `tests/wepp/interchange/module_loader.py` | `4f91be174c50b78f70caf803ce236aceda9808d5144822ae994832a951c30d40` |
| WEPPpy `tests/wepp/interchange/test_module_loader.py` | `40882e1a7dbb34fb253d1f895f073db8f7c7ee3a1fcce46b7f68b12d4ecbf7c9` |

# Independent QA review

Reviewer: delegated secondary QA reviewer, separate from the implementing and
primary correctness reviewers. Date: 2026-09-07.

## Scope and candidate

This review follows closure of COR-01, COR-02, and COR-03 in the
[independent correctness review](independent-correctness-review.md). It evaluates
maintainability, cohesion, observability, and regression-test quality for the
native producer, WEPPpy facade, required-native boundary, and README preview.
The [repair record](review-fixes.md) and current source were inspected together.

confirmed: Direct readback matched these candidate SHA-256 values:

- `wepp_interchange/src/totalwatsed.rs`:
  `ba295333c0a84f88c01fb2b8ee78c2a3f86f752633da65a216c0095922d4bdbd`.
- `release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so`:
  `bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354`.

No tests, builds, or resource workloads were launched by this reviewer, to keep
the parent's final timing measurements undisturbed. The evidence below consists
of source inspection and readback of completed validation, not new executions.

## Assessment

confirmed: The Python facade has a coherent orchestration boundary: resolve
paths, scalar options, and ash controller metadata; invoke the required native
API; return the established output Path. Table processing and Parquet publication
remain in the native crate. Existing dependencies and the existing atomic sink
are reused. No new production fallback or broad exception handler was introduced
by this extraction; retained ash lookup behavior remains existing debt.

The native reader and aggregate functions have distinct responsibilities.
Comments explain why metadata compaction exists, which physical fields must
survive it, the empty dictionary sentinel, and the twelve-worker bound. The
representation is more complex than a standard projected reader, but its
measured memory purpose and dedicated physical-layout tests justify that
complexity. A future Parquet upgrade must retain these layout regressions.

The completion log now uses the native summary's output paths. Its regression
uses the three-path totalwatsed3 signature, so it would expose the previous
incorrect WAT input path in the output field. Error translation retains causes.

## Test quality and evidence

confirmed: The frozen native comparator checks schema metadata, column order,
types, nullability, row count, null masks, exact integer/date values, and the
specified floating tolerances. Oracles are fixed previous-producer outputs,
including the three independently discovered nullable-area/NaN regressions.
They are not recomputed by the implementation under test. The public-facade
comparisons additionally exercise metadata construction rather than only the
native API's opaque metadata pass-through.

The physical-layout tests cover dictionary and non-dictionary files, gzip/zstd,
absent statistics, unused nested fields, empty groups, and more than sixteen
groups. Eleven malformed-footer tests mutate fresh local copies, require an
ordinary RuntimeError rather than PyO3 PanicException, preserve previous output
and input bytes, and check temporary-file cleanup. Their positive layout cases
also caught the empty-dictionary regression during repair.

The README tests exercise empty, short, and multi-group inputs, forbid the old
full-table read, and detect a second preview batch. The missing-native facade
test checks both a successful call without DuckDB and a missing-API failure
that preserves the prior published file. These tests assert material behavior.

Completed logs read from `target/totalwatsed3-evidence/`:

| Evidence | Result |
| --- | --- |
| `review-installed-release-tests.log` | 89 native module tests passed |
| `independent-security-final-tests.log` | 42 totalwatsed3 tests passed |
| `review-final-wepppy-tests.log` | 83 interchange/startup tests passed; one skipped |
| `review-facade-parity.log` | 25 frozen public-facade cases passed |
| `review-rust-tests.log` | 95 library plus 17 TC_OUT test executions passed |

The broader historical 7,720-test result predates the three new preview tests
and final native repairs. Targeted final-candidate evidence closes those touched
paths; it must not be described as a fresh complete-suite run of this candidate.

## Non-blocking follow-ups

- `wepp_interchange/src/totalwatsed.rs:107`, `:627`, and `:906`: compact sums and
  final-row assembly rely on positional metric indices, a 32-bit validity mask,
  and parallel frozen field definitions in Rust and Python. Before extending
  metrics, introduce named indices for the affected positions and an explicit
  mask-capacity invariant; retain the frozen schema/parity tests. This can be a
  small behavior-preserving change, without redesigning the hot path.
- `tests/wepp_interchange/test_totalwatsed3.py:144`: the local compact-Thrift
  mutator is deliberately limited to freshly generated fixtures. Keep it local
  and document its numeric field paths when extending corruption coverage;
  do not turn it into a second general Parquet parser. Its unique-match and
  complete-consumption assertions already fail loudly on unsupported layouts.

Existing WEPPpy module-purge behavior requires the missing-native test to patch
the loaded call's globals. The test documents this and uses monkeypatch cleanup;
it is acceptable here. A shared stable module fixture would be a separate test
harness improvement if this pattern recurs.

## Verdict and limits

Approve QA for the identified repaired candidate. No new blocking code-quality
or test-robustness finding remains within this scope. The residual debt above
does not require another source or release change before publication.

Final resource/workflow acceptance, dedicated security disposition, remote LFS
retrieval, and container publication retain their separate gates. Publication
notes and the validation summary must be refreshed with final counts, hashes,
and status; their pre-review values are historical evidence, not final receipts.
This review does not claim production deployment or independently repeated
performance results.

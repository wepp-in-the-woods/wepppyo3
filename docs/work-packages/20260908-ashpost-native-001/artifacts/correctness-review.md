# Independent native AshPost correctness review

Reviewer: `/root/ash_contract_review`. Closure checked 2026-09-08, 20:10 UTC.

**Confirmed: no remaining blocking correctness finding in the reviewed native
aggregation and statistics implementation.** Four reported issues have been
resolved within the supported producer contract. This review does not substitute
for the package's remaining integration, benchmark, and release gates.

## Scope and authority

Reviewed `wepp_interchange/src/ashpost.rs`, `ashpost_stats.rs`, and the generated
sort vectors against frozen Python AshPost from WEPPpy
`abebd09239f398af7627924c4c000ff3229a01ee` and the
[native contract](native-contract.md). The
[operator-confirmed scope](accepted-scope.md) governs: use the existing writer
for individual output files. Multi-file transactions, rollback, storage migration,
and NFS publication mechanisms are outside this implementation review.

The reviewer made no production code changes. Closure probes loaded the rebuilt
`target/release/libwepp_interchange_rust.so` directly in the WEPPpy Python 3.12
virtual environment and used newly generated, disposable inputs. Frozen source
fixtures were not modified.

## Findings and closure

| Finding | Severity | Confirmed closure |
| --- | --- | --- |
| H.wat all-NULL runoff groups became zero; valid IEEE NaN was also dropped beside finite contributions. This changed corrected streamflow. | Medium | `ashpost.rs` now distinguishes Arrow NULL from valid NaN, tracks non-null sums, and propagates NaN groups. The all-NULL and four mixed-operand probes below match frozen Python. |
| Empty optional totalwatsed3 with a partial schema failed required-column projection, although Python takes its empty-hydrology path. | Medium | The reader checks Parquet row count before required projection. A zero-row file containing only `year` succeeds with the existing four zero correction columns. |
| Malformed selected H.wat was silently bypassed when totalwatsed3 was absent. | Medium | H.wat scanning is independent of whether totalwatsed3 has rows. Both implementations fail for malformed selected H.wat; native reports the contract's `ValueError`. |
| Accepting Float32 model metrics changed legacy arithmetic and output widths. | Medium | Native now rejects this unsupported variant with `ValueError`. Both actual model producers create Float64 metric arrays; the two frozen ten-file fixtures contain 440 unit-bearing metric fields, all Float64. The contract records this explicit restriction. This is not a Float32 parity claim. |

Additional confirmed checks: ordinary two-day corrected flow remains `[1, 1]`
with matching daily Arrow widths; infinite hydrology input is rejected under the
documented invalid-numeric contract; an output alias to a model input is rejected
without changing that input's bytes.

One documentation clarification was sent to the implementer: the general
skip-null/NaN sum statement in the native contract should explicitly refer to
model metrics. H.wat follows SQL SUM semantics as described below. The reviewed
code already makes that distinction.

## Independent closure probes

The generated fixture has one one-hectare hillslope and two daily rows. Original
streamflow and runoff are 1 mm, basin area is 10,000 square meters, and ash runoff
is 0.1 mm. Mixed H.wat groups have two rows on the first day and no second-day
match. NULL below is the output Parquet null, not an IEEE NaN payload.

| H.wat QOFE values | H.wat Area values | DuckDB first-day SUM, cubic meters | Python corrected flow, mm | Native corrected flow, mm |
| --- | --- | --- | --- | --- |
| valid NaN, 0.1 | 10000, 10000 | NaN | NULL, NULL | NULL, NULL |
| NULL, 0.1 | 10000, 10000 | 1 | 1, NULL | 1, NULL |
| valid NaN, 0.1 | NULL, 10000 | 1 | 1, NULL | 1, NULL |
| NULL, 0.1 | valid NaN, 10000 | 1 | 1, NULL | 1, NULL |

The SQL oracle was the original expression
`SUM(QOFE * 0.001 * Area)`, filtered by selected WEPP ID and grouped by year and
julian. Each mixed-case assertion compared the actual frozen Python and native
Parquet corrected-flow columns. The separate all-NULL case matched `[NULL, NULL]`.

Commands executed successfully:

```sh
/workdir/wepppy/.venv/bin/python /tmp/ashpost_edge_review.py
/workdir/wepppy/.venv/bin/python /tmp/ashpost_null_nan_closure.py
```

Session-local detailed evidence, summarized durably in the tables above:

- `/tmp/ashpost-edge-review-x9p4bfbw/results.json`: seven edge scenarios.
- `/tmp/ashpost-edge-review-nrfd704r/mixed-null-nan-results.json`: four operand cases.

Importing the frozen module emitted a Redis authentication diagnostic in this
local environment. Aggregation, DuckDB, and Parquet execution completed; these
probes did not exercise NoDb persistence or the RQ workflow.

## Return-period ordering

Confirmed by source inspection: native descending ordering reverses the current
non-NaN order, applies the scalar NumPy indirect quicksort, reverses the result,
and appends NaNs in their existing order. It retains the row order between
successive measures, uses average tied ranks, and preserves the legacy recurrence
selection and zero-positive-event behavior. The implementation includes NumPy's
insertion threshold, partition tie movement, depth handling, and heap fallback.

The earlier independent algorithm investigation matched 1,540 seeded arrays and
12 structured arrays against NumPy 1.26.0/pandas 2.2.2; the structured cases
included actual heap-fallback inputs. Fifteen named ordering cases are preserved
in [argsort-oracles.json](argsort-oracles.json) and represented in the generated
Rust test. The implementer reports the expanded Rust tests passing; this closure
did not rerun the entire crate suite.

## Reviewed evidence identity

SHA-256 values captured at closure:

| File, relative to wepppyo3 | SHA-256 |
| --- | --- |
| `wepp_interchange/src/ashpost.rs` | `4d437bdd2b0dc99377987c6465f0c5039c428ef468ddb2ac9f17d17d66c60211` |
| `wepp_interchange/src/ashpost_stats.rs` | `70f1dc57bbfcdec0ba5a057060a73fd307d88e197c38682c100f5e1317d18a11` |
| `wepp_interchange/src/ashpost_sort_tests.rs` | `7f54b9db198650dbadfba23a403574ec334cf1924eb75dd67bc06fdf15e9270b` |
| `target/release/libwepp_interchange_rust.so` | `c6b746bb77be39d38321a365df1762fd8d88ce0522f3a51215d5bb8bfdddf248` |
| `target/ashpost-evidence/python-baseline-ashpost.py` | `7c9aa06cbca1019fb557ef1be5cb19c0e56f9e905099ea8bc191c2f836a620a0` |
| `docs/work-packages/20260908-ashpost-native-001/artifacts/argsort-oracles.json` | `8658beb9cdd052e7126440fdba8959c8706284211bf298c45c89c7068f81acb5` |

## Residual risk and coverage boundaries

- Return-period tie compatibility is tied to the frozen scalar NumPy baseline;
  a different NumPy CPU-dispatched sorting implementation can have different tie
  order. The frozen ordering is explicit and tested.
- The reviewer checked native aggregation and recurrence formulas and the
  reported edge corrections. Python scheduler integration, complete RQ execution,
  Compose identity/mount parity, OR-202 measurements, and release provenance
  remain separate package checks owned by the executing agent.
- Explicit invalid-input rejection, including unsupported Float32 model metrics
  and infinite numbers, follows the new native boundary contract. It does not
  promise exact legacy exception classes for malformed inputs.
- Individual file writes retain the accepted existing behavior. This review
  makes no guarantee that a concurrent reader sees a snapshot across five files.

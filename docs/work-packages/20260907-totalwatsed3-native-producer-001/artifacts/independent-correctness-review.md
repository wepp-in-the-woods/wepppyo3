# Independent correctness review

Reviewer: delegated correctness reviewer, independent of the implementing agent.
Date: 2026-09-07. Scope: native source, frozen contract/schema, release API,
WEPPpy facade, required-native boundary, and bounded README preview.

## Findings

### COR-01 - Medium: absent WAT area poisons optional weighted sums

confirmed: `wepp_interchange/src/totalwatsed.rs`, `aggregate_hill` soil and element
join branches, converted an all-null area aggregate to NaN with `area.get()` and
then added that NaN to the daily numerator. The denominator correctly skipped
the absent area. The previous DuckDB producer skips SQL NULL products, so an
otherwise valid hillslope can supply a nonnull watershed result.

Independent reproduction changed only hill 1's Area values to null in scratch
copies of the `normal` fixture and executed the exact previous WEPPpy producer
from revision `3c51780f50e2599ef72b03214c35bf20538e55c4`. Its TSMF was
`[0.3, 0.3, 0.3]`, QRain `[0.4, null, 0.4]`, and QSnow `[0.2, null, 0.2]`;
the initial native release returned null for all three columns on every day.
The unchanged watershed Area was `[6000, 6000, 6000]` in both outputs.

Required closure: skip both weighted numerator and denominator when no nonnull
area exists, retain actual NaN as a distinct present value, freeze this old
producer oracle, and verify the rebuilt release against it.

Status: closed. The repair checks `area.seen` for both numerator and denominator
and uses `area.value`; the frozen `null_area` regression passes against the
rebuilt release.

### COR-02 - Medium: NaN ash filter metadata selects the wrong first value

confirmed: `wepp_interchange/src/totalwatsed.rs`, `aggregate_ash` first year0 and
days-from-fire selectors, treated a present Float64 NaN as the first value.
The previous pandas `groupby.first` skips NaN. With a duplicate date whose first
filter value is NaN and second is valid, native filtering discarded the entire
date's ash contribution.

Independent reproductions covered both year0 and days-from-fire-only inputs.
For each, the previous producer emitted ash_transport `[1.8, 0.9, 0]`; the
initial release emitted `[0, 0.9, 0]`. Other fields and paths remained the frozen
ash fixture inputs. These are parity defects in admitted numeric/null states,
not proposed changes to ash formulas or first-year eligibility.

Required closure: ignore NaN when choosing each first filter value, preserve
other existing value behavior, freeze both old-producer oracles, and verify the
rebuilt release against them.

Status: closed. Both selectors skip NaN, and the frozen `ash_nan_year0` and
`ash_nan_days` regressions pass against the rebuilt release. A separately rerun
old/new days-from-fire probe in Compose also matches exactly.

### COR-03 - Medium: initial metadata validation rejects empty dictionary chunks

confirmed: The initial security repair in `totalwatsed.rs:323` required the
dictionary page offset to precede the data page offset unconditionally. Valid
empty PyArrow dictionary chunks use zero as the absent data-page offset.
The checked-in empty PASS fixture has zero values, dictionary offsets 4, 19,
and 34, data offsets zero, and compressed sizes 15 bytes; these dictionary
chunks are entirely within the 2552-byte file.

The independent final-candidate run against SHA-256
`455eda290a6a730783db685df43e717b2437c756fcc185f0819e2de2df61427c`
passed 39 tests but failed `empty_inputs`, `empty_pass`, and the zstd/dictionary
physical-layout regression with `column range outside input file`.

Required closure: allow the absent-data-page sentinel only for zero-value
chunks while retaining all nonnegative count/offset and actual chunk-range
bounds, then rerun both valid-layout and malformed-footer cases.

Status: closed. The sentinel exception requires `num_values == 0` and
`data_page_offset == 0`; nonnegative checks and dictionary/chunk end bounds
remain enforced. All valid-layout and malformed-footer regressions pass on the
final reviewed release.

## Independent evidence

The initial reviewed shared object had SHA-256
`0e681131bde2790af53e1cda9d7292aba99a1aa1debb5cc71b3ee8c84956ba2f`.
The following runs were performed by this reviewer, separately from first-party
evidence:

- In `/workdir/wepppyo3`,
  `PYTHONPATH=/workdir/wepppyo3/release/linux/py312 /workdir/wepppy/.venv/bin/python -m pytest tests/wepp_interchange/test_totalwatsed3.py -q`:
  28 passed, one warning, 6.98 seconds. This includes three real oracles, frozen
  optional/ash/legacy/subset cases, publication failures, and physical metadata
  reconstruction across gzip/zstd, dictionary settings, missing statistics,
  empty groups, nested unused columns, and more than sixteen groups.
- In `/workdir/wepppy`,
  `wctl run-pytest tests/wepp/interchange/test_totalwatsed3.py tests/wepp/interchange/test_interchange_documentation.py tests/docker/unit/test_wepppyo3_interchange_startup_contract.py -q`:
  20 passed, two warnings, 22.05 seconds.
- Separate old/new differential probes found COR-01 and both COR-02 variants.
  Old source was obtained with `git show` at the exact revision above; it was
  loaded as an isolated module, never replaced by the current native facade.
  The final ash-days probe used the running Compose runtime via
  `wctl exec weppcloud env PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312 /opt/venv/bin/python /workdir/wepppyo3/target/totalwatsed3-evidence/independent_review_ash_days_probe.py`.
  The first local ash probe completed despite existing unauthenticated Redis
  import diagnostics; the clean Compose probe reproduced the same defect.

## Repair verification

confirmed: Release SHA-256
`751004fc253fb3ef018a187217a2519c426b172f179ae04b93095cc7d0b21948`
closed COR-01 and COR-02. The same independent native command passed all 31
totalwatsed3 tests in 7.67 seconds, including the three newly frozen cases; the
same WEPPpy facade/preview/startup command passed 20 tests in 25.35 seconds.
The separate Compose ash-days probe produced matching ash_transport
`[1.8000000000000003, 0.9000000000000001, 0.0]` from old and native producers.

The security reviewer subsequently identified a malformed negative chunk-size
PanicException path. Its first metadata-validation repair caused COR-03,
detected by the independent
42-case run: 39 passed, three failed in 8.23 seconds. This failed candidate
was withheld from approval.

confirmed: The final reviewed release SHA-256 is
`bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354`.
The independent installed-release command then passed **42 tests**, one warning,
in 8.16 seconds. This covers all 22 synthetic and three real oracle comparisons,
publication checks, physical-layout regressions, and eleven malformed-footer
cases. COR-01, COR-02, and COR-03 remain closed on this artifact.

The reviewed `wepp_interchange/src/totalwatsed.rs` source SHA-256 is
`ba295333c0a84f88c01fb2b8ee78c2a3f86f752633da65a216c0095922d4bdbd`.
Final repair references: null-area guards at lines 745 and 797; ash selectors
at lines 853 and 856; metadata validation and sentinel at lines 297 and 324.
The WEPPpy startup preflight pin and its existing contract-test literal both
match the final release SHA by direct readback. The Python facade and preview
implementation remained the candidate already independently validated above.

### Canonical oracle runtime correction

The first independent null-area and ash-year0 differential probes used host
PyArrow 16.1.0. Their old/new comparisons establish the defects, but the captured
oracle creator metadata must match the canonical Compose PyArrow 23.0.1 runtime.
The implementing agent recaptured both oracles from the exact old producer in
Compose and preserved the earlier red-probe evidence. The days-from-fire oracle
already used Compose. This recapture changes no formula or tolerance.

The reviewer independently compared the replacement small tables to the retained
host oracles. Null-area field types, null masks and values are exactly equal.
Ash-year0 field types and null masks are exactly equal; six float columns differ
by at most `8.88e-16`, within the unchanged `rtol=1e-10`, `atol=1e-12` contract.
All other non-pandas metadata is identical, and `creator` is the only changed
pandas metadata key. The implementing agent then reported all 25 public-facade
oracle comparisons passing in Compose; this last facade rerun is first-party
evidence, distinct from the independent runs and table comparisons above.

## Source review conclusions and limits

confirmed: Full-key WAT authority, left-joined PASS dates, last-OFE lateral flow,
soil/element join multiplicity, area/depth conversion, baseflow recurrence,
sediment formulas, optional-column null behavior, schema metadata, legacy
aliases, and ash orchestration were compared against the previous producer.
The reviewed repairs address two gaps not exercised by the initial fixtures.

The compact reader preserves projected physical chunk offsets, dictionary
offsets, codec, encodings, value counts, and Arrow schema while reconstructing
at most sixteen groups per scan. Numeric batches are limited to 8192 rows;
per-worker join state covers one hillslope and global state covers dates.
The README implementation reads schema and its first three-row batch, including
empty and short-file handling, and does not materialize the full table.

No output schema, formula, NoDb persistence, RQ dependency, route authorization,
or public facade argument change was identified. Missing native support and
execution failures retain the established explicit boundary; publication uses
the existing failure-atomic sink. The paired startup hash must follow the
rebuilt artifact before publication.

Residual coverage limits: arbitrary hostile Parquet metadata, all numeric
overflow/cancellation patterns, inconsistent calendar keys, concurrent mutation
of source files, and every CPU/thread-count combination were not exhaustively
tested. Full input footers are initially parsed before compaction. This review
does not independently repeat resource benchmarks or the complete RQ chain;
those remain separately recorded acceptance evidence. It does not authorize
production rollout or establish Kubernetes runtime equivalence.

## Verdict

Approve correctness for the final reviewed release identified above. All three
findings are closed with independent source inspection and rebuilt-release
evidence; no open correctness blocker remains in this review scope. The separate
security, final resource/workflow, remote LFS, and container publication gates
retain their own required evidence and disposition.

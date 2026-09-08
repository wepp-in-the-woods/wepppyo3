# Independent review fixes

The user authorized independent reviewer agents as part of completing the review
and container publication request. Correctness and security agents reviewed the
actual implementation and exercised additional cases against the installed release.

## Correctness

COR-01: unseen WAT area sums became NaN optional-field numerators, poisoning valid
soil/element contributions from other hillslopes. Accumulate numerator and weight
only when the matching area sum has a value, preserving SQL NULL product behavior.

COR-02: first ash year0/days_from_fire selection retained a NaN instead of skipping
it like pandas groupby.first. Skip NaNs while selecting those metadata values;
retain other existing semantics. Frozen null_area, ash_nan_year0 and ash_nan_days
cases reproduce the exact previous WEPPpy producer at revision
3c51780f50e2599ef72b03214c35bf20538e55c4. Capture scripts and independent review
artifacts preserve provenance. Original fixture files and oracles remain unchanged.

## Security and observability

SEC-02: negative Parquet compressed sizes reached a downstream Rust assertion and
escaped the normal Python execution-error boundary as PanicException. Validate
footer row/value counts, sizes and physical column ranges before projection and
reader construction. Invalid locations return an ordinary interchange error.
This is bounded validation of a confirmed failure, not a hostile-file sandbox or
an exhaustive decoder allocation audit.

The completion log now uses native output_paths instead of assuming the second
positional argument is the output. For totalwatsed3 that argument is H.wat.parquet.
The existing log regression now exercises a three-path native signature.

COR-03: the first range validator rejected valid zero-value dictionary chunks
with data_page_offset=0. Permit only this absent-data-page sentinel while retaining
all nonnegative and actual chunk-range checks. Existing empty/layout regressions
and all eleven malformed-footer cases pass on the final rebuilt release.

The first null-area/year0 captures used host PyArrow 16.1.0. Recapture the same
previous producer in canonical Compose PyArrow 23.0.1, preserving the original
host probe evidence. Field/null/value parity remains within the unchanged
comparator tolerance; this aligns creator metadata with the existing oracles.

## Validation status

Final release bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354
passes 89 installed native-module tests and 25 public-facade oracle comparisons.
Independent correctness and security checks each pass 42 focused native cases;
security additionally passes 12 malformed-input/path probes. Final resource,
workflow, QA and publication receipts are recorded as they complete.

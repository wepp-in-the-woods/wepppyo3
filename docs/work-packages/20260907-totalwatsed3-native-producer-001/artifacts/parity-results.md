# Native candidate parity

## Static:

confirmed: The current producer and the approved current-producer single-OFE
oracle are authoritative. See package.md, Approved single-OFE oracle authority.
The old WEPPpy decimal-pleasing output remains unchanged as historical evidence.
Its prior mismatch is retained in oracle-comparison.json and python-oracle.md.

## Ran:

confirmed: The native candidate matches all 79 fields of all three approved
real-fixture oracles: 4,018 single-OFE rows, 7,670 MOFE rows, and 16,071
performance-fixture rows. Column order, Arrow types, nullability, field/schema
metadata, date/integer values, null masks, and floating values are compared;
rtol=1e-10, atol=1e-12. All twenty benchmark outputs also pass.

confirmed: The targeted Python suite passes 20 tests against the built candidate.
Thirteen frozen synthetic cases cover ordinary MOFE inputs, missing and null
optional WAT columns, no OFE columns, legacy day/OFE names, mixed row-group
ordering, duplicate joins, subsets, empty subset/input, null lateral flow, typed
ash with duplicate dates, and ash without year0. Additional tests assert
last-OFE lateral flow, atomic staging cleanup on failed publication, preservation
of a prior output on missing input, and rejection of input/output aliasing.
See native-tests.log and tests/wepp_interchange/test_totalwatsed3.py.

Frozen synthetic inputs/oracles are under tests/fixtures/totalwatsed3/contracts.
They were generated with the unchanged Python producer and explicit baseflow,
subset, and ash metadata in each case.json. all-fixture-checksums.json records
their hashes. capture_contracts.py requires the pre-retirement producer revision
recorded in python-oracle.md; it is evidence tooling, never a runtime fallback.

confirmed: Rust tests pass 112 executions across the library and TC_OUT test
binary, including the new numeric tests and existing atomic publication tests.
See cargo-test.log. These validations do not establish complete parity for
every malformed/nonfinite input or all optional-edge combinations.

## Remaining review and coverage

The package stopped at its timing gate before full independent/security review.
Source inspection identifies a remaining nullable PASS-class edge: the candidate
zero-fills missing class mass before deriving sediment concentration, whereas
the Python producer derives concentration before the later merge fill. A
partially null class can therefore require a different concentration result.
This needs a regression case and correction before integration.

Nonfinite Interception also needs coverage: Python's nan_to_num clamps infinite
depths, whereas the candidate currently only replaces NaN. Neither edge occurs
in the approved real fixtures or thirteen synthetic cases above. Do not infer
complete contract closure from the passing suite.

## Edge closure after accepted timing disposition

confirmed: The candidate now computes sediment concentration before class-null
fill and clamps nonfinite Interception exactly like the original producer.
Six additional frozen cases pass: nullable PASS class, infinite/NaN
Interception, absent soil/element, null soil/element, single-OFE null lateral
flow, and empty PASS. Results are in edge-parity-results.json. All nineteen
synthetic cases are retained under contracts/ with original-producer oracles.

## Final resumed validation

confirmed: All nineteen frozen synthetic contracts and three real oracles pass;
the complete targeted suite passes 26 tests. All six scaling outputs pass strict
parity. Null PASS classes and infinite Interception follow-ups are closed.

## Metadata compaction validation

confirmed: The optimized source passes 28 targeted tests (nineteen frozen
contracts, three real oracles, publication safeguards, and two physical-layout
regressions). All six generated scales pass. See scaling-optimization.md.

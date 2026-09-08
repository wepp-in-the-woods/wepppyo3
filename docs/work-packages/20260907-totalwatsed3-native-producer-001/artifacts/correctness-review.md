# Correctness review preparation

Reviewer: implementing Codex agent; independent review remains pending.
Contract: package.md ownership/correctness sections and native-contract.md.
User goal: build the paired release and use native-only daily watershed production
without changing the published data contract.

## Valid-state matrix

| State | Required behavior | Direct evidence |
| --- | --- | --- |
| Required PASS/WAT absent | Explicit failure; existing output remains | native missing-input/publication tests |
| Optional soil/element or ash absent | Preserve legacy null/zero rules | frozen absent_soil_element and no-ash real oracles |
| Empty inputs or subset | Empty schema-bearing output | empty_inputs, empty_pass, empty_subset |
| Populated single/multiple OFE | Preserve every column, metadata, date, null and tolerated float | three real oracles; facade-parity-results.json |
| Supported legacy columns | Preserve day/OFE aliases and absent OFE behavior | legacy and no_ofe contracts |
| Null/nonfinite metrics | Match approved current producer | null_pass_class, null_soil_element, infinite_interception |
| Ash present with types/subsets | Preserve paths, areas, type/density and first-year rules | ash_types, ash_no_year0, subset; facade parity |
| Duplicate/mixed row groups | Preserve SQL join multiplicity and hillslope selection | duplicate_joins, mixed_groups |
| Hostile output alias or unpublishable target | Reject/preserve inputs or previous target; clean stage | direct native alias and failed-publication tests |
| Metadata layout differs | Preserve reads across codecs, dictionaries, omitted statistics, empty groups, nested unused columns | physical-layout regression tests |
| Huge valid input followed by README | Bounded preview, successful job and subsequent job | real Compose RQ evidence after SEC-01 fix |

## User-visible errors and partial state

Missing required files remain FileNotFoundError. Missing/stale native support uses
the established WeppInterchangeUnavailableError; native errors use
WeppInterchangeExecutionError with the cause preserved. Optional absence remains
supported. Native publication uses the existing ParquetSink atomic protocol.
The first RQ attempt demonstrated that valid output may exist when a subsequent
README stage fails; the corrected preview removes that confirmed OOM path without
recomputing or changing output semantics.

## Validation and remaining gate

75 installed-release tests, 22 public-facade oracle comparisons, and all eight
isolated downstream suites pass. The final WEPPpy interchange/startup suite passes
83 tests with one existing skip. The real Compose dependency chain verifies native
production, README, strict parity, TotalWatbalReport, query catalog and a subsequent
job. Broad suite: 7,720 passed, 72 skipped in 791.47 seconds. Its collection preceded
the three new preview regression cases; the final 83-test interchange/startup
run and real worker workflow separately validate the preview correction.

This is a state/evidence inventory for independent review, not a claim that all
hostile files or every state/flag combination are covered. Independent review and
publication remain open and must not be inferred from green tests alone.

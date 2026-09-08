# Scaling optimization

confirmed: User requested reducing the measured 1.7906-times peak ratio toward
1.5. No correctness or resource gate was relaxed.

## Diagnosis and implementation

Temporary source instrumentation measured ParquetMetaData::memory_size and
/proc/self/status VmRSS after each Source::open. The four original metadata
trees total about 33.9 MB at 586 hillslopes and 359.0 MB at 5,860 hillslopes.
These estimates exclude allocator overhead and can count shared pointers more
than once. The per-source records are in profile-586.log and profile-5860.log;
the corresponding unmodified measurement harness results are preserved in JSON.
The instrumentation was removed from the final source.

Projecting retained metadata to consumed columns first reduced the measured
peak ratio to 1.6728 (project-scale-measurements.json). The final candidate also
stores compact chunk locations, codecs, value counts, and encoding lists.
Full statistics are used only for singleton-hillslope identification. Readers
reconstruct at most sixteen row groups of metadata per scan. Mixed-hillslope
groups still use an ID scan; original Arrow types, chunk offsets, and join
multiplicity are preserved. Existing Arrow/Parquet APIs provide decoding; no
dependency, numerical formula, output schema, or fallback was added.

## Measured result

All six scale outputs pass strict oracle parity. Same pinned Forest image,
12 GiB container limit, twelve CPUs, one warm-up per container, and the same
fixture-generation and comparison methods as the failed sweep. The 586 case
includes three consecutive measured calls; other cases include one.

| Hillslopes | Measured seconds (first call) | Whole-container peak bytes |
| --- | --- | --- |
| 73 | 0.745098 | 441,741,312 |
| 146 | 1.192318 | 444,407,808 |
| 293 | 2.064386 | 452,685,824 |
| 586 | 3.816981 | 468,377,600 |
| 2344 | 13.480796 | 565,919,744 |
| 5860 | 34.228371 | 679,981,056 |

confirmed: Near-incident peak is 679,981,056 bytes (648.48 MiB), versus
468,377,600 bytes (446.68 MiB) at 586 hillslopes: **1.45178 times**, below
1.5. The large peak decreased 25.59% from 913,870,848 bytes. The margin to
1.5 is 22,585,344 bytes (21.54 MiB); this is measured acceptance, not proof
of constant memory at arbitrary row-group counts. Compact metadata still grows
with row-group count. No OOM, container restart, or leaked staging file occurred.
Wall time scales approximately with input size.

Raw commands, observations, parity results, memory events, and consecutive-call
baselines are in compact-scale-commands.json and compact-scale-measurements.json.
Original failed evidence in memory-scaling-results.md remains historical.

## Regression validation

The original nineteen synthetic contracts and three real oracles remain exact
within the frozen tolerances. Two added tests cover Gzip without dictionary
encoding and Zstd with dictionary encoding, omitted statistics, empty row groups,
more than sixteen groups, and unused nested columns with multiple leaves.
All 28 targeted tests pass; Rust tests pass 112 executions. The final nested
column adjustment was independently rerun (2 passed, 26 deselected).
Canonical release and WEPPpy integration remain outside this optimization change.

## Five-repeat timing recheck

confirmed: On the 586-hillslope fixture, native median is 3.813276 seconds
versus Python 3.541915 seconds (107.6614%). Native time is 0.76% above the
previously accepted candidate's 3.784380 seconds. The original 105% target
remains unmet; retain the user's prior acceptance of 107.18% as a scoped
historical decision, not a claim that this new measurement meets 105%.
The worst native sample is 3.846994 seconds, below 115% of Python median.
Native maximum peak is 458,346,496 bytes, 45.49% of Python's 1,007,476,736.
MOFE native median is 0.817713 seconds versus Python 0.853500 (95.81%),
below its 110% limit. All twenty measured outputs pass strict parity.
See compact-benchmark-summary.json, compact-benchmark-measurements.json,
and compact-benchmark-commands.json. No tolerance or gate was changed.

## Ten-call stability check

confirmed: A separate container ran one warm-up and ten measured calls without
restart. Anonymous memory after each call (bytes): 164859904, 157773824,
163401728, 159260672, 150466560, 156516352, 154578944, 161251328, 166281216,
158531584. This fluctuates without accumulating; final anonymous memory is
below the first measured call. Whole-container post-call baseline grows with
file cache as eleven output files are retained. Peak is 495,763,456 bytes;
all ten measured outputs pass parity and no temporary files remain.
This extended sample is supplemental and does not replace or inflate the
586-hillslope denominator used for the reported 1.45178 scaling ratio.
Commands and measurements are in compact-repeat-command.json and
compact-repeat-measurement.json.

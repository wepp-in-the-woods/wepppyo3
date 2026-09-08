# Independent performance review

Reviewer: `ash_performance_review`; date: 2026-09-08.
Status: approved. Standalone, repeated-call memory, complete-workflow performance,
and measured source/binary binding gates are accepted. No review condition remains.

## Findings

### Resolved medium finding: repeat sampler retained growing state

Confirmed: the initial `benchmark.py` retained samples in a Python list inside
the measured container. The corrected sampler writes JSONL incrementally and
the parent computes its summary by streaming that file. Both now keep bounded
collection state. The original measurements remain historical evidence.

Disposition: resolved by inspection and corrected measurements. Repeated-call
RSS and anonymous-memory evidence below separates caller residency from file
cache. No manual collection or allocator trimming occurs between native calls.
Container peaks still include instrumentation and file cache; they are not
exact native allocation totals. No unresolved medium/high static performance
finding remains.

## Standalone measurements

Confirmed: independently recomputed all 20 accepted small-fixture measurements
under `target/ashpost-evidence/benchmarks/{fixture}/{mode}-dcc822-{1..5}/`.
These corrected streaming-sampler measurements supersede the initial `80d36e`
series. Each measurement has its command, container state, and raw JSONL samples.
Commands alternate Python/native within each repetition and use fresh containers,
UID 1000, GID 993, CPU affinity 0-11, and 12 GiB memory/swap limits.

| Fixture | Python median seconds | Native median seconds | Native maximum seconds | Median sampled incremental reduction |
| --- | ---: | ---: | ---: | ---: |
| canine-liar | 1.139256 | 0.192636 | 0.194766 | 81.82% |
| assisted-weakness | 1.122148 | 0.188217 | 0.190429 | 81.98% |

Confirmed: both median runtime ratios are below 0.170; every native timing is
below 0.172 times its fixture's Python median. Both the 105% median and 115%
individual timing limits pass with substantial margin.

Confirmed: conservative checks also pass the 50% incremental memory reduction
and 2 GiB limits. The largest native lifetime peak minus its pre-call baseline
is 18,440,192 bytes for canine-liar and 18,669,568 bytes for assisted-weakness.
Those are upper bounds for the timed intervals. The smallest corresponding
Python sampled increments are 89,030,656 and 97,857,536 bytes. Even comparing
these least favorable bounds yields reductions above 79.2% for both fixtures.
No post-warm-up baseline is subtracted from a previous aggregation warm-up peak.

Confirmed: small-run sampler median intervals are approximately 1.3 ms; the
largest observed interval is 4.36 ms. A separate process avoids the earlier GIL
sampling concern. Samples remain lower bounds; lifetime cgroup peaks are
preserved independently. All accepted small-run memory-event counters are zero.

Confirmed: corrected OR-202 standalone Python completed in 159.391494 seconds
under 12 GiB, with peak 9,024,032,768 bytes. The corrected three-call native run
took 44.968490, 44.673652, and 47.250132 seconds; its container peak across all
three calls was 420,913,152 bytes. Every call passes the 105% baseline timing
limit. These are a standalone Python run and consecutive native calls, not the
complete scheduler comparison. Both containers exited successfully with zero
OOM, restart, and memory-limit events. Evidence:
[Python log](benchmark-OR-202-python-baseline-streamed-1.log),
[Python state](benchmark-OR-202-python-baseline-streamed-1.state.json),
[native log](benchmark-OR-202-native-repeat-repeat-streamed-1.log), and
[native state](benchmark-OR-202-native-repeat-repeat-streamed-1.state.json).

## Repeated-call memory disposition

Confirmed: an additional eight consecutive calls used the corrected sampler
without garbage collection or trimming between calls. Evidence is under
`target/ashpost-evidence/benchmarks/OR-202/native-repeat-repeat-streamed-8/`,
with [container state](benchmark-OR-202-native-repeat-repeat-streamed-8.state.json)
and [measurement log](benchmark-OR-202-native-repeat-repeat-streamed-8.log).

| Call | Caller RSS KiB | Cgroup anonymous bytes | Cgroup file bytes |
| --- | ---: | ---: | ---: |
| 1 | 452544 | 298733568 | 174501888 |
| 2 | 452544 | 299405312 | 176566272 |
| 3 | 454608 | 308527104 | 178601984 |
| 4 | 460984 | 315060224 | 180690944 |
| 5 | 478836 | 333340672 | 182759424 |
| 6 | 455840 | 309792768 | 184823808 |
| 7 | 459296 | 313331712 | 265961472 |
| 8 | 479136 | 333647872 | 466006016 |

Confirmed: caller RSS stayed within approximately 442-468 MiB. Anonymous memory
fell by 23.55 MB at call 6; call 8 was only 307,200 bytes above call 5. Caller
RSS at call 8 was only 300 KiB above call 5. File cache explains approximately
90% of the cgroup increase between calls 1 and 8. The total peak was
809,869,312 bytes, with zero OOM, memory-limit, or restart events. Both corrected
repeat output directories contained exactly the five expected parquet files,
with no temporary residue at review time.

Inference: the observed caller memory band is consistent with bounded allocator
residency, rather than retained state growing on every call. The repeat-memory
gate is accepted over these eight calls. This does not establish a universal
long-duration leak bound; the 9 GiB total-container workflow gate remains
independent and includes file cache.

Confirmed: the first six calls took approximately 45 seconds each; the last two
took 62.81 and 167.02 seconds while file-cache reclaim/refault counters appeared.
The executor reports concurrent fixture-copy I/O during that interval. That
causal explanation is an inference; the last two calls are retained as
contention observations and do not support a steady-state speedup claim.

## Harness and implementation assessment

Confirmed: `benchmark.py` streams source bytes before timing, including H.wat
and totalwatsed3, without materializing source tables. Both timed paths include
Python discovery, five output files, and compact mappings. Documentation and
facade checks run outside both timed paths. Garbage collection occurs only in
setup. `run_benchmarks.py` adds a host-side 10 ms target stop monitor at 10 GiB.
Its exact image and command are preserved with each run.

Confirmed: [verify_outputs.py](verify_outputs.py) compares schemas including
metadata, ordered values at `rtol=1e-10`, `atol=1e-12`, and return-period
structures. It also compares small-fixture documentation, version manifests,
and facade values. The final rerun selects `native-dcc822-1` for each small
fixture and the corrected streamed OR-202 Python baseline.
[wrapper-parity.json](wrapper-parity.json) records all five products for both
small fixtures and OR-202. This review inspected the verification code and its
recorded result; it did not rerun expensive work.

Confirmed: `ash.py:_run_hillslope_work` keeps the WAT loader in the parent,
submits at most `2 * max_workers` futures, and exits the executor/helper before
native post-processing. The outer list contains compact work descriptions,
not DataFrames. Static inspection found no remaining watershed-sized input
retention on the success path. Native batches and aggregate maps are bounded
by batch size and distinct hill-day/output keys.

Confirmed: OR-202 H.wat has 113,717,835 rows in 2,595 row groups. Every group has
usable WEPP-ID statistics with equal minimum and maximum IDs. The parent
DuckDB filter can prune unrelated hillslopes. The following workflow includes
the per-hillslope arrays, plots, concatenation, process imports, and consumers.

## Complete Compose workflow disposition

Confirmed: the real Compose worker completed both jobs in queue
`ashpost-acceptance-d0d9a7f807ef`:

- OR-202: `725b93d1-42b6-48b7-b3c3-d903a3e4c547`, all 1,090 model tasks,
  followed by native post-processing and downstream validation.
- Subsequent canine-liar: `ee88b470-1a6b-4a41-86ca-deaa606dd402`, all ten model
  tasks and native post-processing in the same RQ worker without restart.

Both jobs are `finished` with null exceptions. Result records contain the five
output row counts and hashes plus successful facade and catalog validation.
The harness additionally requires README and version-manifest files. Normal
RQ workhorse PIDs differ between jobs; the RQ worker and container persist.
Evidence: `target/ashpost-evidence/rq-runs/result.json`,
`target/ashpost-evidence/rq-runs/phases.jsonl`,
[Compose log](compose-rq.log), [Compose state](compose-state.json), and
[Compose configuration](compose-acceptance.yml).

Confirmed: kernel cgroup `memory.peak` was 4,756,385,792 bytes (4.43 GiB), below
the 9 GiB gate and leaving 7.57 GiB below the 12 GiB hard limit. The independent
host sampler observed 4,744,290,304 bytes. The container exited zero with zero
OOM, memory-limit, restart, or threshold-stop events. Identity was UID 1000,
GID 993, group 993, CPU affinity 0-11; the override specifies `WEPPPY_NCPU=12`,
12 GiB memory/swap limits, and the harness sets umask 0022.

Confirmed: independently inspected all 14,353 host cgroup samples and joined
them to all 1,090 completion counts. Anonymous-memory medians were approximately
3.68-3.72 GB during completion bands 100-699, then approximately 3.90 GB during
900-1090. Total memory was nonmonotonic as file cache grew and was reclaimed.
There is modest later anonymous growth, so this review does not claim a flat
memory curve. The bounded in-flight implementation and observed plateau/drop
profile support the no-watershed-retention gate for this workload.

| OR-202 phase | Recorded cgroup bytes |
| --- | ---: |
| Model start | 348852224 |
| Executor closed | 630845440 |
| Native post start | 675962880 |
| Post complete | 1622880256 |
| RQ pipeline complete | 2139455488 |

Confirmed: the nearby post-start sample contained 429,088,768 anonymous bytes,
demonstrating release of most model-process residency before post-processing.
The post-complete sample contained 528,506,880 anonymous bytes and
1,068,003,328 file-cache bytes. After the OR-202 workhorse exited, the subsequent
small job started near 260.6 MB anonymous memory despite retained file cache.
The final sample was 200,126,464 anonymous bytes and 1,471,119,360 file bytes.
The sampled `pids.current` count includes threads; its common modeling value of
1,195 must not be described as 1,195 worker processes.

Disposition: complete-workflow performance and memory gates accepted. The
model-to-post boundary released its large process residency, both jobs and their
consumers completed, and total usage remained below the gate throughout.

## Release binding and residual limits

Confirmed: independently verified every byte count and SHA-256 in
[release-integration-manifest.json](release-integration-manifest.json): 37 native
entries, including all 33 Rust source files, and four WEPPpy production entries.
The built and canonical release shared objects have identical bytes and SHA-256
`c6b746bb77be39d38321a365df1762fd8d88ce0522f3a51215d5bb8bfdddf248`.
The declared image ID exists locally and matches the recorded launch commands;
all three declared toolchain versions match their executable readback.

Confirmed: both frozen Python files match their declared hashes and are byte
identical to `git show abebd09239f398af7627924c4c000ff3229a01ee` for the
corresponding ash-transport source paths. The corrected small summary matches
the independently audited `dcc822` measurements. The final wrapper-parity log
records success for both final small candidates and the streamed OR-202 oracle.
The source/binary binding review condition is closed.

The `dcc822` standalone commands do not explicitly set `WEPPPY_NCPU=12`;
standalone AshPost does not use that scheduler setting, while the Compose
override sets it explicitly. Later launch commands add it. Commands use
read-only source mounts; the verified manifest identifies their final content.

Performance review approval applies to this measured artifact and the specified
fixtures. Remote commit/LFS and clean-checkout verification remain executor-owned
publication steps. The repeated-call evidence is finite, and the full-workflow
result is one OR-202 plus one subsequent small job; neither establishes an
unmeasured production-wide speed or memory guarantee.

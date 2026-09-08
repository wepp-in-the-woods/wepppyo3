# Forest benchmark: timing gate failed

## Static:

confirmed: The package requires native median <=105% of Python median on
throbbing-sylvan and <=110% on incommensurate-stickball. It explicitly stops
execution when the native median exceeds the wall-time gate.
Native source/binary hashes and build command are in
[native-build.json](native-build.json). No release artifact was refreshed.

## Ran:

confirmed: Five measured executions per implementation per fixture, each in a
fresh one-shot Forest container after one untimed warm-up. Runs alternated
Python/native. The exact argument vectors are in
[benchmark-commands.json](benchmark-commands.json); raw observations, CPU
affinity, memory limit/events, user/system/wall time, bytes, output paths, and
parity are in [benchmark-measurements.json](benchmark-measurements.json).
Scripts: [benchmark.py](benchmark.py), [run_benchmarks.py](run_benchmarks.py).

The image was the existing Forest worker image, pinned by SHA-256, with
Python 3.12, the normal installed Python/native packages, UID:GID 1000:993,
CPUs 0-11, WEPPPY_NCPU=12, and a 12 GiB cgroup limit with no extra swap.
Inputs and both repositories were mounted read-only. Each output path was
unique under the disposable evidence mount; no oracle was overwritten.

Cache condition: warm after the untimed execution, with shared host caches
left intact. memory.peak covers warm-up plus the timed execution in each
container and was read before loading the comparator's output tables.
These are isolated producer measurements, not live RQ-worker acceptance.
Equivalence to the deployed openwepp.org image was not independently
established; the required live workflow gate remains unrun.

| Fixture | Python median (s) | Native median (s) | Native/Python | Gate |
| --- | ---: | ---: | ---: | --- |
| incommensurate-stickball | 0.879993 | 0.817676 | 92.9185% | pass, <=110% |
| throbbing-sylvan | 3.530796 | 3.784380 | 107.1821% | **fail**, <=105% |

The throbbing-sylvan limit is 3.707336 seconds. The candidate misses by
0.077044 seconds. Its slowest measured run is 3.798565 seconds, so the separate
115% worst-run gate passes. All twenty measured outputs pass the strict
comparator, including full schema metadata.

On throbbing-sylvan, maximum Python whole-container peak is 1,008,467,968 bytes;
maximum native peak is 504,229,888 bytes (480.87 MiB). The native/Python peak
ratio is 49.999594%, narrowly passing the 50% reduction requirement by only
4,096 bytes relative to half the Python peak. This margin is not robust
evidence of repeatable superiority at the exact 50% boundary.
The absolute <9 GiB and incremental <=6 GiB requirements pass these
observations. No recorded cgroup OOM or OOM-kill events occurred.

Native MOFE peak is 496,349,184 bytes versus Python 490,467,328 bytes.
The package states a MOFE timing gate, not a separate 50%-memory gate.
No claim of a MOFE memory improvement is made.

[benchmark-summary.json](benchmark-summary.json) contains exact ratios and gate
booleans. Execution stopped before scaling, live Compose RQ acceptance,
WEPPpy retirement, release refresh, review, commit, or push.

## Accepted timing disposition (2026-09-07)

The user explicitly accepted the measured 107.18% native/Python median on the
586-hillslope fixture (exact ratio 107.1821%, native 3.784380 s, Python 3.530796 s).
This satisfies the timing disposition for this candidate and authorizes continued
execution. It is a scoped acceptance of the measured result, not a general
relaxation of correctness, memory, scaling, worst-run, or workflow gates.
Remaining contract corrections and integration should not materially regress
this accepted performance. Retain the measurement and original 105% target as
evidence rather than reclassifying the original measurement as a target pass.

## Metadata-compacted candidate recheck

See [scaling-optimization.md](scaling-optimization.md#five-repeat-timing-recheck)
and compact-benchmark-summary.json for the new five-repeat result: native
3.813276 seconds, Python 3.541915 seconds (107.6614%), all outputs passing
parity. Original measurements and their scoped acceptance remain historical.

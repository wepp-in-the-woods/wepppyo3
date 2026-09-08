# Execution results

## Delivered behavior

Static: required native AshPost streams projected 8,192-row batches into keyed
aggregates and the existing five output files. WEPPpy supplies ordered scalar
metadata and retains NoDb, versions, documentation and catalog ownership.
The hillslope scheduler loads at most twice the worker count of inputs in flight
and closes its executor before post-processing. Scientific model code is unchanged.
Existing individual-file ParquetSink behavior is reused; no transaction,
rollback/recovery, generation or NFS storage mechanism was introduced.

## Parity and workflow

Ran: `wrapper-parity.json` compares final `dcc822-1` small outputs and OR-202
native output against fresh Python on identical source inputs. All five tables
match schemas, metadata, row order and values at rtol 1e-10 / atol 1e-12;
return-period scalar types and mappings match. Both small-model facades,
generated README and version manifests match. `scheduler-output-parity.json`
records exact byte equality for all 20 freshly regenerated hillslope files
under old/new schedulers, covering Srivastava and Watanabe dynamic.

Ran: `compose-result.json` records OR-202 (1,090 hillslopes) and then canine-liar
completed on the same real RQ worker. The actual `run_ash_rq` path includes
model execution, post-processing and the later totalwatsed3 rebuild. All five
outputs, NoDb/report facades, generated docs, version and catalog checks pass.
Kernel cgroup peak: 4,756,385,792 bytes (4.43 GiB). Limit: 12 GiB. OOM, limit,
restart and abandoned-job counts: zero. See `compose-state.json`,
`compose-phases.jsonl`, `compose-rq.log` and the independently audited
`performance-review.md`. This is a Forest Compose acceptance run, not deployment.

Ran: source readback matches 1,115 OR-202 NoDb/Parquet files on HPC and 42
small-fixture source records. `source-readonly-verification.json` and
`frozen-source-manifest.json` preserve the checks. Large fixtures remain in
ignored disposable storage, outside Git.

## Measured performance

Ran: five warm, isolated runs per small fixture/implementation use the same
image, UID/GID, CPU affinity and 12 GiB limit. Corrected streaming sampler data
are in `final-benchmark-measurements.json`; raw commands, states and hashes are
indexed in `README.md` and `raw-evidence-manifest.json`.

| Fixture | Python median | Native median | Incremental memory reduction |
| --- | --- | --- | --- |
| canine-liar / Srivastava | 1.139 s | 0.193 s | 81.8% |
| assisted-weakness / Watanabe dynamic | 1.122 s | 0.188 s | 82.0% |

Ran: corrected OR-202 Python standalone baseline took 159.4 seconds at 8.40 GiB
peak; native standalone took 44.4 seconds at 0.32 GiB. This is about 72% lower
runtime and 96% lower total peak for this fixture. Standalone timings include
manifest discovery, five output writes and mappings, but exclude docs/facades;
they do not represent full model/RQ runtime.

Ran: eight consecutive OR-202 native calls retained a bounded approximately
442-468 MiB caller RSS band, with no temporary residue. Later cgroup growth was
mainly file cache. First six call times were approximately 45 seconds; last two
were affected by concurrent disposable-fixture copy I/O and are not used as
uncontended timing evidence. This finite repeated-call observation is supported
by static bounded-state review, not a claim of zero allocator/cache variation.

## Validation and review

Ran: installed native tests 139 passed; Rust 98 library + 17 writer tests passed;
WEPPpy focused ash tests 23 passed; combined interchange/report tests 85 passed,
1 skipped. Both ash stubtests, stub inventory, RQ graph, broad-exception gate,
and scoped two-pass/per-file isolation check passed. No randomization plugin
was installed, so isolation runs did not shuffle. Raw command logs are indexed
beside this report. Full WEPPpy suite passed: 7,742 passed, 72 skipped, 3,105 warnings in
792.90 seconds; see `full-wepppy-tests.log`.

Static/Ran: correctness, security, QA and performance reviews have no unresolved
medium/high finding. Initial full-suite attempts exposed a stale startup hash
expectation and test import cleanup that deleted real modules; both are fixed.
The module-identity regression and combined/report isolation runs pass.
One subsequent full-suite attempt and an unnecessarily broad default isolation
run were deliberately interrupted while narrowing that test-only diagnosis.

## Release boundary

Static/Ran: `release-integration-manifest.json` binds native source/package
hashes, Python/Rust toolchain, integration source and the acceptance image.
Native-first commit/push and clean remote verification remain to complete at
this checkpoint. The canonical binary SHA-256 is
`c6b746bb77be39d38321a365df1762fd8d88ce0522f3a51215d5bb8bfdddf248`.
WEPPpy commits use `[skip ci]` because master push otherwise publishes an image.
No registry publication or deployment is part of this package.

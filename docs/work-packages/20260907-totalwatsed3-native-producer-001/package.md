# 20260907-totalwatsed3-native-producer-001

## Status

- state: in_progress
- date: 2026-09-07
- execution host: `forest` via `ssh forest`
- repositories: `/workdir/wepppyo3` and `/workdir/wepppy`

## Objective

Implement `totalwatsed3.parquet` as a required, bounded-memory native producer
in `wepppyo3.wepp_interchange`, integrate it into WEPPpy, and fully retire the
Python/DuckDB/pandas producer without a runtime fallback. Prove exact contract
parity on single-OFE and real MOFE fixtures and prove wall-time and peak-memory
acceptance on a 586-hillslope Forest/HPC fixture under the open-wepp.org worker
limit of 12 GiB.

## Incident basis

The open-wepp.org batch run `nasa-roses-202608-psbs` produced 17 worker-container
OOM events. Recoverable worker logs ended at `_build_totalwatsed3`. A large leaf
had approximately 9.6 GB of `H.*.parquet` inputs; live DuckDB selected a 12.3
GiB memory limit inside a 12 GiB container while the surrounding RQ/Python
runtime was already resident, and usable DuckDB spill was not configured.

The existing producer materializes DuckDB query results as multiple pandas
frames, merges and sorts them, converts the result to Arrow, and combines Arrow
chunks before writing. The native implementation must stream projected Parquet
batches and bound aggregation state primarily by simulation duration, not by
hillslope or input-row count.

## Ownership boundary

- WEPPpy continues to own NoDb/RQ orchestration, run paths, configuration,
  status logging, dependency ordering, and user-visible artifact publication.
- `wepppyo3.wepp_interchange` owns the deterministic
  `H.*.parquet -> totalwatsed3.parquet` transformation.
- The native API is required in production. Missing native support must fail
  explicitly; do not preserve or add a Python, DuckDB, or pandas fallback.
- This package does not change WEPP physics, formulas, units, output schema, or
  downstream report contracts.

## Forest and fixture locations

Run all production-scale work on `forest`. Forest mounts the HPC NFS dataset at
`/wc1`; `throbbing-sylvan` is stored on HPC and is consumed through that mount.
Treat both source run trees as read-only. Copy only the named Parquet artifacts
into repository fixture directories, verify checksums, then operate exclusively
on the copies.

### Single-OFE parity fixture

Reuse the checked-in WEPPpy fixture:

`/workdir/wepppy/tests/wepp/interchange/fixtures/decimal-pleasing/wepp/output/interchange/`

It contains 67 single-OFE hillslopes and all required/optional consolidated
inputs plus an existing `totalwatsed3.parquet` oracle.

### MOFE parity fixture

HPC source through Forest:

`/wc1/runs/in/incommensurate-stickball/wepp/output/interchange/`

Copy these five files into a dedicated Git LFS fixture directory under
`/workdir/wepppyo3/tests/fixtures/totalwatsed3/incommensurate-stickball/`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `H.pass.parquet` | 10,220,960 | `53b272bd1dfee2169ae2680cee26d32e561b1f6486fcaec753e64f33ce3cec75` |
| `H.wat.parquet` | 28,590,445 | `3f10ff9f094484e82ef806818279a1565b2d9e4325c0a18b93810e2ae7da173b` |
| `H.soil.parquet` | 16,281,551 | `3f724eaf85777668b4d6d12487402d3aeab78f5f1ea07889e43b4e218b941f3a` |
| `H.element.parquet` | 1,404,015 | `d2e4ad9857de1b202022ee5db9cd919a9f938cb706760da5ec2c06d3f32bddaf` |
| `totalwatsed3.parquet` | 1,305,733 | `40ce2dac5edd5fe2fae323df47e033e25f578c745f46eb3b567287b700f22342` |

Total LFS payload: 57,802,704 bytes (55.12 MiB).

### Performance and memory fixture

HPC source through Forest:

`/wc1/runs/th/throbbing-sylvan/wepp/output/interchange/`

This is the canonical Blackwood/Lake Tahoe performance fixture with 586
hillslopes. Copy these five files into
`/workdir/wepppyo3/tests/fixtures/totalwatsed3/throbbing-sylvan/` under Git LFS:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `H.pass.parquet` | 192,344,012 | `abc5a61f8de2cb0d775a2f1deb8ee7f5feca57f130190efac1937342de085974` |
| `H.wat.parquet` | 169,477,490 | `303707b3997b0d2a9335e315bba27279f75713a0cdf185c30b8436a74aa5068c` |
| `H.soil.parquet` | 95,126,144 | `654bd84a46c7241f72a11d413b12d8211aa68b46cc0befabbe712169a6b4892e` |
| `H.element.parquet` | 13,109,322 | `e90c25de7764d69edb630acf6cb8510886da4ca9c7712bdbd08ef77bb30fcdc9` |
| `totalwatsed3.parquet` | 2,941,016 | `261b4180bfe6839afab9b712f1235bc11af1e4afdbf3fd26a7b8dfca2069e2fa` |

Total LFS payload: 472,997,984 bytes (451.09 MiB). Combined new MOFE and
performance LFS payload: 530,800,688 bytes (506.21 MiB).

Add a fixture manifest containing source run IDs, source paths, byte sizes,
SHA-256 values, OFE distribution, simulation duration, row counts, Parquet row
groups, schema fingerprints, and oracle provenance. Verify `git lfs ls-files`
and inspect staged blobs to prove that repository objects are LFS pointers.

## Included scope

- Freeze the existing Python producer output as the oracle before deleting it.
- Add a narrow Python-callable native function to the existing
  `wepppyo3.wepp_interchange` module.
- Stream only projected columns from bounded Arrow record batches/row groups.
- Preserve the full date key, aggregation formulas, units, column order, Arrow
  types, nullability, schema metadata, baseflow recurrence, sediment and ash
  calculations, optional-field behavior, `wepp_ids` filtering, and the MOFE
  last-OFE-only `latqcc` rule.
- Write through the existing failure-atomic Parquet machinery.
- Add Rust unit/contract tests and Python release-API tests.
- Add single-OFE, MOFE, and performance/memory fixture coverage.
- Update WEPPpy to require the native producer and remove the Python producer,
  DuckDB implementation, fallback behavior, implementation-only tests, and
  obsolete documentation.
- Refresh the canonical py312 release artifact and provenance as required by
  the wepppyo3 release procedure.
- Validate downstream return-period, water-balance, DSS, WATAR, batch, culvert,
  migration, query-engine, and RQ-stage consumers.

## Explicitly out of scope

- WEPP model or binary changes.
- Output schema, formula, units, or public API changes.
- Raising the open-wepp.org 12 GiB worker limit.
- Reducing `WEPPPY_NCPU` as the primary fix.
- Adding DataFusion, Polars, or another query engine without a separate
  dependency-evaluation decision.
- Mutating the two HPC source runs.
- Deploying to openwepp.org; production deployment and batch acceptance require
  a later explicitly authorized rollout package.

## Proposed native contract

Keep the existing WEPPpy public `run_totalwatsed3(...)` facade if downstream
callers rely on it, but make it an orchestration-only wrapper over a required
native function. The native function should accept file paths, output path,
baseflow scalar values, optional `wepp_ids`, optional ash inputs/metadata, and
schema version. It must return only a compact write/telemetry summary, never a
full Python DataFrame or Arrow table.

Do not finalize the exact native signature until the preimplementation contract
artifact maps every existing argument and optional ash behavior.

## Required phases

1. Baseline inventory and fixture provenance.
2. Python-oracle capture and semantic comparator implementation.
3. Native contract and bounded-memory design review.
4. Rust implementation with focused unit tests.
5. Single-OFE and real-MOFE parity closure.
6. Forest wall-time, memory, scale, and repeatability benchmarks.
7. Required WEPPpy integration and complete Python-producer retirement.
8. Cross-repository validation, release refresh, independent review, commits,
   pushes, and final handoff.

## Correctness gates

- Identical output column order, Arrow types, nullability, schema metadata, row
  count, and date keys.
- Exact equality for integer/date fields.
- Floating-point parity at `rtol=1e-10`, `atol=1e-12`; any exception requires a
  named column, quantified maximum error, cause, and explicit review decision.
- Parity covers single-OFE, MOFE, missing/present optional soil and element
  fields, ash/no-ash behavior, empty inputs, legacy interchange schemas, and
  `wepp_ids` subsets.
- Last-OFE-only MOFE lateral flow is directly asserted.
- Atomic publication and cleanup are proven under injected write failure.
- All named downstream consumers pass with the native-only implementation.

## Forest performance and memory gates

Use a dedicated one-shot Forest container matching the open-wepp.org worker
runtime, with a 12 GiB cgroup memory limit and the normal Python/native package
set. Record exact commands, revisions, fixture checksums, CPU allocation, page
cache condition, input/output bytes, wall/user/system time, and cgroup
`memory.peak`. Do not overwrite the oracle; use unique output paths.

After one untimed warm-up, run each implementation five times on
`throbbing-sylvan`:

- Native median wall time must be no greater than 105% of Python median.
- No native run may exceed 115% of Python median.
- Preferred native target is no greater than 75% of Python median.
- Peak whole-container memory must be below 9 GiB, with a preferred target of
  at most 8 GiB; any observation above 10 GiB rejects the implementation.
- Native incremental peak above the pre-call worker baseline must be at most 6
  GiB, preferably at most 4 GiB.
- Native peak must be at least 50% lower than Python peak unless Python cannot
  complete under 12 GiB, in which case successful native completion with the
  required headroom is the governing gate.
- Three consecutive executions must complete without a materially increasing
  post-call baseline, container restart, or leaked temporary file.

Run the same parity path on `incommensurate-stickball`; native median wall time
must be no greater than 110% of Python median.

Because 586 hillslopes are smaller than the 6,364-hillslope incident leaf, add
a deterministic noncommitted scale generator that repeats and renumbers
`throbbing-sylvan` row groups. Measure approximately 73, 146, 293, 586, 2,344,
and 5,860 hillslopes. Peak memory at 5,860 hillslopes must be no greater than
1.5 times the 586-hillslope peak and must remain below 9 GiB for the complete
container. Wall time may scale approximately linearly with bytes read; record
and investigate superlinear behavior.

Finally execute the native-only producer through the real Forest Compose RQ
worker path with `WEPPPY_NCPU=12` and a 12 GiB limit. Require peak container
memory below 9 GiB, zero restarts, zero `AbandonedJobError`, parity success,
completion of downstream post-processing, and successful acceptance of a
subsequent job.

## Intended write set

### wepppyo3

- `.gitattributes`
- `Cargo.lock`
- `wepp_interchange/Cargo.toml` only if existing dependencies are insufficient
- `wepp_interchange/src/**/*.rs`
- `tests/wepp_interchange/**/*.py`
- `tests/fixtures/totalwatsed3/**`
- `release/linux/py312/wepppyo3/wepp_interchange/**`
- `README.md`
- `docs/module-registry.md`
- `docs/release-provenance.md`
- `docs/work-packages/README.md`
- `docs/work-packages/20260907-totalwatsed3-native-producer-001/**`

### WEPPpy

- `.gitattributes` only if WEPPpy receives fixture pointers
- `wepppy/wepp/interchange/**`
- `wepppy/rq/**` only where native-only integration or telemetry requires it
- `tests/wepp/interchange/**`
- named downstream tests affected by the native-only contract
- relevant interchange/operator/developer documentation
- a WEPPpy work-package or release note that records the cross-repository
  integration and native revision

Do not modify unrelated dirty files in either repository.

## Validation

Minimum commands, augmented by narrower tests discovered during execution:

```sh
cd /workdir/wepppyo3
cargo fmt --check
cargo test -p wepp_interchange_rust
python3 -m pytest tests/wepp_interchange
git lfs ls-files
git diff --check

cd /workdir/wepppy
wctl run-pytest tests/wepp/interchange
wctl run-pytest tests/wepp/reports
wctl run-pytest tests/rq/test_wepp_rq_stage_post.py
wctl run-pytest tests/rq/test_batch_rq_retry_selection.py
wctl run-pytest tests/rq/test_culvert_rq_nodir_guards.py
wctl run-pytest tests/wepp/interchange/test_watershed_totalwatsed_export.py
git diff --check
```

Run broader suites when touched contracts or failures indicate they are
required. Validate the built py312 release artifact through its installed
Python import path, not only the Cargo test binary.

## Commit and publication sequence

1. Keep each repository on its existing branch unless the operator explicitly
   authorizes branch creation.
2. Commit and push the wepppyo3 implementation, LFS objects, tests, docs, and
   release artifact first.
3. Record that exact wepppyo3 revision in the WEPPpy integration evidence.
4. Commit and push the WEPPpy native-only integration and Python retirement.
5. Do not claim completion until both remote revisions and all LFS objects are
   verified downloadable from a clean checkout on Forest.

## Exit criteria

- The two fixture sets are committed through Git LFS with verified source
  checksums and provenance.
- Native parity passes for single-OFE, real MOFE, optional fields, ash, legacy
  interchange schema, and subset cases.
- The canonical `throbbing-sylvan` performance and memory gates pass on Forest.
- The generated near-incident scale gate demonstrates bounded memory.
- The Forest Compose native-only RQ integration gate passes under 12 GiB.
- WEPPpy contains no executable Python/DuckDB/pandas totalwatsed3 producer and
  no runtime fallback.
- Required release artifacts, documentation, and module registry are current.
- Both repositories are clean, committed, pushed, and independently reviewed.

## Stop conditions

Stop and preserve evidence if output semantics differ, source fixture checksums
do not match, a source run would be mutated, native peak exceeds 10 GiB, the
5,860-hillslope peak exceeds 9 GiB or grows materially with hillslope count,
the native median exceeds the wall-time gate, LFS objects are not remotely
available, release provenance cannot be established, or integration requires a
new dependency or output-contract change outside this package.

## Security impact

- security_impact: high
- dedicated_security_review_required: yes
- rationale: production-native code parses multi-gigabyte files and atomically
  replaces shared-run artifacts. Review path validation, temporary-file
  placement, symlink behavior, malformed Parquet handling, bounded allocation,
  panic/error translation, and failure-atomic publication. Fixture manifests
  must contain no credentials or user-sensitive metadata.

## Approved single-OFE oracle authority (2026-09-07)

The user approved designating the captured current-producer output as the
single-OFE parity oracle and resuming execution, while preserving the old
WEPPpy fixture. The active oracle is now
`tests/fixtures/totalwatsed3/decimal-pleasing/totalwatsed3.parquet` in wepppyo3,
SHA-256 `e7c6e2f0a37093a59540dff0b67bbb44a25f9abdea6e145a6bd65d4c37809550`.
Input files continue to come from the unchanged WEPPpy decimal-pleasing fixture.
The new oracle was captured from WEPPpy revision
`3c51780f50e2599ef72b03214c35bf20538e55c4`, with gwstorage=0, bfcoeff=0.04,
dscoeff=0, and no ash. It preserves current PASS-runoff semantics and all
79 columns. Reverting to the older WAT-Q runoff formula was rejected because
this package is a faithful extraction of current production behavior.
The old 68-column fixture remains historical evidence, not the native parity
authority. No numerical tolerance or other package gate changes.

## Accepted timing disposition (2026-09-07)

The user explicitly accepted the measured 107.18% native/Python median on the
586-hillslope fixture (exact ratio 107.1821%, native 3.784380 s, Python 3.530796 s).
This satisfies the timing disposition for this candidate and authorizes continued
execution. It is a scoped acceptance of the measured result, not a general
relaxation of correctness, memory, scaling, worst-run, or workflow gates.
Remaining contract corrections and integration should not materially regress
this accepted performance. Retain the measurement and original 105% target as
evidence rather than reclassifying the original measurement as a target pass.

## Scaling optimization request (2026-09-07)

The user requested reducing the measured 1.7906-times memory ratio toward 1.5.
This authorizes profiling and implementation improvements under the existing
parity and resource limits. Compact source metadata now measures 1.45178 times;
see artifacts/scaling-optimization.md. The original fixture, output contracts,
and failed measurements are preserved. No scaling threshold was relaxed.

## Release/integration authorization (2026-09-07)

After the optimized 1.45178 scaling ratio and 107.6614% timing recheck were
reported, the user directed: "build the release and integrate in wepppy".
Proceed with that measured candidate; retain the original targets and measured
results as evidence. Add the mandatory paired Docker startup hash/API update to
the write set. Integration also requires bounding interchange README previews:
the native output succeeded, but the old full-table preview read OOM-killed the
12 GiB RQ work horse. Read schema plus the existing three preview rows instead;
no artifact schema, formulas, or documentation contents are intentionally changed.

## Review and container publication authorization (2026-09-07)

The user requested completing review and container publication. Finish the
independent review gates, publish the native repository and verify remote LFS
retrieval, then pin that native commit in WEPPpy
`.github/workflows/publish-weppcloud-image.yml`. Publish WEPPpy on its existing
master branch through the canonical GHCR workflow and verify the immutable image
contains the paired native release and facade. This extends publication scope to
the common runtime image; production deployment remains outside this package.

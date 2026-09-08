# 20260908-ashpost-native-001

## Status

- state: in progress
- date: 2026-09-08
- execution host: `forest` via `ssh forest`
- repositories: `/workdir/wepppyo3` and `/workdir/wepppy`
- baseline revisions: wepppyo3 `05328ddcd0a8f91dfeae4c25f533fd69d74dd963`;
  WEPPpy `abebd09239f398af7627924c4c000ff3229a01ee`

## Operator-confirmed scope

The operator confirmed on 2026-09-08 that this is a memory optimization using
the established hillslope interchange -> totalwatsed3 pattern. Write complete
files individually using the existing writer behavior; aggregate completed
inputs. No new multi-file transaction, all-five staging barrier, rollback or
recovery system, generation-directory layout, migration, or NFS-specific
publication mechanism is required. Those assistant-added proposals are
superseded. Preserve normal failure propagation and regeneration on retry.

## Objective

Replace WEPPpy's whole-watershed pandas `AshPost` aggregation with required,
bounded-memory Rust processing in `wepppyo3`, while changing the Python ash
hillslope producer so completed work and its input data are released promptly.
Preserve the existing per-hillslope model files, AshPost outputs, NoDb state,
reports, documentation, and catalog behavior. Prove both Srivastava and Watanabe
dynamic model compatibility and complete the real OR-202 workflow below the
open-wepp.org worker's 12 GiB cgroup limit.

## Incident basis

Openwepp.org batch job `8eb973ed-d7e6-4b5d-be24-b371237cea1e` ran
`nasa-roses-202608-psbs`. Kubernetes OOM-killed three 12 GiB batch containers
with exit code 137: OR-8 on Dell-03, OR-202 on Dell-04, and WA-77 on Dell-02.
RQ later classified the interrupted leaves as `AbandonedJobError`.

All three workers logged every hillslope ash task complete: OR-8 `2182/2182`,
OR-202 `1090/1090`, and WA-77 `2336/2336`. Their per-hillslope parquet files
exist. Each `ash/post/` directory was created but remained empty, locating the
immediate failure in the first AshPost aggregation before publication.

The producer in `wepppy/nodb/mods/ash_transport/ash.py` also retains an `args`
list containing every hillslope's climate and water-balance DataFrames, submits
all tasks and futures at once, and uses up to `WEPPPY_NCPU` spawned processes
(`12` in production). `AshPost.watershed_daily_aggregated()` then reads every
hillslope parquet into a list, concatenates the watershed, makes two deep
copies, and reads the inputs again. Native AshPost must not conceal the producer
retention defect; both boundaries are included.

## Ownership boundary

- WEPPpy owns run discovery, WEPP/Topaz translation, burn class and area
  metadata, model selection, per-hillslope execution orchestration, NoDb state,
  report facades, documentation generation, catalog activation, and RQ status.
- `wepppyo3` owns the bounded transform from a validated manifest of
  per-hillslope ash parquet files to the complete set of AshPost parquet outputs
  and compact return-period telemetry.
- Per-hillslope scientific models remain in WEPPpy. This package changes their
  scheduling and object lifetimes, not their equations or output schema.
- Native AshPost support is required. The production path must have no
  executable pandas/Dask/DuckDB aggregation fallback.

## Fixture locations and provenance

Use the fixtures in place on Forest/HPC. Treat every source run as read-only and
write captures and candidate outputs only under a package-owned disposable
directory. Record source revision, configuration stem, model, input manifest,
file sizes, row counts, schemas, and SHA-256 checksums before testing.

Compare fresh unchanged-Python and native outputs on identical frozen inputs,
including the hydrology visible at AshPost entry. Preserve historical outputs
separately. The existing RQ workflow rebuilds totalwatsed3 after AshPost, so a
later upstream generation need not reproduce an earlier saved post output.
Do not change that workflow order or relax numerical tolerances.

### Srivastava single-OFE contract fixture

Use Forest project `canine-liar`:

`/wc1/runs/ca/canine-liar/`

This exercises the Srivastava ash model with single-OFE hillslopes. Freeze the
current Python AshPost outputs, return-period dictionaries, `AshPost` facade
properties, generated documentation, version manifest, and catalog-visible
datasets before removing the Python producer.

### Watanabe dynamic single-OFE contract fixture

Use Forest project `assisted-weakness`:

`/wc1/runs/as/assisted-weakness/`

This exercises the Watanabe dynamic model with single-OFE hillslopes. It is a
separate required parity fixture; Srivastava success cannot stand in for it.

### Production-scale memory and performance fixture

Use OR-202 on host `hpc` at:

`/tank/kubernetes/weppcloud/weppcloud-wc1/pvc-d4c5528b-3204-438b-909a-61af286d6fea/batch/nasa-roses-202608-psbs/runs/OR-202/`

The source contains 1,090 completed per-hillslope ash outputs from the failed
production attempt. Capture a Python oracle only in a sufficiently provisioned,
controlled baseline process if it can finish safely. Never rerun or mutate the
HPC source tree. Forest has verified noninteractive access with `ssh hpc`; copy
only the required files into a disposable Forest fixture directory before
testing. Candidate output must go to that disposable clone or a redirected
output directory. If the baseline cannot complete under 12 GiB, preserve that
failure as evidence and compare candidate outputs against a successful
high-memory oracle plus the smaller exact fixtures.

Do not commit the complete OR-202 project or duplicate its parquet payloads.
Commit only small purpose-built edge fixtures if tests cannot express a
contract with generated data. Put large reusable fixtures in Git LFS and record
why they are necessary, their provenance, sizes, and checksums.

## Existing contract to preserve

Freeze behavior before implementation. Preserve all five `ASH_POST_FILES`
outputs and their exact filenames:

1. `hillslope_annuals.parquet`
2. `watershed_annuals.parquet`
3. `watershed_daily.parquet`
4. `watershed_daily_by_burn_class.parquet`
5. `watershed_cumulatives.parquet`

Preserve column order, Arrow types, nullability, metadata, units, descriptions,
row ordering, compression compatibility, `ASHPOST_VERSION`, version manifest,
return-period and burn-class dictionaries, cumulative semantics, recurrence
defaults, zero-area behavior, absent-hillslope behavior, and both ash-model
schemas. Preserve `AshPost.return_periods`, `cum_return_periods`,
`burn_class_return_periods`, `hillslope_annuals`, `watershed_annuals`,
`pw0_stats`, `ash_out`, documentation generation, and catalog activation.

This is faithful extraction. Do not change scientific formulas, unit
conversions, Weibull/recurrence selection, fire-year rules, field names,
version-removal policy, public facades, or model calibration. Any ambiguity
must be resolved by a frozen oracle and documented; semantic correction needs
a separate operator decision.

## Native contract

Add a narrow API to `wepppyo3.wepp_interchange`; freeze its final Python
signature and Rust-side data contract in an artifact before implementation. It
must:

- accept a destination directory and ordered manifest containing input path,
  Topaz ID, area in hectares, and burn class, plus the optional hydrology paths
  and independent ash WEPP-ID selection needed by existing daily calculations;
- validate that paths and metadata are one-to-one and reject duplicates,
  traversal, symlink escape, incompatible schemas, missing required columns,
  invalid IDs/classes/areas, malformed parquet, and mixed model contracts;
- project and stream row groups or bounded record batches rather than collecting
  watershed source tables/rows; keyed output aggregate state is allowed;
- aggregate hillslope annual, watershed annual/daily, burn-class daily, and
  cumulative products with state bounded by the output key cardinality;
- calculate existing return-period structures without returning full tables to
  Python;
- write complete parquet files individually through the existing writer pattern;
  propagate failures normally and clean up each writer's temporary file using
  established behavior; do not add a multi-file transaction or rollback system;
- preserve existing schemas and Arrow metadata exactly; and
- return compact telemetry and return-period dictionaries only.

No panic may cross the Python boundary. Use checked conversions and stable,
specific Python exceptions. Do not add a new external dependency unless the
precedent and capability gate is documented and accepted.

## Bounded hillslope execution contract

Refactor `Ash.run_ash()` so modeling does not accumulate watershed-sized Python
state. Construct one hillslope work item at a time and keep at most a small,
documented multiple of `max_workers` submitted futures in flight. When a future
finishes, consume its result, remove all references to its arguments and future,
and submit the next item. Do not build a list of every hillslope DataFrame.

The executor must be fully shut down before native AshPost starts. Release the
pending map, completed futures, iterator locals, and any last large DataFrame;
prove with measurement that resident memory returns to a stable post-model
baseline. Preserve fail-fast cancellation, exception propagation, progress
counts/order semantics, output filenames, deterministic values, and serial mode.
Do not lower `WEPPPY_NCPU`, raise memory limits, disable multiprocessing, or use
manual garbage collection as the primary fix.

## Included scope

- Freeze Python oracles for both model variants and all AshPost outputs/facades.
- Implement and test native streaming AshPost in `wepppyo3`.
- Integrate it as required native functionality in WEPPpy with no fallback.
- Bound per-hillslope scheduling and promptly release inputs/results.
- Preserve existing file-writing, versioning, documentation, catalog, NoDb, and RQ behavior.
- Refresh the canonical py312 native release and provenance, WEPPpy startup
  preflight, stubs, native pin, and relevant documentation.
- Validate focused modules, the complete WEPPpy suite, and a real Forest Compose
  RQ workflow under a 12 GiB limit.
- Complete independent correctness, QA, performance, and security reviews and
  resolve all medium/high findings before publication.

## Explicitly out of scope

- Changing either ash model's scientific calculations or calibration.
- Replacing the per-hillslope ash model with Rust.
- Raising the 12 GiB production limit or reducing concurrency as the solution.
- Changing queue topology, RQ dependency behavior, user routes, or batch retry
  policy.
- Mutating fixture projects, the HPC batch tree, or active production jobs.
- New multi-file transactions, rollback/recovery systems, generation-directory
  storage, migrations, and NFS-specific publication redesign.
- Deploying to openwepp.org. Deployment requires a later explicit request after
  workload state is checked.

## Correctness gates

- Exact schema, metadata, ordering, integer, categorical, filename, version,
  documentation, and facade parity on `canine-liar` and `assisted-weakness`.
- Floating values must match at `rtol=1e-10`, `atol=1e-12`; every exception
  requires quantified evidence and operator disposition.
- Exact per-hillslope file hashes before and after the scheduling refactor, or
  column/value parity when nondeterministic parquet metadata prevents byte
  equality.
- Edge coverage for empty manifests, missing files/columns, null/nonfinite
  values, zero area, duplicate metadata, invalid burn classes, malformed
  parquet, mixed schemas, ordinary write failure, symlinks, and stale output
  version removal.
- Existing return-period dictionaries, documentation, query catalog, NoDb
  persistence, and report/UI consumers continue to work.
- Static inspection confirms no executable whole-watershed Python AshPost
  producer or runtime fallback remains.

## Forest performance and memory gates

Measure a one-shot container matching the Forest/open-wepp.org worker image,
UID 1000, GID 993, umask 0022, CPU allocation, `WEPPPY_NCPU=12`, and a hard
12 GiB cgroup limit. Record exact revisions, image ID, fixture hashes, cold/warm
cache condition, wall/user/system time, output hashes, `memory.current`,
`memory.peak`, `memory.events`, process count, exit status, and restart count.

For each small fixture, compare five warm Python-oracle and five final-native
AshPost runs. The native median wall time must be no greater than 105% of the
Python median and no native run may exceed 115% of the Python median. Native
incremental peak must be at least 50% lower and below 2 GiB. If a metric is too
short for stable timing, use sufficient repetitions without modifying sources.

For OR-202, run the complete bounded hillslope scheduler followed immediately by
native AshPost through the real Forest Compose RQ path. Require:

- total container `memory.peak` below 9 GiB, providing at least 3 GiB headroom;
- zero `oom`, `oom_kill`, restart, `AbandonedJobError`, or incomplete output;
- no monotonic memory growth with hillslope count and no watershed-sized
  retained input/future collection;
- all five post files, facade values, documentation, catalog activation, NoDb
  completion, and RQ success; and
- acceptance of a subsequent small job in the same worker without restart.

Run three consecutive OR-202 native AshPost calls against disposable outputs.
The post-call baseline must not materially increase and temporary files must not
accumulate. Native OR-202 wall time must not exceed 105% of a successful Python
baseline. If Python cannot finish under 12 GiB, native must finish under the
9 GiB gate and its timing must be reported without claiming a speed ratio.

## Intended write set

### wepppyo3

- `Cargo.lock` and `wepp_interchange/Cargo.toml` only if existing dependencies
  are insufficient
- `wepp_interchange/src/**/*.rs`
- `tests/wepp_interchange/**/*.py`
- `release/linux/py312/wepppyo3/wepp_interchange/**`
- `README.md`, `docs/module-registry.md`, `docs/release-provenance.md`
- `docs/work-packages/README.md`
- `docs/work-packages/20260908-ashpost-native-001/**`

### WEPPpy

- `wepppy/nodb/mods/ash_transport/ash.py` and `ashpost.py`
- ash-transport stubs, versioning, documentation, and README as required
- required-native interchange/startup integration and native pin/provenance
- focused tests under the ash-transport, interchange, NoDb, RQ, report, query,
  and UI snapshot areas
- no unrelated files or fixture payloads

## Validation

Minimum commands, augmented by narrower tests discovered during execution:

```sh
cd /workdir/wepppyo3
git lfs pull
git lfs fsck
cargo fmt --check
cargo test -p wepp_interchange_rust
python3 -m pytest tests/wepp_interchange --maxfail=1
python3.12 -c "import wepppyo3.wepp_interchange"
git diff --check

cd /workdir/wepppy
wctl run-pytest tests/nodb/mods/test_ashpost_no_data.py tests/nodb/mods/test_ash_transport_run_ash.py tests/nodb/mods/test_ash_multi_year_model_alex_static.py --maxfail=1
wctl run-pytest tests/wepp/interchange --maxfail=1
wctl run-pytest tests/nodb --maxfail=1
wctl run-pytest tests/rq --maxfail=1
wctl check-rq-graph
wctl check-test-stubs
wctl run-stubtest wepppy.nodb.mods.ash_transport.ash
wctl run-stubtest wepppy.nodb.mods.ash_transport.ashpost
python3 tools/check_broad_exceptions.py --enforce-changed --base-ref origin/master
python3 tools/code_quality_observability.py --base-ref origin/master
wctl run-pytest tests --maxfail=1
git diff --check
```

Validate the installed canonical py312 release and the real Compose worker, not
only Cargo's test extension or a standalone benchmark.

## Commit and publication sequence

1. Keep both repositories on their existing branches.
2. Commit and push the reviewed wepppyo3 implementation, tests, docs, and release
   artifact first.
3. Pin that exact native revision and shared-object hash in WEPPpy.
4. Commit and push the WEPPpy integration only after all cross-repository gates
   pass.
5. Verify both remote revisions and LFS objects from clean Forest checkouts.
6. Do not publish a registry image or deploy without separate authorization.

## Exit criteria

- Hillslope scheduling is bounded and releases each completed work item.
- Required native AshPost is wired with no whole-watershed Python fallback.
- Both model fixtures have exact contract parity.
- OR-202 completes through the real Compose RQ path below 9 GiB with zero OOM,
  restart, abandoned job, incomplete output, or memory-growth finding.
- A subsequent job succeeds in the same worker.
- Existing writer behavior, malformed input, path, schema, version, facade, documentation,
  catalog, NoDb, and RQ gates pass.
- Full validation and independent reviews have no unresolved medium/high finding.
- Both repositories are clean, committed, pushed, and clean-checkout verified.

## Stop conditions

Stop and preserve evidence if fresh Python/native scientific or public output
semantics differ on identical frozen inputs,
fixture provenance cannot be verified, any source fixture would be mutated,
candidate peak reaches 10 GiB, OR-202 OOMs or restarts, memory grows with total
hillslopes rather than bounded in-flight/output state, native timing misses the
gate, successful completion leaves missing or invalid outputs, a new dependency or public
contract change is required without approval, or remote/release provenance
cannot be verified.

## Security impact

- security_impact: high
- dedicated_security_review_required: yes
- rationale: native code will parse thousands of shared-run parquet files and
  write individual output files, while Python process scheduling
  and shared NoDb workflow boundaries change. Review path containment, symlink
  handling, malformed parquet, allocation bounds, integer overflow, panic/error
  translation, temporary-file permissions, normal write failures, cancellation,
  worker cleanup, and failure observability.

# Security review: native AshPost implementation

## Verdict and scope

- Reviewer: independent `ash_security_review` agent; date: 2026-09-08.
- Security impact: high; dedicated review required.
- Scoped implementation review: **PASS**. Unresolved high: 0; medium: 0;
  low: 0. The findings below were fixed and independently checked.
- This does not replace correctness, performance, installed-release, full-suite,
  or production-equivalent Compose acceptance. Those remain package gates.
- Native base: `8fc2afa91775825a8f6901d63bc7a0d957bdf146` plus reviewed worktree.
  WEPPpy base: `abebd09239f398af7627924c4c000ff3229a01ee` plus reviewed worktree.

Reviewed native `ashpost.rs`, `ashpost_stats.rs`, the `lib.rs` entrypoint,
individual ParquetSink integration, WEPPpy `ashpost.py`, and
`ash.py::_run_hillslope_work`. Scope follows [accepted-scope.md](accepted-scope.md),
[native-contract.md](native-contract.md), and the canonical WEPPpy
`docs/schemas/output-scope-contract.md` section "AshPost file-production scope".
The operator rejected the earlier transaction/publication redesign. Findings
from that superseded proposal are not implementation blockers.

## Threat model

The entrypoint is internal Python orchestration over service-produced run files;
this change adds no route, upload, user-supplied filesystem root, or subprocess
command construction. Run-directory ownership, authorization, and writable
ancestor paths retain the existing trusted-run assumptions used by interchange
and report producers. Arguments and malformed files still receive validation.

Static input symlinks/traversal and output aliases are rejected. Component
checks followed by pathname opens do not protect against an actor concurrently
replacing trusted ancestors. Output ancestors retain existing pathname behavior.
This review does not claim race-proof containment against an actor already
able to rename service run directories, or require a new filesystem boundary.

Individual output publication is intentional. A later file failure can leave
earlier complete outputs; a retry regenerates the set. There is no all-five
transaction, rollback system, reader snapshot, or transaction with NoDb,
documentation, version manifests, and catalog. Existing writer termination and
temporary-file cleanup limits remain applicable.

## Findings and closure

| ID | Severity | Finding and concrete failure path | Remediation and independent closure | Status |
| --- | --- | --- | --- | --- |
| ASH-SEC-01 | Medium | Alias checks initially omitted consumed optional hydrology/H.wat identities. Using an optional input at a canonical output pathname allowed successful overwrite of that input. | All consumed optional input identities now participate in the existing individual output check. Direct same-path probes for both inputs raise ValueError with every input hash unchanged; regression tests also cover hardlink aliases. | Resolved |
| ASH-SEC-02 | Medium | Finite ash conversion and selected H.wat runoff arithmetic could overflow. Runoff infinity was subsequently clamped into plausible zero corrected flow. Raw infinite hydrology was also accepted. | Numeric reads reject infinity, aggregation retains overflow for rejection, output overflow is checked, and runoff/hydrology arithmetic is checked before clamping. Direct finite `QOFE=1e308`, `Area=1e308` probe now raises ValueError before output publication. | Resolved |
| ASH-SEC-03 | Medium | An all-null UTF8 model metric bypassed row-level type checks and was published as zero. | Model metric schema validation rejects unsupported layouts before scanning; numeric validation precedes null handling. The all-null UTF8 probe and Float32 regression reject explicitly while valid Float64 null/NaN cases pass. | Resolved |
| ASH-SEC-04 | Low | Writer I/O errors were flattened into generic OSError strings, losing the expected PermissionError subtype. | InterchangeError::Io now converts its underlying I/O error through PyO3. UID 1000 readonly-directory probe raises PermissionError and leaves no output/stage residue. | Resolved |

No risk acceptance was used to close these findings. The typed I/O conversion
still leaves `errno` unset; the message contains `os error 13` for permission
denial. The native contract requires the appropriate OSError subtype, which is
preserved. Do not claim machine-readable errno preservation.

## Surface checks

| Surface | Review result |
| --- | --- |
| Valid-state noninterference | Empty manifest, absent/empty optional hydrology, Float64 null/NaN metrics, zero area, valid runoff null propagation, ordinary output creation, restrictive existing mode, and successful retry are exercised by direct native tests. Both real-model parity and complete workflow remain separate gates. |
| Authentication, sessions, CSRF, authorization | No changed route or authentication boundary. Existing run access policy remains Python/service responsibility. |
| Secrets and credentials | No added secret, token, credential mount, or credential logging in reviewed code. |
| Input validation | Keys, manifest metadata, required model schema, duplicate identities/IDs, malformed parquet, traversal, static input symlinks, and infinite values fail explicitly. Optional consumed inputs receive alias protection. |
| Filesystem and run scope | Fixed output filenames; per-file existing writer behavior and mode-preservation option. No output generation migration or new filesystem mechanism. Trusted ancestor assumption and termination limits are explicit above. |
| Worker and subprocess lifecycle | Large per-hill inputs are loaded in the parent as slots become available. Pending futures are bounded by `2 * max_workers`; results are consumed and references released. The executor context exits before post-processing. The documented exception boundary logs, cancels pending futures, and re-raises; it does not report false success. No shell/subprocess command surface was added. |
| Agentic tooling and MCP | Review activity used disposable generated files and read-only source inspection. No source fixture, production service, or external message was mutated. |
| Network and external integration | No new egress path or external dependency in reviewed implementation. |
| CI/CD and supply chain | Review tested a specifically hashed local release-build extension. Installed canonical release, exact pins, remote provenance, and any later workflow changes require package validation; this review does not authorize image publication or deployment. |
| Data integrity and NoDb | Existing NoDb, versioning, documentation, and catalog ownership remains in Python. Ordinary exceptions propagate. No queue topology or lock semantics were changed by the reviewed integration. |
| Logging and incident readiness | Explicit schema/value errors and ordinary filesystem exceptions remain observable. Failed individual output production remains a failed call; retry behavior is tested. |

Static inspection found no concrete panic path in the new scalar recurrence
sorting/indexing or entrypoint. No new unsafe Rust was added. Projection and
8192-row batches avoid whole-source table collection; aggregation retains
output-key state. This is not an absolute memory bound for arbitrary malicious
Parquet metadata/pages or unbounded output cardinality. Representative cgroup
acceptance remains required; no new parser resource-limit architecture is
inferred from this review.

## Validation evidence

Ran from `/workdir/wepppyo3`:

```bash
WEPP_INTERCHANGE_TEST_BINARY=/workdir/wepppyo3/target/release/libwepp_interchange_rust.so /workdir/wepppy/.venv/bin/python -m pytest tests/wepp_interchange/test_ashpost.py -q
```

Result: **28 passed**, one existing pytz deprecation warning, 0.78 seconds pytest
reported time. This exercises real generated Parquet files and individual writer
failure/retry rather than mocking the filesystem boundary.

Additional direct probes loaded that same extension with `importlib` under
Python 3.12/PyArrow 16.1.0. One-row generated fixtures were created in unique
temporary directories under `target/ashpost-evidence` and removed after each probe.

| Probe | Confirmed result |
| --- | --- |
| Hydrology input named `watershed_daily.parquet`, destination equal to input root | ValueError: output aliases input; all source hashes unchanged. |
| Selected H.wat input at that canonical output pathname | Same rejection and unchanged source hashes. |
| Finite H.wat QOFE and Area both `1e308` | ValueError: runoff aggregate overflow. |
| Finite wind metric `1e308`, manifest area 10 ha | ValueError: aggregate overflow. |
| All-null UTF8 wind metric | ValueError: model metric must be Float64. |
| Infinite optional hydrology Streamflow | ValueError: infinite numeric value. |
| Destination mode 0500, UID/GID 1000/1000 | PermissionError; source unchanged; no output or temporary residue. |

An initial permission probe additionally asserted `errno == 13`; that assertion
failed because PyO3 leaves errno unset. A corrected probe verified the required
subtype, unchanged input, and no residue. This diagnostic limit is recorded
above rather than presented as a successful errno check.

These direct probes used disposable local storage and the reviewer host identity;
they are not substitutes for UID 1000/GID 993 Compose acceptance or NFS workload
measurement. No source fixture or production output was modified.

Documentation: `markdown-doc lint --path
docs/work-packages/20260908-ashpost-native-001/artifacts/security-review.md
--no-ignore` from the native repository validated one file with no errors or
warnings. The wctl invocation counted zero files for this native-repository path,
so it was not treated as successful document validation. Spelling preview and
`git diff --check` were also checked.

## Reviewed identities

SHA-256 at closure (paths relative to the native repository unless marked):

| File | SHA-256 |
| --- | --- |
| `wepp_interchange/src/ashpost.rs` | `4d437bdd2b0dc99377987c6465f0c5039c428ef468ddb2ac9f17d17d66c60211` |
| `wepp_interchange/src/ashpost_stats.rs` | `70f1dc57bbfcdec0ba5a057060a73fd307d88e197c38682c100f5e1317d18a11` |
| `wepp_interchange/src/lib.rs` | `770bca6b9c94f14ce22c5a81ab68ade63486b1ef090bb38b159688a71eec9a09` |
| `tests/wepp_interchange/test_ashpost.py` | `3f64ac2c13945ae76d9acac0fc24eef84edfb449550964ecffa942b330ecca94` |
| `target/release/libwepp_interchange_rust.so` | `c6b746bb77be39d38321a365df1762fd8d88ce0522f3a51215d5bb8bfdddf248` |
| WEPPpy `wepppy/nodb/mods/ash_transport/ash.py` | `17b8ab600483e5e7b983456382693f64c566202362f45f10457127e7bee221f1` |
| WEPPpy `wepppy/nodb/mods/ash_transport/ashpost.py` | `e3237756cfcc78f192e4300f97f3cdd74945121fef6bb077123e0c8971b94801` |

Implementation changes after these identities need impact review; this artifact
does not approve unseen changes. Package owner records final acceptance and
remaining validation in the tracker.

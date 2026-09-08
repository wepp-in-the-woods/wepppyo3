# Suggested work-package revisions

Superseded by the operator-confirmed simple scope in `accepted-scope.md` and
`../package.md`. Transaction, rollback/recovery, staging-barrier, migration, and
NFS-specific requirements in this historical proposal are not active gates.


Status: investigation complete; recommendations only, not operator-approved.
No native AshPost implementation or canonical contract amendment was made.
This recommendation supersedes the generation-symlink recommendation in
`resumption-decision.md`. The original package remains blocked pending revision.
A complete proposed package is provided in `package.revised.md`; its diff is
`package-revisions.diff`.

## Recommendation

Keep `ash/post/` an ordinary directory. Stage and validate all five parquet
products before publication, then atomically replace each file. Attempt rollback
on ordinary handled publication failures, with an explicit indeterminate/error
outcome when restoration or NFS outcome verification fails. Do not promise a
simultaneously visible five-file generation or a transaction with NoDb, README,
version manifest, and catalog. Defer generation pointers and reader migration to
a separate package if concurrent snapshot consistency becomes a requirement.

Freeze fresh unchanged-Python AshPost outputs against the exact same input
snapshot used by the native candidate. Preserve existing run outputs as
historical evidence, but do not require them to match an upstream file rewritten
after they were produced. Preserve current RQ ordering and scientific formulas.

Keep the existing numerical tolerances, memory/timing thresholds, both models,
12-worker concurrency, full-suite requirements, independent reviews, and
native-first publication sequence. Add actual NFS acceptance rather than
letting an ext4 disposable clone stand in for the run filesystem.

## What the investigation established

| Evidence | Result and implication |
| --- | --- |
| Restored Srivastava fixture | Ten hillslope outputs and five post files exist; initial fresh Python capture matches saved parquet values and metadata. |
| Small source hashes | All 42 selected files still matched their copied inputs/state/outputs after capture. |
| Watanabe upstream isolation | Previous Python totalwatsed3 and current native totalwatsed3 reproduce current upstream streamflow, with maximum differences 3.11e-15 mm and zero respectively. |
| Saved Watanabe daily output | Seven original-flow values and five corrected-flow values exceed tolerance, maximum 0.06032218 mm. Ash aggregation columns match. |
| Workflow ordering | `run_ash_rq` calls `Ash.run_ash`/AshPost before rebuilding totalwatsed3. The upstream file is 1.355 seconds newer than saved AshPost. |
| NFS worker-identity probe | UID 1000/GID 993, umask 0022: directory exchange fails EINVAL; directory flock, file flock, hardlinks, per-file rename, and rollback work. |
| Publication visibility probe | Sequential replacement exposes complete new/old files between renames; rollback restores the tested old set and 0640 mode. |
| Reader audit | Facades, reports, query planning, and documentation independently open paths; NoDb mappings and catalog have separate persistence boundaries. |
| OR-202 footer inspection | 1,090 files, 28,404,669 rows, about 6.45 GiB fixed numeric buffers before pandas concatenation/copies. Memory acceptance has not run. |

Ran evidence: `watanabe-hydrology-investigation.json`,
`small-fixture-provenance.json`, `saved-watanabe-output-drift.json`,
`atomic-worker-probe.json`, `nfs-publication-investigation.json`, and
`watanabe-oracle-repeatability.json`. Two fresh Watanabe captures agree exactly
in all five table values/schema/metadata; return-periods, facades, README, and
version manifest captures are byte-identical.
Static evidence: source anchors and independent reviews below.

Inference: the saved Watanabe AshPost consumed an earlier upstream generation.
The exact earlier bytes and their calculation history were not captured. Do
not claim that ash assimilation itself changed Streamflow: current native
`totalwatsed.rs` computes Streamflow independently of its ash metrics. The
upstream isolation compared only hydrology, with ash inputs omitted; it is not
full totalwatsed3 parity evidence.

## Publication alternatives

| Alternative | Compatibility and limits | Recommendation |
| --- | --- | --- |
| Staged per-file replacement in ordinary post directory | Preserves existing paths/readers/archive behavior. Complete old/new files can coexist briefly or after failed publication. | Use for this memory-remediation package. |
| Add backup/restore to per-file replacement | Restores prior files on tested recoverable errors; termination, failed rollback, or ambiguous NFS errors can leave recovery work. | Use existing owned transaction machinery after qualifying it on NFS. |
| Generation directory and symlink pointer | Needs initial migration, pin-once readers, retention, archive/fork changes, cleanup changes, and catalog/browse handling. Pointer replacement alone is not a multi-read snapshot. | Defer to a separate consistency package. |
| Directory rename to absent/empty post | Works in the probe but does not handle populated reruns. Clearing old outputs solely to enable it weakens preservation. | Optional initial-state optimization; not required. |

The existing `wepp_interchange/src/parquet.rs::commit_staged` already implements
staging, sequential rename, hardlink backup, and rollback. Primitive probes do
not establish that its complete implementation is suitable unchanged. In
particular, it tracks a path as published only after rename returns success.
On NFS an error can be returned after the server performed a rename; the
implementation must reconcile the affected path or report an indeterminate
outcome. See the [Linux rename(2) NFS caveat](https://man7.org/linux/man-pages/man2/rename.2.html#BUGS).

## Exact scope of the proposed changes

1. Replace the package's set-atomic publication promise and related stop gate
   with the explicit per-file/rollback contract in the proposed package.
2. Make immutable point-in-time inputs the parity basis, including H.wat,
   totalwatsed3, ordered metadata/translation, and the native release identity.
   Compare fresh Python and native results before any later upstream rebuild.
3. Add the hydrology paths and independently selected ash WEPP IDs to the native
   API contract. A present-ash-file manifest alone cannot preserve daily output.
4. Clarify that bounded output-key aggregate state is allowed. Prohibiting all
   watershed-day state contradicts the existing output-cardinality allowance;
   the prohibition is on retaining watershed source rows/tables.
5. Require a preimplementation state matrix and exact types, null/nonfinite
   policies, schema metadata, recurrence tie behavior, and telemetry shape.
   Define cooperating writer ownership and response to changed inputs/destination.
   Do not pretend the preliminary API draft is frozen.
6. Qualify publication, security, identity/group/mode, cancellation, and retry
   tests on actual disposable NFS storage. Keep the OR-202 full workflow and
   subsequent-job gates; record mounts and phase-level memory/process samples.
7. Correct the focused test command: the module-local tests directory contains
   only data fixtures. Use existing `tests/nodb/mods/test_ash*` and route tests,
   then add the required new coverage under the actual test suite.
8. Preserve native-first commit/push authorization and the prohibition on image
   publication/deployment. The revised publication guarantee still needs an
   accepted canonical checkpoint before code changes.

## Required state and failure matrix before implementation

| State or event | Proposed treatment / characterization obligation |
| --- | --- |
| No ash files | Preserve no-data behavior: no new post files, three mappings cleared, existing catalog policy preserved. |
| Metadata without an ash file | Skip that file as before; separately preserve hydrology's ash-type-based WEPP-ID selection. |
| Present zero-row parquet | Freeze the existing failure and choose a specific compatible native validation error; do not silently treat it as absent. |
| Optional hydrology absent or empty | Preserve existing four zero correction/solids columns when hydrology is empty. |
| Hydrology present with missing columns, missing H.wat, or unmatched dates | Characterize individually, preserving legacy explicit errors or null behavior; do not fill all cases with zero. |
| Null/NaN metrics, null keys, infinity, mixed schemas | Freeze generated edge oracles and explicit valid/invalid classification before native types are ratified. |
| Matching-major destination | Preserve ordinary directory/unrelated content; snapshot prior canonical files at native entry for rollback. |
| Absent/malformed/incompatible version | Preserve current version-removal policy; old-set guarantees begin after that policy, not before it. |
| Failed preparation or validation | No canonical file replaced. |
| Recoverable error after replacement begins | Attempt restoration and verify outcome; return explicit failure even if restoration succeeds. |
| Failed restoration, ambiguous NFS response, process termination | May retain complete mixed generations and recovery files; never claim normal success or unconditional rollback. Preserve recovery evidence; retry must regenerate the complete set. |
| Cleanup failure after all five replacements are confirmed | Report cleanup problem without falsely claiming publication was rolled back. |
| NoDb, README/version, catalog failure after parquet publication | Preserve error propagation; no claim of one transaction covering these separate resources. |
| Symlink/special-file/ancestor substitution | Validate containment before producer cleanup and Python invalidation, anchor operations to validated identities, and leave external sentinels untouched. |

## Investigation limits and remaining gates

No OR-202 modeling, native AshPost benchmark, large Python OOM reproduction,
NFS server-fault injection, ACL equivalence proof, or full native transaction
integration test ran. Text-file primitive probes are not final parquet workflow
acceptance. Same-client NFS locking success does not prove cross-host contention.
No performance acceptance or implementation readiness is claimed.

Watanabe historical drift is consistent with an earlier hydrology generation, but
its exact earlier upstream values remain unreconstructed. Reproducible fresh
Python/native comparisons on identical frozen inputs avoid altering tolerances
or scientific behavior to accommodate that history.

## Source anchors and independent review

- WEPPpy `wepppy/rq/project_rq.py:2464` and `:2474`: Ash/AshPost then upstream rebuild.
- WEPPpy `wepppy/nodb/mods/ash_transport/ashpost.py:500`: hydrology input read.
- WEPPpy `wepppy/nodb/mods/ash_transport/ashpost.py:994`: independent facade file reads.
- WEPPpy `wepppy/nodb/mods/ash_transport/ash.py:642`: cleanup through post paths.
- WEPPpy `wepppy/nodb/mods/ash_transport/ashpost_versioning.py:88`: ordinary-directory removal.
- WEPPpy `wepppy/query_engine/core.py:147`: individual dataset resolution.
- WEPPpy `wepppy/query_engine/activate.py:571` and native `wepp_interchange/src/catalog.rs:219`: catalog traversal.
- WEPPpy `wepppy/rq/project_rq_archive.py:296`: ordinary-tree archive traversal.
- Native `wepp_interchange/src/parquet.rs:170`: existing staged transaction helper.
- Native `wepp_interchange/src/totalwatsed.rs:978`: Streamflow precedes separate ash metrics.

Independent correctness and security reviewers both recommend the ordinary
layout with staged per-file replacement and explicit consistency limits.
Security additionally requires private staging, preserved access restrictions,
pre-invalidation containment, NFS outcome reconciliation, and clear post-commit
cleanup semantics. Performance review recommends retaining H.wat reads in the
parent, bounding work construction as well as futures, and measuring the whole
phase transition. These are investigation recommendations, not final approvals.

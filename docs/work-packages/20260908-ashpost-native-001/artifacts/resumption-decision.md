# Resumption decision required

Superseded by the operator-confirmed simple scope in `accepted-scope.md` and
`../package.md`. Transaction, rollback/recovery, staging-barrier, migration, and
NFS-specific requirements in this historical proposal are not active gates.


Historical proposal: superseded by `suggested-work-package-revisions.md` after
the operator requested further investigation. Generation symlinks are no longer
the recommendation for this memory-remediation package.

## Confirmed evidence

Ran: `canine-liar` now contains 10 completed Srivastava hillslope outputs and
all five post files. Fresh unchanged Python reproduces the saved parquets.
All 42 selected small-fixture input/state/output hashes still match source.
Source fixtures were not mutated.

Ran: Watanabe ash aggregation columns reproduce, but the saved daily table's
original streamflow differs from current `totalwatsed3` on seven rows (maximum
0.060322179824123134 mm); corrected streamflow differs on five rows. Fresh
Python copies the current input streamflow as expected. No native code exists.
Evidence: `saved-watanabe-output-drift.json`, `small-fixture-provenance.json`.
Saved and freshly recomputed outputs are preserved separately under
`target/ashpost-evidence/`. The initial capture's overall parity summary was
overbroad; per-file drift and this artifact are authoritative for its outcome.

Ran: Linux directory exchange succeeds on disposable ext4 but returns EINVAL
on the actual `/wc1` NFS mount. Worker-equivalent UID 1000/GID 993, umask 0022,
network-disabled container reproduces failure without changing either directory.
An existing symlink pointer can be replaced atomically there. Evidence:
`atomic-filesystem-probe.json`, `atomic-worker-probe.json`. All disposable probe
directories were cleaned; no production fixtures were touched.

## Proposed operator disposition

Allow a storage-layout amendment: immutable sibling generation directories with
`ash/post` as a relative symlink to the committed generation. Each generation
contains the complete five-file set, version manifest, and generated README.
Build and validate the generation first, then atomically replace the symlink.
Preserve filenames and resolved schemas, metadata, units, facade results, and
catalog paths. Require contained links to owned generations; reject arbitrary
links. Update version cleanup, producer cleanup, catalog/file readers, and tests
for the new layout before shipping.

Converting an existing nonempty ordinary `post` directory cannot be atomic on
this NFS filesystem. Permit only an explicit quiescent migration with readers
and writers excluded: move the old directory to a generation, install the
pointer, and roll back before releasing the exclusion if installation fails.
No automatic live migration, production mutation, or deployment is proposed.
The exact exclusion/reader protocol must be ratified and independently reviewed
in the ancestor checkpoint before implementation. If that scope is undesirable,
revise the atomic-publication requirement instead; do not silently weaken it.

For Watanabe, use fresh unchanged-Python outputs generated from the verified
snapshot as the native oracle, retaining the inconsistent saved output as
historical evidence. This changes no formulas or source files. Alternatively,
the operator can restore/rerun the Watanabe fixture and require stored-output
parity again.

## Review disposition and remaining work

Independent correctness review: two medium findings remain open: enumerate
state outcomes and finish exact native types/schema/nonfinite handling before
freezing the API. Hydrology inputs and ordered metadata scope were accepted.
Independent security review: high blocker on NFS atomic publication; generation
layout and first migration require explicit compatibility authorization. Two
medium security gaps also remain: private-stage access and post-commit cleanup
semantics; containment before legacy invalidation and race-safe file identities.
Independent performance review: retain the H.wat loader in the parent and bound
futures; OR-202 has 28,404,669 input rows (~6.45 GiB fixed numeric buffers).
No memory/performance success is claimed.

No implementation, ancestor commit, push, image publication, or deployment has
occurred. WEPPpy canonical proposal was moved into `output-contract-proposal.md`
until ratification. Native and integration plans remain active and blocked.

# Investigation review disposition

Superseded by the operator-confirmed simple scope in `accepted-scope.md` and
`../package.md`. Transaction, rollback/recovery, staging-barrier, migration, and
NFS-specific requirements in this historical proposal are not active gates.


Scope: operator requested continued investigation and suggested work-package
revisions. These reviews assess the recommendations, not implementation or
production acceptance. No canonical checkpoint or production code was approved.

## Correctness review

Independent reviewer: ash_contract_review.
Result: no major scope/correctness contradiction in the final recommendation.
Corrections incorporated: upstream comparison described as Streamflow-only;
historical causation labeled inference; focused test paths corrected to real
test modules; canonical output-scope contract explicitly included in write set.

## Security review

Independent reviewer: ash_security_review.
Result: recommendation suitable for user review, no additional medium/high
blocker in the proposal. Corrections incorporated: containment starts before
producer cleanup as well as version invalidation; caught failures propagate,
while abrupt termination uses existing RQ failure handling; the checkpoint
must define cooperating writer ownership and changed-state handling.

Prior implementation obligations remain open: exact native/state policies,
staging access and ownership, race-safe containment, qualified native rollback,
NFS indeterminate outcomes, cancellation, and actual workflow acceptance.
Primitive probes are explicitly not final transaction/concurrency proof.

## Validation of investigation artifacts

Ran: markdown-doc lint of the package directory with --no-ignore validated all
Markdown files without errors. Investigation scripts parsed with Python AST;
JSON artifacts parsed; git diff --check passed. The wctl wrapper returned zero
files for external native-repository paths, so standalone markdown-doc from the
native repository supplied the actual nonzero-file validation.

No production tests were rerun: this turn changed only investigation scripts,
evidence, and proposed/living package documentation. No native AshPost code,
canonical contract change, commit, push, registry publication, or deployment.

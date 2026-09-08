# Execute bounded ash modeling and native AshPost package

Execute `docs/work-packages/20260908-ashpost-native-001/package.md` end to end on
`forest` across `/workdir/wepppyo3` and `/workdir/wepppy`.

Read both repositories' complete root and applicable nested `AGENTS.md` files,
the complete package, and `prompts/active/execplan.md` before editing. Maintain
the ExecPlan, tracker, evidence index, and review dispositions throughout.

Treat this as faithful extraction. Freeze the existing Python outputs before
removing the producer. Do not change model equations, schemas, units,
return-period semantics, public facades, version behavior, or catalog behavior.
Required native AshPost must have no executable whole-watershed Python fallback.

Use these read-only source fixtures:

- `/wc1/runs/ca/canine-liar/` for Srivastava single-OFE parity;
- `/wc1/runs/as/assisted-weakness/` for Watanabe dynamic single-OFE parity; and
- host `hpc` path
  `/tank/kubernetes/weppcloud/weppcloud-wc1/pvc-d4c5528b-3204-438b-909a-61af286d6fea/batch/nasa-roses-202608-psbs/runs/OR-202/`
  for production-scale memory, performance, and end-to-end acceptance. Forest
  has verified noninteractive SSH access to `hpc`; copy required inputs to a
  disposable Forest directory before testing.

Never write into those paths. Capture oracles and candidates in disposable,
package-owned locations. The OR-202 acceptance must exercise bounded hillslope
scheduling followed immediately by native AshPost through the real Forest
Compose RQ worker at `WEPPPY_NCPU=12` and a 12 GiB cgroup limit. It must remain
below 9 GiB, produce all outputs, finish RQ successfully, and accept a subsequent
job without restart. A standalone native success is insufficient.

Follow the operator-confirmed simple scope in package.md: individual complete
files using existing writer behavior, as in hillslope interchange -> totalwatsed3.
Do not revive superseded multi-file transactions, rollback/recovery systems,
generation layouts, migrations, or NFS-specific publication requirements.

Complete independent correctness, QA, performance, and high-impact security
reviews. Resolve every medium/high finding, run focused and full validation,
commit/push wepppyo3 before WEPPpy, and verify clean remote checkouts plus LFS
objects. Do not publish an image or deploy to openwepp.org; those require later
explicit authorization.

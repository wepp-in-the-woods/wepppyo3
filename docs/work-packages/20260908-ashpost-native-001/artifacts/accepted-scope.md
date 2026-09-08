# Operator-confirmed simple scope

The operator confirmed on 2026-09-08 that individual-file production followed
by aggregation is the established hillslope interchange -> totalwatsed3 pattern
and matches the original intent. The assistant-added publication redesign was
unnecessary and is removed from the active package and ExecPlan.

Implement bounded hillslope scheduling and streaming native AshPost; preserve
results, filenames, schemas, per-hillslope model equations, ordinary failure
propagation, and regeneration on retry. Reuse existing writer behavior. No new
multi-file transaction, all-five staging barrier, rollback/recovery system,
generation-directory storage, migration, or NFS-specific mechanism is required.
Do not use the superseded proposal reviews as implementation blockers.

Keep both model parity fixtures, numerical tolerances, memory/performance gates,
real Compose acceptance, and independent implementation reviews. Compare fresh
Python/native outputs using identical frozen inputs; the later RQ totalwatsed3
rebuild explains why historical upstream generations must be kept separate.
No scientific or workflow-order correction is part of this scope.

Durable specification: WEPPpy `docs/schemas/output-scope-contract.md`, section
"AshPost file-production scope". Execution sources: `../package.md`,
`../tracker.md`, and `../prompts/completed/execplan.md`. Native API details and edge
oracles remain implementation preparation, not a storage redesign checkpoint.

At this scope-confirmation checkpoint, implementation had not begun. Follow the
completed plan for implementation and acceptance results. Commit/push sequencing and
image/deployment exclusions remain as specified by the package.

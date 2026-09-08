# Unratified output contract proposal

Superseded by the operator-confirmed simple scope in `accepted-scope.md` and
`../package.md`. Transaction, rollback/recovery, staging-barrier, migration, and
NFS-specific requirements in this historical proposal are not active gates.


Blocked by NFS publication and oracle disposition; not canonical authority.

## Ash transport post-processing

Implementation conformance pending (20260908-ashpost-native-001).
AshPost retains its five version-1.0 parquet filenames, schemas, metadata,
units, row order, public facade dictionaries, version invalidation, generated
documentation, and catalog activation. Both Srivastava and Watanabe dynamic
models keep their existing equations and per-hillslope files.

Aggregation must use the required native implementation without a Python
whole-watershed fallback. Hillslope work is constructed lazily, with at most
2 * max_workers submitted futures, and all executor/input references are
released before post-processing. This bounds memory at the demonstrated OOM
boundary without changing concurrency or scientific results.

Native inputs are an ordered contained-file manifest plus optional WEPP daily
hydrology, H.wat, and independently selected ash WEPP IDs. Missing hillslope
files retain skip semantics; an empty manifest produces no new post files and
clears the three NoDb return-period mappings. Invalid paths, schemas, metadata,
or malformed parquet fail explicitly. A failed native publication preserves
the previous complete output set. All five parquet files are staged together
and published with one atomic directory operation, preserving permissions and
unrelated regular files. Existing major-version invalidation remains Python
policy before aggregation.

Compatibility includes float32-rounded hectares and grouped area sums,
first-year filtering at days_from_fire <= 365, existing calendar-date collapse,
cumulative selection by year0 then julian, sequential return-period tie ordering,
and current daily hydrology corrections. Annual area remains the sum of
hillslope-day areas. These historically observable choices are preserved for
faithful extraction; scientific corrections require a separate decision.

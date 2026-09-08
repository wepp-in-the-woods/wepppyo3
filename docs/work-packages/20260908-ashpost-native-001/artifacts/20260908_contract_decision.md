# AshPost contract decision

Superseded by the operator-confirmed simple scope in `accepted-scope.md` and
`../package.md`. Transaction, rollback/recovery, staging-barrier, migration, and
NFS-specific requirements in this historical proposal are not active gates.


Operator authorization: user requested execution of this package and reran the
missing Srivastava fixture to resume it. Package includes native-first commits
and pushes; excludes registry publication and deployment.

Starting revisions: native 8fc2afa91775825a8f6901d63bc7a0d957bdf146;
WEPPpy abebd09239f398af7627924c4c000ff3229a01ee.

Canonical authority: WEPPpy `docs/schemas/output-scope-contract.md`, section
"Ash transport post-processing"; unchanged shared
`docs/schemas/nodb-persistence-concurrency-contract.md`; nearest ash AGENTS
scientific, version, documentation, and facade invariants.

Delta: require bounded scheduling and native aggregation with atomic five-file
publication; preserve all valid scientific/output behavior. Classification:
incident remediation with an explicit resource-lifetime and publication
contract. No schema migration, new dependencies, parameterization change, queue
change, or model change is authorized. The detailed proposed API is recorded in
`native-contract.md`; hydrology paths and independent ash IDs are necessary to
preserve already existing daily columns.

Compatibility/regression plan: freeze both model oracles and generated edge
cases; exact schema/metadata/order/dictionaries and numerical tolerance 1e-10 /
1e-12; check absent/empty/populated/versioned/malformed states; prove real
filesystem failure atomicity and permission behavior without mocks; demonstrate
unchanged hillslope outputs and all downstream facades/docs/catalog/NoDb/RQ in
OR-202 at 12 workers under the specified 9 GiB peak gate. Preserve no-data skip
behavior. Invalid or hostile native inputs fail explicitly without publication.

Security impact high: new parquet parser and filesystem transaction boundary,
worker cancellation/lifetime. Required independent contract reviews pending;
implementation may start only after their disposition and ancestor commit.

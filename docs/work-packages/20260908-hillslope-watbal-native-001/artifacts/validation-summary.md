# Validation summary

Ran: all package minimum gates pass on Forest. Raw logs are retained here.
Static: independent reviews have no unresolved medium/high findings.

| Gate | Result |
| --- | --- |
| Full WEPPpy suite | 7732 passed, 72 skipped; full-wepppy-suite.log |
| Installed native interchange suite | 107 passed; native-release-tests.log |
| Rust interchange crate | 95 unit + 17 integration passed; native-rust-tests.log |
| NoDb suite | 2090 passed, 26 skipped; nodb-tests.log |
| Interchange suite | 73 passed, 1 skipped; interchange-tests.log |
| Combined report/receipt/RQ | 54 passed; combined-tests.log |
| Startup/receipt/RQ | 53 passed; reviewed-integration-tests.log |
| Batch UI/API consumers | 35 passed; batch-consumers.log |
| RQ graph | Pass; only source-line references regenerated; final-rq-graph.log |
| Stub completeness | Pass; final-stubs.log |
| Report stubtest | Pass; report-stubtest.log |
| Broad exception enforcement | Pass, net delta -2; final-broad-exceptions.log |
| Documentation lint | 15 files pass; doc-validation.json |
| Cargo formatting / git whitespace | Pass in both repositories |
| Parity/performance/memory/Compose | Pass; forest-acceptance.md |

The full suite collected before four final report edge tests were added and
before the report test's global optional-dependency stubs were removed. Those
changes are covered by the subsequent 54-test combined run; production code was
unchanged after the full suite began. No failed assertion was skipped.

## Additional diagnostic limitation

The optional source-only `BatchRunner` stubtest stops before API comparison on
existing base-stub/source typing errors. The unchanged d8fbe9f4a detached
checkout reproduces them: `locked()` is declared as a Generator instead of a
context manager, and controller attributes/types are incomplete. See
baseline-batch-stubtest.log and additional-batch-stubtest.log. Broad NoDb stub
repair is outside this bounded extraction. The package's required
`check-test-stubs` and the report's actual stubtest pass.

## Tooling and failed attempts

The first full suite stopped at a hard-coded old startup artifact hash. Updating
that expected provenance pin allowed the complete rerun to pass. The first
combined baseline exposed global pyproj stubbing in the report test; removing
those optional-dependency stubs allows the final combined run to pass.

Absolute /workdir aliases caused markdown-doc's root-prefix panic. Repository-
relative paths pass; doc-validation-alias-panic.json preserves the failure.
Two spelling-preview suggestions affect untouched prior prose and were retained.
The smallest tooling follow-up is canonicalizing root/path aliases before lint.

Clean-checkout verification initially exposed harness omissions: same-named
pytest modules from two repositories collide in one collection; batch tests
require writable `/wc1` and Redis locks. Separate repository test invocations,
a disposable `/wc1` tmpfs, and an isolated Redis container provide those
requirements without production data or service access. Failed attempts are
preserved in clean-*-failed.log.

Final clean remote checkouts pass startup hash validation, 62 WEPPpy tests,
and 18 native tests. Both worktrees remain clean. All 127 fixture hashes and
Git LFS integrity pass. Source revisions and the empty image-publishing run
list are recorded in clean-remote-verification.json.

# Independent security review

## Findings

| ID | Severity | Surface | Finding and required remediation | Evidence | Status |
| --- | --- | --- | --- | --- | --- |
| SEC-01 | High | Post-producer documentation | Reading entire inputs for a three-row preview exhausted the 12 GiB worker. Read schema and a bounded preview batch. | First-party [security review](security-review.md), `_summarize_file` in WEPPpy `interchange_documentation.py:117`, preview regression tests, [Compose result](compose-rq-result.json). | Resolved; repair independently inspected. |
| SEC-02 | Medium | Native Parquet error boundary | A negative column compressed size reached Parquet's `byte_range` assertion and escaped as PyO3 `PanicException`, outside Python `Exception` and the required execution-error wrapper. Validate signed metadata and physical chunk ranges before compact reconstruction/indexing. | `totalwatsed.rs:297` and `Source::open:358`; new malformed-footer regression cases in `tests/wepp_interchange/test_totalwatsed3.py:233`; [before/after evidence](independent-security-evidence.json). | Resolved on final release. |
| SEC-03 | Medium | Valid-state noninterference | The initial SEC-02 guard rejected valid empty dictionary chunks whose absent data-page offset is zero. Admit that sentinel only for zero-value chunks while preserving nonnegative metadata and actual file bounds. | `totalwatsed.rs:323`; `empty_inputs`, `empty_pass`, and zstd/dictionary physical-layout regressions; COR-03 in [correctness review](independent-correctness-review.md). | Resolved on final release. |

SEC-02 exploit prerequisite: an adversary or corrupting process can replace a
consolidated input with malformed Parquet before this producer runs. The review
did not identify a new public endpoint accepting arbitrary native paths. The
confirmed effect is escape from WEPPpy's native exception translation; inputs
and prior output remained intact. This review did not demonstrate worker death,
code execution, or arbitrary file overwrite. RQ has its own outer task boundary.

No unresolved medium/high finding or new risk acceptance is requested.

## Metadata and triage

- Reviewer: delegated `security_reviewer`, independent of the production-code
  implementer. The reviewer authored the bounded malformed-footer tests only.
- Date: 2026-09-07.
- Repositories: wepppyo3 `main` and WEPPpy `master`, reviewed working trees before
  commit/publication. Source identities are in [evidence](independent-security-evidence.json).
- Final shared-object SHA-256:
  `bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354`.
- Security impact: high; dedicated review required. Native file parsing,
  atomic shared-run publication, and the paired startup artifact pin change.
- Related reviews: [independent correctness review](independent-correctness-review.md)
  and [independent QA review](independent-qa-review.md), both inspected after
  their final-candidate findings were closed. Correctness, QA, resource/workflow
  acceptance, and publication remain separate
  gates; security sign-off does not substitute for their evidence.

Threat assumptions: WEPPpy owns run authorization, directories, controller
metadata, and input generation. Native code is a file-transform API executing
with the worker's existing privileges, not a filesystem sandbox. Published
images and their source revisions are trusted by the operator. Source files
and parent directories remain stable while a transformation runs.

## Boundary checks

**Valid states.** The final release passes absent optional sources, empty PASS
and empty all-input states, populated single-OFE/MOFE, ash/no-ash, supported
legacy aliases, subsets, and physical-layout variations. The empty dictionary
sentinel repair is narrowly conditioned on `num_values == 0 && data_offset == 0`;
it does not exempt negative values or out-of-file chunk ends.

**Input metadata and allocation.** `Source::open` validates metadata before
projection, index scans, or compact reconstruction. File/group row counts,
group byte sizes, column value counts/sizes, and data/dictionary offsets must be
nonnegative. Checked chunk bounds must fit the opened file; nonempty data pages
must lie within their chunk. `scan` reconstructs at most sixteen row groups and
uses 8192-row numeric batches. Workers are capped at twelve and retain per-date
aggregates plus one hillslope's joins. These are measured-workload bounds, not a
claim that arbitrary hostile Parquet cannot exhaust resources.

**Paths and publication.** `produce` checks direct and canonical input/output
path aliases, including optional soil, element, and ash inputs. Sources are
opened read-only. The existing `ParquetSink` creates same-directory staging
files with `create_new` (`parquet.rs:397`), closes the writer before publication,
locks output directories, rejects nonregular existing targets using
`symlink_metadata` (`parquet.rs:201`), and publishes by rename with backup-based
rollback. Drop cleanup covers ordinary failures. Direct probes confirmed that
unrelated and dangling output symlinks are rejected without changing their
targets; replacing a hard-linked output changes its directory entry while
preserving the input inode and bytes.

**Python/native and startup.** PyO3 owns argument values across GIL release and
maps `InterchangeError` to ordinary Python errors (`lib.rs:55`, `to_py_err`).
The required-native WEPPpy boundary raises explicit unavailable/execution errors
with causes; no Python/DuckDB fallback remains. Startup verifies required API
symbols, resolved module origins, and the exact paired shared-object hash in
`docker/wepppyo3-interchange-preflight.py:20`. The logging repair uses the native
summary's actual output path for this three-path signature.

**Other surfaces.** This producer adds no route/auth/session/CSRF behavior,
NoDb writes, queue edges, subprocesses, network calls, or external dependency.
The controller ash lookups retain existing behavior. Secrets are not arguments
to the new native API. Reviewed fixture/provenance and Compose evidence does
not introduce a credential dependency. The existing GHCR workflow uses trusted
master pushes/manual dispatch, pinned actions, and the dedicated builder;
publication must retain these controls while updating the literal native
revision pin. Verifying the published image must read its own packaged code
and extension without host source mounts masking the image contents.

## Independent validation

Final installed release, without a development-binary override:

```sh
cd /workdir/wepppyo3
PYTHONPATH=/workdir/wepppyo3/release/linux/py312 /workdir/wepppy/.venv/bin/python -m pytest tests/wepp_interchange/test_totalwatsed3.py -q
```

Result: **42 passed**, one preexisting timezone deprecation warning, 8.05 seconds;
see [test log](independent-security-final-tests.log). This includes eleven new
malformed-footer cases that require an ordinary `RuntimeError`, unchanged input
bytes and prior output, and zero staging leaks. The negative-compressed-size
case was independently demonstrated to fail before the repair with
`PanicException`, so the new tests exercise the actual defect.

Twelve additional direct native probes passed on the same release: truncated
footer, invalid footer magic, missing required metric, string metric, null and
fractional date keys, exact/relative/symlink input aliases, unrelated/dangling
output symlinks, and hard-linked output publication. Each preserved all input
bytes and left no staging files. Results: [probe receipt](independent-security-final-probes.json).

The exact public WEPPpy facade was then exercised without mocks in the canonical
Forest Compose runtime (PyArrow 23.0.1), using a copied input with negative column
compressed size. It raised `WeppInterchangeExecutionError` with an ordinary
`RuntimeError` cause and preserved input bytes and prior output without staging
leaks. This directly closes SEC-02 at the stated Python/native boundary;
see [facade receipt](independent-security-facade-probe.json) and the preserved
[probe source](independent-security-facade-probe.py).

```sh
cd /workdir/wepppy
wctl exec weppcloud env PYTHONPATH=/workdir/wepppy:/workdir/wepppyo3/release/linux/py312 /opt/venv/bin/python /workdir/wepppyo3/target/totalwatsed3-evidence/independent_security_facade_probe.py
```

Documentation validation: native relative links, `uk2us` spelling preview,
`git diff --check`, and `markdown-doc lint` from the native repository passed.

The reviewer also inspected the earlier real Compose RQ evidence under UID
1000, GID 993, umask 0022 and a 12 GiB limit, including successful downstream
processing and a subsequent job. That evidence is first-party workflow
validation, not an independently rerun workflow or final-image attestation.

## Residual limits and verdict

The full Parquet footer is parsed before compaction. Decoder page/dictionary
allocations, maliciously huge schemas/footers, arbitrary date cardinality,
inconsistent statistics, and every malformed page encoding were not exhaustively
fuzzed or resource-capped. These inputs remain inside the existing trusted
run-artifact processing boundary; this release does not claim an upload/parser
sandbox. Concurrent hostile replacement of parent directories or inputs is
also outside the stable-run-tree assumption. The existing rename transaction
is failure-atomic, not a power-loss durability guarantee.

**Security gate: pass for the reviewed final release.** Unresolved findings:
high 0, medium 0, low 0. The three recorded findings are closed with source and
direct rebuilt-release evidence. No production rollout is authorized by this
review; Forest evidence does not establish Kubernetes identity/mount parity.
Repository/LFS publication and the immutable image receipt still require their
own successful validation before the parent package closes.

Security reviewer sign-off: delegated `security_reviewer`, 2026-09-07.
Package owner and publication disposition: parent orchestrator, recorded in the
active package tracker and final disposition.

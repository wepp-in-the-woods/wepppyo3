# Independent review and disposition

Static: independent source/contract reviews; Ran: reviewer probes and referenced validation logs distinguish executed evidence.

Security impact: high. Reviews were independent, read-only agents in this
execution; authors did not approve their own amendments. Dates: 2026-09-08.

## Correctness review

Reviewer: `/root/contract_review` (reviewer role).

Preimplementation review confirmed populated/empty Arrow schemas, canonical
integer keys, NaN/null OFE area exclusion, and existing fresh-lock transaction
precedent. Implementation review independently reproduced nested lock rejection
poisoning the owner's signature. Fixed by requiring successful acquisition
before failure invalidation; the outer-writer regression passes. Reviewer
confirmed closure in source.

Late evaporation overflow and an OS file-size limit now exercise failure after
staging starts. Both preserve prior cache bytes and remove staging files.
Reviewer independently probed these paths and confirmed their behavior; committed
regressions additionally fix the old inaccurate staging glob.

Verdict: no unresolved medium/high correctness findings.

## QA review

Reviewer: `/root/qa_review` (qa_reviewer role).

The medium gap was missing producer-specific late-failure cleanup coverage.
The evaporation-overflow and RLIMIT_FSIZE/OSError regressions close it. The
null-OFE test also checks precipitation retention, avoiding an area-only false
positive. Reviewer confirmed closure by reading the final tests.

Verdict: no unresolved medium/high QA findings.

## Dedicated security review

Reviewer: `/root/transaction_contract_review` (security_reviewer role).

| ID | Severity | Finding and disposition |
| --- | --- | --- |
| SEC-01 | Medium | Receipt exceptions could skip root finalizer linkage/deferred release. Both now precede receipt publication; injected-error regressions pass. |
| SEC-02 | Medium | Blanket RuntimeError catches could hide programming/recursion failures. Catches now name operational exceptions; ValueError, RuntimeError, and RecursionError propagation regressions pass. |
| SEC-03 | Medium | Cache replacement could widen restrictive mode bits. The opt-in sink mode is applied at staging creation, restored before data writes, and retained on publication. 0600/0640/0660 regressions pass. |

Reviewer inspected final source and the 107-test installed-release and 53-test
NoDb/RQ/startup logs, and confirmed the final shared-object/preflight SHA-256:
`fe5b2c156b361181fe52004399a6ce131b3b43f92797ae744350d6e9f5713917`.

Confirmed controls include checked numeric keys, explicit schema/value errors,
bounded aggregation state, canonical alias and nonregular destination rejection,
atomic publication, fresh durable hydration, preserved external lock tokens,
and no new unsafe Rust, dependency, subprocess, or network surface.

Residual limitations: orchestration paths and service-owned UID/GID caches are
trusted; metadata and distinct-key cardinality still contribute to memory;
projected batches cannot independently cap malicious decoder allocations;
cleanup under process termination/unlink failure is best-effort; existing
compound Redis unlock failures can replace the primary top-level exception.

Verdict: security gate pass, no unresolved medium/high findings. Final Compose
and remote/provenance verification remain separate package acceptance gates.

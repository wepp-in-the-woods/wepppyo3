# Evidence index
Current authority: [accepted scope](accepted-scope.md), [native contract](native-contract.md),
and [package](../package.md). The operator confirmed the established
individual-file writer and aggregate-reader pattern. Earlier transaction,
recovery and NFS publication proposals are superseded history.

## Execution evidence

- Ran/Static: [execution results](execution-results.md) summarize delivered behavior,
  parity, memory, validation and release boundaries. `release-integration-manifest.json`
  binds the tested source and shared object; `build-Cargo.lock` preserves the
  generated dependency lock. `clean-*-tests.log` and `clean-*-lfs.log` record
  remote-checkout validation; `image-workflow-no-publication.json` confirms
  no new image workflow ran. `full-wepppy-tests.log` records
  7,742 passing tests; focused, Rust, native and hygiene logs sit beside it.

- Ran: `small-fixture-provenance.json` and `frozen-source-manifest.json` identify
  read-only source snapshots; `raw-evidence-manifest.json` indexes large local
  measurement files by path, size and SHA-256.
- Ran: `capture_oracle.py`, `verify_outputs.py`, `wrapper-parity.json` and
  `scheduler-output-parity.json` cover both models, all five tables, schemas,
  mappings, facades, documentation, versions and exact scheduler output bytes.
- Ran: `small-benchmark-summary.json` uses the corrected `dcc822` five-per-mode
  measurements. `final-benchmark-measurements.json` preserves those measurements,
  the streamed OR-202 Python baseline and consecutive native calls.
- Ran: `benchmark.py` and `run_benchmarks.py` record isolated container commands,
  state and logs as `benchmark-*.command.json`, `*.state.json` and `*.log`.
  Earlier `506ceb`, `670a6c`, `80d36e`, `baseline-1` and `repeat-1` attempts are
  preliminary; final performance claims use the reviewed corrected sampler.
- Ran: `compose-command.json`, `compose-state.json`, `compose-result.json`,
  `compose-phases.jsonl`, `compose-jobs.json`, `compose-selection.json` and
  `compose-rq.log` preserve the actual OR-202 then canine-liar RQ sequence.
  `compose-acceptance.yml`, `prepare_rq.py`, `ashpost_acceptance.py` and
  `run_compose_acceptance.py` reproduce it in disposable directories.
- Ran/Static: `correctness-review.md`, `qa-review.md`, `performance-review.md`
  and `security-review.md` record independent review and finding dispositions.
- Ran: `argsort-oracles.json` and `verify_argsort_oracles.py` freeze recurrence
  sorting semantics against NumPy/pandas.

## Historical investigation (superseded)

- Ran: `fixture-preflight-blocker.json` records the initial missing-input
  blocker, subsequently resolved by the operator's rerun.
- Ran: `saved-watanabe-output-drift.json`,
  `watanabe-hydrology-investigation.json` and
  `watanabe-oracle-repeatability.json` explain historical upstream drift and
  establish repeatable fresh Python oracles on identical inputs.
- Static: `output-contract-proposal.md`, `20260908_contract_decision.md`,
  `resumption-decision.md`, `suggested-work-package-revisions.md`,
  `package.revised.md`, `package-revisions.diff` and
  `investigation-review-disposition.md` preserve rejected proposal history.
  Their transaction/recovery gates do not apply to this implementation.
- Ran: `atomic-filesystem-probe.json`, `atomic-worker-probe.json`,
  `nfs-publication-investigation.json` and `probe_nfs_publication.py` are
  historical probes only, not requirements or implementation prerequisites.

Large source and generated Parquet payloads remain outside Git under
`target/ashpost-evidence/`. All results are fixture-specific observations.

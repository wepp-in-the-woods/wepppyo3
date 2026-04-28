# Claim Discipline

This document keeps `wepppyo3` messaging evidence-backed. Use it when writing
README text, work packages, benchmark summaries, release notes, or procurement
language that mentions `wepppyo3`.

## Claim Labels

Use these labels explicitly in evidence notes:

- `confirmed`: directly observed in source, release files, tests, local command output, or checked-in artifacts.
- `inference`: a conclusion drawn from confirmed evidence. State the assumption.
- `hypothesis`: plausible but not yet measured or proven.

Do not label a performance statement `confirmed` without workload, command,
fixture or run id, repetitions where applicable, and result.

## Approved Clean Claim

`wepppyo3` is WEPPpy's native kernel and interchange substrate: Python-callable
Rust modules for contract-sensitive hydrology, climate, raster, WEPP/SWAT
interchange, roads, MOFE, SBS, and visualization workloads where Python
orchestration should remain but the hot path belongs in owned Rust.

## Avoid These Claims

- Avoid: "`wepppyo3` makes WEPPpy faster."
  Use: "`confirmed`: module X was faster on workload Y under command Z."

- Avoid: "`wepppyo3` replaces Python."
  Use: "`wepppyo3` replaces selected Python hot paths while WEPPpy keeps orchestration."

- Avoid: "All `wepppyo3` modules are production-critical."
  Use: the maturity labels in [module-registry.md](module-registry.md).

- Avoid: "The release tree is fully traceable."
  Use: "`confirmed`: the release tree has a package version; `inference`: stronger provenance needs a manifest."

## Figure Specification

Create a three-band architecture figure when visual communication is needed:

1. Top or left band: WEPPpy Python orchestration with routes, NoDb controllers,
   RQ workers, run directories, reports, and query-engine integration.
2. Center band: `wepppyo3` native substrate with grouped Rust modules: climate,
   raster characteristics, WEPP/SWAT interchange, roads/MOFE, SBS, and visualization.
3. Bottom or right band: native peers and artifacts: Peridot watershed graph
   abstraction, `weppcloud-wbt` delineation tools, WEPP/SWAT model binaries,
   GDAL/PROJ rasters, and Parquet outputs.

Arrow rule: show WEPPpy calling stable native kernels. Do not draw Rust as
replacing the WEPPpy application.

## Metrics Definitions

### Adoption Surface

Count of WEPPpy modules and tests that import each `wepppyo3` module. Use this
to prioritize documentation and support work. Do not treat import count as proof
of correctness or performance.

Evidence label guidance:

- `confirmed`: exact scan command and count are recorded.
- `inference`: callsite count suggests production importance.
- `hypothesis`: broader adoption will improve maintainability.

### Runtime Leverage

Bounded performance gain on a named workload. Required fields are command,
fixture or run id, hardware or host class, repetitions, wall time, memory when
available, and output parity or compatibility note.

Evidence label guidance:

- `confirmed`: benchmark artifact includes the required fields.
- `inference`: similar workload likely benefits for the same reason.
- `hypothesis`: unmeasured modules may benefit from future benchmarking.

### Contract Centrality

Importance of the schema, parser behavior, raster semantics, or failure behavior
owned by a module. A module with high contract centrality affects downstream
outputs even if it is not the largest runtime consumer.

Evidence label guidance:

- `confirmed`: downstream outputs and callsites are named.
- `inference`: failure would block or corrupt a workflow based on those callsites.
- `hypothesis`: centralizing the contract will reduce future defects.

## Evidence Notes Template

Use this format in artifacts:

```text
confirmed: <direct observation>. Evidence: <file, command, test, or artifact>.
inference: <conclusion>. Assumption: <why the evidence supports it>.
hypothesis: <future claim>. Needed evidence: <benchmark, test, operator decision>.
```

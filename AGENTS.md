# AGENTS.md
> AI Coding Agent Guide for wepppyo3

## Authorship
**This document and all AGENTS.md documents are maintained by GitHub Copilot / Codex which retain full authorship rights for all AGENTS.md content revisions. Agents can author AGENTS.md document when and where they see fit.**

## Purpose

This is the root agent guide for `/workdir/wepppyo3`. Keep it concise. Put deep
module details in `README.md`, `docs/`, crate-local docs, or tests.

## Orientation

`wepppyo3` is WEPPpy's native kernel and interchange substrate: Python-callable
Rust modules for contract-sensitive hydrology, climate, raster, WEPP/SWAT
interchange, roads, MOFE, SBS, and visualization workloads where WEPPpy keeps
Python orchestration but selected hot paths belong in owned Rust.

Canonical posture docs:

- `README.md` - top-level positioning, module summary, build/install notes.
- `docs/module-registry.md` - module maturity, release artifact, callsites, tests, and evidence labels.
- `docs/architecture-and-boundaries.md` - ownership boundaries with WEPPpy, Peridot, and `weppcloud-wbt`.
- `docs/release-provenance.md` - canonical py312 release package and provenance gaps.
- `docs/claim-discipline.md` - required evidence labels and communication rules.

## Repository Shape

Workspace members are listed in root `Cargo.toml`.

Primary PyO3 modules:

- `cli_revision` -> `wepppyo3.climate`
- `raster_characteristics` -> `wepppyo3.raster_characteristics`
- `roads_flowpath` -> `wepppyo3.roads_flowpath`
- `sbs_map` -> `wepppyo3.sbs_map`
- `swat_interchange` -> `wepppyo3.swat_interchange`
- `swat_utils` -> `wepppyo3.swat_utils`
- `watershed_abstraction` -> `wepppyo3.watershed_abstraction`
- `wepp_interchange` -> `wepppyo3.wepp_interchange`
- `wepp_viz` -> `wepppyo3.wepp_viz`

Internal support crates:

- `geneva_core`
- `raster`

Canonical deployable package:

- `release/linux/py312/wepppyo3/`

Do not edit copied `.so` release artifacts unless the task explicitly includes a
build/release refresh. If release artifacts are touched, update provenance notes
and run import plus targeted tests for the affected module.

## Boundary Rules

- WEPPpy owns routes, NoDb state, RQ orchestration, run directories, reports, and user-facing workflows.
- `wepppyo3` owns narrow Python-callable Rust kernels and file interchanges embedded in WEPPpy workflows.
- Peridot owns standalone watershed graph abstraction and CLI behavior.
- `weppcloud-wbt` owns WBT/TOPAZ-style delineation and hydrology command-line tools.
- WEPP and SWAT binaries remain the model engines; `wepppyo3` parses and transforms selected inputs/outputs.

Before adding a new module, ask whether the primary caller is WEPPpy Python and
whether the work is a bounded kernel, parser, raster scan, or file transform. If
not, this repo may be the wrong home.

## Change Scope Discipline

- Keep Python public import paths stable unless the user explicitly asks for a breaking change.
- Preserve existing WEPPpy file/schema semantics when moving work into Rust.
- Do not add silent fallback behavior. If a Python wrapper falls back, it must be explicit and logged on the WEPPpy side.
- Treat broad performance claims as unproven unless backed by a checked artifact with fixture, command, repetitions, and result.
- Use `confirmed`, `inference`, and `hypothesis` labels for evidence notes and claims.
- Do not introduce new external dependencies without a clear need and a short precedent/capability check.

## Documentation Maintenance

When docs change:

- Keep `README.md` as the orientation and quick-start surface.
- Update `docs/module-registry.md` when adding, removing, renaming, or materially changing a PyO3 module or internal support crate.
- Update `docs/architecture-and-boundaries.md` when ownership boundaries or routing rules change.
- Update `docs/release-provenance.md` when release layout, build/copy workflow, package versioning, or shared-object provenance changes.
- Update `docs/claim-discipline.md` when communication claims, metrics definitions, or evidence-label policy changes.
- Keep historical specs under `docs/` intact unless the task explicitly asks to update their contracts.
- Prefer links to canonical docs over duplicating long explanations across files.
- Use ASCII prose unless the edited file already has a clear reason for Unicode.

Manual doc checks from repo root:

```sh
git diff --check
for f in README.md AGENTS.md docs/*.md; do diff -u "$f" <(uk2us "$f") || true; done
python3 - <<'PY'
from pathlib import Path
import re, sys
root = Path.cwd()
files = [Path('README.md'), Path('AGENTS.md'), *Path('docs').glob('*.md')]
pattern = re.compile(r'\[[^\]]+\]\(([^)]+)\)')
errors = []
for path in files:
    if not path.exists():
        continue
    for match in pattern.finditer(path.read_text()):
        target = match.group(1).strip()
        if not target or target.startswith(('http://', 'https://', 'mailto:', '#', '/')):
            continue
        target_path = target.split('#', 1)[0]
        if not target_path:
            continue
        resolved = (path.parent / target_path).resolve()
        try:
            resolved.relative_to(root.resolve())
        except ValueError:
            errors.append(f'{path} -> {target}: outside repo')
            continue
        if not resolved.exists():
            errors.append(f'{path} -> {target}: missing {resolved}')
if errors:
    print('\n'.join(errors))
    sys.exit(1)
print(f'validated {len(files)} markdown files, 0 missing relative links')
PY
```

If `uk2us` is unavailable, note that in the handoff and still run `git diff --check`.

## Validation Entry Points

Use the narrowest validation that proves the change.

Docs-only changes:

```sh
git diff --check
```

Rust source changes:

```sh
cargo fmt
cargo test -p <crate_name>
```

Python wrapper/import changes in the release tree:

```sh
python3.12 -c "import wepppyo3.<module>"
python3 -m pytest tests/<module_or_domain>
```

Full validation, when warranted:

```sh
cargo fmt
cargo test
python3 -m pytest tests
```

GDAL/PROJ-dependent crates may require host libraries and environment variables.
If validation is blocked by environment rather than code, record the exact
command and failure in the handoff.

## Git and Workspace Rules

- Do not create or switch branches unless the user explicitly asks.
- Do not revert user changes or unrelated dirty work.
- Do not run destructive commands such as `git reset --hard` or `git checkout --` unless explicitly requested.
- Ignore generated `target/` outputs unless the task is specifically about build artifacts.
- Treat `release/linux/py312/wepppyo3/` as deployable package content, not scratch space.

## Handoff Expectations

Final notes should include:

- What changed and why.
- Exact files changed.
- Validation commands and results.
- Any unrun validation with a reason.
- Residual risks or follow-up packages if release provenance, benchmark claims, or module maturity need deeper work.

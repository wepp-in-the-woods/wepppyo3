# MOFE input validation (candidate source, 2026-11-02)

`confirmed`: source slope writers now reject enabled buffer lengths that are
nonfinite or nonpositive, validate automatic source profiles with the explicit
profile validator, preserve normalized segment endpoints, and reject lengths
that round to zero before opening the output. Equal/overlong buffers retain one
segment. No new minimum-length policy or silent segment merging is introduced.

`confirmed`: watershed MOFE map fractions must be strictly increasing. Equal
boundaries previously assigned cells to a zero-width segment; both mapping and
counting paths now reject them through the shared validator.

Validation: four `wepp_interchange_rust` MOFE unit tests and five
`watershed_abstraction_rust` unit tests pass with Python 3.12 linkage. A private
candidate extension was loaded directly to exercise Python binding errors and
read actual slope files. WEPPpy retains the artifacts in its
`20261102_mofe_input_producer_audit` package. The deployable shared objects were
not replaced. This is producer evidence, not native WEPP model acceptance or a
production release. Native capacity, PMET adapter mapping and FC/HK ownership
remain separate dependencies.

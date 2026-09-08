# Fixture manifest

## Static:

confirmed: Sources, expected bytes, and expected SHA-256 values are specified in
[package.md](../package.md). The two HPC source directories remained read-only.
Only the ten named Parquet files were copied. All processing used repository
copies or read-only repository fixtures and disposable output directories.

## Ran:

confirmed: All ten copied files match the package's byte sizes and SHA-256 values.
[fixture-checksums.json](fixture-checksums.json) records source paths, checksums,
rows, row-group counts, and SHA-256 fingerprints of serialized Arrow schemas.

confirmed: MOFE has 68 hillslopes: 9 with one OFE, 12 with two, 11 with three,
6 with four, and 30 with five. There are 7,670 simulation days, 1990-2010;
H.wat and H.soil each contain 1,840,800 rows in 68 row groups.
The performance fixture has 586 single-OFE hillslopes and 16,071 days,
1980-2023; H.wat and H.soil each contain 9,417,606 rows in 586 row groups.

confirmed: The ten files are staged using the scoped attribute
`tests/fixtures/totalwatsed3/**/*.parquet filter=lfs diff=lfs merge=lfs -text`.
`git lfs ls-files` lists all ten. Every staged blob was compared with the exact
three-line LFS pointer containing its recorded SHA-256 and byte count; all passed.
The combined payload is 530,800,688 bytes. No fixtures have been committed,
uploaded, or verified through a clean remote checkout.

confirmed: The existing decimal-pleasing fixture has 67 hillslopes and 4,018
output days. Its 68-column frozen output is incompatible with the current
79-column producer; see [parity-results.md](parity-results.md). It was not changed.

Oracle provenance: HPC outputs have the exact package-supplied checksums.
Their original producer revisions are not established by the Parquet files.
The current WEPPpy revision reproduces both HPC outputs under the strict
comparator. The single-OFE frozen and regenerated output hashes are recorded in
[oracle-comparison.json](oracle-comparison.json).

## Resumed execution additions

confirmed: the approved current-producer single-OFE oracle and nineteen synthetic
contract cases are now preserved alongside the HPC fixtures. 108 Parquet
files are staged as verified Git LFS pointers. all-fixture-checksums.json records
all file hashes and sizes; case.json files record synthetic options and ash
metadata. The original WEPPpy decimal-pleasing fixture remains unchanged.
No LFS upload, commit, or remote checkout verification has occurred.

## Independent review additions

Three extra old-producer contracts cover nullable WAT area and first-NaN ash
year0/days-from-fire metadata. The 127 Parquet files comprise the preserved
original fixtures plus these additional cases. Canonical Compose captures use
PyArrow 23.0.1; earlier host probe output remains in disposable evidence and
independent review notes. All fixture identities are in all-fixture-checksums.json.

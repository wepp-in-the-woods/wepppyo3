# AgFields sub-field native compatibility evidence

This note records the native compatibility boundary captured for the 2026-07-16
AgFields sub-field interchange work. Canonical release installation evidence is
maintained separately in `docs/release-provenance.md`.

## Pre-change release

`confirmed`: Before Rust edits, the canonical py312 shared object was
`release/linux/py312/wepppyo3/wepp_interchange/wepp_interchange_rust.so` with
SHA256
`7419203c8b91db1b595590b7c9a28040662d5fad9fdf8b182a17c85a76d518e4`.

The ordinary golden used source-ordered `H1`, `H2`, and `H3` reports from the
designated forest acceptance corpus. EBE, ELEMENT, and SOIL used calendar start
year 2008; PASS explicitly used `legacy_ascii`. Every output contained three
row groups in source order and exact ordinary metadata:
`dataset_version=1.2`, `dataset_version_major=1`,
`dataset_version_minor=2`, and `schema_version=1`.

| Family | Rows | Row-group rows | Pre-change Parquet SHA256 |
| --- | ---: | --- | --- |
| PASS | 18,630 | 6,210 / 6,210 / 6,210 | `4c3e6c396d85ca2a847f6f69a5f87d90f131b02874caf8a9c8388d6dfbe8026c` |
| EBE | 556 | 187 / 177 / 192 | `cfec60983bc8eacbfdf1bdd28f87f0b00f05c69672819e5814c847522c4ba544` |
| ELEMENT | 1,747 | 586 / 571 / 590 | `8411e430447358f7928f8504d5b2c7ea954c7c160bce358be97d33288b0cc943` |
| LOSS | 15 | 5 / 5 / 5 | `97b1fea6c6839ea65fd619d9ae1ec950cc7f11a231fc992c259d1f00fc94f4ae` |
| SOIL | 18,630 | 6,210 / 6,210 / 6,210 | `f3b7cc7bbdd3b280d6053446384cba667891fc3a469bc3bd553cfc980be0e444` |
| WAT | 18,630 | 6,210 / 6,210 / 6,210 | `65a129a78b1d76f9590a1e20f9c630aa85a29bf0afe4da0f7db08ff0240a46cf` |

## Candidate parity

`confirmed`: The release-mode candidate at
`target/release/libwepp_interchange_rust.so` had SHA256
`8c42edd0a8e1b03bdaf423355a12414180c709efaac3e379e5dd23e6cc77214e`
when this evidence was captured. It was subsequently installed atomically into
the canonical py312 release tree and verified there; see release provenance.

All six candidate ordinary outputs matched their pre-change goldens by full
Arrow table equality, exact schema including field and dataset metadata, source
order, row count, row-group count, and per-row-group size. Parquet byte hashes
differed after rebuilding because unordered metadata serialization changed;
there was no logical schema or value difference. The ordinary public signatures
were also unchanged.

The dedicated AgFields tests additionally prove full nonidentity column parity
using Arrow `ArrayData` equality, including null buffers and floating-point bit
patterns, for all six report families. PASS covers both legacy ASCII and schema-2
HBP. Identity coverage includes two sub-fields sharing a field and a third in a
different field, source order, one row group per source, exact empty schemas,
non-positive/duplicate/mismatched descriptors, Python int32 extraction, and
late-source failure atomicity.

Validation captured with the candidate:

- `cargo check -p wepp_interchange_rust`: passed;
- `cargo test -p wepp_interchange_rust`: 80 unit and 16 integration tests
  passed; and
- temporary py312 candidate package, `tests/wepp_interchange`: 46 tests passed.

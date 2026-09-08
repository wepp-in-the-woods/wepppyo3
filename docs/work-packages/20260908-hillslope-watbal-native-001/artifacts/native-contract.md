# Native API contract (preimplementation draft)

Static: independently reviewed preimplementation API; Ran: parity and failure tests are indexed in validation-summary.md.

Classification: faithful extraction; no report formula or cache contract change.
Operator authorization: execute this package, including the required native API.
Source baseline: 21989c44fe941399f5e4f2d323614463e50809d5; WEPPpy baseline:
d8fbe9f4ab270b6a7ab119d7cdda55d3cb628986.

## Interface

    hillslope_watbal_wepp_ids(wat_path: str) -> list[int]
    hillslope_watbal_to_parquet(wat_path: str, output_path: str,
                              topaz_by_wepp_id: dict[int, int],
                              pandas_metadata: str | None = None) -> dict

ID discovery returns sorted distinct i64 WEPP IDs using projected 8192-row
batches. WEPPpy owns translator/manifest/raw-Roads mapping and existing logs.
The producer returns input_rows, rows_written, and ofe_keys counts only.
Mapping telemetry remains in WEPPpy because all decisions precede the writer.
The GIL is released while Rust reads, aggregates, and writes.

## Data and compatibility

Read only wepp_id, ofe_id, water_year, P, Dp, QOFE, latqcc, Ep, Es, Er,
and Area. Aggregation state is bounded by distinct Topaz/year and WEPP/OFE
keys; no source-row collection is returned or retained. Supported numeric Arrow types are Int8/16/32/64, UInt8/16/32/64,
Float32/64, and Null, with checked integer keys; strings, nested types,
nonintegral keys, out-of-range keys, and infinite values fail explicitly.
Null and NaN flux/area become zero. A null or NaN OFE contributes flux but no area,
matching pandas groupby dropping the null OFE key. Null WEPP/year keys fail.
Track first area by WEPP/OFE, sum OFEs in key order and then translate areas.
Seven independent compensated flux sums preserve pandas groupby accuracy.
Only after group sums compute (Ep + Es) + Er.

Output columns match the package's eight-column order; populated keys are
nullable Int64 and metrics nullable Float64. Zero-row Null-typed projected columns are accepted. Empty output uses nullable Null
columns, matching the empty object-typed pandas frame. Sort by Topaz/year.
WEPPpy supplies pandas metadata generated from the compact schema for the
installed pandas version; tests compare metadata against frozen outputs.
No source metadata propagates to the cache. Existing cache version, sidecars,
legacy reads, source freshness, units, and iterators remain in WEPPpy.

## Failure boundary

Missing paths and write errors raise OSError; malformed schemas/values raise
ValueError; missing mappings raise KeyError. Reject canonically aliased source/output
paths, output symlinks, and nonregular existing destinations. Source symlinks
are supported for disposable fixture workflows. Publish via create_new sibling
temporary file, writer close, then rename; clean the temporary on every error.
Never truncate an existing cache. Preserve existing access bits at staging-file
creation and publication for service-owned UID/GID files; first creation obeys
umask. Existing dependencies suffice; no new crate.

## Regression plan

Compare frozen single-OFE, MOFE, and 586-hillslope summary/cache metadata,
keys, ordering, and report iterators at rtol=1e-10 and atol=1e-12. Exercise
merged mappings, nulls, empty data, malformed schema/keys, missing mapping,
missing source, destination directory/symlink, and failure cleanup. Validate
legacy/stale cache and baseline/Roads through the existing public facade.
The compact schema and generated cache are unchanged downstream inputs.

Canonical H.wat keys are integer Arrow fields (WEPPpy
wepppy/wepp/interchange/hill_wat_interchange.py:22-29). Incidental pandas
coercion of fractional or string keys is outside that valid interchange contract.

Independent correctness review confirmed populated/empty schemas and checked
integer boundary. Disposition: treat NaN OFE as null, canonicalize aliases,
and retain writer I/O classification as OSError. Implementation reviews found and resolved late-failure test coverage and
restricted-cache mode preservation. Final validation and publication are tracked
in the active ExecPlan.

# PASS components v3 integration

2026-10-09. General-release integration authorized by Roger. This contract
extends existing Roads and AgFields composition, not the model routing equations.

## Compatibility

Read legacy and WEPP_PASS_COMPONENTS 3 files. Reject experimental v1/v2 and
unknown versions. All sources in a combine call must use the same version;
regenerate inputs with the selected build rather than mixing releases. Keep
public column/Parquet schemas unchanged. Preserve internal component operands
through composition and emit SRC3 after every EVENT in an extended file.

## Composition

Use existing represented-area scaling (AgFields) or additive scaling (Roads).
For active components, sum scaled Vr and Vs in m3. Combine Qr in m3/s through
the existing triangular peak superposition using source tcs. Inactive sources
contribute their legacy runvol and peakro as the unsplit component; do not infer
a return fraction for unsupported producer cases.

For each active source, normalize its 24 nonnegative hourly weights by their
sum and multiply by scaled Vs. Sum these volume weights by hour. Their absolute
units are immaterial to v3; their ratio defines its equivalent support.
Zero-area sources contribute nothing. If aggregate Vs is zero emit inactive
SRC3; otherwise emit the combined operands at supplemental precision.

Leave existing combined EVENT volume, peak, duration, tcs, oalpha and sediment
composition unchanged. CHRQIN consumes the separately combined Qr and the v3
return-support rule. No physical hillslope erosion is recomputed by composition.
This scalar aggregation does not promise exact superposition of separately
routed sources: maximum storm support and aggregated return weights can smooth
the input. It makes no new assumption about measured mixed-flow timing.

## Gates

Test active/inactive and pure return, single-source identity, unequal represented
areas, zero-area inputs, invalid/version-mixed inputs, round-trip payloads and
unchanged legacy tests. Exercise generated same-build combined files through
the native watershed path. Never strip the component extension as a fallback.
Mutation and disturbed-parameterization studies remain pre-deployment checks
owned by Roger; release and vendoring do not authorize deployment.

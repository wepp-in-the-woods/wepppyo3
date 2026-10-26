# Roads flow direction input

The Python function `wepppyo3.roads_flowpath.trace_downslope_flowpath` accepts
keyword-only `wbt=False`. Existing callers keep the TOPAZ keypad encoding.
WEPPpy Roads is WBT-only and always passes `wbt=True` for native Whitebox D8
rasters. The adapter reuses Peridot's in-memory conversion before tracing; it
does not modify raster files. No value-based encoding inference is performed.

Whitebox compass values: NE=1, E=2, SE=4, S=8, SW=16, W=32, NW=64, N=128.
Corresponding TOPAZ values: 3, 6, 9, 8, 7, 4, 1, 2. Unknown values normalize to
no-flow and retain explicit invalid-direction termination. Tracer results,
channel masks, seed/shape checks, loop/edge/step guards and profile metrics keep
their existing semantics. An incompatible old extension rejects the keyword;
there is no silent fallback. Deploy the paired caller and native artifact.

Regression tests use independent GeoTIFF neighbor positions for all eight
Whitebox codes and preserve TOPAZ/CLI parity. The WEPPpy package
`20261026_roads_wbt_flow_direction` records actual-project trace and full Roads
output validation. A native test pass is not a claim of production deployment.

# Original memory scaling and repeatability

Superseded for the current candidate by [scaling-optimization.md](scaling-optimization.md).
The failed measurements below remain unchanged historical evidence.

confirmed: All six scale outputs pass strict oracle parity. Each used a separate
12 GiB container, twelve CPUs, one warm-up, and one measured call; the 586-hillslope
container used three measured calls. Whole-container peak includes warm-up and is
captured before Python oracle generation/comparison. Raw commands and measurements
are in scale-commands.json and scale-measurements.json.

| Hillslopes | First measured seconds | Peak bytes |
| --- | --- | --- |
| 73 | 0.849699 | 440,721,408 |
| 146 | 1.343275 | 470,315,008 |
| 293 | 2.800468 | 477,855,744 |
| 586 | 3.880972 | 510,377,984 |
| 2344 | 13.695810 | 647,618,560 |
| 5860 | 33.354449 | 913,870,848 |

confirmed: 5,860/586 peak ratio is 1.7905765, exceeding the package maximum
1.5. The largest peak is 913,870,848 bytes (871.54 MiB), below 9 GiB. No OOM,
superlinear wall-time growth, or leftover staging files was observed. The scaling
stop condition applies independently of the user's accepted 107.18% timing.

confirmed: Three consecutive measured calls at 586 hillslopes had anonymous
memory after each call of 221,233,152, 206,757,888, and 216,875,008 bytes.
This does not show monotonically retained anonymous state; small file-cache growth
accompanies retained output files. No process restart occurred between calls.

Generation: use repository copies only, select sorted original hillslopes for
subsets, and repeat the 586-hillslope inputs with disjoint wepp_id offsets for
larger scales. Preserve one row group per hillslope and all original values.
The disposable generator is deliberately uncommitted; its hash and per-scale
manifests are in scale-generation-manifest.json. Small-scale oracles use the
unchanged Python producer. Larger-scale oracles multiply additive mass, volume,
and area fields of the approved oracle by the replication factor; depths,
concentrations, and dates remain unchanged. See measure_scale.py for exact fields.

inference: Retaining complete ArrowReaderMetadata for four source files may
contribute to memory growth with row-group count. This has not been isolated by
profiling. Investigate metadata residency on resumption before changing the gate.

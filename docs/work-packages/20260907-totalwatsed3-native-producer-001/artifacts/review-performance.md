# Final reviewed-release performance

Candidate: bf21f5e5aea9a7c690b7f48926d74f8bb412269d4127b74a166a1e0ef598a354.
The final source/build identity is in native-build.json. Benchmark scripts load
the exact retired Python producer from WEPPpy revision
3c51780f50e2599ef72b03214c35bf20538e55c4, not the new native-only facade.
Materialize that historical source with git show into
`target/totalwatsed3-evidence/review-previous-producer.py` before reproduction.
The scripts and command arrays preserve all other runtime options.

## Timing and parity

Five isolated warm-cache observations per producer and real fixture retain the
12 GiB limit, CPUs 0-11 and WEPPPY_NCPU=12. All twenty outputs pass strict parity.
The 586-hillslope median is 3.868492 seconds native versus 3.629247 Python:
106.5921%, within the user's accepted timing disposition. Worst native execution
is 3.877188 seconds, below 115% of Python median. Native peak is 457,904,128 bytes
versus Python 1,008,013,312, below half. MOFE median is 0.828750 native versus
0.962131 Python (86.1369%). Raw receipts: review-benchmark-measurements.json.

## Scaling and cache accounting

The first final-release six-scale run returned raw peaks 470,810,624 bytes at
586 hillslopes and 974,950,400 at 5,860: **2.07079**, a failed 1.5 ratio. Preserve
review-scale-measurements.json and review-scaling-progress.log unchanged.
The largest post-call file-cache charge rose by 293,638,144 bytes versus the
prior passing compact-reader run, while post-call anonymous memory rose by only
135,168 bytes. This strongly supports cache charging as the explanation, but
samples are before/after the call, not at peak; no adjusted peak is inferred.

Both sweeps include an in-container warm-up. The controlled repeat additionally
reads every input outside each measured container before launch, symmetrically
for all six sizes. This controls host-cache warmth and charge ownership, not
merely measured-call warm-up. It retains the unchanged raw memory.peak gate;
no cache subtraction or anonymous-memory substitution is used. The repeat has
raw peaks 473,591,808 and 681,893,888 bytes: **1.4398346**, below 1.5. All six
sizes and all eight measured outputs pass parity. See
review-warm-scale-measurements.json and run_review_warm_scales.py.

This establishes the controlled host-cache result, not cache-independent 1.5x
scaling. Both largest absolute observations remain below the 9 GiB resource
limit. Independent review of the component deltas supports this interpretation;
Linux documents cache ownership in
[the cgroup memory ownership contract](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-ownership).
The generation algorithm, source fixture hashes, thresholds, and failed receipts
remain preserved. No production workload or resource limit was changed.

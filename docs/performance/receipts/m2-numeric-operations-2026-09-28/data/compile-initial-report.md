# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| large-block-200 | 2444 | pr53 | 40/40 | 1.5165e+06 | 1.612 |
| large-block-200 | 2444 | pr52 | 40/40 | 1.8441e+06 | 1.325 |
| large-block-200 | 2444 | candidate | 40/40 | 1.81192e+06 | 1.349 |
| m1-500 | 28890 | pr53 | 40/40 | 1.00261e+07 | 2.881 |
| m1-500 | 28890 | pr52 | 40/40 | 9.30281e+06 | 3.106 |
| m1-500 | 28890 | candidate | 40/40 | 1.024e+07 | 2.821 |
| m2-500 | 25054 | pr53 | 40/40 | 8.96071e+06 | 2.796 |
| m2-500 | 25054 | pr52 | 40/40 | 8.50908e+06 | 2.944 |
| m2-500 | 25054 | candidate | 40/40 | 9.3335e+06 | 2.684 |
| nonapp-500 | 24390 | pr53 | 40/40 | 7.3999e+06 | 3.296 |
| nonapp-500 | 24390 | pr52 | 40/40 | 7.36373e+06 | 3.312 |
| nonapp-500 | 24390 | candidate | 40/40 | 7.38994e+06 | 3.3 |

Speed relative to pr53 (only cases fully successful on both engines):

- large-block-200/pr52: 0.8224×
- large-block-200/candidate: 0.837×
- m1-500/pr52: 1.078×
- m1-500/candidate: 0.9791×
- m2-500/pr52: 1.053×
- m2-500/candidate: 0.9601×
- nonapp-500/pr52: 1.005×
- nonapp-500/candidate: 1.001×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

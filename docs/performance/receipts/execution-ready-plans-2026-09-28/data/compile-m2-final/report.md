# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| large-block-200 | 2444 | candidate | 30/30 | 1.31006e+06 | 1.866 |
| large-block-200 | 2444 | pre53 | 30/30 | 1.24254e+06 | 1.967 |
| large-block-200 | 2444 | pr52 | 30/30 | 1.06044e+06 | 2.305 |
| m1-500 | 28890 | candidate | 30/30 | 1.10104e+07 | 2.624 |
| m1-500 | 28890 | pre53 | 30/30 | 1.0865e+07 | 2.659 |
| m1-500 | 28890 | pr52 | 30/30 | 9.6111e+06 | 3.006 |
| m2-500 | 25054 | candidate | 30/30 | 1.00407e+07 | 2.495 |
| m2-500 | 25054 | pre53 | 30/30 | 9.91271e+06 | 2.527 |
| m2-500 | 25054 | pr52 | 30/30 | 9.11917e+06 | 2.747 |
| nonapp-500 | 24390 | candidate | 30/30 | 7.7579e+06 | 3.144 |
| nonapp-500 | 24390 | pre53 | 30/30 | 7.43531e+06 | 3.28 |
| nonapp-500 | 24390 | pr52 | 30/30 | 7.42598e+06 | 3.284 |

Speed relative to candidate (only cases fully successful on both engines):

- large-block-200/pre53: 1.054×
- large-block-200/pr52: 1.235×
- m1-500/pre53: 1.013×
- m1-500/pr52: 1.146×
- m2-500/pre53: 1.013×
- m2-500/pr52: 1.101×
- nonapp-500/pre53: 1.043×
- nonapp-500/pr52: 1.045×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

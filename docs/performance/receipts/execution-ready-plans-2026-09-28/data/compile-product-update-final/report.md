# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| product-update-500 | 28890 | candidate | 30/30 | 1.17722e+07 | 2.454 |
| product-update-500 | 28890 | pre53 | 30/30 | 1.05093e+07 | 2.749 |
| product-update-500 | 28890 | pr52 | 30/30 | 1.007e+07 | 2.869 |

Speed relative to candidate (only cases fully successful on both engines):

- product-update-500/pre53: 1.12×
- product-update-500/pr52: 1.169×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

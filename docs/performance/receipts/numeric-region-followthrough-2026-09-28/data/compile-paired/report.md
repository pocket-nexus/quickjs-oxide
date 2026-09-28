# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| lexical-loop-500 | 44890 | head | 30/30 | 2.92154e+07 | 1.537 |
| lexical-loop-500 | 44890 | candidate | 30/30 | 3.05146e+07 | 1.471 |
| product-update-500 | 28890 | head | 30/30 | 1.61564e+07 | 1.788 |
| product-update-500 | 28890 | candidate | 30/30 | 1.71432e+07 | 1.685 |

Speed relative to head (only cases fully successful on both engines):

- lexical-loop-500/candidate: 0.9574×
- product-update-500/candidate: 0.9424×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

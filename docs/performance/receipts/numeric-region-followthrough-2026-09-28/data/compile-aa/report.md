# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| lexical-loop-500 | 44890 | same_a | 30/30 | 2.57469e+07 | 1.744 |
| lexical-loop-500 | 44890 | same_b | 30/30 | 2.5741e+07 | 1.744 |
| product-update-500 | 28890 | same_a | 30/30 | 1.48509e+07 | 1.945 |
| product-update-500 | 28890 | same_b | 30/30 | 1.46267e+07 | 1.975 |

Speed relative to same_a (only cases fully successful on both engines):

- lexical-loop-500/same_b: 1×
- product-update-500/same_b: 1.015×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

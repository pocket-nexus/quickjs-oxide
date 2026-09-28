# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| field-local-500 | 38390 | a2 | 24/24 | 6.13009e+06 | 6.263 |
| field-local-500 | 38390 | b2 | 24/24 | 6.05389e+06 | 6.341 |
| lexical-loop-500 | 44890 | a2 | 24/24 | 1.49595e+07 | 3.001 |
| lexical-loop-500 | 44890 | b2 | 24/24 | 1.42738e+07 | 3.145 |

Speed relative to a2 (only cases fully successful on both engines):

- field-local-500/b2: 1.013×
- lexical-loop-500/b2: 1.048×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

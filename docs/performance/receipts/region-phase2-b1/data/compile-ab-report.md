# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| field-predicate-500 | 31890 | a1 | 24/24 | 5.21164e+06 | 6.119 |
| field-predicate-500 | 31890 | b1 | 24/24 | 5.95781e+06 | 5.353 |
| lexical-loop-500 | 44890 | a1 | 24/24 | 1.40523e+07 | 3.195 |
| lexical-loop-500 | 44890 | b1 | 24/24 | 1.42784e+07 | 3.144 |

Speed relative to a1 (only cases fully successful on both engines):

- field-predicate-500/b1: 0.8748×
- lexical-loop-500/b1: 0.9842×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| field-predicate-500 | 31890 | left | 24/24 | 5.51928e+06 | 5.778 |
| field-predicate-500 | 31890 | right | 24/24 | 5.61768e+06 | 5.677 |
| lexical-loop-500 | 44890 | left | 24/24 | 1.46607e+07 | 3.062 |
| lexical-loop-500 | 44890 | right | 24/24 | 1.47006e+07 | 3.054 |

Speed relative to left (only cases fully successful on both engines):

- field-predicate-500/right: 0.9825×
- lexical-loop-500/right: 0.9973×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

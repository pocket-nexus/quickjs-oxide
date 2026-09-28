# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| field-local-500 | 38390 | left | 24/24 | 5.98028e+06 | 6.419 |
| field-local-500 | 38390 | right | 24/24 | 5.71548e+06 | 6.717 |
| lexical-loop-500 | 44890 | left | 24/24 | 1.4789e+07 | 3.035 |
| lexical-loop-500 | 44890 | right | 24/24 | 1.41744e+07 | 3.167 |

Speed relative to left (only cases fully successful on both engines):

- field-local-500/right: 1.046×
- lexical-loop-500/right: 1.043×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

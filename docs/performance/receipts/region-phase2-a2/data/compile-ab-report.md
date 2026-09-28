# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| lexical-loop-500 | 44890 | b1 | 24/24 | 1.49151e+07 | 3.01 |
| lexical-loop-500 | 44890 | a2 | 24/24 | 1.47112e+07 | 3.051 |
| preinc-pair-500 | 34890 | b1 | 24/24 | 8.54163e+06 | 4.085 |
| preinc-pair-500 | 34890 | a2 | 24/24 | 8.61928e+06 | 4.048 |

Speed relative to b1 (only cases fully successful on both engines):

- lexical-loop-500/a2: 1.014×
- preinc-pair-500/a2: 0.991×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| lexical-loop-500 | 44890 | left | 24/24 | 1.47081e+07 | 3.052 |
| lexical-loop-500 | 44890 | right | 24/24 | 1.45467e+07 | 3.086 |
| preinc-pair-500 | 34890 | left | 24/24 | 8.65759e+06 | 4.03 |
| preinc-pair-500 | 34890 | right | 24/24 | 8.5778e+06 | 4.067 |

Speed relative to left (only cases fully successful on both engines):

- lexical-loop-500/right: 1.011×
- preinc-pair-500/right: 1.009×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| product-update-500 | 28890 | candidate_a | 30/30 | 1.55229e+07 | 1.861 |
| product-update-500 | 28890 | candidate_b | 30/30 | 1.49847e+07 | 1.928 |

Speed relative to candidate_a (only cases fully successful on both engines):

- product-update-500/candidate_b: 1.036×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

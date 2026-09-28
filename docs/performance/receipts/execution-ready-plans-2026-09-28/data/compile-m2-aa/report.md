# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| large-block-200 | 2444 | candidate_a | 30/30 | 1.23465e+06 | 1.98 |
| large-block-200 | 2444 | candidate_b | 30/30 | 1.29033e+06 | 1.894 |
| m1-500 | 28890 | candidate_a | 30/30 | 1.00688e+07 | 2.869 |
| m1-500 | 28890 | candidate_b | 30/30 | 9.9795e+06 | 2.895 |
| m2-500 | 25054 | candidate_a | 30/30 | 9.61075e+06 | 2.607 |
| m2-500 | 25054 | candidate_b | 30/30 | 9.50681e+06 | 2.635 |
| nonapp-500 | 24390 | candidate_a | 30/30 | 1.01991e+07 | 2.391 |
| nonapp-500 | 24390 | candidate_b | 30/30 | 1.1498e+07 | 2.121 |

Speed relative to candidate_a (only cases fully successful on both engines):

- large-block-200/candidate_b: 0.9568×
- m1-500/candidate_b: 1.009×
- m2-500/candidate_b: 1.011×
- nonapp-500/candidate_b: 0.887×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

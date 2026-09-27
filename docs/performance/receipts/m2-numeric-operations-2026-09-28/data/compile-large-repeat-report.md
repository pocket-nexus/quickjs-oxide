# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| large-block-200 | 2444 | pr53 | 100/100 | 976666 | 2.502 |
| large-block-200 | 2444 | pr52 | 100/100 | 802542 | 3.045 |
| large-block-200 | 2444 | candidate | 100/100 | 963230 | 2.537 |

Speed relative to pr53 (only cases fully successful on both engines):

- large-block-200/pr52: 1.217×
- large-block-200/candidate: 1.014×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

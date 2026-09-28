# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| react-realworld | 248694 | baseline | 24/24 | 5.66297e+07 | 4.392 |
| react-realworld | 248694 | integrated | 24/24 | 5.72092e+07 | 4.347 |
| solid-realworld | 187879 | baseline | 24/24 | 2.81236e+07 | 6.68 |
| solid-realworld | 187879 | integrated | 24/24 | 2.87181e+07 | 6.542 |
| vue-realworld | 961475 | baseline | 24/24 | 1.23274e+08 | 7.8 |
| vue-realworld | 961475 | integrated | 24/24 | 1.24623e+08 | 7.715 |

Speed relative to baseline (only cases fully successful on both engines):

- react-realworld/integrated: 0.9899×
- solid-realworld/integrated: 0.9793×
- vue-realworld/integrated: 0.9892×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

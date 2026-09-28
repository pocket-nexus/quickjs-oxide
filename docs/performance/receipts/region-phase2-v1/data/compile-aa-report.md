# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| react-realworld | 248694 | left | 24/24 | 5.78877e+07 | 4.296 |
| react-realworld | 248694 | right | 24/24 | 5.79554e+07 | 4.291 |
| solid-realworld | 187879 | left | 24/24 | 2.7898e+07 | 6.734 |
| solid-realworld | 187879 | right | 24/24 | 2.82069e+07 | 6.661 |
| vue-realworld | 961475 | left | 24/24 | 1.25028e+08 | 7.69 |
| vue-realworld | 961475 | right | 24/24 | 1.25402e+08 | 7.667 |

Speed relative to left (only cases fully successful on both engines):

- react-realworld/right: 0.9988×
- solid-realworld/right: 0.989×
- vue-realworld/right: 0.997×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

# Front-end compile matrix

Each sample is one fresh process; the probe prints exactly one ns line. Runtime/Context construction, source I/O and teardown are excluded. Ratios are reference_ns / engine_ns; >1 means the engine is faster than the reference.

| Case | Bytes | Engine | Runs | Median ns | MB/s |
| --- | ---: | --- | ---: | ---: | ---: |
| large-block-200 | 2444 | pr53 | 100/100 | 1.07454e+06 | 2.274 |
| large-block-200 | 2444 | pr52 | 100/100 | 875500 | 2.792 |
| large-block-200 | 2444 | candidate | 100/100 | 1.0525e+06 | 2.322 |
| m1-500 | 28890 | pr53 | 100/100 | 8.29558e+06 | 3.483 |
| m1-500 | 28890 | pr52 | 100/100 | 7.66096e+06 | 3.771 |
| m1-500 | 28890 | candidate | 100/100 | 8.5056e+06 | 3.397 |
| m2-500 | 25054 | pr53 | 100/100 | 7.66452e+06 | 3.269 |
| m2-500 | 25054 | pr52 | 100/100 | 7.25406e+06 | 3.454 |
| m2-500 | 25054 | candidate | 100/100 | 7.97915e+06 | 3.14 |
| nonapp-500 | 24390 | pr53 | 100/100 | 6.29902e+06 | 3.872 |
| nonapp-500 | 24390 | pr52 | 100/100 | 6.24404e+06 | 3.906 |
| nonapp-500 | 24390 | candidate | 100/100 | 6.30542e+06 | 3.868 |

Speed relative to pr53 (only cases fully successful on both engines):

- large-block-200/pr52: 1.227×
- large-block-200/candidate: 1.021×
- m1-500/pr52: 1.083×
- m1-500/candidate: 0.9753×
- m2-500/pr52: 1.057×
- m2-500/candidate: 0.9606×
- nonapp-500/pr52: 1.009×
- nonapp-500/candidate: 0.999×

Failed, incomplete and timed-out samples stay visible and are excluded from ratios. This report never infers a whole-engine score from a subset.

# Ordinary execution path: integrated draft gate

This receipt records the plain-build gate for the O1/P1/E1 draft stack on
2026-09-29. The cumulative reference is #53 (`90c85e3d`). The staged heads
are O1 `bfc8def1`, P1 `104f8318`, and E1 `29abc017`. E1 remains a draft:
the integrated app workload gate is not met.

All timing runs used Rust 1.88.0 release binaries without the `profiling`
feature. `scripts/benchmark/fixed.py` authenticated each workload and output,
then ran each sample in a new process with fresh application state and cold
per-process caches. The app manifest SHA-256 is
`88a7effb232534e8ba1a2fc1ae3a05ad5ec46d197475d24e1c1a9a662d9ec193`.
The fixed V8 Crypto manifest SHA-256 is
`6365a60931c686da749b61eb579144b0bdece8f5bc6667a0c92ff960d39c5f2d`.
These are whole-process fixed-work measurements, including preparation.

| Paired comparison | Workload | Repeats | Median time ratio |
| --- | --- | ---: | ---: |
| E1 / #53 | React RealWorld | 16 | 1.0115 |
| E1 / #53 | Solid RealWorld | 16 | 1.0152 |
| E1 / #53 | Vue RealWorld | 16 | 1.0082 |
| E1 / #53 | V8 Crypto, fixed work | 12 | 0.9947 |
| O1 / #53 | React RealWorld | 12 | 0.9993 |
| O1 / #53 | V8 Crypto, fixed work | 8 | 0.9804 |
| P1 / O1 | React RealWorld | 16 | 1.0085 |
| E1 / P1 | React RealWorld | 16 | 1.0080 |

Eight paired `perf stat -e instructions:u` runs measured a median of
3,669,576,606 retired user instructions for #53 and 3,686,188,351 for E1
on React (ratio 1.0045). Crypto measured 12,842,194,692 and 12,880,122,255
respectively (ratio 1.0030). Time and instruction count therefore point in
different directions for Crypto; neither should be represented as a broad
execution win.

Focused diagnostics used six Node-authenticated programs, 100,000 loop
iterations each, with eight paired whole-process samples. Median E1 / #53
ratios were 0.9977 for `value = record.value`, 1.0358 for
`node = node.next`, 1.0413 for `if (record.value)`, 1.0000 for values from
calls, 1.0014 for an always-effectful getter, and 1.0172 for a data-to-getter
phase change. The scripts and raw local runner results are under
`/tmp/oxide-ordinary-perf/ordinary-focused` and
`/tmp/oxide-ordinary-perf/integrated-final`; those paths are machine-local.

The profiling build was kept separate from plain timing. On React, final
diagnostics counted 185,334 local store completions, 519 store observation
requests, 138,967 local named-read completions, 71,974 borrowed named-read
completions, 74,688 named-read driver handoffs, 46,283 new frame
materializations, and 344,331 repeated active-PC publications. #53 also
materialized 46,283 frames on this workload. The counters show local work,
but they do not show a new reduction in the main React driver boundary.

The plain `qjs` binary grew from 9,391,120 bytes at #53 to 9,405,496 bytes
at E1. Profiling reported identical React instruction count, inline code
bytes, verified stack maximum, function count, owned execution layouts, and
final heap-state counts (20,681 live; 4,315 vacant) between #53 and E1.
Compilation time and allocation have not been isolated with the compile probe;
no claim about a preparation-cost improvement is made.

The complete workspace test suite passed, including 2,011 library tests and
908 conformance tests (one ignored). Profiling Clippy passed with warnings
denied. The accessor tests cover cold and warm selection, data-to-accessor
transition, selected getter ownership across the frame handoff, proxy
fallback, and exactly-once effects.

The acceptance rule stops expansion here. O1 restores the established numeric
shortcut while adding ownership-aware ordinary transfers. P1 delivers a data
cache-miss selection improvement and exposes accessor selection. E1 carries
selected accessors to the driver and removes its extra materialize/redecode
retry, but broad ready-driver pre-dispatch materialization still applies to
driver reads. The planned relocation of synchronous driver completions into
the existing execution scope remains incomplete. No further execution family
should be added on the strength of these local counters.

Plain binary SHA-256 values: #53
`608b85ea8eab7e020fe8cebdcf2229845db7bffa8b2b29ce7f33d6cd3d7c6994`,
O1 `82f23866e316935e9bb82df86962b11a3e5298756bad7c7ccc0e343c99101aea`,
P1 `11b765882395bbea84dbd8608593665a0d93be44bdd46f2afacbeb13e865be76`,
and E1 `4a2fb2f71d1a084e629fe27d7a39d63da12c45deb1ac341d1771955d17fa4925`.

# B1: borrowed own-field predicate

Implementation: `0bee5cd8` and `3ceeb4e0`, stacked on A1. A direct
local/argument static-field read consumed by a truthiness branch now has a
physical guard entry before the unchanged ordinary fallback. A successful
own-data cache probe borrows the current value and computes the full language
ToBoolean result under the heap borrow. It creates no property-result owner or
operand-stack temporary. Accessors, proxies, missing fields, cold and
megamorphic sites enter the ordinary read exactly once. The guard and fallback
share the original logical property-cache site and source attribution.

## Fixed-work local comparison

The A1 and B1 plain release binaries ran three Node-authenticated synthetic
development workloads. Each uses 200,000 field branches; the accessor case
checks that every getter fires. Four balanced ABBA/BAAB fresh-process samples
were measured per engine and workload. The figures are median whole-process
wall ratios, including startup and compilation. The adjacent A/A series uses
the B1 binary on both sides. Raw process samples, exact outputs, hashes and
build identity are preserved in [data](data).

| Workload | B1 / A1 wall | Same-binary A/A wall |
| --- | ---: | ---: |
| Own object-valued field | 0.928× | 0.994× |
| Accessor field | 1.045× | 1.001× |
| Alternating own shapes | 0.945× | 1.015× |

The own-data gains support the intended removal of result promotion and stack
traffic. The always-accessor regression remains meaningful after the cold-site
shortcut: the guard still costs a dispatch before ordinary execution. It is
disclosed as a limitation, not described as a speedup. This series does not
represent an independent application holdout or the original V8 score.

The B1 binary is 9,086,792 bytes versus 9,072,712 bytes for A1. Its SHA-256
is `38ce709305768c483766a7e97c4b8e993ee2fe150c0d2f9eeebd7aa7c7e5641d`.
The A/B raw archive SHA-256 is
`dba214c782334ad63d2378451c45a37efff9ffccef182d4216d07eb9bb1838ed`;
the A/A archive SHA-256 is
`b7dbd99a48e9c17fcdb490739af9565a5bf339d4693a0651af65a42d7319fdd6`.

The standalone Script compile probe ran a 500-function field-predicate corpus
and the pre-existing 500-function lexical-loop control. Each side had 24
alternating fresh-process samples. B1/A1 median compile times were 1.143×
for the selected field corpus and 1.016× for the control; the corresponding
B1/B1 A/A ratios were 1.018× and 1.003×. The field-specific compile cost is
meaningful and is a candidate for reducing the selection/layout work in a
later change. The probe excludes runtime/context construction, source I/O,
and teardown. The corpus hash, probe build identity and samples are in
[data](data).

Focused differential tests cover truthiness representations, HTMLDDA,
object-valued fields with throwing `valueOf`, same-shape value changes,
accessor/proxy effects and thrown identity, self references, shape changes,
recovery from a long accessor phase, negated and loop branches, and uncertain
bindings. The full workspace suite passed 2,008 engine tests and 908 oracle
tests (one ignored), plus all other targets. Rust 1.88 workspace Clippy passed
with warnings denied.

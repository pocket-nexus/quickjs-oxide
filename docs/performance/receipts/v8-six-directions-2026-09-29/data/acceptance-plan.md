# Six V8 priorities — frozen acceptance scope

Baseline: 7723e825 (2026-09-29); plain and profiling release Rust 1.88.0, fat LTO, CGU=1.
Goal: reduce complete fixed-work process time for the six requested V8-v7 bodies.
Hard constraints: safe Rust, full generational identities, JS observable ordering,
ownership, error cleanup, unchanged upstream assertions/admission/baselines.
Allowed exchange: bounded code growth and extra guards on nonparticipants, subject
to all-eight and real-application regression checks. No tail-latency/peak-memory
improvement claim from timing or instruction counts.

Historical QuickJS Score / m0 ratios are prioritization supplied by the user;
they are not timing ratios, contemporary baseline measurements, or attributable
speedup potential. The initial existing-binary pilot uses ec291e62; its only
Rust difference from 7723e825 is a documentation URL. Fresh baseline builds and
fresh profiles are retained for acceptance.

Candidates:
- Existing own scalar named store: bypass driver handoff using existing runtime
  contracts; readonly/accessor/missing/reference/exotic cases use original path.
- Number::Int ToInt32: preserve known i32 fact instead of float modulo; Float
  branch unchanged. Actual old release has trunc/fmod calls; independent wrapper
  confirms optimizer does not recover the relation.
- Canonical append: preserve existing slot owners, acquire
  only new slot/shape edges. Prefix validation stays O(width); do not claim
  constant-time property creation or reduced shape construction cost.
- Empty closure owner: secondary small candidate; prioritize append evidence.

Acceptance order: targeted semantic and mechanism tests, isolated plain A/B,
combined A/B, all eight cases plus external RealWorld apps, independent profile,
instructions, peak RSS, binary size, workspace + full Test262 regression gate.
A/A and A/B use fresh processes, same source hashes, CPU 2, ABBA/BAAB. Stop all
agent builds/tests before timing; desktop background processes remain and are
not claimed to be isolated. Bootstrap intervals are descriptive, not universal
confidence claims; retain failed and unfavorable outcomes.

Research (primary sources, consulted 2026-09-29):
- https://bellard.org/quickjs/quickjs.pdf — shared shapes, atoms, deterministic
  reference counting. Design context only; local profile establishes priority.
- https://v8.dev/blog/fast-properties — separate elements/named properties,
  shape transitions, fast vs dictionary storage. Do not infer JIT/IC benefits
  for this interpreter without local mechanism and timing evidence.

Final cross-check: original unmodified adaptive V8-v7 harness for the six
requested suites, baseline/candidate 2 repetitions each in ABBA order. Minimum
32 timed runs and upstream warmup/setup policy remain unchanged. Scores exclude
setup/teardown and are reported separately from fixed-process timing. Two pairs
are corroboration with limited resolution, not grounds to assert tiny gains.

Review correction: Slots::try_reserve still rebuilds a spilled Vec and clones
old slots. The append candidate removes old-owner churn and whole-layout edge/atom
work, not all slot copying or all allocation. Reserve reuse is a separate future
mechanism, not included in measured gains.

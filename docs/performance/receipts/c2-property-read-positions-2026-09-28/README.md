# C2: decoded fallthrough for same-frame property reads

C2 is stacked on the C1 numeric continuation change. `execute_frame` attaches
the decoded fallthrough to static-key and computed-key property read actions,
including cache and dense-array guard misses. The ready driver passes it to
`read_progress`; synchronous linked-own and prepared reads carry it through
result publication without calling `Frame::next_pc()` to rediscover the
instruction boundary.

The operation still owns its existing completion protocol. A known fallthrough
does not advance `resume_pc`; `publish_read_result` retains the property path's
ordering, including advancement before the final result push. Tests cover
ordinary and retained receiver/key outputs, a constrained-output failure,
zero recovery-helper calls during covered completion, fault attribution, and
the one-time effects of a throwing getter. A warmed inline field-cache hit is
a negative control: it creates no property action.

Object-key conversion, getters, proxies, super-property reads and other
pending replies retain their existing operation and frame identities. Their
remaining recovery calls are explicit in `read_pending` and
`complete_read_recovering`; the `property_legacy_fallthrough_recovery_decode`
diagnostic counts those sites, not ordinary instruction decoding. Writes,
calls, suspension and interpreter-entry validation remain outside C2.

## Reproduction

`manifest.json` fixes five workload sources, expected outputs and byte hashes.
The static string and computed object/frame cases test candidate same-frame
completion; the warmed field case tests the inline path; the getter case
checks deferred work. Profile counts, rather than JavaScript source spelling,
determine which path each workload actually reaches. Release comparisons use
fresh plain and profiling builds of C1 and C2 with Rust 1.88.0, the same
repository release profile and distinct target directories.

## Evidence

Measurement and validation results will be recorded here after the final
rebased candidate is built.

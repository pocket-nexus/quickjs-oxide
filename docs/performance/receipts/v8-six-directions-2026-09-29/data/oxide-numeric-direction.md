# Crypto Number ToInt32 candidate

Objective: reduce fixed-workload execution time, with JavaScript conversion/order, safe Rust and release build settings unchanged. No interface work is required.

Chosen mechanism: preserve an existing Int(i32) representation through Number::int32 rather than widening to f64 and applying truncation and modulo again. This directly serves VM bitwise/shift handlers, numeric dense-read fused handlers, unary bit-not, and masked-index consumers. Float input still uses the existing conversion kernel.

Evidence: fresh six-item profile identifies execute_frame at 47.33% and binary_number_result at 3.01% of sampled Crypto cycles. Logical profile maps am3 dense_read_binary pc 24 to 744,364 hits. This is coverage evidence, not an estimate of removable total runtime. In am3, the operation reads a numeric limb and applies a bitmask. Parent actual release binary still widens Int with cvtsi2sdl and calls trunc/fmod; the Rust 1.88 isolated wrapper also retains fmod on an Int input. Candidate wrapper is movl/ret.

Changed file: src/engine/value/number/operations.rs. Commit 03046503. Number::int32 branches on the representation it already has. No persistent metadata, no new opcode, no continuation or ownership contracts change.

Correctness: Int represents the full signed 32-bit range, which is an identity domain for ECMAScript ToInt32. Existing Float conversion handles NaN/infinities/negative zero/fractional and wrapping cases unchanged. The new unit test checks both representations, integer bounds and Float conversion boundaries. Rust 1.88 cargo test --locked --lib --jobs 2 numeric: 74 pass. int32 filter: 6 pass. fmt --all -- --check passes. End-to-end existing tests include signed/unsigned shifts, bitwise precedence, coercion hints/order, abrupt completion and BigInt exclusion.

Alternative 1 (deferred): replace Float ToInt32 modulo with IEEE-754 exponent/significand extraction. This needs separate all-exponent/random-bit differential testing and evidence Float conversion contributes material cost. The current profile does not justify expanding the patch.

Alternative 2 (deferred): add more fused arithmetic opcodes or mutate register/VM interfaces. Existing execute_frame and stack overhead indicate a direction, but source-level instruction count alone does not establish a particular contraction's value; no additional opcode is justified by this patch.

RayTrace: no claim that integer conversion addresses its main bottleneck. Its Float-heavy vector arithmetic and object allocation/property accesses need separate evidence. Existing general arithmetic contracts remain adequate; no numeric architecture rewrite is proposed.

Acceptance pending: root agent runs serial ABBA/BAAB paired release measurements, instruction counts and integration six-item check, and records binary-size cost. Mechanism success alone does not establish runtime benefit.

External semantics reference (reviewed 2026-09-29): https://tc39.es/ecma262/multipage/abstract-operations.html#sec-toint32 . ToInt32 truncates and wraps modulo 2^32. Reference API correctness relies on this contract, not on QuickJS implementation internals.

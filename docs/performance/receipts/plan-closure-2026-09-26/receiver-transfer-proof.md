> Historical rejected candidate: this proof concerns receiver transfer in `dcc334aa` / `5e3dedc9`. The delivered `b280ec8b` keeps a guarded receiver copy instead; see `final-correctness-review.md` and the main receipt.

# Method receiver single-owner transfer candidate

Source: `receiver-transfer-source`, based on `4ef28f74`. This note describes the candidate before compilation or measurement.

## Ownership and failure boundary

The caller stack owns one direct `JsValue` receiver edge. The checked ordinary operand witness is created after reading receiver, arguments and callee, then moves only through the private no-write, no-callback ordinary entry chain. The witness does not itself prove arbitrary future slot contents or heap liveness.

`OrdinaryCall::install` completes call-storage reservation, resume-PC check, captured-local flag preparation and `FrameStore::prepare_push` before entering `SlotStore::push_ordinary_frame`. The latter checks parent/window identity and layout, reserves slot and window capacity, copies any needed argument owner edges, and initializes named locals. An initial-local failure now releases every unpublished parameter/local binding; caller slots remain untouched. Its argument-copy failure already rolls back the staged suffix.

Only after those checks does `push_ordinary_frame` take the receiver from the caller slot and put the same edge into `CallInput`. It moves outgoing arguments, releases the authenticated direct-object callee using the infallible internal release primitive, updates window metadata, then returns the child window and `CallInput` together. The caller installs both into its cold frame without another fallible call. `CallInput::Drop` eventually releases the receiver edge. The `this` value and its normalization path are unchanged.

This candidate assumes the VM's existing invariant that the live caller slot owns a valid receiver handle. The previous extra retain could diagnose a deliberately corrupted internal handle; the new path transfers it. Neither the ordinary operand witness nor this change makes invalid internal handles a supported JavaScript state.

## Focused checks to run after the coordinated measurement window

- `method_receiver_owner_moves_to_call_input_until_frame_teardown`: one receiver strong edge before and after installation, alive while `CallInput` exists, released after drop.
- `failed_method_argument_copy_keeps_receiver_and_rolls_back_suffix`: a later retain failure leaves the caller receiver and argument owners in place and clears staged copies.
- `failed_named_local_retain_clears_staged_parameter_and_earlier_local`: a test-only stale function handle forces the named-local retain to fail after parameter copying; the unpublished parameter/local suffix is cleared while caller owners remain.
- `method_receiver_survives_nested_return_and_caught_throw`: nested method calls, caught exception, original `this`, and returned value.
- `method_receiver_general_and_native_fallback_stay_callable`: non-ordinary method callees still use general/native paths.
- Existing `scalar_elision_preserves_arity_but_reference_originals_survive_parameter_writes`, `ordinary_retain_failure_keeps_the_entire_parent_and_rolls_back_suffix`, `checked_ordinary_operands_reject_depth_change_without_moving_owners`, and `checked_ordinary_operands_preserve_method_domain_error_order` cover adjacent invariants.

External fixed probes are in `receiver-probes/`; no engine has run them yet. A/B must compare the candidate against the same `4ef28f74` product base and check semantic outputs before interpreting measurements.

The profiling-only logical storage ledger records one callee clear for either method shape and one additional owner move for a method receiver. This counts the changed ownership operation without treating the receiver as released.

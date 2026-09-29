# Operand push: independent mechanism and correctness receipt

## Identity and scope

Independent candidate commit: `e635b49d70cfd0191b0e6954f40ef0190c595d3c`.
Baseline: `7723e82579b27b94fd5d8b1430d927ff7e6513b2`.
Clean source worktree: `/tmp/oxide-stack-push`.
Branch: `perf/operand-push-cold-errors`.
Only modified source: `src/engine/vm/stack.rs` (16 additions, 6 deletions).
No test, baseline, opcode, frame contract or profiling counter was changed.

The goal is lower complete fixed-work execution time for the requested V8 workloads.
This receipt establishes source-level correctness reasoning and generated-code mechanism;
root's independent paired measurements determine whether the candidate should be adopted.
It does not claim a time, instruction-count, memory-peak or tail-latency improvement.

## Profile evidence and candidate selection

The existing Crypto cycles sample attributes 6.56% self cycles to FrameSlots::push,
6.15% to FrameCursor::commit_push, and 2.46% to SlotStore::operand_push_index.
These sampled percentages identify a direction; they are not removable runtime estimates.
The two push wrappers are separate generated entry points, not a nested call chain.
The 12,534,392 binary_number_in_place events do not go through push and are not used
as this candidate's execution coverage count.

FrameTransaction already authenticates the active frame/window once. FrameSlots::push
uses push_current and does not repeat check_current. The opportunity is therefore not
removing frame authentication. The old operand_push_index contains two inline
Error::internal construction paths (allocation, construction, exceptional cleanup).
Although marked #[inline], its real release body remains out of line and prevents
sharing the slots[index] bounds fact with the following install_operand.

## Mechanism

Move the two error constructors into private #[cold] #[inline(never)] functions,
following the existing operand_stack_underflow and operand_slot_not_a_value pattern.
The original #[inline] on operand_push_index is retained; no inline(always) was added.
The compiler can then inline the short successful check into installation.

This is local implementation organization using the existing borrowing contract.
No unchecked memory access, stale identity shortcut, capacity proof object,
new result type, persistent metadata or bytecode specialization was introduced.

## Contract and error-order audit

The exact operational order remains:

1. Compare window.depth against window.operands().len().
2. On failure construct Error::internal with the exact original message:
   `owned operand stack exceeds verified capacity`.
3. Compute window.operands().start + window.depth.
4. Index self.slots[index], retaining Rust's bounds check.
5. Reject an occupied slot with the exact original Error::internal message:
   `owned operand push would replace a live value`.
6. Return the validated index; install_operand stores the new direct value and
   increments depth using the existing code.

The successful path makes no callback or observable release between the slot check
and installation. The short exclusive store/window borrow protects the fact.
The compiler merging redundant bounds checks does not remove the source's capacity
or vacancy validation. It retains one actual bounds comparison and its panic branch.

push_pending_current still obtains the index before value.take(); a failed push leaves
its pending owner with the caller. push_owned still checks the window and destination
before replacing the caller's value with Undefined. Frame authentication, full window
identity, stack-underflow behavior and the existing fault/resume publication remain
unchanged. Ordinary push's existing ownership contract is unchanged.

The review re-read the complete 7723e825..e635b49d diff after building: only the two
error expressions moved. No test passed because a check, input, error, or operation
ordering was deleted or weakened. Error allocation now executes in a separate function
on the same rejection branch. No public error value or error message changes.

## Actual release machine code

Both binaries are Rust 1.88.0 plain release, fat LTO, codegen-units=1, no profiling.

| Symbol | Baseline bytes | Candidate bytes |
| --- | ---: | ---: |
| FrameCursor::commit_push | 100 | 99 |
| FrameSlots::push | 100 | 99 |
| SlotStore::operand_push_index | 478 | no standalone symbol |
| operand_stack_capacity_exceeded | absent | 232 |
| operand_push_replaces_live_value | absent | 232 |

Baseline each push wrapper saves/restores r15, r14, r12 and rbx, calls
operand_push_index, branches on Result, and performs an additional Vec bounds check
before installation. operand_push_index has already checked the same Vec index.

Candidate each successful wrapper has no call instruction and no register save/restore
prologue. It performs the original capacity check, one Vec bounds check, the vacancy
check, payload store and depth update. Failure branches transfer to the cold error
constructors; the bounds panic branch remains. operand_push_index no longer exists as
a standalone symbol. This confirms the intended mechanism without changing inline
annotations further.

Assembly files (original full functions):
`/tmp/oxide-stack-asm/baseline-FrameCursor-commit_push.asm`
`/tmp/oxide-stack-asm/baseline-FrameSlots-push.asm`
`/tmp/oxide-stack-asm/baseline-SlotStore-operand_push_index.asm`
`/tmp/oxide-stack-asm/candidate-FrameCursor-commit_push.asm`
`/tmp/oxide-stack-asm/candidate-FrameSlots-push.asm`
`/tmp/oxide-stack-asm/candidate-SlotStore-operand_stack_capacity_exceeded.asm`
`/tmp/oxide-stack-asm/candidate-SlotStore-operand_push_replaces_live_value.asm`

Full unstripped file bytes: baseline 9,414,256; candidate 9,414,440; difference +184.
ELF `.text` section: baseline 6,292,233 bytes; candidate 6,292,633 bytes; difference +400.
These file sizes include ELF metadata and do not by themselves measure executed-code
size or instruction-cache effects. Code layout changes remain a possible cost requiring
measurement; mechanism success alone does not establish a performance benefit.

## Build and targeted correctness validation

Commands run from `/tmp/oxide-stack-push`:

```sh
RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/benchmark/build.py \
  --plain-only --plain-target /home/eric/.cache/oxide-six-build-stack --jobs 2
cargo +1.88.0 test --locked --lib --jobs 2 \
  --target-dir /home/eric/.cache/oxide-six-test-stack engine::vm::stack
cargo +1.88.0 fmt --all -- --check
```

Build succeeded from clean commit e635b49d, 1m32s reported by Cargo.
Targeted test result: 47 passed, 0 failed, 0 ignored, 0 measured,
1,919 filtered out; test execution 0.23s. fmt passed.
This is a targeted result, not a claim that the full workspace or Test262 was rerun
for this isolated candidate.

The tests include:
- failed_capacity_and_shape_checks_do_not_change_live_windows;
- primitive_transaction_pending_owners_survive_failed_commits;
- frame_transaction_keeps_owners_across_gc_and_partial_output_failure;
- linked_owning_read_pending_getter_is_selected_once_without_consuming_base;
- dense_read_leaf_preserves_declined_inputs_and_neighboring_operands.

The test files are unchanged. No allocator-failure injection or new independent
malformed occupied-slot test was added; occupied-slot guard/error equivalence is
established by the source diff and retained generated branch, not claimed from such a test.

Test log: `/tmp/oxide-six-test-stack.log`.
Build runner log: `/tmp/oxide-six-build-stack.log`.
Binary: `/home/eric/.cache/oxide-six-build-stack/release/qjs`.
Binary SHA256: `ee5ee8a9556994da0c48da696ffe9afe4ea37a9430af075949b2ac2266ffb81f`.
Build receipt: `/home/eric/.cache/oxide-six-build-stack/release/qjs.build.json`.

After the above validation all agent build/test workloads stopped. The root agent runs
the separate paired timing; no agent-side benchmark, perf measurement, or additional
test was run during that measurement window.

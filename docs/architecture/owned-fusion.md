# Owned execution spans

S08's finite fusion is an immutable execution projection, not a new serialized
opcode set. `FunctionBytecodeData` owns the plan, and rooted execution snapshots
share it. The verified instruction array remains canonical, including all source
PCs, branch targets, handler addresses, resume addresses and legacy opcodes.
Publishing scans the existing instruction control contract once. A function with
no candidate has no plan allocation. A candidate-bearing function stores one byte
per canonical PC, with zero at ineligible PCs. Only span starts are tagged.

Supported spans:

| Span | Canonical instructions | Logical weight |
| --- | --- | --- |
| UpdateLocal prefix | GetLocal[/Check], Inc/Dec, SetLocal[/Check] | 3 |
| UpdateLocal postfix | GetLocal[/Check], PostInc/PostDec, PutLocal[/Check] | 3 |
| UpdateLocal discard | GetLocal[/Check], Inc/Dec, PutLocal[/Check] | 3 |
| UpdateLocal with discarded result | Prefix/postfix span followed by Drop | 4 |
| CompareBranch | Lt/Lte/Gt/Gte/Eq/Neq/StrictEq/StrictNeq, IfTrue/IfFalse | 2 |
| Primitive AddStore | Add, PutLocal[/Check] | 2 |
| Primitive AddStore with discarded result | Add, SetLocal[/Check], Drop | 3 |

Any branch, catch or gosub target inside a proposed span rejects it. The next PC
after a control boundary is also an entry, covering structured gosub and
suspension resumption. Span patterns contain no explicit call, handler operation or
suspension. AddStore has a primitive operand guard and executes in the cold completion path. A target at a trailing Drop leaves that Drop outside the span.
Original instructions remain executable at all canonical addresses.

UpdateLocal requires a normal non-const local definition and a live Direct
Number binding. Captured, uninitialized, BigInt and coercible values retain the
original path. No result is changed before the Number guard succeeds. The same
`Number::update` kernel preserves overflow, signed zero, NaN and infinity; postfix
returns the original Number. Number owners require no retain, release drain or
GC. CompareBranch requires two Numbers and applies the original comparison before
testing branch polarity; negating `<` never becomes `>=`.

AddStore consumes operands already read at their canonical sites. Both must be
primitive, and the destination must still be a Direct initialized normal mutable
local; otherwise the unchanged conversion/store path runs. Shared primitive
addition executes after the Add PC is published and RunSlots has ended. A throw
leaves the binding unchanged. On success the store PC is published before the
replacement and last-owner release. A trailing Drop only removes the redundant
assignment-result copy; its local owner remains live. This is a cold completion
optimization and does not move String/BigInt allocation into the continuous run
borrow. Captured, TDZ and const stores are excluded.

## Current run PC representation

The current S08/S09 candidate keeps resume PC local and writes fault PC directly
to Frame at each actual dispatch/span entry. A Drop guard materializes resume
when run returns normally, propagates an error, takes a cold exit, suspends, or
unwinds in Rust. Runtime active-frame PC publication stays in the driver at its
existing observation boundaries; direct Frame fault writes do not imply an
additional Runtime publication.

UpdateLocal and CompareBranch retain their span-entry fault sites and existing
resume targets. AddStore still publishes its separate addition/store sites in
the surrounding completion path. Canonical source/debug tables are unchanged.
Only one Frame PC is deferred, so documentation must not describe both Frame
fields as written only at run exit. Async CPU sampling still has no arbitrary-
instant exact JavaScript PC guarantee. This representation was selected from
ordinary A/B measurements; source-level write counts alone did not predict
throughput, and final full-matrix acceptance remains pending.

## Observation points

| Observation | Span behavior |
| --- | --- |
| JS conversion, call or catchable throw | Number spans fall back before mutation. AddStore can throw during shared primitive addition at the published Add PC; object conversion falls back before consuming inputs. |
| Host call, allocation, GC or release drain | Number spans cannot perform these operations. AddStore allocates only outside RunSlots and publishes the store PC before replacing/releasing the old binding. |
| Return, yield, await, handler entry or resume | Outside spans; canonical PCs and existing driver publication remain authoritative. |
| Backtrace and source location | Canonical source tables are unchanged. Number spans cannot throw internally; AddStore distinguishes addition from binding-release PCs. |
| Logical instruction profiling | Counts every canonical operation and its original intermediate stack depth. |
| Fuel, interrupt and single-step hooks | The current owned engine exposes none. Introducing such a hook must disable spans or first prove its budget covers the full logical weight; it must fall back canonically when observation falls inside a span. |
| Asynchronous CPU samples | Resume remains local during run; direct Frame fault writes and driver Runtime publication do not promise an exact JavaScript PC at arbitrary sampling instants. |

For isolated A/B exports, replace `FusionPlan::update` with `None` to disable only
UpdateLocal, or `FusionPlan::compare_branch` with `false` to disable only
CompareBranch; replace `FusionPlan::add_store` with `false` to disable only the
cold AddStore completion. Preserve these changes only in immutable experiment trees, record
source hashes and binary identity, and retain canonical correctness checks. No
experimental feature flag belongs in production. Compile profiling measures
plan construction as `Fusion`; there is no additional relocation pass.

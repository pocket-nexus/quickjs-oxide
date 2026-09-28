# Workspace architecture

This document describes the current implementation and its responsibility
boundaries, including the numeric operations on PR #53.
PR #52 (`996663f771afdabdc69d52c94bd4d2fb392e27b1`) is the earlier
execution baseline. [The optimization roadmap](performance/roadmap.md)
distinguishes delivered numeric operations from proposed generalizations.
[Primitive VM results](primitive-vm.md) retain earlier measurement provenance.

## Packages and module owners

One root package, `quickjs-oxide`, contains the complete interpreter.
`src/lib.rs` is its only top-level Rust source file.

```text
src/
  lib.rs
  source/                 exact source bytes, positions and Unicode support
  regexp/                 pattern compilation, programs, matching and interruption
  engine/
    compiler/             lexing, parsing, scopes, resolution and lowering
    code/                 instructions, drafts, verification and publication
    value/                JS values, strings, numbers and runtime conversions
    object/               properties, shapes and object internal methods
    atom/                 interned names, symbols and property keys
    heap/                 raw records, storage, retention, tracing and collection
    vm/                   frames, calls, dispatch, exceptions and suspension
    realm/                global bindings, prototypes and initialization
    builtins/             language builtin algorithms and native call dispatch
    modules/              module instances, loading, linking and evaluation
    jobs/                 queued computations, retained roots and cleanup
    host/                 environment capability contracts
    api/                  Runtime/Context and embedding operations
```

This is a map of current owners, not a requirement to retain every internal
file or interface. Future execution changes are described in the
[roadmap](performance/roadmap.md). Shared use alone does not require a separate
crate, service trait or forwarding layer.

| Package directory | Production workspace dependencies |
| --- | --- |
| Repository root: quickjs-oxide | None |
| adapters/native: quickjs-oxide-host | quickjs-oxide |
| adapters/web: quickjs-oxide-web-host | quickjs-oxide |
| apps/cli | quickjs-oxide, native adapter |
| apps/web | quickjs-oxide, web adapter |
| conformance/test262 | quickjs-oxide with test262-host, native adapter |

Applications construct `Runtime::new_with_host_services(provider)`. Adapters
implement environment capabilities such as clocks, timezone, random seed and
output; they do not implement JS coercion or execution policy. Applications
own inputs, module-loading policy, diagnostics and job driving. The engine
has no production dependency on its adapters, applications or conformance
runner.

## Current compilation and execution

The compiler produces linear `FunctionIr/IrOp` and then stack instructions.
Shared operation and source-site data now lives in `compiler/model/ir.rs`,
lexical identities and declaration-order indexes in `model/scope.rs`, and
binding storage/declaration records in `model/bindings.rs`. Resolution and
lowering consume those owners explicitly. Completed function artifacts live
in `model/ir/function.rs`. `parser/context.rs` owns the lexer cursor, grammar
context and temporary per-function reference/control state. During parsing,
`parser/builder.rs` owns each function's single IR allocation and emission
state; its consuming `finish` rejects unclosed scopes or controls before
moving the artifact into `FunctionTree`. Temporary state is then dropped,
while the completed body-boundary fact remains available to suspension
metadata validation. Grammar methods live in the corresponding parser modules;
the existing class, destructuring and resolution algorithms remain in use.
`compiler/relocation.rs` owns fragment insertion, prefix relocation and the
lowered instruction offset map. `flow.rs` exposes structural block entries
for local rewrites and builds a temporary basic-block use/effect graph for the
numeric operations. Its flat input arena and stack of producer IDs are discarded
after selection. The graph's potential effects describe possible calls,
allocation and throws; it does not provide general ownership-release or
layout-invalidation proofs. Required normal/exception/resume stack facts remain with
the code verifier. `optimize.rs` owns the bounded constant-branch
rewrite and its ordered QuickJS late-throw source-site projection. It runs the
projection before rewriting and preserves physical instruction slots.
Profiling builds expose scoped compile, execution, site and call counters
through `api/profiling/cost.rs`; default builds contain no instrumentation
hooks.

`Instruction` is temporary compiler IR. Publication validates it, encodes a
single `ExecCode` word stream, and drops the IR. Each instruction starts with a
word containing a 16-bit opcode header and a 16-bit short operand; wide operands
use following 32-bit words.
The header also encodes the verified word width, so dispatch does not recount
extensions on every execution.
The encoder and verifier share the opcode/operand contract. Temporary numeric
plans contain compiler instruction ranges and targets. Publication resolves
their continuations to execution-word offsets, validates entries and operands,
and retains only execution operands; it discards the temporary graph and plan.
`ExecCode` stores
instruction boundaries so control-flow targets, resumed frames and
diagnostics use execution-word offsets. The published function retains the
word stream, constants and metadata; product builds have no second instruction
array or fusion plan. Tests may retain compiler IR for assertions. Most
specializations still recognize bounded instruction patterns. Numeric regions select
operations from temporary use/effect facts and limited exact shapes after lowering;
the broader binding,
ownership and effect planner in the roadmap remains proposed.

`vm/execute.rs` is the only instruction loop. `FrameCursor` borrows the frame
window for one short operation at a time, moves or copies owned values, and
keeps fault/resume word positions local. It publishes the fault PC before
observable release. Cursor drop writes both positions back to the frame. The
explicit frame stack and driver handle operations that can call JavaScript,
throw or suspend. The object, conversion and iterator algorithms retain their semantic ownership
in their respective modules; none dispatches compiler `Instruction` values.

Publication selects ordinary, cache-aware, numeric span and numeric-region opcodes. Ordinary
local and argument reads enter directly, with no specialization probe. A
numeric span hit skips its following generic words; a failed guard executes
the same position's ordinary read and then those generic words. The verifier
rejects any span with a control-flow entry into its middle. Field and array
cache misses similarly enter the new execution flow's general handlers.
For discarded-result `sum += array[i] * scale`, `out = array[i] * scale`,
`array[i] += delta`, and numeric element comparison branches, selection can
publish one operation and a validated descriptor. The original words remain
in the same stream for generic fallback. Each selected interval excludes only
overlapping specialization; publication validates sources, block-local lexical
proofs, entry restrictions, continuations and logical stack requirements.
Direct local/argument and numeric constant inputs are supported. The array
update also accepts one planned array-element product as its delta, covering
`x[i] += dt * s[i]` without a new opcode family. Initialized, uncaptured
lexical `let` destinations and `const` sources can qualify when predecessor
intersection proves initialization across reachable ordinary CFG edges.
Unproven exception or resume entries, captured, dynamic and mapped bindings
use ordinary encoding.

At execution, Number guards and a genuine Array's own dense or materialized
Number element admit a read. Local accumulation requires an existing Number;
product assignment can replace an initialized direct undefined, null, Boolean
or Number without observable release. Owned destination values fall back.
Local operations retain a short-lived `FrameSlots` destination through
computation and one write. An array update admits an
existing writable own Number element, computes and writes within one mutable
heap borrow. For an array-product delta, both source values are read before the
target is written, including when the arrays alias. A frozen target or accessor
element misses. A comparison branches
directly without an intermediate Boolean owner. Guard failure runs the
original first read and generic continuation without changing state. This is
a small set of planned operations, not a production adaptive region engine.
The field cache retained by published code is read-only; writes use the
general property operation without an unused write-cache allocation.
The `quicken_same_width` helper is test-only and checks allowed opcode pairs,
operand counts and encoded width. There is no production adaptive quickening
policy or retry/backoff mechanism. Shared words currently use `Rc<[Cell<u32>]>`;
this is not a frozen, portable program image.

### Where execution facts are established

| Boundary | Current responsibility | What it does not establish |
| --- | --- | --- |
| Lowering / `verify_parts` | Validate reachable stack states, control flow and constant references; compute maximum stack usage | Current JS value types, object state, or a complete local-initialization/storage proof for future direct operations |
| Publication / heap allocation | Link atoms and constants, validate function metadata, retain child/constant roots, publish iteratively and clean up failures | Cross-runtime portability of linked identities |
| `ExecCode` encoding / verification | Check headers, operand counts, widths, boundaries, targets, selected span contracts and numeric-region descriptors/entries/continuations | Every proposed binding/ownership/effect guarantee beyond the selected shapes |
| Frame installation / execution entry | Reserve frame storage, authenticate its window, check the resume word boundary | That arbitrary saved values stayed unchanged across re-entry |
| Short execution borrow | Inspect actual binding/value/storage facts and consume scoped access | Validity across callbacks, layout mutation, suspension or observable release |

Removing the earlier standalone verifier removed a redundant historical stage;
it did not eliminate stack validation, metadata checks or `ExecCode::verify`.
A sealed type or immutable words do not themselves eliminate Rust bounds checks.
The encoder still constructs operand descriptions for sizing and emission;
the complete generated instruction specification is proposed work.

`Frame::next_pc` currently decodes from `fault_pc`, and each `execute_frame`
entry checks the resume boundary. Carrying already-known continuations through
trusted internal completion is a roadmap item. Dynamic Number/index/descriptor
and identity checks remain necessary where facts can change.

### Frame ownership and observation

`SlotStore` owns separate original-argument, parameter, local and operand
windows. Its initialized backing is reused; inactive slots contain no owners.
`FrameTransaction` exclusively borrows the store/window, while `FrameSlots`
provides short access. References must end before re-entry or an operation that
invalidates their facts. Suspension moves owners into saved storage; restoration
validates dynamic frame state before execution resumes.

The driver handles semantic continuations on an explicit frame stack. Ordinary
JS callbacks do not recursively start another Rust interpreter; true host
re-entry retains its host-stack budget. Materialization exposes frames and roots
for observation. The current action classification is broad, and ordinary stores
outside the direct Number path can request materialization. This describes #52;
it is not a rule that every helper or scalar replacement must publish a frame.
The roadmap makes release handling depend on the displaced owner and the
operation's actual effects.

Own numeric Array reads currently support dense and materialized data slots,
including frozen data properties. Planned local operations use this read path
with one heap access and commit to an admitted destination. Planned numeric
array updates use one mutable heap borrow for an existing writable own Number
element in either storage mode. Other array writes retain their existing
guards and general path. Read eligibility does not imply writability.
Homogeneous numeric backing and typed cell/shape arenas remain proposals.

## State and semantic boundaries

`Runtime` is defined in `api/runtime.rs`. `RuntimeInner` and `RuntimeState`
belong to `heap/runtime` and own the shared heap/atom domain and cleanup.
Runtime methods live with their behavior: properties in `object`, conversions
in `value`, bindings in `realm`, publication in `code`, execution in `vm`
and builtin algorithms in `builtins`. These are inherent implementations of
one Runtime type, not parallel runtimes.

Heap records retain raw data and reference edges; storage operations maintain
those edges. Language algorithms remain with their semantic owner. Active
rooted values and long-lived heap records must preserve the same retention
and collection model. Execution, suspension and restoration transfer owners
without weakening full generations, Object/Symbol identity or root retention.

The following boundaries apply across internal reorganizations:

- Source and module inputs preserve exact bytes and source positions.
  Compiler constants use the restricted `PrimitiveValue` representation;
  compilation does not create live Object or Symbol roots. Full `Value`
  preserves Object and Symbol identity.
- Drafts reach execution through one publication boundary: it links names,
  retains roots and rolls back failures. Published handles and runtime-owned
  identities cannot be reused across runtimes.
- Pure Number and string helpers remain separate from runtime coercion.
  ToPrimitive, property operations and builtin callbacks may execute JS.
  Borrowed slots, heap views and buffer access must obey their callback and
  mutation boundaries; their validity cannot be inferred from a directory.
- Strings preserve exact UTF-16 code-unit semantics, including lone
  surrogates. Latin1 and UTF-16 storage forms must agree for equality and
  hashing. Unicode algorithms and checked-in generated tables belong to
  `source/unicode`.
- The `regexp` module owns pattern programs and matching. The JS RegExp
  object, property access, coercion and replacement callbacks belong to
  `engine/builtins/regexp`. Source and regexp are engine siblings sharing
  the existing string carrier, not independent Cargo packages.
- Module-host callbacks can reenter through their originating Context.
  JS-valued failures preserve the exact thrown value, including Object and
  Symbol identity. They are not converted to text diagnostics.
- Jobs own pending computation and its retained roots. Applications decide
  when to drain the queue; internal VM restructuring must preserve Promise,
  async and module ordering.

## Public API and internal visibility

Embedders use `engine::api`, the only public engine module.
`src/lib.rs` has no legacy root reexports. Test-support and Test262 hooks
remain explicit opt-in surfaces; detached VM fixtures are unit-test-only.

Use explicit imports from the actual owner and the narrowest visibility
needed by callers. A shared Runtime type does not justify wildcard imports
through its implementation module. New public capabilities belong in
`api`; helpers must not expose a second value system or execution path.

The narrow BC5 bytecode-archive reader and its gates were removed with the
verify/publication simplification: upstream bytecode is a version-bound cache
rather than an interface. Internal compiler and code types do not promise a
stable external bytecode format or an independent compiler product.

## Documentation and verification

Cross-module responsibilities are maintained here; active redesign decisions
belong in the [optimization roadmap](performance/roadmap.md). Local algorithm and
ownership contracts belong beside their Rust types and functions. Add a directory guide only when it provides
useful navigation or operating instructions; there is no per-directory
README requirement.

`scripts/checks/check-source-layout.py` checks source ownership, Rust module
reachability and the public API boundary. It does not inspect documentation.
When source owners change, update the relevant boundary checks and their
negative cases to follow the real production route, not just new filenames
or fingerprints.

Use the [verification entry point](../README.md#verify) and the affected
owners' tests. Frozen oracle and Test262 receipts refer to their recorded
source; a refactor or documentation edit does not renew them. The
[primitive VM results](primitive-vm.md) preserve earlier measurements; the
[#52 rewrite receipt](performance/receipts/vm-rewrite-2026-09-27/README.md) identifies
its own tested source snapshot. Documentation changes renew neither receipt.

# Workspace architecture

This document describes the current implementation and its responsibility
boundaries. The [primitive VM overview](primitive-vm.md) summarizes the
execution-core redesign: the legacy execution path is retired and the
unified `root_call` core — explicit JS frames, one driver and owned domain
continuations — is the only engine.

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
file or interface. The VM plan specifies the new compiler, code and execution
structure. Shared use alone does not require a separate crate, service trait
or forwarding layer.

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
for local rewrites and delegates required normal/exception/resume stack facts
to the existing code verifier. `optimize.rs` owns the bounded constant-branch
rewrite and its ordered QuickJS late-throw source-site projection. It runs the
projection before rewriting and preserves physical instruction slots.
Profiling builds expose scoped compile/legacy-dispatch counters through
`api/profiling/cost.rs`; default builds contain no instrumentation hooks.

Expression intermediates live on an operand stack; numbered locals do not
make this a register VM. The current execution path splits arguments and
locals in `RuntimeVmHost` from the operand stack in `VmActivation`, with
additional active-frame tracking. Ordinary JS calls recursively enter the
Rust interpreter. These are the principal ownership and driving boundaries
that the pending plan replaces.

Code verification and transactional publication already exist. Published
instructions and constant storage already share immutable arrays; the plan
must account for remaining per-call projections and roots rather than treat
sharing as a missing feature. Code representation and publication belong in
`code`; active pc, stack position and call state belong in `vm`.

The redesign keeps stack instructions and the complete language frontend.
It introduces explicit JS frames, one execution driver, domain-owned callback
state and concentrated slot ownership. The final architecture, the delivered
stages and the measured results are recorded in the
[primitive VM overview](primitive-vm.md).

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
and collection model. The VM plan details the ownership transfer required
when execution suspends or resumes.

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
belong in the VM plan. Local algorithm and ownership contracts belong beside
their Rust types and functions. Add a directory guide only when it provides
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
[primitive VM overview](primitive-vm.md) preserves the completed redesign
decisions and results.

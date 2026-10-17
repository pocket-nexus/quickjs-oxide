# Rust embedding API

api is the boundary through which Rust callers use quickjs-oxide. It exports
Runtime, Context, Value, errors and root-protected handles, and organizes the
compile, execute, module, job and diagnostic entry points that hand requests
to the engine's responsibility modules.

heap manages the Runtime's shared storage; compiler, code, VM, object,
builtins and the other semantic modules implement the language algorithms.
Public entry points delegate to those algorithms and keep internal bytecode,
frames and storage layouts private.

Callers inject HostServices explicitly; the application owns input, module
loading policy, output and job driving. The ModuleLoader `normalize`,
`check_attributes` and `load` callbacks receive the requesting Context and
preserve reentrancy and JS thrown-value semantics.

Test262 and differential-testing support sit behind dedicated features and
stay outside the default production API. See
[workspace architecture](../../../docs/architecture.md) for cross-package
responsibilities.

## The API over the new core

`api::core` is the same boundary over `oxide-core` (new-core phase 6,
section 2), built beside the current API until every consumer has moved
(phase 6 M2 and M3) and the current engine is deleted (M4). It keeps the
current API's shapes wherever the new core gives them their meaning: `Value`
is the same enum, with `ObjectRef` and `SymbolRef` owning handles in the
core's handle table; errors stay pending exceptions; keys and descriptors
keep their types. What differs:

- One realm per runtime: `Runtime::context`, `new_context` and `Context::new`
  all return it.
- One host trait, `oxide_core::host::Host`, for output, the clock, the time
  zone, the random seed, files and promise-rejection events.
- Module loaders receive a `Scope`, the callback's view of the runtime, and
  return `ModuleSource` (source with `import.meta`, JSON, JSON5 or a module
  compiled through the scope). Calling the API from inside a callback
  without the scope is `RuntimeError::Reentered`.
- `RuntimeOptions` configures the heap and stack limits; `MemoryReport`
  replaces the heap counts; `GcPolicy` and the reports (`gc_report`,
  `ic_report`, `disassemble`) replace the profiling API.

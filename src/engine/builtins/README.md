# Language builtins

builtins implements the ECMAScript built-in objects and functions, plus the
explicitly enabled qjs helper behavior. Each built-in domain owns its
constructors, prototype methods and algorithms; the native call selector
dispatches calls to those implementations.

A built-in method typically interleaves argument conversion, property access,
storage operations and user callbacks. Conversion uses value, property
semantics use object and JS execution uses the VM. The built-in algorithm
determines the order of these steps and the effects already performed when an
exception occurs midway; generic storage batching does not stand in for it.
Promise reactions, jobs and async driving each keep their own
responsibilities.

Environment capabilities come through the host contract; concrete system and
browser implementations live in `adapters`. heap stores the raw payloads and
reference edges of built-in objects; language behavior stays here.

[Buffers and views](array_buffer/README.md) documents the shared-storage
boundary of the binary builtins. Other small method groups are documented in
source.

`continuation` is the closed registry of native algorithms; it only selects a
domain step and holds no property or conversion algorithms. Each domain's
Step/Resume types store the phase and required roots, and synchronous entry
points and the VM driver share the same algorithm. The VM `call/request`
module adapts typed per-domain requests and replies; explicit child frames
advance callbacks, and callback-free leaf functions run directly.

Synchronous builtins, Promise/generator, module and host/API entry points all
advance through the same owned driver; see
[workspace architecture](../../../docs/architecture.md) for atomicity
requirements.

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

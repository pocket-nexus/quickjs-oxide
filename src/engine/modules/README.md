# JavaScript modules

modules owns ECMAScript module instances and the dependency graph from load
requests through linking and evaluation, maintaining live bindings, module
namespaces, cyclic dependencies and top-level await. Dynamic import and
asynchronous dependency completion run through the same module lifecycle.

Applications supply name normalization, attribute checks and source loading
policy through ModuleLoader. code provides module drafts and published code,
heap stores instance records, the VM runs module code and jobs carry deferred
completion. The module layer decides graph state transitions and error
propagation; file-system and browser loaders live outside it.

Loader callbacks can reenter the engine through the requesting Context.
Request order, visible module identity, failure state and the Object/Symbol
identity of thrown values stay consistent; arbitrary JS exceptions are kept
as values rather than reduced to strings, and completed load phases are not
re-run.

See [workspace architecture](../../../docs/architecture.md) for module
integration with the unified VM driver.

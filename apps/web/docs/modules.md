# Modules

A runtime runs ES modules as well as scripts. The embedder compiles a main module, and a module loader that the embedder writes supplies every module it imports. This page covers both, plus JSON modules, import attributes, `import.meta` and dynamic `import()`.

## Compiling and running a module

`Context::compile_module_with_filename` compiles module source under a name. `Context::execute_module` links the module and what it imports, then evaluates it. The result is a promise that settles when evaluation finishes. Top-level `await` is allowed, so the promise can stay pending until the job queue runs.

<!-- include: examples/modules.rs#run-module -->

The compile methods differ only in their input:

| Method | Input |
| --- | --- |
| `compile_module(source)` | `&str`, named `<input>` |
| `compile_module_bytes(source)` | `&[u8]`, named `<input>` |
| `compile_module_with_filename(source, filename)` | `&str` |
| `compile_module_bytes_with_filename(source, filename)` | `&[u8]` |
| `compile_module_bytes_with_options(source, &options)` | `&[u8]`, named `options.filename` |
| `compile_module_bytes_with_import_meta(source, filename, import_meta)` | `&[u8]`, with initial `import.meta` properties |

Each one returns a `ModuleBytecodeRef`, whose `name()` is the filename. The filename is the module's name: other modules import it under that name, stack traces show it, and relative specifiers in it resolve against its directory. Compiling a second module under the same name replaces the first for later imports.

A module has no completion value. To get results out, have the module store them on `globalThis`, or import it with `import()` from a script and read the namespace object, as shown under [Dynamic import](#dynamic-import).

## Errors

A module fails in one of three places:

- A syntax error makes the compile call return `Err(RuntimeError::Exception(error))` with the `SyntaxError`.
- An import that cannot be loaded or linked makes `execute_module` return `Err(RuntimeError::Exception(error))` with the loader's error.
- An error thrown while the module runs rejects the evaluation promise.

<!-- include: examples/modules.rs#failures -->

<!-- include: examples/modules.rs#missing-import -->

## Module loaders

An `import` of a module the embedder did not compile goes to the runtime's module loader. `Runtime::set_module_loader` installs it. Without a loader, such an import throws `ReferenceError: could not load module filename '…'`.

A loader implements the `ModuleLoader` trait:

- `normalize(scope, referrer, specifier, attributes)` turns a specifier into a module name. `referrer` is the name of the importing module or script. The default follows QuickJS: a specifier starting with `.` resolves against the referrer's directory (`./math.mjs` imported by `lib/main.mjs` is `lib/math.mjs`), and any other specifier is the name unchanged.
- `load(scope, name, attributes)` returns the module with that normalized name as a `ModuleSource`.

Each normalized name is loaded and evaluated once per runtime. Later imports of the same name share the module.

`ModuleSource` has four forms:

| Variant | Module |
| --- | --- |
| `Source { text, import_meta }` | JavaScript source, with initial `import.meta` properties |
| `Json(bytes)` | a JSON module: its default export is the parsed value |
| `Json5(bytes)` | a JSON5 module |
| `Compiled(module)` | a `CompiledModule` that `ModuleScope::compile_module` already compiled |
| `Synthetic { exports }` | a module whose exports are the given `(name, value)` pairs; `default` is the default export |

`ModuleSource::source(text)` builds `Source` with an empty `import.meta`. Without a loader, `Context::define_module(name, exports)` makes a synthetic module importable by `name` ([Rust functions](./functions.md#native-modules)).

This loader serves modules from a map and treats `.json` names and `with { type: "json" }` imports as JSON:

<!-- include: examples/modules.rs#loader -->

Installing it and importing through it:

<!-- include: examples/modules.rs#use-loader -->

Modules compiled through `Context::compile_module*` or returned by a loader stay alive as long as the runtime.

### The ModuleScope

Both loader methods receive a `&mut ModuleScope<'_>`, which gives access to the runtime while the callback runs. With it a loader can:

- create an error to throw with `ModuleScope::new_native_error(kind, message)`;
- create an object with `ModuleScope::new_object()`;
- create Rust functions and host objects with `ModuleScope::new_function` and `ModuleScope::new_host_object`, for a synthetic module's exports;
- compile module source itself with `ModuleScope::compile_module(name, text, import_meta)`, which returns a `CompiledModule` or the `SyntaxError` to throw.

A loader throws by returning `Err(value)`. The value becomes the exception of the `import` that triggered the load, or the rejection reason of the `import()` promise.

During a callback the runtime is already in use. Calling any `Runtime` or `Context` method from inside the callback returns `Err(RuntimeError::Reentered)`; the `ModuleScope` is the only way in. Values made through the scope are ordinary handles and outlive the callback.

This loader overrides `normalize` to accept only `std:` specifiers and compiles each module through the scope:

<!-- include: examples/modules.rs#normalize -->

## Import attributes

`normalize` and `load` both receive the import's attributes as `ModuleImportAttributes`:

- `Absent` when the import has no `with` clause;
- `Present(attributes)` when it has one, possibly empty. Each `ModuleImportAttribute` has a `key` and a `value`, both `JsString`.

`ModuleImportAttributes::effective()` returns `None` for an absent or empty clause and the entries otherwise, matching what QuickJS passes to its loader. The engine does not interpret attributes. The loader decides which keys and values it accepts and what each one means. The `qjs` command, for example, accepts only `type`, loads `type: "json"` as JSON and `type: "json5"` as JSON5, and throws a `TypeError` for any other key.

## import.meta

A module's `import.meta` object starts with the properties its source supplies: `import_meta` in `ModuleSource::Source`, in `ModuleScope::compile_module`, or in `Context::compile_module_bytes_with_import_meta`. Each property is a `(String, Value)` pair. The `qjs` command sets `url` to the module's `file://` URL and `main` to `true` for the main module and `false` for imported ones.

<!-- include: examples/modules.rs#import-meta -->

## Dynamic import

`import(specifier)` works in modules and in scripts. It goes through the same loader. It returns a promise that the job queue settles: with the module's namespace object once it has loaded and evaluated, or with the error the loader or the module threw.

<!-- include: examples/modules.rs#dynamic-import -->

## Choosing script or module

`quickjs_detect_module_bytes(source)` applies QuickJS's heuristic for source that names no goal: it returns `true` when the first token after an optional `#!` line and comments is `export`, or `import` not followed by `.` or `(`. The `qjs` command runs a file as a module when its name ends in `.mjs` or this function returns `true`.

The complete program is [`examples/modules.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/modules.rs); the loader the `qjs` command uses is `FileModuleLoader` in [`apps/cli/src/main.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/apps/cli/src/main.rs).

```sh
cargo run --quiet --example modules
```

## Where to go next

- [Promises and jobs](./jobs.md): run the jobs that settle module evaluation and `import()`.
- [Host services](./host.md): output, clocks and files for the code your modules run.
- [Errors and limits](./limits.md): exceptions, `RuntimeError` and resource limits.

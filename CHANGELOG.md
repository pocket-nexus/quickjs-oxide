# Changelog

Notes for releases after 0.1.1 are on
[GitHub Releases](https://github.com/pocket-nexus/quickjs-oxide/releases),
generated from pull-request titles. Versions follow
[Semantic Versioning](https://semver.org/); before 1.0, minor versions may
change the embedding API.

## 0.1.1

### Fixed

- Deeply nested source code throws a catchable `SyntaxError: stack overflow`
  at depths close to QuickJS's instead of aborting the process.
- `JSON.parse` accepts 10,891 levels of nesting and its reviver 3,630,
  matching QuickJS at top level; deeper input throws a catchable
  `stack overflow`.
- Assigning to an object whose prototype is a module namespace creates the
  property on that object, as in QuickJS, instead of throwing a `TypeError`.

### Performance

- The lexer and parser compile source 1.1× to 1.35× faster.

### Embedding

- `declare_thread_stack` tells the engine the stack size of a host thread, so
  parsing can use the whole stack. The `qjs` command line evaluates on a
  32 MiB thread.

## 0.1.0

The first release of quickjs-oxide, a JavaScript engine in safe Rust that
reproduces the behavior of QuickJS 2026-06-04.

### Engine

- The language and built-ins of QuickJS 2026-06-04, including classes and
  private elements, generators and async functions, Proxy and Reflect,
  BigInt, typed arrays, `SharedArrayBuffer` and `Atomics`, `WeakRef` and
  `FinalizationRegistry`, and RegExp with Unicode sets.
- ECMAScript modules: static and dynamic `import`, top-level `await`, import
  attributes, JSON modules and `import.meta`.
- Test262: 83,304 of the 83,562 variants in QuickJS 2026-06-04's scope pass.
  Every failing variant is one QuickJS also fails, and early errors carry
  QuickJS's exact messages and positions.
- Every crate forbids `unsafe` code; the garbage collector, compiler, VM,
  regular expression engine and Unicode tables are safe Rust.

### Distribution

- The `qjs` command line, with prebuilt binaries for Linux (x86_64 and
  aarch64, static musl), macOS (x86_64 and arm64) and Windows (x64), installable
  with `cargo binstall`.
- The Rust embedding API: `Runtime`, `Context`, host services and module
  loader callbacks.
- WebAssembly packages for browsers and Node.js, and a versioned browser
  playground.

### Known limitations

- The `std` and `os` modules, the REPL, `Worker`, `bjson` serialization,
  `qjsc` and the C API are not yet supported; see
  [compatibility](docs/compatibility.md).
- Deeply nested source code can exhaust the native stack while parsing and
  abort the process instead of raising a catchable `SyntaxError`.

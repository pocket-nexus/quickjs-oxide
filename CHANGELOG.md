# Changelog

All notable changes to quickjs-oxide are recorded here. Versions follow
[Semantic Versioning](https://semver.org/); before 1.0, minor versions may
change the embedding API.

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

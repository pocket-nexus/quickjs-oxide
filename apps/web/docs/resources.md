# Resources

New to Rust or to JavaScript engines? These links are a good place to start.

## Rust

- [The Rust Programming Language](https://doc.rust-lang.org/book/): the official book, from installation to ownership, traits and concurrency.
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/): the language taught through short runnable examples.
- [Learn Rust](https://www.rust-lang.org/learn): the Rust project's index of books, guides and references.
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/): techniques for measuring and improving the speed and memory use of Rust programs.

## JavaScript

- [ECMAScript Language Specification](https://tc39.es/ecma262/): the standard that defines JavaScript, published by TC39.
- [Test262](https://github.com/tc39/test262): the official conformance test suite for ECMAScript; see [JavaScript support](./compatibility.md) for how quickjs-oxide runs it.
- [MDN JavaScript reference](https://developer.mozilla.org/en-US/docs/Web/JavaScript): guides and a reference for every built-in object and statement.

## QuickJS

- [QuickJS](https://bellard.org/quickjs/): Fabrice Bellard's small embeddable JavaScript engine, the engine quickjs-oxide follows.
- [QuickJS documentation](https://bellard.org/quickjs/quickjs.html): the `qjs` command line, the standard library modules and the C API.
- [QuickJS 2026-06-04 source](https://bellard.org/quickjs/quickjs-2026-06-04.tar.xz): the release whose behavior quickjs-oxide matches.
- [QuickJS on GitHub](https://github.com/bellard/quickjs): the source repository.

## Interpreters

- [Crafting Interpreters](https://craftinginterpreters.com/): Robert Nystrom's book that builds a tree-walking interpreter and then a bytecode virtual machine.
- [A Bytecode Virtual Machine](https://craftinginterpreters.com/a-bytecode-virtual-machine.html): the book's second part, on bytecode, the dispatch loop and the value representation.
- [Garbage Collection](https://craftinginterpreters.com/garbage-collection.html): the book's chapter on a mark-sweep collector and finding roots.
- [The Garbage Collection Handbook](https://gchandbook.org/): the standard reference on automatic memory management.
- [Inline caching](https://en.wikipedia.org/wiki/Inline_caching): an overview of the technique behind fast property access.
- [Tracing garbage collection](https://en.wikipedia.org/wiki/Tracing_garbage_collection): an overview of mark-sweep and related collectors.

## Profiling

- [perf](https://perfwiki.github.io/main/): the Linux profiler; `perf stat` counts retired instructions for the project's benchmarks.
- [Valgrind](https://valgrind.org/): the instrumentation framework the tools below run on.
- [Callgrind](https://valgrind.org/docs/manual/cl-manual.html): exact per-function instruction counts, read with `callgrind_annotate`.
- [Massif](https://valgrind.org/docs/manual/ms-manual.html): a heap profiler that shows peak memory and what allocated it.
- [DHAT](https://valgrind.org/docs/manual/dh-manual.html): a heap profiler that counts allocations and how their bytes are used.
- [nm](https://sourceware.org/binutils/docs/binutils/nm.html): lists a binary's symbols with their sizes, used to read the size of the dispatch loop.
- [Profile-guided optimization](https://doc.rust-lang.org/rustc/profile-guided-optimization.html): building a Rust program with a recorded profile, with [llvm-profdata](https://llvm.org/docs/CommandGuide/llvm-profdata.html) to merge the profiles.

The engine's own diagnostics (`qjs --gc-stats`, `--memory-report`, `--ic-stats` and `--disassemble`) are described in the [profiling guide](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/profiling.md); [Optimizations](./optimizations.md) explains what they count.

## This project

- [quickjs-oxide on GitHub](https://github.com/pocket-nexus/quickjs-oxide): the source, the design notes under `docs/` and the release history.
- [Issue tracker](https://github.com/pocket-nexus/quickjs-oxide/issues): bug reports and feature requests.
- [Architecture](./architecture.md) and [Embed quickjs-oxide](./embed.md): how the engine is built and how to use it.

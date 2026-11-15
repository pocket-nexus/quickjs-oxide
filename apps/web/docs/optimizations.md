# Optimizations

quickjs-oxide is an interpreter: it compiles JavaScript to bytecode and runs that bytecode in a loop. Its speed and its memory use come from how the bytecode is shaped, how values and objects are laid out, how property access is cached and how the heap is managed. This page walks through each technique, where it lives in the source and what it buys. The engine's core is the crate `quickjs-oxide-core` in [`core/`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/README.md); the compiler is in the root crate under [`src/engine/compiler`](https://github.com/pocket-nexus/quickjs-oxide/tree/main/src/engine/compiler). Both have no unsafe code.

| Technique | Where | Effect |
| --- | --- | --- |
| Register bytecode | `src/engine/compiler/register`, `core/src/bytecode` | fewer instructions than stack code; operands read in place |
| One dispatch loop | `core/src/vm/interp.rs` | calls and returns without Rust recursion; hot opcodes inline |
| NaN-boxed values | `core/src/value.rs` | every value is one 8-byte word; numbers need no allocation |
| Shapes and validity cells | `core/src/shape.rs` | objects with the same layout share it; a prototype chain is checked with one flag |
| Inline caches | `core/src/vm/ic.rs` | a named property read is a shape compare and a load |
| Array fast paths | `core/src/vm/interp.rs`, `core/src/vm/iter.rs` | element access and `for-of` over arrays allocate nothing |
| Strings and atoms | `core/src/heap/string.rs`, `core/src/heap/atom.rs` | one byte per character for Latin-1 text; property keys compare as integers |
| Typed arenas | `core/src/heap/arena.rs` | records packed in vectors; free slots found from a bitmap |
| The value heap | `core/src/heap/values.rs` | property values and elements in one vector, in size classes |
| Mark-sweep collector | `core/src/heap/gc.rs` | collects only at allocation; memory bounded by a budget |
| Reclaiming run-time code | `core/src/heap/code.rs` | code from `eval` and `new Function` is freed when unreachable |
| Linear-time compilation | `src/engine/compiler` | compile cost grows with the source size, not faster |
| Startup layout | `apps/cli/qjs.order` | the code that every run executes sits together in the binary |

## Register bytecode

The front end parses source into QuickJS-shaped stack code: instructions that push and pop an operand stack. The register lowering ([`src/engine/compiler/register`](https://github.com/pocket-nexus/quickjs-oxide/tree/main/src/engine/compiler/register)) turns each function into register code for the core. It walks the stack code with a symbolic stack in which every stack depth is a fixed register, so the stack depth is the register allocator: register 0 is `this`, then the parameters, the locals and one temporary per depth. Pushes and pops that only move values between the stack and locals disappear, because the instruction that uses a value names its register directly.

An instruction (`Insn`) is 8 bytes: an opcode, three 8-bit register operands and a 32-bit immediate. The immediate carries a constant index, a jump target, a property key or an `i32` literal, so common shapes fold into one instruction: arithmetic with a literal (`AddI`), a comparison and its branch (`JLt`, `JNlt` and their forms against an `i16` literal), and a string concatenation chain (`ConcatN`). The function below

```js
function sum(points) {
  let total = 0;
  for (let i = 0; i < points.length; i++) total += points[i].x;
  return total;
}
```

lowers to this loop (`qjs --disassemble`; `l` registers are locals, `t` temporaries, `p` parameters, `ic` cache sites):

```text
  0007  GetField            t1, p0, .length, ic0
  0008  JNlt                l1, t1, @16
  0009  CheckInit           l0, .total
  0010  CheckInit           l1, .i
  0011  GetElem             t1, p0, l1
  0012  GetField            t1, t1, .x, ic1
  0013  Add                 l0, l0, t1
  0014  Inc                 l1, l1
  0015  Jmp                 @6
```

Each unit is checked once by a validator ([`core/src/bytecode/validate.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/bytecode/validate.rs)) when it is published: every register operand lies inside the frame, every jump target and constant index is in range. The loop then indexes registers with no checks of its own.

## One dispatch loop

`run_at` in [`core/src/vm/interp.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/vm/interp.rs) runs every bytecode function. It is one loop over a `match` on the opcode, with the running frame's code, register base and cache base held in locals.

- **Register windows.** All frames share one register file. A call's window starts at the callee's operands inside the caller's window, so arguments are never copied. The loop takes the running window as a fixed-size array, which lets the compiler drop the bounds check on an 8-bit register operand.
- **Calls stay in the loop.** A call, return or `new` of a bytecode function pushes or pops a 32-byte `Frame` and continues, with no Rust recursion. `Function.prototype.call` and `apply` are rewritten into a direct call of their receiver.
- **Inline fast paths.** Moves, constants, `Int32` and double arithmetic and comparisons, compare-and-branch, cache hits, dense element access, closures' captured variables and calls are handled in the loop body. An overflow or a non-number operand falls to an out-of-line function.
- **Cold code out of line.** Opcodes that no hot path runs (spread, `arguments`, class setup, generators, iteration protocol steps and others) share out-of-line functions such as `rare_op`. The loop is kept under 16,384 bytes of machine code; it is 15,226 bytes.
- **Natives.** Builtins are Rust functions that read their receiver and arguments straight from the register file and call back into JavaScript through a nested loop.
- **Suspension.** Generators and async functions copy their register window into a heap record when they suspend and back when they resume, so a frame that never suspends pays nothing for them.

The release build uses fat link-time optimization with one code-generation unit, so the loop and the functions it calls are optimized together.

## NaN-boxed values

A `Value` ([`core/src/value.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/value.rs)) is a `u64`. Every bit pattern below `0xFFF9_0000_0000_0000` is a double; the rest of the quiet-NaN space holds the other kinds, with the top 16 bits selecting the kind and the low 32 bits carrying the payload: an `Int32`, a boolean or `undefined`, or the index of an object, string, symbol or BigInt record.

Numbers, booleans, `null` and `undefined` therefore need no allocation, and a value copies as a plain integer. `is_number()` is one comparison and `kind()` a branchless table lookup. Payloads are 32-bit arena indices rather than pointers, so a value is the same size on 32-bit and 64-bit targets.

## Shapes and validity cells

An object's property layout is a shape ([`core/src/shape.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/shape.rs)). Shapes form a transition tree: adding a property moves the object to a child shape, and every object built by the same sequence of additions shares the same shapes. The object itself stores only a 20-byte header and its property values. Lookup walks the parent chain for eight properties or fewer and uses a table built on first use beyond that.

An object that has properties deleted or grows past 64 properties switches to dictionary mode: an unshared shape with its own hash table. The global object starts in it.

Prototype chains are guarded by validity cells. A shape caches its prototype's cell; any change to a prototype clears its cell and the cells of every prototype below it. A cached lookup that found a property on a prototype stays valid while the cell's one flag holds, so the cache never walks the chain.

## Inline caches

Each named property access, global access and `new` site in the bytecode has a 16-byte cache entry in the `IcStore` ([`core/src/vm/ic.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/vm/ic.rs)), filled on the first miss:

- **Property reads** (`GetField`) record the receiver's shape, the holder and the slot. An own property hit is one shape compare and a load; a prototype hit adds one test of the chain's validity cell. A string receiver and `length` on arrays and strings have their own entries.
- **Property writes** (`SetField`) record the shape before and after. A write to an existing property is a shape compare and a store; a write that adds a property installs the cached shape when the object has room for the slot.
- **Globals** compare one generation counter, bumped only when a global property is added, removed or reconfigured, so a global read is one load and one compare.
- **Construction** (`New`) learns how many slots instances of a constructor end up with and reserves that many when it allocates the next receiver, so constructors that add properties in `this.x = ...` style do not grow the object.

A site that sees several shapes keeps a row of four entries; past that a read site uses a shared table of 4,096 entries keyed by shape and property. Entries are weak: the collector prunes those that name dead records before their indices are reused. `qjs --ic-stats` reports hit rates per site in a build with the `ic-stats` feature.

## Arrays and iteration

An array keeps its elements as a dense range of values with its length in the object header. `GetElem` and `SetElem` read and write a dense element inline for a non-negative `Int32` index, and typed arrays have their own inline path. Holes are marked with an internal `EMPTY` value.

`for-of`, spread, `Array.from` and the other users of the iteration protocol take a fast path over an unmodified array: when the array has the standard prototype and neither `Array.prototype[Symbol.iterator]` nor the array iterator's `next` differs from the builtin, the loop walks the array by index and allocates no iterator object and no `{ value, done }` results. The walk reads the length live, so it gives the same results as the protocol.

## Strings and atoms

A string ([`core/src/heap/string.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/heap/string.rs)) stores one byte per code unit whenever every unit fits in a byte and two bytes otherwise. A concatenation whose result is 64 code units or longer builds a rope, which is flattened on first use of its contents; a shorter result is copied into a flat string. A `+` chain that the lowering turns into one `ConcatN` joins all its pieces at once instead of building each intermediate string.

Property names are atoms: interned strings found through a hash table that holds 8-byte entries and no second copy of the text. A property key is a 32-bit integer, either an atom, a symbol or an array index below 2³¹ stored directly in the key, so comparing keys is comparing integers. The compiler interns each string of a script once, straight from its Latin-1 bytes when it has them.

## Typed arenas

Every kind of heap record (objects, shapes, strings, symbols, BigInts, validity cells, functions, bytecode records, the internal records of builtins) lives in its own `Arena<T>` ([`core/src/heap/arena.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/heap/arena.rs)): a `Vec<T>` with a side bitmap of live slots and one of marked slots. Allocation takes the lowest free slot from the live bitmap, testing 64 slots per word, so a free slot costs no list entry and reuse is deterministic.

Arenas double while under 32 MiB and grow in 2 MiB steps above that, so slack stays under one step. After a collection, an arena with a long free tail gives the memory back.

## The value heap

Property values, array elements, scope variables and constant tables live in one shared `Vec<Value>`, the value heap ([`core/src/heap/values.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/heap/values.rs)), handed out in ranges. The size classes are the powers of two and three times the powers of two (1, 2, 3, 4, 6, 8, 12, 16, …), so a ten-element array literal takes 12 slots, and a range's size fits in a one-byte code in the object header.

Each class has a free list. Every collection rebuilds the free space from a bitmap of free slots, merging adjacent freed ranges into runs and cutting short runs into the largest classes that fit, so space freed by one size class is reused by the others.

## The collector and its trigger

The collector ([`core/src/heap/gc.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/heap/gc.rs)) is a non-moving mark-sweep. It marks with an explicit stack and the arenas' mark bitmaps, then sweeps every arena. It runs only inside a call that allocates, which takes the heap mutably, so Rust code can never hold a borrowed record across a collection; records never move, so an index stays valid for as long as its record is reachable.

The trigger keeps the heap's reserved bytes within a budget set after each collection:

```text
budget    = max(min_heap, live_bytes × (1 + growth))
threshold = budget − live_bytes
```

Allocations charge their size to a counter, and a collection runs when it reaches the threshold or when growing an arena would take reserved memory past the budget. The defaults are a 1 MiB minimum and growth 2; `RuntimeOptions` sets both, and a heap cap makes an allocation that would cross the cap collect first and throw a catchable out-of-memory error if that frees too little. `qjs --gc-stats` and `qjs --memory-report` show what the collector did and what each arena holds; see [Errors and limits](./limits.md).

## Reclaiming code compiled at run time

Code compiled before the program runs lives for the runtime's lifetime. Code compiled while it runs (`eval`, the `Function` constructors, modules loaded later, and what the embedder compiles with the transient option) lives in the heap ([`core/src/heap/code.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/core/src/heap/code.rs)). The collector frees such a unit, its inline-cache range and the atoms only it names once no function, frame or pending job reaches it, so a program that calls `eval` in a loop or a REPL that compiles each line runs in bounded memory.

Regular expressions are compiled once per distinct source and flags and shared by every `RegExp` object over them; a collection frees programs no live object uses.

## Linear-time compilation

The compiler's passes each walk a function once:

- the register lowering analyzes each function's stack depth, catch regions and join points in one pass and reuses the result for both the function and the children that capture from it;
- the exception-handler table is built in one pass with a stack of open regions, each region knowing its parent;
- name resolution, closure captures and global declarations look names up through indexes;
- debug positions and source locations are assigned in one forward pass over the source.

Compile time and compile memory therefore grow in proportion to the source size, which matters for large bundles and for embedders that compile code at run time.

## Startup layout

The `qjs` binary is linked with a symbol-ordering file ([`apps/cli/qjs.order`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/apps/cli/qjs.order)) that puts first the functions realm setup, a script's compile and the loop's common paths execute. The kernel maps a binary's file pages in 64 KiB windows, so packing those roughly 640 functions together makes fewer windows resident and lowers the memory of short runs. The ordering applies to `x86_64-unknown-linux-gnu` builds.

## Where to go next

- [Performance](./performance.md): how fast quickjs-oxide runs and how it is measured.
- [Architecture](./architecture.md): the crates and the stages a script passes through.
- [Errors and limits](./limits.md): heap caps, stack limits and the collector settings.
- [Execution](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/performance/execution.md) and [the heap](https://github.com/pocket-nexus/quickjs-oxide/blob/main/docs/heap.md): the full design notes behind this page.
- [Resources](./resources.md): reading on interpreters, garbage collection and profiling.

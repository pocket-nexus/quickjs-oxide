# Verus checks for quickjs-oxide-core

The Verus target checks the functional behavior of the URI decoder in both
modes. It also checks the core's value and property-key representations,
collector root and handle operations, and the interpreter's numeric comparison
table. The independent-module proofs also cover mixed Hasher writes, the opcode
catalogue, instruction emission and selected heap, BigInt and codec operations.
[十一个模块的逐项目标与结果](independent.zh-CN.md) · [Verus guide](https://verus-lang.github.io/verus/guide/overview.html) · [全 core 模块清单与验证目标](core-modules.zh-CN.md)

## Run

The tested Linux x86-64 release is [Verus 0.2026.11.22.1ce6a59](https://github.com/verus-lang/verus/releases/tag/release%2F0.2026.11.22.1ce6a59).
Unpack the official binary distribution with its bundled Z3, install its Rust
toolchain, and run from the repository root with Python 3 and GNU patch.

```sh
rustup toolchain install 1.100.0 --profile minimal
python3 dev-support/verus/verify.py --verus /path/to/verus-x86-linux/verus
```

If `verus` is on `PATH`, omit `--verus`. The runner builds the locked core
dependencies using the verifier's Rust toolchain. It copies the checkout's
`core/src` to a temporary directory, inserts the annotations from
`contracts.patch` and `independent.patch`, and runs Verus with `--no-cheating`. Patch context must
match and the patch may only add lines. The production bodies, tag table,
and constant initializers supply the executable code. A failed patch,
type check, proof, or erasure check causes a nonzero exit.

On core revision `be767dae`, the selected target reported `461 verified, 0 errors`
with default features and again with `--all-features` (GC stress and inline-cache statistics).
As negative controls, changing the object tag's table entry to `Kind::String`
and making `RootStack::pop_one` pop twice each failed its own postcondition.
Removing the unordered outcome from a negated comparison failed the opcode
mask proof. Giving unused bytes a nonzero initial mask failed the loop invariant.

The [independent-module results](independent.zh-CN.md) give the scope and outstanding
goals for each project. The selected target also checks whether the 10 inherited
Hasher adapters and their expected primitive calls compile to the same LLVM
definitions on this host. The optional word-arithmetic target requires Kani
0.68.0 on PATH. It checks five fixed-width libcore specifications before Verus
uses them to prove multi-limb addition, subtraction, and both packed instruction
emitters.

```sh
python3 dev-support/verus/verify.py --verus /path/to/verus --word-arithmetic
python3 dev-support/verus/machine.py
python3 dev-support/verus/verify.py --verus /path/to/verus --all-features --independent-bodies /tmp/oxide-verus-modules
```

## URI decoding

For every Rust UTF-16 input slice and either mode, the production `decode`
function returns exactly the UTF-16 sequence or error specified by
`decode_from`. The specification follows percent escapes from left to right,
interprets UTF-8 using integer arithmetic, and implements the two reserved
character policies. For example, `%2f` retains its original spelling in URI
mode and becomes `/` in component mode. Raw UTF-16 code units pass through,
including lone surrogates. An escaped UTF-8 sequence spelling a surrogate is
malformed. The specification also distinguishes missing percent signs, bad
hex digits, and malformed UTF-8 according to the core's error variants.
[Production decoder](../../core/src/builtins/uri.rs) ·
[ECMAScript Decode](https://tc39.es/ecma262/multipage/global-object.html#sec-decode)

The proof follows the actual loops from input to return, including byte
parsing, surrogate-pair output, termination and bounds. The outer loop keeps
the output produced so far plus the decoding of the unread suffix equal to
the result for the complete input. The inner loop checks the UTF-8 payload
after each continuation byte. The checked client `decode_any_slice` derives
the internal index-arithmetic bound from the byte size of a Rust slice.
[Specification and proof clients](uri_proofs.rs) · [Production annotations](contracts.patch)

The round-trip theorem starts with an arbitrary sequence of Unicode scalar
values. A mathematical encoder writes their canonical UTF-8 and escapes
every byte as `%HH`. Calling the production decoder on that text in component
mode returns precisely the UTF-16 representation of the original sequence.
The formal boundary starts after JavaScript argument coercion and ends at the
Rust result, before VM string or exception allocation. It uses Verus's
standard library contracts for Rust operations such as `Vec::push`. The
production URI encoder and legacy `escape`/`unescape` have separate code paths.

Three independent mutations of the decoder each failed verification with
the specification held fixed. Reversing the reserved-character mode broke
the output invariant. Accepting a nonzero overlong UTF-8 sequence broke the
decoder proof. Using the high-surrogate base for the low surrogate broke the
UTF-16 output contract.

## Other coverage

| Code | Property checked |
| --- | --- |
| `Value` | Every 64-bit pattern maps to the documented kind, including the reserved tag's fallback. The table index stays in bounds. Int32, boolean, and heap-ID constructors and accessors agree with the bit layout. All four heap IDs and signed integers round-trip over their entire domains. |
| `PropertyKey` | Every 32-bit pattern decodes as an immediate index, atom, or symbol. Index construction enforces the 31-bit bound. Atom and symbol construction preserve IDs below the 30-bit limit. Conversion to a value preserves the payload. Only atoms and symbols produce a heap root. |
| `RootStack` | Push, pop, indexed reads and writes, marks, truncation, length, and slice access agree with a sequence of values. Restoring a valid mark after pushes restores exactly the original prefix. |
| `Handles` | Create, get, set, and release preserve the specified slot contents. The free list contains each empty slot exactly once. Creation appends or reuses the last free ID. Release preserves other slots. The live count equals the number of slots minus the free count. A checked client releases and reuses a handle while retaining a second live handle. |
| `JUMP_MASKS` | All 256 bytes of the initialized table match a lookup in the opcode catalogue. Each numeric comparison accepts its specified ordering outcomes, including unordered inputs. Short immediate jumps set the immediate flag. Unlisted bytes have zero masks. The initialization loop terminates and stays within the array bounds. |

`Value::kind` uses the standard library's conversion from `bool` to `usize`.
The Verus library leaves that conversion unspecified. The kind contract
explicitly requires that it maps false to zero and true to one. Two Rust
constant evaluations in the verification target check those exact results,
exhausting the input type.

The root-stack contracts require valid indices and marks. Handle mutations
start with a well-formed table, use live IDs for access and release, and store
values other than `Value::EMPTY`. Appending requires the resulting slot count
to fit in a `u32`. The contracts describe completed allocations under the
Verus standard library's `Vec` model. Caller rooting discipline, handle
enumeration, collector reachability and sweep, and JavaScript execution remain
open. The independent-module report specifies the separate machine proofs of
floating-point construction and projection.

## Whole-crate diagnostic

```sh
python3 dev-support/verus/verify.py --verus /path/to/verus-x86-linux/verus --whole-core
python3 dev-support/verus/verify.py --verus /path/to/verus-x86-linux/verus --whole-core --all-features
```

Both commands ask Verus to verify the entire configured crate, including
unannotated code. At `be767dae` both exit 1 during translation, before Verus
can prove the crate. Ten diagnostics reject function pointers, including the
public clock constructor and native function dispatch. Nine reject dynamic
callback types, including bytecode validation and host function bodies.
The Verus frontend explicitly rejects these Rust type forms.
[Verus type translation](https://github.com/verus-lang/verus/blob/1ce6a59bd72713cf486a7660097fe5ac57519617/source/rust_verify/src/rust_to_vir_base.rs)

The table locates all 19 diagnostics on this baseline. They arise before
proof obligations are checked and give no evidence of unsafe execution.

| Diagnostic | Locations in the production source |
| --- | --- |
| Function pointers (10) | [agent](../../core/src/builtins/agent.rs) lines 34, 196, 202 · [atomics](../../core/src/builtins/atomics.rs#L195) · [native dispatch](../../core/src/builtins/mod.rs#L99) · [host clock](../../core/src/host.rs) lines 31, 42 · [runtime compiler](../../core/src/runtime.rs#L738) · [module host](../../core/src/vm/module.rs#L205) · [VM services](../../core/src/vm/mod.rs#L284) |
| Dynamic callbacks (9) | [string captures](../../core/src/builtins/string.rs#L790) · [disassembler](../../core/src/bytecode/disasm.rs) lines 43, 94 · [bytecode validator](../../core/src/bytecode/validate.rs#L75) · [embedder](../../core/src/embed.rs) lines 116, 351 · [VM compiler](../../core/src/vm/compile.rs#L110) · [host body](../../core/src/vm/native.rs#L94) · [waiter](../../core/src/vm/waiter.rs#L43) |

## Open findings

An explicit free leaves the old record's mark in its arena slot. A new record
allocated there survives an immediate sweep. The full collector clears marks
before it visits roots. The [module report](independent.zh-CN.md) records the
reproduction, the two acceptance cases for a fix, and the remaining behavioral
goals in nine of the eleven independent projects. Two bounded Kani checks
timed out, one for a Base64 quantum and one for the date scanner's input
window. Both outcomes remain open. The [core inventory](core-modules.zh-CN.md)
also records the four Wasm modules whose checks require a target build of vstd.

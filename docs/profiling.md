# Profiling and external benchmarks

The optional `profiling` feature implements the memory snapshots, safe partial
allocation trace, lifecycle timing and benchmark workflow proposed in
[the original design report](reports/quickjs-profiling-plan.html). Diagnostics
are off by default. This is an observability baseline, not a CPU/call-stack
sampler or a claim of feature/performance parity with QuickJS.

Historical measurements and validation evidence:
[PocketLab baseline](reports/profiler-baseline.md) and
[CPU hotspot investigation](reports/cpu-hotspots.md). Each report applies to
its recorded source and build; its optimization ordering is not a current
backlog. The [stack VM plan](primitive-vm-plan.md) selects this PR's goals
from [issue #16's post-PR19 investigation](https://github.com/pocket-stack/quickjs-oxide/issues/16#issuecomment-5634660983).
The stack VM migration is in progress; these links do not claim a new benchmark run.

## Build and run

```sh
cargo build --locked --release -p quickjs-oxide-cli --features profiling
./target/release/qjs -d workload.js
./target/release/qjs -T workload.js
./target/release/qjs -q -d
./target/release/qjs -dT --profile-json --profile-output /tmp/new-profile.jsonl workload.js
```

`-d/--dump` requests a snapshot and compile/VM cost collection; `-T/--trace` requests allocation observation.
`-q -d` also takes 100 independent lifecycle samples. `--profile-iterations N`
changes that count (1–10000). Trace holds at most 65536 events by default;
`--profile-events N` changes the limit (0–1000000). Its buffer is allocated once
before runtime creation, never grows while collecting, and counts dropped
events. Dropping the trace handle does not stop the runtime's collector.

Reports go to stderr, preserving script stdout. `--profile-output PATH` creates
a **new** file and refuses existing paths, including the input script. JSON
output is one complete object per line: `oxide-memory-v1`,
`oxide-allocation-trace-v1`, `oxide-compile-vm-cost-v1` when compilation or
execution occurred, and, for `-q -d`, `oxide-lifecycle-v1`. Multiple
records can share the output. Human-readable mode includes accounting notes
and raw lifecycle samples. File-open errors are CLI errors before execution;
subsequent write errors mark the report incomplete on stderr while preserving
the script's result, exit code and original exception.

Snapshots happen before Context destruction, after ordinary pending jobs on
success. Early execution errors still produce an `error-before-context-drop`
snapshot, without advancing jobs. `-q` snapshots are labelled
`initialized-before-context-drop`. Argument/input-file errors before runtime
construction produce no snapshot. The collector never runs getters, Proxy
traps, jobs or an extra GC, and does not retain JavaScript roots. Deferred
releases are observed as they stand, without draining them for a snapshot.
Trace serialization happens after Context and Runtime teardown.

## Data contract and coverage

| Data | Included | Unavailable / interpretation |
| --- | --- | --- |
| Heap population | Object, shape, variable-reference, Context and bytecode node counts; lifecycle states; pending jobs | Logical node counts are not allocation counts or byte totals |
| Owned storage | Arena slots/free indices/zero queue, object property slots, dense array elements, ordinary ArrayBuffer bytes | `used_bytes` measures initialized inline storage and `capacity_bytes` its reserved capacity; nested allocations and allocator headers are excluded |
| Bytecode | Unique instruction slices, deduplicated by their shared storage identity | Rc headers, nested operands, constants and debug data are excluded |
| Static property keys | `bytecode_property_keys`: linked-name count and deduplicated constant-indexed Atom slice bytes, including unused slots | Rc headers and the separate owning references in `auxiliary_atoms` are excluded; no table is allocated for functions without static names |
| Atoms and strings | Live table-backed atom count; immediate integers excluded | Full string storage/counts are unavailable because strings can be shared with atoms, bytecode and embedder values |
| Shared buffers | SharedArrayBuffer wrapper count | Shared backing bytes are unavailable, avoiding duplication across wrappers/contexts/runtimes |
| Allocation events | Actual arena Vec backing-storage allocation, growth and release; stable storage identity; sequence; old/new capacity bytes | `coverage=partial`, `scope=arena-slots-backing-storage`; successful safe Vec capacity transitions only. An `R` does not prove a libc `realloc` call, nor physical relocation |
| Lifecycle | Runtime create, Context create, Context drop, Runtime drop; every raw sample and each phase minimum | Monotonic wall nanoseconds. Excludes process startup and the construction of the host-services value. The sum of independent minima may not correspond to one iteration |

The arena's inline bytes include its record storage. Logical-only node
categories must not be converted to extra inline bytes and added again.
ArrayBuffer views/aliases do not count their backing bytes again; each ordinary
ArrayBuffer owning Vec is counted once. Other object payloads, shape lookup
maps, BigInts, strings, code metadata and host allocations are outside byte
coverage. **The sum of reported categories is not total runtime memory.**
Missing values are `null`, never zero. Total allocator requested/usable bytes,
allocation failures, peak/RSS and cumulative process allocation are explicitly
unavailable. `-T` is not a logical-object trace or a function execution trace.

The safe allocation boundary deliberately preserves `unsafe_code = "forbid"`.
No global allocator replacement is installed. The arena wrapper exposes slice
access, so all its capacity changes pass through its instrumented push, and its
backing allocation is released before the final `F`. A storage ID is scoped to
a runtime and remains stable across growth. Physical addresses are not output.
A trace reports `finished`, `dropped_events`, and `complete_within_scope`; partial
resource coverage remains partial even when no events were dropped. Allocation
requests that abort the process cannot produce a final report.

The Rust API is `Runtime::memory_snapshot()` and
`Runtime::new_with_allocation_trace(host, max_events)`. The latter returns a
runtime and a diagnostic-only `AllocationTrace`. Keep the trace handle, drop
all Context/Value/Runtime handles, then call `trace.snapshot()` for teardown
records. Ordinary runtime construction in a profiling build does not allocate
a trace buffer. Builds without the feature compile out the collector and
reject profiling flags with an explanatory error.

## Compile and VM cost diagnostics

`-d --profile-json` reuses the same CLI and benchmark workload entry. Its
`oxide-compile-vm-cost-v1` record describes the **legacy** execution path:
parse/resolution/lowering attempts and inclusive monotonic wall nanoseconds,
successfully lowered function drafts (including children), final instruction
count and inline typed-code bytes, maximum verified stack, dynamic dispatches,
successful PC publications, and operand depth observed at dispatch boundaries.
Failed parses still count as attempts; lowered drafts are not published-code
or unique-code counts. Inline code bytes exclude boxed operands and metadata.
Phase time includes nested compilation and callbacks, so phase totals are not
additive. Each phase also records the number of storage snapshots and the
largest observed partial owned-Vec capacity: the function arena, operations,
constants, bindings, scopes, local/parameter descriptors, closure variables,
eval environments, and each scope’s binding-index vector. These are boundary
samples (parse completion, resolution entry/exit, lowering entry), excluding
referenced payloads, source, hash tables, and transient worklists. They are not
continuous allocation tracking or a compiler peak-memory total. This instrumented build is not a formal throughput measurement.

The Rust entry is `CostProfile::start()` in `engine::api::profiling`, followed
by `snapshot()`. Collection covers an interval on the calling thread, across
runtimes; it is not a per-Runtime total. The innermost live profile receives
events, and dropping it restores the outer collector, including on unwinding.
The collector owns no Runtime or JavaScript values. Owned-core storage counters
are described below; total call allocations, total retain/release activity and
compiler peak memory remain unavailable.
Without the `profiling` feature, compiler and interpreter hooks are compiled out.

## Disable and measure overhead

Remove `-d/-T` to disable collection. For a binary with the feature compiled out:

```sh
cargo build --locked --release -p quickjs-oxide-cli --no-default-features \
  --target-dir target/plain
./target/plain/release/qjs workload.js
```

Cargo features are additive; avoid other dependencies explicitly enabling
`profiling`. Do not assume a compiled-but-inactive collector is free. The
experiment runner measures four modes (feature absent, compiled inactive,
dump, trace), rotating their order and checking identical stdout. It reports
raw full-process wall times including formatting/I/O, not a fabricated isolated
VM overhead percentage.

## Benchmark without vendoring workloads

Follow [the benchmark tool instructions](../scripts/benchmark/README.md).
The current workflow supports both the pinned QuickJS `tests/microbench.js` and
an **external** checkout of
[ahaoboy/js-engine-benchmark](https://github.com/ahaoboy/js-engine-benchmark).
Only our orchestration, parsers, tests, documentation and result summaries live
in this repository. Third-party benchmark source and generated bundles stay
outside it. No complete QuickJS `std`/`os` module implementation is required.

For this work, run release builds, broader tests, profiler experiments and
benchmarks in the `eric-83am` Herdr PocketLab workspace, under
`/home/eric/Documents/Sources/PocketLab/quickjs-oxide`. The development computer
is limited to minimal checks/tests. Keep timing runs serial and separate from
compilation and correctness tests on PocketLab.

Correctness remains independent of performance. A timeout, unsupported case,
missing score, swallowed benchmark error or partial result cannot become a
zero-time result or contribute to a speed ratio. Keep raw logs to distinguish
those cases, and use unchanged workloads and the same clock/harness on both
engines. QuickJS lifecycle CPU times must not be divided by Oxide wall times.

### 最终码与指令契约

`profiling` feature 下，调用 `CostProfile::capture_disassembly()` 可为该作用域随后成功完成的 lowering 捕获逐函数反汇编。`snapshot().code_disassembly` 按 lowering 完成顺序保存文本；每行包括最终 PC、指令与同一 `InstructionInfo` 的栈状态、控制流、操作数和潜在效果。默认是 `None`，重复启用不清空已有记录。该选项用于诊断，不能用于正式计时；文本不持有 Runtime roots 或原始 IR。

潜在回调/分配效果是通用语义的保守上界，不能据此断言每次 Number 运算都会调用 JS 或分配。可捕获 JS 异常与引擎分配/不变量错误分开；catch、iterator、gosub 和 resume 的动态验证不会被 nominal 栈数量代替。

The non-default `stack-vm` migration configuration adds `owned_instructions`,
`owned_bridge_exits`, and `owned_max_operand_depth` to the same cost snapshot.
An owned instruction is counted after its step commits (a call commits when its
child frame is installed, before the callee returns); a bridge exit is counted
separately and resumes the untouched opcode in the previous VM. CLI reports use
`owned-stack-with-legacy-bridge` when either counter is nonzero. This is partial
coverage, not a claim that a whole sample ran in the new core.

`owned_storage` records successful SlotStore/FrameStore Vec capacity increases,
per-store capacity and frame-depth peaks, slot initialization, reserved/live slot
peaks, logical owner moves, frame/arena cleanup clears, value copies, and narrow
hot releases. Moves include entry/handoff transfers and pop; rotations count the
owners participating, not machine copies. Clears count occupied slots removed by
frame/arena cleanup; a hot release followed by pop is reported separately. Empty
reserved operands contribute to initialization and reserved capacity, not live
slots. Reusing capacity does not count as a new growth event.

`copied_heap_roots` and `hot_heap_root_releases` cover Object/Symbol operations at
the narrow slot boundaries only. Primitive Rc operations, binding/cold-payload
roots, cleanup cascades, window-identity/registry containers and legacy bridge
allocations are excluded. The counters therefore cannot be subtracted to infer
leaks or treated as a total allocation/RC profile. Features compile these hooks
out of ordinary builds; instrumented timings are not formal throughput results.

An S03 debug diagnostic of a 100-iteration addition function returned `4950` and
recorded 1,510 owned instructions, 8 legacy dispatches, 2 slot capacity growths,
2,022 logical slot moves, 604 value copies, and a per-store peak of 6 live slots.
The script/print wrapper uses the bridge; this sample is explicitly mixed. The
separately measured ordinary-call regression requires zero legacy dispatches.


### 字节码调用准备成本

`oxide-compile-vm-cost-v1.call_preparation` 记录成功完成的字节码构帧准备；
函数体随后抛错也保留该事件。新旧执行器共用同一准备入口，计数包含参数与
局部 Vec 的非空 backing allocation、累计实际容量字节、初始化槽数，以及
参数 Value 复制、其中 Object/Symbol root 复制和准备函数中的 callee root
复制。缺少实参的 Undefined padding 计作槽初始化，不计作实参复制；额外
实参保留实际 arity。局部 Vec 显式预留全部定义的容量，填充不再隐含增长。

owned 入口另记录 FrameCold Box 的成功分配、captured-reuse 位标记 Vec
的非空分配和容量，以及进入该帧时独占的原始实参 Vec 容量。最后一项是
**buffer 观察，不是实参分配事件总数**：bound/apply 的中间缓冲区、闭包
快照、原生/旧桥内部容器、暂停 operation 载荷及 allocator 元数据均不在
此字段覆盖内。所有容量为累计观察值，不是同时存活峰值；不能与
`owned_storage` 的逐 arena 峰值相加。root 复制也仅覆盖列明的边界，
不等于全 Runtime retain/release 统计。正式性能仍须使用关闭诊断的构建。

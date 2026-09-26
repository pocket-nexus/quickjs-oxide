# Profiling and external benchmarks

The optional `profiling` feature implements the memory snapshots, safe partial
allocation trace, lifecycle timing and benchmark workflow proposed in the
original design report. Diagnostics are off by default. This is an
observability baseline, not a CPU/call-stack sampler or a claim of
feature/performance parity with QuickJS.

Historical measurement reports are retained locally; each applies to its
recorded source and build. The [primitive VM overview](primitive-vm.md) records
the final architecture and measurement results.

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
`oxide-compile-vm-cost-v1` record describes the owned execution core:
parse/resolution/lowering, blocks, fusion, relocation and publish attempts
and inclusive/exclusive monotonic wall nanoseconds,
successfully lowered function drafts (including children), final instruction
count and inline typed-code bytes, maximum verified stack, and owned instruction
count and operand depth.
Failed parses still count as attempts; lowered drafts are not published-code
or unique-code counts. Inline code bytes exclude boxed operands and metadata.
Inclusive phase time includes nested compilation and callbacks and is not
additive. `exclusive_ns` subtracts measured child phases in the same collector;
uninstrumented work and nested collectors remain charged to their parent.
Publish covers flattening, linking and heap
publication (including the heap boundary's independent checks). Blocks covers
compiler block discovery; fusion covers the owned execution projection.
Relocation covers IR fragment moves and target-bearing lowered instructions,
not the entire lowering/emission loop. Fine-grained diagnostic timers introduce
overhead and must not be compared against ordinary compile-only timings.
Each phase also records the number of storage snapshots and the
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

### 普通调用分段抽样

`bytecode.prepare` 只覆盖通用／owned 参数准备，不能代表普通直接调用。
直接入口按伪随机序列约抽取 1/64 的调用，同一次入口的子阶段共享抽样决定，
重入退出后恢复外层决定。`owned_execution_events["direct_timing.calls"]` 和
`owned_execution_events["direct_timing.sampled_calls"]` 是实际观察到的全量入口数及被选入口数。
`vm_phases` 中带 `.sampled` 后缀的 attempts 和纳秒总量只包含抽中阶段，
不能当作所有调用的总耗时；每阶段最多保留 4096 对原始纳秒样本，
超出部分仍累计 attempts 和时间，并增加 omitted_samples。

| 阶段 | 覆盖范围 |
| --- | --- |
| `direct.prepare.sampled` | 直接入口内的选择、验证和认证；也可能选择 General／Native 或提前失败。 |
| `direct.select.sampled` | 根据当前 callee 选择入口。 |
| `ordinary.validate.sampled` | 普通调用操作数窗口和参数分类。 |
| `ordinary.authenticate.sampled` | 普通 callee 认证，包含缓存命中检查。 |
| `ordinary.install.sampled` | 普通帧安装。 |
| `ordinary.install.slots.sampled` | 安装内部的操作数／局部变量窗口准备，是 install 的子阶段。 |

准备、认证和安装的适用路径不同，attempts 不能直接互作分母。
上述阶段不覆盖进入直接入口之前的派发，也不完整覆盖准备与安装之间的工作；
不得相加后称为“完整调用成本”，不得与 plain 总耗时直接相除得到收益上界。
抽样仍会扰动代码和被抽中调用，用于定位候选，性能准入使用 plain A/A 与 A/B。
`ordinary_install.method/function` 与 `ordinary_install.args0/args1/args2/args3/args4plus`
是全量安装尝试入口事件（包括后续安装失败），分别提供接收者形式和参数数量分布；
不能把它们当作成功安装数。

### 候选跨度与调用点逻辑诊断

`oxide-compile-vm-cost-v1.fusion_diagnostics` 是逻辑事件计数，不采样耗时。
`functions` 仅登记该收集区间**实际进入执行帧**的已发布函数，并不枚举所有
编译或发布的函数。`runtime_id` 与 `bytecode_id`（槽位及发布代数）共同标识
一次 Runtime 内的不可变函数；再加规范字节码 `pc` 才是候选或调用点位置。
这些数字不能跨独立运行直接当作相同函数的 ID。函数清单还复制源码
`function_name`、`filename` 与零基定义行列；剥离 debug 数据、匿名函数或
诊断时暂时无法借用 Runtime 时，对应字段为 `null`。这些文本是独立副本，
不保留 JS 字符串、Atom 或字节码 owner。

| 字段 | 口径 |
| --- | --- |
| `functions[].unfused_read_sites` | 已执行函数中，静态没有任何 fusion flag 的直接 local/argument 读取 PC 数；每个函数登记一次。 |
| `functions[].dense_candidate_sites` / `dense_noncandidate_read_sites` | 直接读取 PC 中发布了 dense span / 未发布 dense span 的静态数量。后者可以仍有其他 fusion 候选。 |
| `dispatch[].visits` / `static_noncandidate_visits` | 每个直接读取 PC 的动态访问次数，及其中静态 fusion flag 为零的访问次数；用于量化查询无候选位置的频率。捕获参数的 `GetArg` 在提前处理分支也计入 dispatch。 |
| `sites[].attempts`, `hits`, `misses` | 每个已发布候选起始 PC **实际进入候选处理器**的尝试、完成和未完成结果；保留的记录满足 `attempts = hits + sum(misses)`。捕获参数的 `GetArg` 在提前处理分支只计 dispatch，不进入 dense 候选，因此不计入这里的 attempts。普通 `guard` 和 dense 的动态失败均回到规范指令起点。`error` 是候选执行时的异常终止，**不表示回退**。 |
| `callsites[]` | 仅覆盖普通驱动器 `enter_selected` 入口观察到的 callee；不是所有 call、construct 或 native 再入口的总账。`callee_identity_changes` 只比较连续的 Object callee 身份，非 Object 会断开连续序列。 |

Dense 失败标签只描述**先前 leaf 失败后、再次只读观察到的首个不满足条件**，
不声称它是唯一原因，也不改变规范执行。`source` 指发布形状或常量不可用；
`binding` 指直接槽不可读；`non_number` 指数值源类型；`index` 指索引不是
非负 Int。数组探针进一步区分 `base_not_object`、`not_array`、
`array_materialized.*`（Array 已转普通属性表示）、`outside_dense_prefix_in_length`
（逻辑 length 内但不在连续 dense 前缀）、`beyond_array_length`、
`dense_non_number` 与读写借用不可用。`room` 指虚拟操作数峰值无法容纳。
诊断探针的观察时间晚于原始失败；若状态不再吻合，则报告
`dense_ready_after_failure` 或较保守的 `commit`/`generation` 等标签。
`array_materialized.*` 在同一次借用内进一步只读查询当前 own slot：
`own_default_number`、`own_nondefault_descriptor`、`own_non_number`、
`own_accessor`、`own_special_slot`、`missing_own_index`。超出 immediate atom
范围或布局不可读分别记为 `index_not_immediate`、`layout_unavailable`，不为诊断
创建 atom。它不调用 getter、不查原型、不扫描整张数组，也不解释整个数组为什么
仍为普通表示。与旧收据的 `array_materialized` 比较时，应汇总此前缀下所有标签；
每次失败仍只记一个标签，不能把聚合值再次加到 attempts。
顺序填充通常保留 dense 前缀，反向从高索引填充会转成普通属性表示；完整的默认
索引集合若在新增索引 0 时符合恢复策略，可以重新转回 dense。
两者不能合并归因为“缓存未命中”。

`owned_execution_events` 还记录数组表示变化：

- `array_storage_dense_materialization` 只在 dense→ordinary 布局提交成功后增加，
  已为 ordinary 的早退和失败事务不计入。后缀 `_gap_write`、`_descriptor_path`、
  `_interior_delete` 区分三个调用位置；descriptor 路径也可能处理跳跃写入，
  不能把它解释成“全部由非默认属性标志导致”。
- `array_storage_dense_recovery_enter` 只计实际调用恢复函数的次数；
  `array_storage_dense_recovery` 计成功恢复。`_reject_*` 是该次检查首先确定的
  拒绝原因：非普通 Array、短 length、槽数上限、槽数不足、索引／命名表分配失败、
  越界索引、非默认 descriptor、非 data 槽、缺失索引或 shape 分配失败。
  `_heap_declined` 保留堆接口 `Ok(None)` 的未细分含义；错误返回不计为普通拒绝。

这些是事件次数，不是对象去重计数，也没有提供转换发生的 VM PC。
恢复只在已接线的新增索引 0 边界尝试：没有 enter 事件不能证明对象不符合恢复条件。
保留的命名属性本身不阻止恢复；单槽 `own_default_number` 也不证明全数组没有孔。

每类 per-PC map 最多记录 16384 个位置，函数清单最多 4096 项，
`omitted` 分别计数超限事件。其中 `omitted.static_functions` 计数函数清单
满额后被拒绝的**登记尝试**；同一未登记函数每次进入帧都可能再次增加，
不能将它解释为不同函数数目。每个调用点只保存最近 callee 身份及最多
四个不同的非 owning ObjectId；`distinct_callees_observed` 在四个以内精确，
`distinct_overflow=true` 后仅是下界。没有任何 callee owner 被诊断保留。
此构建的额外 map、分类借用和 JSON 写入会影响运行时间；正式性能比较
应使用无 `profiling` 特性的 plain 构建及独立 A/B 测量。

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

### 最终码与指令契约

`profiling` feature 下，调用 `CostProfile::capture_disassembly()` 可为该作用域随后成功完成的 lowering 捕获逐函数反汇编。`snapshot().code_disassembly` 按 lowering 完成顺序保存文本；每行包括最终 PC、指令与同一 `InstructionInfo` 的栈状态、控制流、操作数和潜在效果。默认是 `None`，重复启用不清空已有记录。该选项用于诊断，不能用于正式计时；文本不持有 Runtime roots 或原始 IR。

潜在回调/分配效果是通用语义的保守上界，不能据此断言每次 Number 运算都会调用 JS 或分配。可捕获 JS 异常与引擎分配/不变量错误分开；catch、iterator、gosub 和 resume 的动态验证不会被 nominal 栈数量代替。

The same cost snapshot carries `owned_instructions` and
`owned_max_operand_depth`. An owned instruction is counted after its step
commits (a call commits when its child frame is installed, before the callee
returns). Property, conversion, iterator and callback requests stay in the
owned driver; synchronous consumers use the same domain steps. Promise,
generator, module and host/API entries run through the same owned driver as
described in [the primitive VM overview](primitive-vm.md). The counters
describe the measured interval, not every possible path of an intrinsic.
Temporary request/continuation Box allocations and Proxy operation state
storage are outside `call_preparation` coverage.

`run` keeps only the resume PC local; the fault PC is written directly into
Frame at each actual dispatch entry. `owned_execution_events` separates:

| Counter | Current meaning |
| --- | --- |
| `run_frame_fault_pc_write` | Source-level Frame fault assignment at each actual run dispatch entry, including entries that subsequently take a cold/error exit. A fused span does not manufacture writes for its skipped canonical dispatches. |
| `run_frame_resume_pc_write` | Publication of the local resume value when its ProgramCounter guard drops on normal, Result-error, cold, suspension, or Rust-unwind exit. |
| `runtime_pc_publication` | Existing driver publication to the active Runtime frame at observation boundaries; this is not a per-instruction counter. |

These source-level counters are not machine store counts. The resume and
Runtime-publication counters describe observation boundaries rather than
per-instruction stores.

`owned_storage` records SlotStore/FrameStore capacity changes, frame-depth and
slot peaks, logical owner moves, cleanup clears, value copies, and narrow hot
releases. Its slot counts distinguish logical frame extent from initialized
backing storage and allocated capacity:

| Field | Meaning |
| --- | --- |
| `slots_initialized` | Cumulative logical slots reserved by successfully installed frames, including reused slots and unused operand capacity. This compatibility counter does not count physical writes of `None`. |
| `physical_none_initializations` | Cumulative slots first written as `None` when a SlotStore's initialized backing grows. Reusing that backing adds zero. Later owner clears and binding writes are not included. Growth before a failed frame installation still counts. |
| `maximum_initialized_slots` | Largest initialized backing length observed for one SlotStore; includes inactive `None` entries retained after frame return. It can exceed the interval's successful active-extent peak after a failed installation or when collection starts after a larger frame has returned. |
| `maximum_reserved_slots` | Largest successful active frame extent (`active_end`) observed for one SlotStore, including reserved but unused operands. Inactive backing above `active_end` is excluded. |
| `maximum_live_slots` | Largest observed number of occupied binding/value slots in one SlotStore. Unused operands and inactive backing are excluded. |
| `maximum_slot_capacity` | Largest observed SlotStore `Vec::capacity()`, measured in entries. Allocated capacity can exceed initialized length; it is not allocator usable bytes or a process memory peak. |
| `slot_capacity_growths` | Number of observed increases in allocated SlotStore capacity. Reusing capacity or extending initialized length within existing capacity is not a growth event. |

A SlotStore retains its initialized high-water backing until the owning
execution releases it. Every inactive entry is `None`: frame cleanup releases
its owners, and suspension handoff moves them into `FrameStorage`. Retained
backing therefore holds no JavaScript values or roots above `active_end` and
does not consume the logical active-slot budget. Repeated calls can increase
`slots_initialized` while `physical_none_initializations` remains unchanged.
When collection starts after warm-up, a reused push observes the existing
initialized length and allocated capacity without counting earlier physical
initializations or allocation growths.
These are per-store observations within the collection interval, not additive
process-wide peaks or evidence of a throughput improvement.

Moves include entry/handoff transfers and pop; rotations count participating
owners, not machine copies. Clears count occupied slots removed by frame/arena
cleanup; a hot release followed by pop is reported separately. Empty reserved
operands count toward logical initialization and active extent, not live slots.

`copied_heap_roots` and `hot_heap_root_releases` cover Object/Symbol operations at
the narrow slot boundaries only. Primitive Rc operations, binding/cold-payload
roots, cleanup cascades and window-identity/registry containers are excluded.
The counters therefore cannot be subtracted to infer
leaks or treated as a total allocation/RC profile. Features compile these hooks
out of ordinary builds; instrumented timings are not formal throughput results.


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


### 调用临时缓冲区与暂停阶段诊断

owned 普通根调用和普通子调用直接初始化 SlotStore 参数/局部区，因此其
`parameter_buffer_allocations`、`local_buffer_allocations` 为零；原始 argv
独立保留，参数复制与 Undefined padding 仍分别计数。普通未绑定子调用的
`call_outgoing_tail_transferred` 表示原始 argv 从 caller 操作数尾区直接转移，
不是省略参数副本。FrameCold、捕获标记和 unwind regions 只复用清空后的容量，
按最大同时活动帧深度预留；`owned_frame_allocations` 不计复用命中。

`oxide-compile-vm-cost-v1.call_buffers` 按实际生产者分组：
`native.readable`、`bound.raw_snapshot`、`bound.rooted_snapshot`、`bound.merge`、
`apply.indexed`、`arguments.fast_raw/fast_ordering/fast_rooted`、
`function.call_suffix`、`call.boundary_argv`。Array/Arguments 快照也供 spread
使用，因此共享生产者不强行归到 apply 一类。`invoke.argv_carrier` 只观察
已经构造完成的参数容器，不重复计分配或复制。

- `capacity_growths`、`capacity_growth_bytes`、`allocated_capacity_bytes` 只在
  明确的成功 reserve/new-allocation 边界记录；分别是增长次数、净容量增长
  字节和增长后容量字节的累计和。都不是 malloc usable bytes。
- `buffers_observed`、`observed_capacity_bytes` 是缓冲区容量观察；特别是
  `collect<Result<Vec<_>>>` 的内部增长次数不可从最终容量推断，其结果只记录
  observation，不伪称一次分配。失败的 collect 内部部分分配也未覆盖。
- `slots_initialized`、`values_copied`、`values_moved` 是已观察到的初始化、
  Value 复制和 owner 移入数量。初始化包含对应复制/移入，三者不能相加。
  native Undefined padding 只计初始化；indexed apply 的 getter 回复计移入。
- `heap_root_copies` 只计 Object/Symbol root 复制或 raw 到 root 的提升；
  `primitive_rc_copies` 计 String/堆 BigInt 的共享引用复制，短 BigInt 归入
  `immediate_copies`。RawValue 的 ObjectId/Atom 复制单列为
  `raw_heap_edges_copied`，不当作 Runtime root retain；raw 的 String/堆 BigInt
  共享引用另计 `raw_primitive_rc_copies`。

各生产者代表不同真实步骤；bound raw 快照仅共享 Rc slice，记录现有长度
观察，不推断新分配或逐 RawValue 复制；root 提升和合并 argv 才各自发生
列明的 Value 复制。共享容器 Rc 的复制以 `shared_storage_clones` 单列，不计入 primitive Value Rc 字段。`call_preparation` 与 `owned_storage` 仍是另两种部分观察，
与新 map 的统计可能重叠，不能相加作为全调用总数。这里没有全局 allocator
拦截、完整 GC/析构 release 账或所有临时容器覆盖。

`vm_phases` 包含 `bytecode.prepare`、`native.prepare`、`freeze.detach`、
`freeze.owned_export`、`freeze.encode`、`thaw.decode`、`thaw.prepare_owned`。
每项记录 attempts（含错误/展开）、inclusive/exclusive 纳秒，以及前 4096 次
`[inclusive, exclusive]` 原始样本和 omitted_samples。VM 与编译计时共用同一
嵌套时钟；父阶段 exclusive 扣除直接测量子阶段，inclusive 不能相加。
这些阶段是明确的 Rust 操作边界；未计时的 GC/release 工作仍包含在所在阶段中，
不伪称单独 GC 暂停。样本截断后不能把前 4096 次的分位数称为完整调用分布。

暂停探针在事件数不超过上限时可报告完整样本 p50/p95/p99 与最大值。所有诊断
计时和样本保存均只存在于 `profiling` 构建，并有测量开销；正式吞吐和生产
RSS 比较使用关闭 profiling 的独立构建，不与诊断、测试或构建并行。

owned native continuation 现在接收已有独占 argv Vec，沿同一 metadata/realm
验证入口补齐 Undefined padding 后直接交给 NativeArguments，实际 arity
不变；`native.incoming_argv` 只记录该源容器观察。`native.readable` 对此路径
记录 buffer 所拥有 Value 的逻辑转移和实际 padding 增长，不计 argv 复制；
借用式同步入口仍记录新 readable Vec 和真实 Value 复制。两者都保留额外
实参直到 native 完成、抛错或放弃，未把状态等待期间的 owner 提前回收。

Native 调用方的空 argv 容量由 execution-owned pool 回收：`call.native_argv`
记录调用方实际 reserve 的容量增长和从 operand 栈移交的 Value owner；
`call.native_pool` 单独记录空 Vec 容器池元数据的容量增长（元素为 Vec header，
不是 Value）。池按同时存活的 active-frame 深度预留，native readable 中的参数
仅在错误物化、active-frame 退出后释放，再回收空容量。等待中的 native scope
仍独占其完整参数；丢弃 continuation 使用原有 owner 清理，不回收活值。

### Local continuation and recycler allocation producers

The call-buffer ledger additionally observes successful reserves at `query.parents`,
`query.native_scopes`, `query.spare_parents`, `query.free_pool`,
`cold.empty_pool`, `cold.capture_pool`, `cold.region_pool`, `cold.capture_flags`
and `cold.regions`. Reuse and failed reserves add no capacity growth. Recycler
metadata backing and the reusable allocations it points to are separate producers.

`query.pending_box`, `iterator.pending_box` and `cold.frame_box` record each
successful fixed-size Box allocation as capacity 0 → 1, using its payload size.
`executable.published_data_rc` records the lazy immutable metadata allocation
once and each shared snapshot handle clone separately; clones allocate no payload.
Those byte counts exclude allocator metadata and the Rc header. Existing cold-frame
allocation counters overlap `cold.frame_box`; do not add them together.
`native_activation_prepared` counts successful guard publication, not allocations.
These local counters do not constitute global allocator or retain/release totals.

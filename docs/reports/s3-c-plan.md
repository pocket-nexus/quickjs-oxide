# S3-C 实施计划：栈顶缓存、直接存储事务与静态融合

> 状态：设计与待执行计划，2026-09-22。本文不声明阶段 C 已实现或已有性能收益。
> 依据：最新 [performance-architecture.md](performance-architecture.md) §7、§11，
> [阶段 A §8.12 历史实测](s3-a-plan.md#812-第二轮回退修复并行根因确认与实测关闭2026-09-22)
> 与 [§8.13 LTO 双协议对照](s3-a-plan.md#813-窄补丁收尾与-lto-双协议对照2026-09-22)。
> 实现核对快照仍为 `d4f78697`；其后的 C/B 计划及统一协议更新仅涉及文档。
> 现行回退裁决使用 §8.13 的 LTO 列与后续同协议重测，旧无 LTO 热点只作线索。
> 本次同步证据引用与验收入口，不修改 C 的优化顺序、实现范围或接受门槛；
> 不把已有 gc/numeric 改动计为 C 的产出，也不据此宣称前置性能门禁已通过。

## 0. 先定结论与范围

阶段 C 的主线是减少临时操作数在 `SlotStore`、局部变量与 helper 之间的搬运，
保留显式栈执行核心、规范字节码和现有所有权语义。实施顺序为：

**C0 冻结输入与测量 → C1 直接存储事务 → C2 标量单槽 TOS →
C3 带堆边的结果缓存 → C4 有界静态融合 → C6 完整验收。**
**C5 函数指针派发实验是 C4 后的可选支线，可以不启动或以负结果关闭。**

每个子阶段都是独立、可编译、可测试、可删除的交付单元。C2/C3 若未达到
验收门槛，恢复规范栈路径，保留已通过的 C1；C4 仍可独立推进。
不以“缓存写出来了”作为完成，也不为追求覆盖率迁移所有 opcode。

### 0.1 与新架构文档的关系

| 问题 | 本计划的执行规则 |
| --- | --- |
| 总路线 | 遵循 §11：E → 有界残余批 → A4 决策点 → C → B → D。设计与测试准备可先做，正式 C 基线必须记录前三项的结论。 |
| A4 是否必须采用 8B | 不必须。记录“采用 8B”或“保留 16B”的数据裁决后，C 使用已接受的表示。C 不自行改编码。当前 `JsValue` 有 16B 静态断言。 |
| §7 仍写“QuickOp 层融合” | C 在 B 前，当前先用已存在的 `FusionPlan` 与规范 PC 实施；B 以后消费相同认证结果，迁移到 QuickOp。QuickOp 不是 C 的前置依赖。 |
| 是否要重做 dispatch | 单点 `match` 是默认。旧 profile 已排除它是 V8 残余主因，C0 若没有新证据就跳过 C5。 |
| 正式比较用什么构建 | 以 §11 最新验证门禁和 [benchmark README](../../scripts/benchmark/README.md) 为准：双方 fat LTO、CGU=1、无 PGO、无 profiling；所有基线按相同 flags 重建，PGO 双边复核单列。 |
| 能否与 B 并行 | 按 [B 首批计划](s3-b-initial-plan.md) 并行完成只读 QuickOp 译码、验证和独立测试；`run`、`stack` 的接线在 C 接口稳定后顺序进行，发布接线单独集成。 |

### 0.2 成功标准与非目标

成功必须同时有三类证据：规范行为与所有权不变；被点名路径的栈写入、
临时 owner 或指令数确实减少；普通 release 的配对测量达到 §7 门槛。
架构文档的 5–15% 是方向估计，不是承诺，也不是允许其它 case 回退的预算。

本阶段不改变公共 `Value`、BC5 opcode、编译器栈模型、冻结/恢复格式、
GC 模型、deferred release 契约、MSRV 1.88 或 `unsafe_code = "forbid"`。
不引入寄存器 VM、JIT、nightly 尾调用、tracing GC、typed arena、全局
borrow 拆分或新的用户可见执行选项。S2 被撤销的“push 失败改 panic”
与“共享借用下立即 release”不得借 C 重新引入。

## 1. 当前实现地图：复用什么，修改什么

以下是核对源码的事实，区别于后文带“新增”字样的设计。

| 位置 | 已有行为 | C 的动作 |
| --- | --- | --- |
| [value/js_value.rs](../../src/engine/value/js_value.rs) | 16B 内部值，无 `Copy`/`Drop`；堆边须显式 dup/release | 只移动现有值，不改变值类型 |
| [vm/stack.rs](../../src/engine/vm/stack.rs) | `Vec<Option<FrameBinding>>`；窗口保有 args、locals、operands；`depth`、容量、空槽检查 | 缩短 operand→direct local/parameter 的事务路径 |
| [vm/stack/window.rs](../../src/engine/vm/stack/window.rs) | `FrameTransaction` 持有已认证窗口；可反复取得短 `RunSlots` 借用 | TOS 归事务持有，短借用只访问缓存 |
| [vm/stack/number.rs](../../src/engine/vm/stack/number.rs) | 已有消费 Number 对、更新 Number local 的原地事务 | 复用，不重复实现 compare-branch 与 local update |
| [vm/run.rs](../../src/engine/vm/run.rs) | `binary_number`、直接绑定读写、borrowed-base GetField、显式 release/publication 边界 | 接入存储事务、缓存与有界融合 |
| [vm/run/numeric.rs](../../src/engine/vm/run/numeric.rs) | 非 Object primitive 运算在同一事务中驻留；分配/释放发生于短 `RunSlots` 之外 | C3 先接 output→local，保持原 primitive 算法 |
| [code/fusion.rs](../../src/engine/code/fusion.rs) | UpdateLocal、CompareBranch、AddStore、LocalAdd/常量左、method-call span | 认证新候选，保留无融合时无侧表分配的性质 |
| [heap/allocation.rs](../../src/engine/heap/allocation.rs) | 经 metadata/payload 验证和 `bytecode::verify_parts` 后，从确切 code/locals 重建 `FusionPlan` | 新跨度沿同一发布信任边界生成 |
| [code/executable.rs](../../src/engine/code/executable.rs) | 共享不可变 `PublishedFunctionData`，含 code、fusion 与 IC | C 不新增可变执行 IR |
| [vm/run/program_counter.rs](../../src/engine/vm/run/program_counter.rs) | `ProgramCounter` 在正常、错误、unwind 退出时回写 fault/resume；观察前显式发布 | TOS 恢复与其配合，保持规范子 PC |
| [vm/suspend/owned.rs](../../src/engine/vm/suspend/owned.rs)、[vm/suspend.rs](../../src/engine/vm/suspend.rs) | detach/take_frame/freeze/thaw 都消费规范栈 | 进入前缓存必须为空，不增加 TOS 挂起 ABI |

还要单独保留 `run.rs::borrowed_base_field_read`：它直接识别相邻指令，
不属于当前 `FusionPlan`。它已消除基值临时 owner 的 retain/release 往返；
不能为了统一缓存路径重新复制基值，也不能在 B 迁移清单中漏掉它。

当前 `run/fusion.rs::record_span` 明确说明规范派发和融合都没有 instruction
budget。C 记录逻辑指令权重，但不虚构“现有每指令中断检查”，也不新增中断 API。
现有显式帧/宿主递归预算继续由原路径管理。

## 2. 设计不变量与边界协议

### 2.1 事务失败不是一律回滚

必须区分三种结果，建议新内部快路使用明确的 `Completed / Declined / Err`
语义，最终 Rust 类型可复用现有 `Result<Option<T>, Error>`：

1. **Declined**：新快路尚未执行规范效果。PC、栈、目标 binding、owner 数量
   不变；从同一规范指令继续已有实现。
2. **Completed**：预检后完成规定的规范效果，并推进到正确的下一 PC。
3. **Err 或规范 throw**：保留规范路径已经提交的前缀，不能统一恢复到入口。
   例如缺 LHS 的 numeric 路径可能已消费 RHS；postfix 已写入 previous 后，
   第二个输出容量失败必须保留 previous，PC 不推进。

新纯事务在 mutation 前完成窗口、深度、目标、容量、空槽、类型、域及
release readiness 的相关预检。需要 dup 时先成功 dup，再提交目标替换。
预检与提交之间不得插入会调用 JS、变更 binding 或改变 readiness 的操作。
已产生效果的多步跨度按规范子 PC 继续，禁止退回起点重放 getter/coercion。

### 2.2 一个槽位，一个 owner；缓存不减少语义 RC

- move 不 retain、不 release。`PutLocal/PutArg` 消费 top，可把该 owner 直接
  移到 binding；`SetLocal/SetArg` 保留 top，必须保留独立 owner 的 dup。
- displaced binding 仍交给 `release_displaced` 或 `release_outside_slots!`。
  不把释放塞进槽写入或缓存析构，不改变最后一个 owner 的回收时点。
- String、heap BigInt、Object、Symbol 都是 owning edge；`ShortBigInt` 才是
  无堆边值。不得用“primitive”推导“可任意丢弃”。
- cached handle 由事务持有原本那一条强边；借用 binding 的 borrowed-base
  只是临时只读视图，不能跨替换、分配、release 或回调保存借用引用。
- 不引入跨指令的 deferred-release 批处理，不把 release 延到区块末尾。

### 2.3 TOS 的物理布局与状态机

新增单槽缓存放在 `FrameTransaction`，建议实现在
`src/engine/vm/stack/tos.rs`。只缓存一个**逻辑栈顶值**；不是 shadow copy，
不是整个运行帧的新表示。接口名字在此钉定语义，不要求照抄 Rust 签名。

| 状态 | `window.depth` | backing slots | TOS |
| --- | --- | --- | --- |
| Canonical | 逻辑深度 d | 前 d 个 operand 槽均有值 | Empty |
| Cached | 逻辑深度 d，d≥1 | 前 d−1 个有值，第 d−1 个是已预留的 `None` | 拥有第 d−1 个值 |

缓存 Empty 时不得残留 hole。容量上限仍由 `FrameWindow` 的 operand 区间决定；
缓存不提供“额外一格”，不让原本溢出的 push 成功。逻辑栈大小、活 owner 数量
都包括缓存值。

操作规则：

| 操作 | 预检与提交 |
| --- | --- |
| `peek(0)` / `peek(n)` | top 从缓存读，其它按逻辑位置读 backing；深度不足仍返回原错误 |
| push 到空缓存 | 先通过原容量/空槽检查，保留该物理槽为空，值进入 TOS，depth+1 |
| push 到非空缓存 | 先验证新目标槽；成功后把旧 TOS move 到旧 hole，再缓存新值；失败时完全不变 |
| pop | 缓存非空则取走 owner、depth−1；初版不自动把下一 backing 值再提到缓存，避免额外流量 |
| Number binary | 先读取两输入并证明 Number；计算后一次提交，旧 top/hole 清理正确，结果可留 TOS；decline 不动 |
| consume store | 预检目标 direct binding 与源 top 后，top owner move 到目标，返回旧 binding；不绕过语义检查 |
| keep store / Dup | 明确 dup 出独立 owner；完成前任何失败均按现有 pending-owner 纪律清理 |
| `spill_to_backing()` | 只把 TOS move 回已预留 hole，缓存置空；depth、RC、逻辑 live slots 不变 |
| 暂未迁移的 stack 操作 | 先 spill，再调用既有 canonical 实现；初版涵盖复杂 rotate/insert/dup 序列、属性、call 编组 |

`spill_to_backing` 不分配、不借 Runtime、不执行 JS、不 release、无可恢复失败。
它安全的依据是事务独占窗口且 hole 在缓存进入时已认证，而不是取消公共
push 的检查。实现使用安全索引；hole 身份与占用的 debug 检查放在正常入口/
提交前，不在已开始 unwind 的 Drop 内新增 assert/expect 造成双 panic。

`FrameTransaction::Drop` 只做上述纯 move 恢复，覆盖遗漏的 `?` 与 Rust unwind。
它不持有新 `Runtime`、不做 heap 清理，不是新的值 RAII release 包装。
正常观察边界仍显式 spill；Drop 是栈形恢复兜底，不能用来延后 observable release。
在同一操作内部，不允许先取走缓存 owner，再经过可失败步骤，却未把 owner
交给现有 pending carrier；panic 检查也应安排在所有权取走之前。

### 2.4 全部入口和出口必须闭合

| 边界 | 缓存规则 | 代码审计点 |
| --- | --- | --- |
| 同一 run 内纯标量连续指令 | 可保持 Cached | cache-aware 的 `RunSlots` facade |
| 只结束短 `RunSlots` 借用 | 不自动 spill | 否则重新取得 slots 就失去缓存价值 |
| 分配、GC、release、active-PC publication、进入通用 helper | 初版显式 spill 后再结束借用；primitive helper 自有 pending owners 仍按原纪律处理 | `release_outside_slots!`、`resident_property!`、`numeric::complete` |
| 任一 `RunExit`，含 Call、Complete、Materialize | 交回 driver 前 Canonical | 不仅检查 `observes_activation()` 为 true 的出口 |
| Result 错误和 Rust unwind | 事务 Drop 恢复当前已提交栈形，保留正确 fault/resume | 与 `ProgramCounter::Drop` 联合测试 |
| 构造、转换、getter/Proxy、宿主重入、eval | 进入原 driver 前 Canonical | helper/driver 不能读到 hole |
| frame take/pop、尾调用复用、catch/finally 展开 | 操作窗口前 Canonical | `stack/call.rs`、frame_exit、driver |
| Yield/Await、detach、freeze/thaw、恢复失败/放弃 | 不携带缓存；恢复后 Empty | suspend、generator、async 路径 |

给 `FrameTransaction` 增加 Drop 会改变 Rust 的借用结束时机：原来依赖 NLL
提前结束 transaction 借用后再访问 `execution` 的分支，需要显式
`drop(transaction)`。尤其审计 `frame_operations/numeric.rs::try_complete_primitive`
向 `proxy_get_driver::start_numeric` 交接的分支。

`SlotStore::run_window()` 目前可独立返回 `RunSlots`。初版保留其 canonical
入口，或把调用点显式改为 transaction + slots；不得创建临时 transaction
再返回悬空借用。所有 `RunSlots` 构造点和 `FrameTransaction` 直接方法都要
列入清单：只改 run 循环中的 push/pop 不够。`peek` 必须感知 TOS；
`with_local_add_inputs/constant/constant_left` 等未迁移 helper 统一先 canonicalize。

### 2.5 PC、fusion 与后续 B 的共同契约

规范 instruction PC 继续作为异常、源码映射、IC site、挂起恢复、Gosub/Ret
返回地址的唯一外部单位。新 span 保存或从 canonical code 推导：kind、长度、
逻辑栈效果、可失败子 PC、成功后的 resume PC。不要给每个 PC 放宽 descriptor。

新纯 span 必须经发布认证：不能包含内部跳转入口，也不能跨 Call、Eval、
Yield、Await、Throw、Return 或观察边界。沿用 `control_effect().target()`
及 `ends_block()`，测试 Catch、Gosub 和恢复入口。BC5 仍只序列化规范 opcode。

现有 method-call span 是逐步提交的例外：GetField2 已成功，参数再逐个处理，
最后在规范 CallMethod PC 交回 driver。保留这一协议，不强行改成整体回滚。
`complete_local_add` 的错误应继续报告实际 Add PC，不能统一报告跨度起点。

为 B 预留的交接只有认证 facts、handler 语义、逻辑权重及 canonical PC 映射
要求。B 生成 QuickOp 时复用 C 的缓存/所有权测试，并覆盖每个可失败子 PC、
分支与恢复 PC；deopt 不重放已提交前缀。C 不提前建立第二套可变执行 IR。

## 3. 执行阶段与依赖

下列时间是单名熟悉代码的实施者的有效开发估算，不含串行 Test262、测量队列
与回归修复等待。门禁决定能否前进，日期不替代门禁。

| 阶段 | 依赖 | 估算 | 交付物 | 可并行工作 |
| --- | --- | --- | --- | --- |
| C0 | E、残余批、A4 已有可追溯结论 | 1–2 天 | 基线 receipt、矩阵、热点及边界清单 | 补差分测试设计、核对外部 workload |
| C1 | C0 | 2–3 天 | 直接 local/arg 存储事务 | 独立整理 fusion 认证测试 |
| C2 | C1 | 3–5 天 | 单槽标量 TOS、canonical fallback、退出恢复 | 与 B 协商 PC 映射契约 |
| C3 | C2 通过或明确重新打开 spike | 2–4 天 | primitive owning output→store 的缓存路径 | 独立准备 GC/挂起/host 边界测试 |
| C4 | C1；叠加缓存前先确定 C2/C3 取舍 | 2–3 天 | profile 选中的 1–2 种静态融合，或有证据地不新增 | B 只读译码准备 |
| C5 | C4 后新 profile 点名派发 | 1–2 天上限 | 可选函数指针实验结论 | 无需等待它开展 C6 材料整理 |
| C6 | 已接受子阶段全部冻结 | 1–2 天加完整测量 | 最终 receipts、阶段比较、pre-A 累计对照与关闭台账 | 无；重型门禁与计时串行 |

### C0：冻结可比输入，先证明要削的流量

**执行清单**

1. 建立 `target/s3-c/<run-id>/` 证据目录；记录源码身份、现有工作区补丁、
   toolchain、CPU/OS、flags、输入 workload 与二进制 SHA-256。
   `d4f78697` 的 gc/numeric 改动先核对其独立验收结论，禁止悄悄算进 C 收益。
2. 收集 E 后、残余批后及 A4 裁决的 receipt。A4 保留 16B 是合法完成；
   缺证据时标为“C0 前置未满足”，可以继续设计/测试准备，不能声称 C 开始验收。
   旧 A 文档“全部残余关闭后前进”的顺序由新 §11 的有界分配取代；残余逐项归属
   C/B/D，不要求 C 在开始前替其它阶段消灭全部回退。
3. 固定两个阶段比较分母：`saved` 为 E 后同协议保存基线；`previous` 为 C0 或
   上一已接受 C 子阶段。记录各自源码身份；A4 表示若变化，明确各自值表示。
   另保留 `pre_a_release`：从 pre-A `85afd564` 按双方相同 toolchain、target、
   fat LTO/CGU1、无 PGO/无 profiling 重建，记录源码、二进制及输入 receipts。
   每阶段保留相对 previous/saved 的收益与相对 pre-A 的累计回退台账；若 saved
   与 pre-A 为同一源码和构建，可复用产物，但两种比较含义仍须明确区分。
4. 重放 §6 矩阵，重新采样栈流量；旧 LTO-off bigint256 的约 21% 只作线索，
   不直接作为当前收益预算。把分配、释放、native 编组、容器哈希分别归因。
   按 A §8.13 的 LTO 列复查 bigint256（cycles +32.1% / insn +38.8%）、
   typed-index（cycles +9.1%）、prop-delete（cycles +9.7%）及 navier-stokes
   （Score −19.2%）。这些是 C 前的回退证据，不是 C 的收益或已冻结基线。
   只有新证据确认属于栈流量的部分交 C；布局/缓存、arena 及其它成本登记根因
   与残余批/A4/B/D 归属，不把所有 LTO 剩余差距预判为 C 能解决。
5. 在 `profiling` 构建中增加独立诊断：逻辑指令数、实际 dispatch 次数、
   push/pop/replace 次数、候选 span 频率、guard hit/decline、owner retain/release。
   使用已有 profiling 框架；普通构建无计数器写入。
6. 保存当前 `run`、`push_current`、`replace_local_current`、numeric completion
   的热点汇编/指令数。建立“输入→计算→push→pop→store→release”的 owner 图。
7. 审计所有 transaction/RunSlots 构造点、直接 backing 操作、所有 run 出口，
   为每项标记 cache-aware 或 canonical-only；此表是 C2 的接线检查表。
8. 核对历史 58 fixed、67 compile、9 original replay 输入是否可获得。当前
   checkout 缺 README 示例中的 fixed manifest 与 S07 replay receipt，不能
   直接照抄路径。恢复原始文件并验哈希；不能恢复时见 §6.3。
9. 若实施阶段需要对未提交源码做正式测量，先为 `build.py` 增加与 compile
   builder 同类的冻结源码导出/完整 manifest 验证：输出在导出目录外，构建前后
   核验全部文件，不使用祖先 Git 身份，receipt 记录导出/补丁/环境/二进制哈希。
   这是待实现的工具小项，当前 `build.py` 尚无该参数；完成 runner 测试后才准入。
   已有可认证干净快照时无需该扩展，也无需为了本文执行提交。

**关闭条件**：基线可认证，正确性起点无新增失败，热点与接受标准已在计时前
冻结，工作量/时钟/flags 可比，候选排序有原始证据。此阶段不宣称性能改善。

### C1：合并直接存储事务，先减少重复查槽

**修改范围**：新增 `vm/stack/store.rs`，调整 `stack.rs`、`stack/window.rs`、
`run.rs` 的 direct local/parameter store 分支。已有 Number helper 只在确有
重复查槽且 profile 点名时扩充，不重写数值语义。

**执行步骤**

1. 新增 `store_local_from_top` / `store_parameter_from_top` 内部事务；参数明确
   consume 与 keep。预检源 top、目标索引/direct binding、keep 的 dup 条件，
   成功后一步移动/替换，返回 displaced binding。
2. `PutLocal/PutArg` 将 pop→replace 合并；`SetLocal/SetArg` 保留 dup 与 top。
   const、TDZ、captured cell、private binding 的检查和 decline 仍归原 handler。
   先接 local，再接 parameter，避免把 mapped arguments 别名当 direct binding。
3. 原 `SlotReleaseReadiness` 分支选择保持不变。需要物化/release 的路径仍先走
   原观察边界，再提交与释放；不得通过 helper 合并改变 defer 或错误顺序。
4. 记录减少的重复索引、`Option<FrameBinding>` 搬运与临时 `JsValue`，用汇编
   核实是否真正省掉搬运；只因源码行数更少不算通过。
5. 运行 §5 的 store/事务/绑定测试及完整子阶段门禁，再做定向与全矩阵比较。

**验收**：consume 路径不新增 retain；keep 路径仍有独立 owner；失败不提前
替换目标；RC/PC/deferred 结果与规范路径一致；达到 §7 的单项接受规则。
若无可复现收益，保留测量结论，撤销性能改造；测试可独立保留。

### C2：标量 TOS，验证收益与退出协议

**修改范围**：新增 `stack/tos.rs`，调整 `FrameTransaction`、`RunSlots` facade、
`stack/number.rs` 与 `run.rs` 中选定 opcode。测试可拆到专用模块，不继续扩大
已有大文件；遵循 `check-source-layout.py`。

**执行步骤**

1. 按 §2.3 实现 Empty/Cached 状态、spill 和 debug 不变量。初版只允许
   Undefined/Null/Bool/Int/Float/ShortBigInt；heap handle 仍走 canonical storage。
2. 接通 push、peek、pop 与 C1 store。缓存字段持有事务生命周期，不在每次
   `RunSlots` Drop 时刷回。canonical-only facade 入口统一刷回。
3. 首批接 Number binary、简单一元运算、Number 比较与 Bool 分支。已有
   compare-branch/Number local-update 融合优先复用；复杂 shuffle、对象转换、
   call 编组均先 canonicalize。每次扩大覆盖必须由计数器显示有效驻留。
   现有 `binary_number_current` 回调返回 `JsValue`，输入为 Number 不保证
   输出为标量：结果必须再经统一准入；C2 的非标量结果 move 到 canonical
   backing，不能丢 handle 或 decline 后重跑回调。补回调返回 owning 值的低层测试。
4. 实现事务 Drop 的纯 move 恢复，显式补齐 NLL 改变后的 drop 点。检查
   `run_window()` 的独立入口，不引入临时事务借用返回。
5. 对全部 `RunExit`、`?`、panic unwind 注入已缓存 top，测试离开 run 后的
   canonical 栈、PC、depth 与活槽计数。测试 cache-on 与 canonical 两条路径。
6. 新增 profiling 指标：`tos.hit/miss/spill`、按原因拆分 spill、缓存提交数、
   backing 实际读写量。区分逻辑 owner move 与物理 spill，不改旧指标含义。
7. 使用同协议、同源码的两个隔离实验构建比较缓存开关。开关只用于内部测试/
   profiling 或本地实验，不新增公共 API，也不使用 profiling 二进制做正式计时。
   正式接受前再验证最终默认构建，避免开关分支污染结论。

**验收**：状态机/边界全覆盖；原容量失败仍失败；标量目标 workload 有可测的
backing 流量与指令减少，普通 release 达标。汇编若把 TOS 全部 spill 到宿主栈、
或 facade 分支成本抵消收益，最多做两轮有证据的小调整后关闭 spike。

### C3：让带堆边的 primitive 结果直达目标槽

本阶段只在 C2 的所有权恢复协议已通过后接 owning 值。其优先目标是长 BigInt
等运算的 **output→operand push→operand pop→local replacement** 往返，
不重新实验阶段 A 已证伪的 BigInt lease/唯一 arena 节点复用。

**执行步骤**

1. 把缓存准入扩到完整 `JsValue`，move 进缓存沿用那一条强边。先测
   String/heap BigInt，再用 Object/Symbol 做所有权压力与回退测试。
2. `run/numeric.rs::complete` 保持在短 slot 借用之间执行 `primitive_output`。
   调 helper 前 spill 输入；输出生成后通过 pending-owner 接口进入 TOS。
   当前 resident `complete` 的 `push_pending(...)?` 失败出口不能直接当完整
   cleanup 复用：新增与 `frame_operations/numeric.rs::commit_output` 同型的
   收 Result→结束短借用→显式释放未提交 previous/value 流程，保留已提交前缀。
   补 heap BigInt postfix 第二输出失败的 strong-count 测试；普通 `Option` Drop
   不能释放 `JsValue`。这项先作为错误路径修复单独验证，再接缓存，收益分开计。
3. 接通随后 direct `PutLocal/PutArg` 的消费存储；缓存结果直接 move 到目标。
   `SetLocal/SetArg` 仍 dup。释放 displaced owner 前，其它仍缓存的值先 canonicalize。
4. 保留 numeric 部分提交：postfix previous 已提交、next 输出失败时不能反向
   抹掉 previous；缺 LHS 时不得因新增整对预检而恢复已消费 RHS。
5. 仅当 profile 显示有效收益，再接“已有快属性读结果→直接 store”。保持
   borrowed-base 读零临时基值 owner；IC miss/Proxy/getter 先刷回并走原 driver。
6. 测最后一个 Object/String/BigInt/Symbol owner、非空 zero_queue、持有共享/
   独占借用、GC、错误分配、宿主重入、挂起放弃及 tail-call 清理。
   核对 strong count、deferred 顺序、最终无泄漏与既有 ledger。

**验收**：heap owner 从生成到存储没有额外 dup/release；GC/析构时点未延后；
bigint256 固定工作量的 stack traffic/insn 下降且整体门禁通过。若只有代码
复杂度增加，撤回 owning 缓存，保留 C2；不得靠放松 release 契约救收益。

### C4：按 profile 扩展静态融合，独立于 QuickOp

**执行步骤**

1. 先盘点既有 span 与 borrowed-base pair 的命中；扣除它们已节省的 dispatch，
   以候选频率×预计可省搬运/派发排序。每批只选 1–2 种。
2. 优先候选为 AddStore 之外的 Number binary→direct local store，或新证据
   点名的纯比较/分支模式。它们是待选项，不能无 profile 一次穷举所有组合。
3. `FusionPlan` 扩展前给 u8 tag 加显式 `FusionKind` 解码，维持紧凑侧表；
   空计划仍不分配。新增描述符若不可避免，记录 bytes/function 与 compile 开销。
4. 在现有发布认证点识别跨度并证明无内部入口；执行端复用 C1/C2/C3 helper，
   guards 全过才修改状态。TOS 不启用时同样能运行。
5. 每种 span 增加 interior branch/Catch/Gosub、TDZ/const/capture、非 Number、
   NaN/±0/overflow、容量失败及 fault-PC 测试。验证 decline 不重做有副作用步骤。
6. profiling 逻辑指令数保持原程序权重，单独统计真实 dispatch；不得把少计
   opcode 当成加速。记录字节码、侧表、`.text`、compile 时间与 RSS。
7. 每种候选独立 A/B；组合再与 accepted previous、saved 比较，收益不直接相加。

**关闭条件**：接受有稳定收益的候选，其它记录负结果并删除；没有候选达标也可
关闭 C4。B 接收 §2.5 契约和完整 span 清单，不要求 C 实现 quickening。

### C5：可选函数指针派发实验

只有 C4 后 profile 能把剩余成本明确归到派发，才投入最多两天。
默认基线仍是单点 match；不能因标题里有“派发”就必须改它。

1. 只将被点名的热 handler 放到共用宏/函数体后，先测这一重构自身是否回退。
2. 实验使用安全的中央循环 trampoline：handler 返回 `Continue/Exit/Error`，
   主循环调用下一 handler；栈空间 O(1)。这是函数指针派发，不承诺 direct
   threading 或 guaranteed tail call。禁止递归 handler 链赌 LLVM TCO。
3. canonical PC、缓存恢复、错误与冷退出复用同一协议；未实验 opcode 继续
   走 generic match，不把全量 handler 抽取当先决条件。
4. 保存真实 release 的反汇编：间接 call/jmp 站点数、热点位置、host stack
   frame、`.text`；小宿主栈执行长循环，覆盖 debug/MSRV/release 的退出与异常。
5. 无稳定净收益、冷启动/代码体积越界、或内联变化造成其它路径回退，立即关闭。
   保留负结果与 match。任何采用的派发方案在 rustc/MSRV/target 升级时复审汇编。

### C6：阶段收尾与交付 B

1. 冻结最终源码/二进制/输入；移除实验公共开关、无收益分支和临时计数，保留
   有用的 `profiling` 诊断与测试。最终默认构建再完整验证。
2. 按 §5/§6 串行跑全部正确性和测量；至少比较 final/C0、final/saved，子阶段
   表保留 each/previous，并用相同输入和构建协议单列 final/pre_a_release 的
   累计回退台账。C 相对起点有收益不等于阶段 A 回退已追回；该台账不替代 §7
   的既定阶段门禁。Optional PGO 双方重训结果单列，不能冲抵非 PGO 回退。
3. 台账逐项写 accepted、reverted、not-started 或 blocked-by-evidence；bigint256、
   typed-index、prop-delete、navier-stokes、map-string、set-churn、V8 regexp 等
   均有当前数字、比较分母与后续归属，不强行归零或沿用旧无 LTO 的关闭状态。
4. 更新本计划实施结果以及 architecture 的 C 小节：实际范围、负结果、门禁、
   源码/二进制身份、证据路径、收益区间、内存/编译成本与剩余限制。
5. 交付 B：TOS API 与边界清单、span 认证列表、规范 PC 映射要求、差分用例、
   最终无 PGO saved/previous 二进制，以及 pre_a_release 的同协议产物、
   输入 receipts 和未关闭回退台账。B 不重做 C 的所有权协议。

## 4. 实施工作包与回退边界

| 工作包 | 主要文件/产物 | 必须先完成 | 删除或回退时的边界 |
| --- | --- | --- | --- |
| P0 测量地基 | profiling 事件、workload/源码 receipt、C0 台账 | 基线裁决 | 不改变执行语义，可单独保留 |
| P1 消费存储 | 新 `stack/store.rs`、window facade、run local/arg store | 对应失败/owner 测试 | 恢复原 pop→replace，release 分支不动 |
| P2 缓存容器 | 新 `stack/tos.rs`、transaction Drop、canonicalize 入口 | 全构造点/出口清单 | 连同缓存字段及 hole 不变量一起移除 |
| P3 标量接线 | run Number/Bool handlers、number helper | P2 的退出恢复验证 | 统一 spill 后回原 helper |
| P4 owning output | numeric completion、pending push、store 接线 | P2/P3 及 owner 压力测试 | 结果恢复原规范 push，保留原 cleanup |
| P5 新 span | code/fusion、发布认证点、run/fusion | 每个候选的 profile 和认证测试 | 单独撤掉该 kind/tag/handler，不改 BC5 |
| P6 派发实验 | shared handler body、隔离实验构建、汇编报告 | C5 启动证据 | 全部撤掉后默认 match 仍通过同一测试 |
| P7 验收报告 | 阶段结果表、receipt 索引、architecture C 结果 | 最终源码冻结 | 不改历史结果；更正以新增记录说明 |

P2/P3 构成一组接线交付，但实现顺序先容器测试、再逐 opcode 接入。
拥有共享文件的工作包由同一实施者顺序落地；并行人员只负责独立测试、只读
审计或证据整理。实现者在每包完成时填写：改动范围、测试、剩余边界、测量、
接受/回退决定。不得以批量交付掩盖单项退化。

## 5. 正确性验证矩阵

### 5.1 现有测试作为固定回归集

| 约束 | 真实测试位置与名称 |
| --- | --- |
| 失败不消费 pending owner | `stack/window.rs::primitive_transaction_pending_owners_survive_failed_commits` |
| GC 与部分输出错误 | `stack/window.rs::frame_transaction_keeps_owners_across_gc_and_partial_output_failure` |
| 窗口容量/形状失败 | `stack.rs::failed_capacity_and_shape_checks_do_not_change_live_windows` |
| Number 原地更新事务 | `stack.rs::numeric_replacement_is_transactional_and_clears_the_dead_owner`；`stack/number.rs::fused_local_result_capacity_failure_leaves_binding_and_stack_unchanged` |
| shuffle 与独立 owner | `stack.rs::permutations_move_owners_and_failed_insertion_preserves_the_window`；`duplicate_sequence_keeps_order_and_independent_object_owners` |
| 规范部分失败 | `frame_operations/numeric.rs::primitive_transaction_preserves_partial_numeric_input_and_output_errors`；`resident_numeric_preflight_preserves_canonical_missing_left_consumption`；`resident_numeric_postfix_output_failure_keeps_previous_and_pc` |
| 借用/deferred release | `heap/slot_ownership.rs::blocked_borrow_and_deferred_release_do_not_commit_or_drain`；`owned_leaf_release_keeps_borrow_and_pending_boundaries` |
| PC 与 unwind | `run/program_counter.rs::local_pc_preserves_both_values_on_result_error_and_unwind`；`release_boundary_publishes_fault_before_leaving_resident_loop` |
| TDZ/const/coercion | `run/fusion.rs::guarded_fallback_keeps_coercion_const_tdz_and_capture_order`；`signed_zero_overflow_and_postfix_preserve_numeric_representation` |
| 新 span 入口认证 | `code/fusion.rs::all_interior_control_targets_prevent_fusion`；`method_call_spans_reject_effectful_arguments_and_interior_entry` |
| 实际失败子 PC | `conversion_driver/local_add.rs::local_add_throw_reports_add_line_before_error_allocation`；`local_add_identity_exhaustion_retains_canonical_operands_and_add_pc` |
| host 重入、finally、预算 | `driver.rs::selected_native_callback_reentry_preserves_captured_bindings`；`catch_and_finally_resume_owned_frames`；`logical_limit_returns_a_throw_and_unwinds_every_active_frame` |
| 挂起所有权 | `stack.rs::initialized_backing_preserves_take_frame_suspension_handoff_ownership`；`suspend.rs::failed_thaw_releases_partial_roots_without_detaching_heap_state`；`abandoned_thaw_does_not_register_or_keep_the_runtime_alive` |
| tail-call | `stack.rs::outgoing_tail_transfer_is_atomic_and_restores_the_caller_prefix` |

`run/fusion.rs` 的测试模块当前受 `all(test, feature = "profiling")` 控制，
只跑默认 workspace 测试会漏掉它。每个子阶段都运行 profiling lib 测试；
新正确性测试尽量放在默认 `cfg(test)`，纯计数断言才放 profiling 配置。

### 5.2 需要新增的测试

1. **状态机差分**：同一固定操作序列分别用 canonical 与 cached 执行。
   深度覆盖 0、1、2、capacity−1、capacity、超容量；覆盖 push/peek offsets/
   pop/consume-store/keep-store/dup/shuffle，比较结果、失败位置、已提交前缀、
   逻辑 depth、所有 canonical 槽和所有权数。包含“cache 非空时下一 push 失败”。
2. **所有出口差分**：为每类 RunExit 与 helper handoff 构造非空 TOS；在明确
   的 Result 错误和 unwind 点退出，验证 Drop 只恢复当前栈形。错误 PC、resume
   PC 与 canonical 路径一致，且不会重复执行 coercion/getter。
3. **真实语义边界**：Number 溢出转 Float、NaN、±0、ShortBigInt↔heap BigInt、
   String concat、Symbol 报错、TDZ/const、captured/mapped arguments、Proxy/
   getter 改变 binding、异常分配、eval、native callback 重入。
4. **最后 owner 压力**：TOS 独占最后一条 Object/String/BigInt/Symbol 边；
   spill/store/throw/GC 后验证计数与释放顺序。共享/独占 borrow 和非空队列时
   行为与旧路径一致；循环相关对象的回收与弱引用用既有 fixture 验证。
5. **挂起清理**：cached 结果紧邻 yield/await/return/throw；正常恢复、抛入、
   return 关闭、放弃 generator、失败 thaw 和跨 runtime 恢复拒绝均无残留边。
6. **融合认证**：对每个新增跨度的每个 interior PC 插入外部入口，必须拒绝
   融合；模拟 guard failure、部分执行后的错误、实际可失败行号。

这些测试验证事务与可观察行为，不逐行照抄缓存实现。语义差分可用 Rust 单测
中的内部模式选择，不需要两个生产 VM。QuickJS oracle 与完整 Test262 再作
独立兜底，不能用“新旧都犯同一错误”的内部差分替代一致性门禁。

### 5.3 每阶段的门禁层次

- 每个工作包：定向测试、fmt、source-layout；涉及数值/fusion 时同时跑
  profiling 对应测试。先修正确性，再开始计时。
- 每个被接受的 C1–C4 子阶段：workspace all-targets、profiling lib/CLI、
  MSRV clippy、rust-only、完整 Test262 receipt、定向及保护矩阵。
- C6：以上全部，加 docs/test262-host、BC5 pin、58 fixed/67 compile/9 original
  的完整矩阵及内存检查。C5 被采用时还须派发/宿主栈审计。

Test262 的通过条件是完整冻结向量 `pass=79982 / eligible=80032 /
total=102037` 与各项结果吻合，不只是 pass 总数。源码改变后 `--check` 会
成功认证旧 receipt 并打印 stale；它不证明新源码正确。`--focused` 对 stale
源码直接拒绝，此时跳过该模式并运行 `--full` 获取 current-source receipt，
不按错误提示去修改 `dev-support/test262/current.conf` 或重基线。

## 6. 性能、内存与证据矩阵

### 6.1 固定测量集合

| 集合 | 覆盖 | 重复与口径 | 用途 |
| --- | --- | --- | --- |
| C 定向固定工作量 | local/arg consume 与 keep store、Number binary/compare、分支、primitive output→store、频繁边界 flush | C0 先 pilot，再冻结源文件、参数、精确结果；两次独立实验，每轮 5–10 次交错采样 | 判断缓存是否节省目标流量 |
| BigInt 固定工作量 | 32/64/256 位；256 必须贯穿所有阶段 | 固定迭代与结果，plain `perf stat` 和 wall 分开测；不让校准改变工作量 | 分离栈税与分配/释放成本 |
| scaling 全集 | 21 非 regexp case × 32/128/512/2048，加 regexp-groups ×32/128，共86组 | operations=32768，repeat≥5；整进程 wall | 覆盖 map-string、set-churn、arguments/mapped、typed-index、array、scope 等保护项 |
| property probe | int/object/string 三组 | iterations=5000000，repeat=7；诊断 wall/ns per iteration | 保护已有 borrowed-base 与 IC 收益 |
| QuickJS microbench | empty_loop/prop_read/array_read/func_call/int_arith、bigint32/64/256_arith | repeat=5；同 Date.now 时钟，原始 ns/op | 定性辅助；毫秒分辨率不能证明细微收益 |
| V8-v7 | 全八单项与 original combined | repeat=5；Score 越高越好，suite source revision 固定 | 保护真实复合程序；combined 单独呈现 |
| 历史 fixed | 58 manifest entries | repeat=10；原输入哈希和结果契约不变 | 正式全量回归矩阵 |
| compile replay | 原 67 个源文件、公共 compile probe | repeat=10；编译耗时越低越好 | 限制发布/fusion 表与编译成本 |
| original replay | 原八套及 combined，共9项 | repeat=5；原 Score | 保持与历史 workload 的身份连续 |
| 内存与布局 | 活帧数/operand 深度、函数数量、owning BigInt/String、反复 eval/释放 | 独立 peak RSS/稳态/释放后活节点；布局 `size_of`、侧表 bytes、`.text` | 发现缓存和 fusion 元数据的成本、泄漏 |

C0 新定向 workload 必须含 canonical 计算出的精确 stdout，由 Node（可用时）
或 pinned QuickJS 独立核验。记录“生成器版本＋生成文件哈希＋参数＋expected”，
不是只保存一个循环片段。不要把 `scaling.py --case bigint256` 写入脚本：
BigInt 不在该 runner 的 case 集合中。

`property_read_probe.py` 当前不轮换引擎，也不自动留 binary hash/build receipt；
因此仅作诊断。在 stage receipt 中补齐这些信息；需要用它裁决微小差异时，
先让其生成的固定脚本进入已认证、交错采样的 runner，或独立升级探针并测试。

### 6.2 采样与归因纪律

1. 测量主机按 benchmark README 使用 PocketLab；双方同 CPU 集、toolchain、
   release flags、无 PGO。显式记录 Cargo profile override、Rust flags、target。
   `build_compile_probe.py` 的独立 workspace 需要额外传入 LTO/CGU 环境覆盖。
2. 先 A/A 测噪声，后交错 A/B；各 runner 的独立输出目录必须全新。若目标
   脚本太短，先延长工作量再冻结，两边一起变，不能只给一边加 warm-up。
3. 构建、Rust 测试、Test262、profiling、perf、RSS 与正式计时严格串行。
   profiling 只选热点与核计数；正式时间不取 profiling 二进制。
4. `perf stat` 的 instructions/cycles、`perf record`、`time -v` 分开运行。
   不把硬件事件不可用填成零，也不把 cycles 或 wall 的下降全归因于 dispatch。
5. `OwnedStorageCost.slot_moves` 是逻辑 owning transfer，不是机器指令/字节。
   物理改善需新 backing/tos 事件、instructions 与汇编共同支持。每次 fill/spill
   不得重复计逻辑 live slots 或 RC。
6. 任何失败、超时、stderr/score 契约不合格均保留并使对应组不具备比较资格；
   不能从几何均值删掉失败项。Score 与耗时分开聚合；不得混合不同 protocol。

### 6.3 缺失历史输入的处理

当前仓库没有 `docs/reports/data-structure-fixed-final.json`，也没有 README
所指 S07 replay receipt/源文件目录。`target/final-run` 的 A 阶段记录虽存在，
不能冒充这些历史输入，也不能将 LTO-off 二进制直接作 C 的正式分母。

C0 按原 evidence 索引找回 manifest/receipt/源文件，逐文件核验 SHA-256；
只重建文档有明确配方且可验原哈希的内容。恢复不成则登记缺口，另行冻结新
系列供开发迭代，标注不可与旧系列连续比较。既定完整验收缺证据时 C6 保持
未关闭；不能把子集试跑当 58/67/9 全量通过。

## 7. 接受、复核与停止门槛

以下是本计划新增的执行标准，不是既有实测结论。C0 必须在看 candidate
结果前将 workload、阈值与噪声估计写入 receipt。

| 项目 | 接受/复核规则 |
| --- | --- |
| 正确性 | 任一结果、owner、释放时序、PC、挂起/恢复或完整 Test262 回归，立即停止并修复/回退该子阶段；性能不能补偿 |
| 单个优化收益 | 至少一个预注册目标 workload 的 wall/cycles 改善 ≥ `max(2%, 2×noise)`，在第二次独立实验复现，并有 backing 流量或 instructions/汇编的因果证据 |
| 只有指令减少 | instructions 降≥5% 但时间未可分辨，可继续有限 spike；不足以默认接入复杂缓存，不能报告为已实现时间收益 |
| 完整保护矩阵 | 每个执行耗时矩阵的 geomean after/before≤1.01；Score 矩阵对应 before/after≤1.01；compile 单独按下一项门槛，不混成单一总分 |
| 单项退化 | >3% 进入独立复核；复核仍 >5% 则撤销相关优化；3–5% 必须定位并修复到保护阈值内，不能只靠总均值掩盖 |
| C 总体目标 | C0 预定目标集合时间 geomean≤0.97 是投资目标；未达成则按单项规则保留小收益并明确未达目标，不以“预计5–15%”冒充实测 |
| 编译/代码体积 | compile geomean>1.02 或 `.text` 增>3% 触发复核；找不到更紧凑实现则关闭相应 fusion/派发实验 |
| 内存 | RSS 增量超过 `max(基线3%, 1 MiB)` 复核，并解释事务/侧表理论上界；反复执行或释放后持续增长、owner ledger 不归位直接失败 |
| 无法辨别 | A/A 噪声太大或独立两轮方向不一致，记 inconclusive，不能默认接受；先改善实验或结束当前 spike |

`noise` 定义为同一 workload 两轮 A/A 中配对时间比相对 1 的绝对偏差的
95 分位数；每轮至少5对，接近门槛时增到10对后重新固定裁决。它是保守的
实验阈值，不称为正式置信区间。正式表同时给中位数、离散范围、重复数和原样本。

C2/C3 各最多两轮有热点依据的微调，仍无可复现净收益就记录负结果并关闭。
C5 最多两天。关闭实验不要求丢弃独立且已通过的 C1/C4；每次回退后重测最终
组合。RSS 稳定也不替代所有权测试，反之单测无泄漏也不证明 RSS 无回退。

## 8. 命令手册

本节命令是实施阶段的操作手册，本次编写文档未执行这些重型构建/测量。
从仓库根目录运行，使用 Bash。路径参数由 C0 receipt 填为实际绝对路径；
不存在的历史输入先按 §6.3 恢复。所有 runner 子目录都必须尚不存在。

### 8.1 准备每轮证据目录与构建协议

```bash
set -euo pipefail
export C_STAGE="$PWD/target/s3-c/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$PWD/target/s3-c"
mkdir "$C_STAGE"
unset RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS
unset CARGO_BUILD_TARGET RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER
unset LLVM_PROFILE_FILE
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
rustc -vV > "$C_STAGE/rustc.txt"
cargo -V > "$C_STAGE/cargo.txt"
git rev-parse HEAD > "$C_STAGE/source-head.txt"
git status --short > "$C_STAGE/source-status.txt"
git diff HEAD --binary > "$C_STAGE/source.patch"
python3 - <<'PY'
import json, os
from pathlib import Path
keys = ["CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
        "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
        "CARGO_BUILD_TARGET", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"]
(Path(os.environ["C_STAGE"]) / "build-environment.json").write_text(
    json.dumps({k: os.environ.get(k) for k in keys}, indent=2) + "\n")
PY
```

还要审计 `.cargo/config*` 与 target-specific Rust flags，确认没有隐含 PGO 或
不对称参数；上面的 unset 不是完整环境认证。阶段 receipt 必须哈希所有参与
构建的源码，包含未跟踪的新实现文件，`git diff` 本身不覆盖它们。

`build.py` 当前要求干净 worktree。以下只在已认证的干净阶段快照里执行，
saved/previous/candidate 及 pre_a_release 分别构建认证，使用各自独立输出目录；
不能为通过脚本擅自提交现有工作区。尚未冻结的实现可做定向诊断，不能声称
正式测量已完成。

```bash
python3 scripts/benchmark/build.py --jobs 2 \
  --plain-target "$C_STAGE/plain" --profile-target "$C_STAGE/profiling" \
  > "$C_STAGE/build-cli.log" 2>&1
python3 scripts/benchmark/build_compile_probe.py \
  --repo "$PWD" --output "$C_STAGE/compile-probe"
```

plain 为 `$C_STAGE/plain/release/qjs`；compile 为
`$C_STAGE/compile-probe/target/release/oxide-compile-probe`。
`build.py` 的 receipt 不完整记录 Cargo profile 环境覆盖，compile builder
也不自动继承主仓库 profile；因此 `build-environment.json` 与实际 build log
必须随二进制一起归档，双方均应用本节环境。冻结源码导出另按 compile builder
的 `--source-manifest` 协议认证，不假冒成某个父目录的 Git 源码身份。

### 8.2 正确性命令

先用测试名筛选 C 当前涉及的回归集；本段为每个接受子阶段的完整 Rust 门禁：

```bash
cargo fmt --all -- --check
python3 scripts/checks/check-source-layout.py
./scripts/checks/check-rust-only.sh
cargo +1.88.0 clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo test --locked -p quickjs-oxide --lib --features profiling
cargo test --locked -p quickjs-oxide-cli --test profiling
cargo test --locked -p quickjs-oxide-cli --test profiling --features profiling
python3 -m unittest discover -s scripts/benchmark -p 'test_*.py'
```

C6 追加（修改相关边界的子阶段可提前运行）：

```bash
cargo test --locked --workspace --doc
cargo test --locked --workspace --features test262-host \
  --lib --bins --test unsupported_diagnostics
cargo test --locked --workspace --features test262-host --test oracle test262_
node scripts/checks/check-bc5-pinned-opcodes.mjs --self-test
node scripts/checks/check-bc5-pinned-atoms.mjs --self-test
```

BC5 的 `--self-test` 验证门禁自身；C6 还须对已认证 pinned oracle 源码运行
`--source "$C_QJS_SOURCE/quickjs-opcode.h"` 与
`--source "$C_QJS_SOURCE/quickjs-atom.h"`，分别替换上面对应的 `--self-test`。
`C_QJS_SOURCE` 从 pinned oracle receipt 填入，不使用任意系统安装版本。

Test262 清理 `GIT_*`、认证旧 receipt，并按 source freshness 选择路径。
`--full` 固定写入仓库 `target/test262-full.*`，因此及时复制到本轮目录。

```bash
python3 - <<'PY'
import os, shutil, subprocess
from pathlib import Path
out = Path(os.environ["C_STAGE"]) / "test262"
out.mkdir(exist_ok=False)
env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
env["TEST262_WORKERS"] = "2"
command = ["./scripts/test262/test-test262.sh"]
check = subprocess.run(command + ["--check"], env=env,
                       text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
(out / "check.log").write_text(check.stdout)
print(check.stdout, end="")
check.check_returncode()
if "Test262 baseline source is current:" in check.stdout:
    with (out / "focused.log").open("w") as log:
        subprocess.run(command + ["--focused"], env=env,
                       stdout=log, stderr=subprocess.STDOUT, check=True)
elif "Test262 baseline source is stale:" in check.stdout:
    (out / "focused-skipped.txt").write_text(
        "Source stale: focused rejects this source; use full without changing current.conf.\n")
else:
    raise RuntimeError("Unknown source freshness; inspect check.log")
with (out / "full.log").open("w") as log:
    result = subprocess.run(command + ["--full"], env=env,
                            stdout=log, stderr=subprocess.STDOUT)
for suffix in ("tsv", "jsonl"):
    source = Path(f"target/test262-full.{suffix}")
    if source.exists():
        shutil.copy2(source, out / source.name)
result.check_returncode()
PY
```

若 full 在产生新报告前失败，旧固定文件可能仍存在；判定以本轮 log、退出码、
源码 fingerprint 和报告身份共同认证，不能仅因归档目录有 TSV 就记通过。

### 8.3 配对计时参数

C0 的路径表要为每次比较用 `export` 设置以下环境变量，禁止依赖隐式旧目录：

| 变量 | 内容 |
| --- | --- |
| `C_BEFORE`、`C_AFTER` | 已认证 before/after plain qjs 的绝对路径 |
| `C_BEFORE_COMPILE`、`C_AFTER_COMPILE` | 相同 flags 的 compile probe |
| `C_OUT` | 本次比较唯一目录，例如 `$C_STAGE/vs-previous`；其它比较分别用 `vs-saved`、`vs-pre-a` |
| `C_CPU` | 测量主机允许的固定 CPU 编号，双方相同 |
| `C_FIXED_MANIFEST`、`C_FIXED_WORKLOADS` | 已恢复认证的58项 manifest/目录 |
| `C_REPLAY_RECEIPT`、`C_REPLAY_WORKLOADS` | 已恢复认证的67源文件 receipt/目录 |
| `C_MICROBENCH` | pinned QuickJS `tests/microbench.js` 的实际路径 |
| `C_V8_SOURCE` | 已冻结 revision 的外部 js-engine-benchmark checkout |
| `C_FIXED_BIGINT256` | C0 冻结的固定工作量 JS，非自适应校准脚本 |

pre-A 累计对照时，`C_BEFORE` 指向已认证的 `pre_a_release`，`C_AFTER` 仍为
当前候选，沿用相同输入、工作量与结果契约。不得用历史 LTO-off 产物代填。

设置后先检查；其它输入在对应命令前同样用 `test -f/-d` 核对。计时命令
可整体在 `taskset -c "$C_CPU" bash` 中顺序执行，以固定没有 `--cpu` 选项的 runner。

```bash
: "${C_BEFORE:?set from C0 receipt}"
: "${C_AFTER:?set from C0 receipt}"
: "${C_OUT:?use a new comparison directory}"
: "${C_CPU:?select a permitted measurement CPU}"
test -x "$C_BEFORE"
test -x "$C_AFTER"
test ! -e "$C_OUT"
mkdir -p "$C_OUT"
```

### 8.4 完整 scaling 与上游套件

regexp-groups 的 size 必须小于255，不能直接全 case ×默认 sizes。

```bash
python3 - "$C_BEFORE" "$C_AFTER" "$C_OUT" <<'PY'
import subprocess, sys
sys.path.insert(0, "scripts/benchmark")
from scaling_workloads import CASES
before, after, out = sys.argv[1:]
common = ["python3", "scripts/benchmark/scaling.py",
          "--engine", f"before={before}", "--engine", f"after={after}",
          "--operations", "32768", "--repeat", "5", "--timeout", "180"]
cases = [arg for case in CASES if case != "regexp-groups"
         for arg in ("--case", case)]
subprocess.run(common + cases + ["--sizes", "32", "128", "512", "2048",
               "--output", f"{out}/scaling"], check=True)
subprocess.run(common + ["--case", "regexp-groups", "--sizes", "32", "128",
               "--output", f"{out}/scaling-regexp"], check=True)
PY

python3 scripts/benchmark/property_read_probe.py \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" \
  --iterations 5000000 --repeat 7 --output "$C_OUT/property-diagnostic"

python3 scripts/benchmark/run.py --suite microbench --source "$C_MICROBENCH" \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" \
  --case empty_loop --case prop_read --case array_read --case func_call \
  --case int_arith --case bigint32_arith --case bigint64_arith --case bigint256_arith \
  --repeat 5 --timeout 1800 --output "$C_OUT/microbench"

python3 scripts/benchmark/run.py --suite v8-v7 --source "$C_V8_SOURCE" \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" \
  --repeat 5 --timeout 1800 --output "$C_OUT/v8-isolated"
python3 scripts/benchmark/run.py --suite v8-v7 --source "$C_V8_SOURCE" \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" --case all \
  --repeat 5 --timeout 1800 --output "$C_OUT/v8-combined"
```

### 8.5 历史完整矩阵

本段只在 §6.3 的输入恢复/认证完成后执行；不能创建空 manifest 绕过门禁。

```bash
test -f "$C_FIXED_MANIFEST"
test -f "$C_REPLAY_RECEIPT"
test -d "$C_FIXED_WORKLOADS"
test -d "$C_REPLAY_WORKLOADS"
python3 scripts/benchmark/fixed.py \
  --manifest "$C_FIXED_MANIFEST" --workload-dir "$C_FIXED_WORKLOADS" \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" \
  --repeat 10 --cpu "$C_CPU" --output "$C_OUT/fixed"
python3 scripts/benchmark/replay.py --mode compile \
  --receipt "$C_REPLAY_RECEIPT" --workload-dir "$C_REPLAY_WORKLOADS" \
  --engine before="$C_BEFORE_COMPILE" --engine after="$C_AFTER_COMPILE" \
  --repeat 10 --output "$C_OUT/compile"
python3 scripts/benchmark/replay.py --mode original \
  --receipt "$C_REPLAY_RECEIPT" --workload-dir "$C_REPLAY_WORKLOADS" \
  --engine before="$C_BEFORE" --engine after="$C_AFTER" \
  --repeat 5 --timeout 1800 --output "$C_OUT/original"
```

### 8.6 指令数、RSS 与派发形状

以下是一对独立诊断样本；按 C0 固定的轮次交替 before/after 顺序，各轮另建
目录，不能把这一对当完整统计。硬件事件受主机支持约束，保留 perf 的原始报错。

```bash
perf stat -x, -o "$C_OUT/bigint-before.perf.csv" \
  -e cycles:u,instructions:u,branches:u,branch-misses:u -- \
  taskset -c "$C_CPU" "$C_BEFORE" "$C_FIXED_BIGINT256" \
  > "$C_OUT/bigint-before.perf.stdout" 2> "$C_OUT/bigint-before.perf.stderr"
perf stat -x, -o "$C_OUT/bigint-after.perf.csv" \
  -e cycles:u,instructions:u,branches:u,branch-misses:u -- \
  taskset -c "$C_CPU" "$C_AFTER" "$C_FIXED_BIGINT256" \
  > "$C_OUT/bigint-after.perf.stdout" 2> "$C_OUT/bigint-after.perf.stderr"
/usr/bin/time -v -o "$C_OUT/bigint-before.rss.txt" \
  taskset -c "$C_CPU" "$C_BEFORE" "$C_FIXED_BIGINT256" \
  > "$C_OUT/bigint-before.rss.stdout" 2> "$C_OUT/bigint-before.rss.stderr"
/usr/bin/time -v -o "$C_OUT/bigint-after.rss.txt" \
  taskset -c "$C_CPU" "$C_AFTER" "$C_FIXED_BIGINT256" \
  > "$C_OUT/bigint-after.rss.stdout" 2> "$C_OUT/bigint-after.rss.stderr"
objdump -drC "$C_BEFORE" > "$C_OUT/before.asm"
objdump -drC "$C_AFTER" > "$C_OUT/after.asm"
size -A "$C_BEFORE" > "$C_OUT/before.sections.txt"
size -A "$C_AFTER" > "$C_OUT/after.sections.txt"
```

RSS 样本还须配固定活帧/函数数量与反复创建释放的 workload；`time -v` 只给
进程 peak，不证明释放后回收或内部强边正确。C5 的间接分支审核只数已定位的
dispatch/handler 热区，使用实际 target 的指令格式，不能用全二进制 grep 总数
替代控制流判断。
上述各 stdout 必须与 C0 冻结的 expected 一致，非预期 stderr 保留并判为无效
样本。`:u` 采用用户态事件口径；改采其它口径必须另标，不能与旧计数混算。

## 9. 开工清单与结果模板

下一位实施者从以下顺序开始，不需要重新设计总体路线：

1. 核对最新 HEAD/工作区是否仍与本文一致，补齐 E/残余/A4 三个 receipt；
   若发生表示或契约变化，只更新相关接口/测试，不能静默跳过门禁。
2. 完成 C0 源码/输入/构建认证与 A/A，固定目标集合、测量阈值及边界清单。
3. 先实现 P1 local consume-store 测试与事务，再 parameter/keep；按 §5 通过后测量。
4. 只有 P1 与基线结论明确后才接 P2/P3；每个扩大缓存准入的动作都附 owner
   转移图、canonical fallback 与可复现的流量证据。
5. 依 §3 逐项执行；每阶段结束填写下表，任何“待测”都不记 accepted。

| 字段 | 要填写的内容 |
| --- | --- |
| 阶段 / 日期 / 实施者 | C0–C6、实际执行日期、责任人 |
| 起点与候选身份 | HEAD/完整源码清单及 patch hash、未跟踪文件 hash、binary SHA-256 |
| 协议 | rustc/target/CPU、fat LTO/CGU1/无PGO/无profiling、所有覆盖参数 |
| 变化与归因 | 实际改动、栈读写/owner/dispatch/insn 变化，不写未经测量的收益 |
| 正确性 | 定向、workspace、profiling、MSRV、Test262 current-source receipt 链接 |
| 性能 | each/previous、final/C0、final/saved；分矩阵 geomean、单项、原样本、噪声 |
| pre-A 累计回退 | pre_a_release 源码/构建/输入 receipts、候选/pre_a_release 同协议结果、未关闭项与后续归属 |
| 内存/编译/代码体积 | RSS、释放后 ledger、side-table bytes、compile 与 `.text` |
| 决策 | accepted / reverted / not-started / inconclusive / blocked-by-evidence，以及原因 |
| 剩余项 | 对应 C/B/D 或其它独立设计，不重复计算或隐去回退 |

最终归档至少包含 `stage-receipt.json`、`decision.md`、全部 build receipts、
源码身份、workload manifests、原 stdout/stderr/samples、Test262 完整报告、
profiling/perf/RSS/汇编索引。`target/` 证据按仓库既有习惯保留在本地；新文档
记录路径和哈希，不把缺失证据写成已验证，也不把本计划本身当作 C 实施结果。

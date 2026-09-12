# 原语 VM 实施设计：模块、算法与交付顺序

状态：拟实施计划，2026-09-12；没有新引擎实现或性能收益声明。

**交付方式：全部在一个 PR 内完成。** 具体顺序见[逐 commit 计划](primitive-vm-commit-plan.md)：C01–C32 主提交，类型专化按证据决定是否插入 X01/X02。P0–P9 是验收分类，不代表多个 PR；后文 A–H 也不单独合并。逐 commit 计划优先规定实施顺序，架构与问题目标保持不变。

本文供实现与审查新 VM 的贡献者使用。读完后应能领取一个交付项，知道修改哪些责任、维护哪些不变量，以及如何证明其完成。架构方向见[目标架构提案](primitive-vm-plan.md)，性能依据见[PR19 profile](reports/post-pr19-profile.md)，参考研究见[Tachyon 架构报告](reports/tachyon-architecture.md)。这里把方向收敛为工作假设、接口契约和交付依赖，不重新罗列所有历史方案。

## 1. 目标与明确选择

**项目有两个必须同时完成的目标：建立更合理、可维护的目标 VM 架构；解决 issue #16 中本轮覆盖的明确问题。** 架构先进性和长期维护价值可以独立支持必做项，不要求每项架构工作都立即对应一笔可单独测出的加速。最终仍须用冻结负载、正确输出、语义回归和同口径 A/B 证明问题得到解决。

**阶段性收益不能替代架构完成，架构完成也不能替代问题解决。** P0–P9 按此双重目标推进；其中 P8 类型专化仍是按证据进入的可选优化，具体布局实验不改变必须交付的架构能力。

### 必须交付的架构能力与实验选择

| 必做架构能力 | 架构完成证据 | 仍可实验定稿的实现 |
| --- | --- | --- |
| 直接操作执行槽的原语内核 | 热执行不经逐 local/argument 的万能 host 协议；效果边界明确 | 寄存器主方案与同底座优化栈方案；指令编码、分派与融合 |
| 显式普通 JS 调用帧 | 同一驱动推进调用/返回；帧、参数、返回目标和身份明确 | 连续或分段 arena、冷热字段布局和参数窗口策略 |
| 清晰的执行值与生命周期模型 | 内部执行值与外部 rooted handle 职责分开；所有运行/挂起/回复状态可枚举 roots | tagged/NaN-box、handle 位宽、tracing/RC 的具体策略 |
| 统一异常与恢复协议 | 转换、内置回调、异常、generator/async 使用明确阶段与完成协议；保持各自规范行为 | 阶段存储、request 大小、分配方式；完整 Task 政策仍由 #20 定义 |
| 明确的数据流、验证与发布 | 语义 CFG 表达绑定、异常/恢复边；发布物有完整验证，纯代码与运行时绑定分离 | SSA/分配算法的复杂度、编码和 maps 的具体布局 |
| 可读、可维护的代码组织 | 状态唯一归属、数值语义唯一实现、受限依赖；第 11 节四种修改路径成立；旧重复生产路径退出 | 模块命名、文件粒度及经测量需要的热冷拆分 |

具体选择可以因实验而调整，但必须保留对应架构能力。选择栈执行不意味着恢复旧 VmHost；选择 RC 也不能恢复无处不在的隐式 owning handle 协议。若需要改变必做能力或最终问题范围，须显式修订目标，不能因短期收益足够而静默删掉。

### 本轮原语 VM 的问题范围

编号对应 [issue #16 的 PR19 profile 更新](https://github.com/pocket-stack/quickjs-oxide/issues/16#issuecomment-5634660983)中的优化清单，而不是独立 GitHub issue 编号。

| #16 条目 | 要解决的具体问题 | 必须交付的证据 |
| --- | --- | --- |
| 第 1 项 | 普通 JS 调用消耗过多 native 栈，默认原始 Earley-Boyer 溢出 | 默认生产配置取得有效独立及组合成绩；有限/无限递归、小栈线程、JS→native→JS 重入正确；量化 native frame/深度。增大预算不算修复 |
| 第 5 项 | Number 运算、位运算和关系比较仍经过昂贵通用数值流程 | 确切标签路径减少转换/中间搬运；int/float、Crypto、Navier-Stokes 的交错耗时与指令数改善，保留完整数值/转换语义 |
| 第 7 项 | 每次函数调用的存储、元数据和 root 生命周期成本高 | func_call/closure 的调用吞吐、分配与复制/保活成本下降；arguments、eval、捕获、跨 realm、异常和 async/generator 正确 |
| 第 9 项 | 局部更新需要重复读槽、更新、写回和 dispatch | 真实发布字节码/执行诊断证明重复操作减少；循环负载改善，保留前后缀、溢出、TDZ、捕获、mapped arguments 与对象转换 |
| 第 10 项 | 每条指令发布 PC 并重复定位/核对活动帧 | 先独立量化成本；若成立，减少重复管理且回溯、fault/resume、重入、暂停与调试观察正确。execute_inner 总占比不能作为该项收益 |

第 1/5/7/9 项有明确症状或成本证据；第 10 项仍需独立归因。若测量表明第 10 项不是重要成本，应记录证据并降低其专门性能实验的优先级；明确的 PC/帧状态与观察边界仍是必做架构能力，不能以缺少独立收益为由取消。

其他条目（静态 Atom、Array、RegExp、shape、属性缓存、TypedArray）仍是 issue #16 的独立待解决项。本轮完整矩阵需要防止它们退化，但不能把运行通过等同于已优化，也不能声称这一轮关闭整个 issue。

### 双重目标的交付门槛

- 每个实现变更注明贡献的架构能力、问题条目或两者，并给出对应的结构/语义验证与必要测量。基础架构提交可以暂时没有独立性能收益；完成接口或文件重组不能标记问题已解决。
- 围绕调用/帧推进第 1/7/10 项，围绕原语/局部数据流推进第 5/9 项，同时完成上表必做能力。允许跨模块甚至整体替换现有实现，不要求保留旧结构。
- CFG、执行内核、值/根、异常与恢复作为完整架构共同设计。其必需的跨模块改造可以成为前置工作；GC 算法、标签布局、编码与存储形式通过原型定稿，不预先要求所有候选都实现。
- 首个切片用于验证方案，真正完成问题必须进入生产路径并通过相关语义及完整性能回归。可独立完成的解决项及时提交到同一 PR 并验证，但它们是阶段成果，不单独合并，也不代表整体架构任务完成。
- 新 profile 决定后续优化的重点和具体实现。局部修补即使已改善指标，仍要完成已确定的目标架构；架构已完成但问题未解决时，继续归因和优化，不能以重写完成结项。

重构单位是完整的“编译 → 发布 → 执行 → 调用/转换 → 恢复”流程，包含值与根管理。现有文件结构、VmHost、栈字节码、owning Value、RC 和递归调用都不是约束。保留的是 JS 可观察语义、已承诺的宿主契约和真实验收证据。

首版目标如下。标为实验的选择应在实现切片后定稿，不能为了所有可能性预建通用框架。

| 事项 | 首版工作假设 | 改变选择所需证据 |
| --- | --- | --- |
| 基础执行 | 通用值寄存器字节码，每条操作有完整语义 | 同底座优化栈 VM 在代表负载上的比较 |
| 普通调用 | 显式帧，外层循环推进 | 不保留 Rust 递归作为普通调用后备 |
| 内部值 | Copy tagged word + 有身份的 heap handle | 布局、handle 访问和根管理测量 |
| 回收 | 单线程、非移动 STW tracing 原型 | 正确性、暂停与峰值/保留内存；RC 是独立对照 |
| 指令编码 | 清晰的解码指令模型；32-bit 基础字配扩展字作为候选 | 与紧凑编码比较编译、解码、大小和机器指令 |
| 编译中间表示 | 一种带异常/恢复边的语义 CFG；可提升绑定使用 SSA | 不引入两套表达相同语义的长期 IR |
| 存储 | 每个可独立暂停的执行拥有连续寄存器 arena | 挂起复制/保留与分段寻址比较 |
| 专化 | 通用执行器完成后再加入有界类型块 | 新 profile 证明有足够可消除成本 |
| 包与抽象 | 一个引擎 crate 内的具体模块和受限可见性 | 独立生命周期/消费者出现后才考虑拆 crate 或 trait |

“原语内核”首版指 Number/Bool/null/undefined 等无需分配、执行 JS 或访问可变 heap 的路径。String、BigInt、Symbol、Object 仍有完整语义，但其需要 heap、分配或回调的操作通过语义服务执行。后续可增加有明确借用契约的只读 heap 快路，不能为追求命中率重新把整个 Runtime 传进内核。

## 2. 可读性与维护性是架构验收项

1. **一个状态只有一个权威存放位置。** 当前指令位置、返回目标、pending completion、绑定身份各有唯一 owner。缓存只存在于有明确进入/退出规则的短执行区间。
2. **控制流写在可搜索的 enum 和 match 中。** 不用字符串操作名、动态注册表、嵌套闭包捕获或异构 Any 容器隐藏 JS 恢复流程。
3. **模块按语义责任划分。** 不按“fast/slow/new/legacy”永久复制整套解释器，不按每条 opcode 创建一份模块。
4. **接口按能力收窄。** 原语代码获得指令、槽和预算；收集器获得 roots 和 heap；宿主获得显式 rooted API handle。不能通过方便的 context 对象重新访问所有服务。
5. **不为每个内部函数创建 trait。** 分派、值、帧先用具体类型；宿主能力和确实需要替换的外部接口才使用抽象。
6. **性能拆分以证据为准。** 热分派可保留一个完整 match；冷语义移到命名明确的函数。禁止为了“整洁”给每条算术指令增加多层适配，也禁止为了“快”把完整引擎塞进一个 execute 函数。
7. **证明放在执行点附近。** alias、root、无回调、PC 和恢复约束写在相应类型/方法的文档中，不只写在总架构文档。
8. **优化是可删除的。** 去掉类型专化、缓存和融合后，通用新 VM 仍能完整执行。不能依赖旧引擎提供缺失语义。
9. **文件拆分遵循可独立解释的责任。** mod.rs 主要声明模块和受限导出；避免 helpers/utils/common 成为杂物层。不同 opcode 家族共享算法，不通过过度宏生成隐藏语义。
10. **每项新增复杂度都有使用者。** 不预建 JIT 插件、通用调度器或通用 effect 框架；当前明确的调用、异常、GC 和暂停需求足以决定接口。

审查时应能从一条 Add 追到数值计算或对象转换，从一次 Call 追到参数根和返回槽，从一个 Throw 追到 handler 和清理过程。跨文件跳转多到难以说明一个操作时，需要合并责任或简化接口。

## 3. 目标代码组织与依赖

以下路径是新架构的实施建议，不要求保留同名旧模块，也不要求一次建立所有文件。

```text
src/engine/
  compiler/
    syntax/             解析；可先复用现有成熟能力
    bindings/           作用域、捕获、初始化与动态绑定分类
    ir/                 一种语义 CFG、builder、验证与打印
    passes/             常量、copy、死值、控制流、存活分析
    backend/            寄存器分配、并行 move、编码与 maps
  code/
    instruction.rs      解码指令和操作数类别
    format.rs           编码、解码及宽操作数
    verify.rs           结构、控制流、布局与元数据验证
    published.rs        不可变代码及常量描述
    instance.rs         runtime/realm 绑定、IC/专化状态
  value/
    word.rs             内部值及标签访问
    number.rs           共同 Number 算法
    handle.rs           heap 身份；不实现 root 所有权
  heap/
    store.rs            对象表和分配、外部 backing 记账
    roots.rs            宿主/临时根的登记
    collector.rs        标记、weak/ephemeron、回收
    trace.rs            各持有值的数据类型的边枚举
  vm/
    state.rs            帧、寄存器、PC、执行状态
    interpreter.rs      连续执行与 opcode 分派
    numeric.rs          原语命中；复用 value::number
    call.rs             参数布局、帧进入/离开
    unwind.rs           字节码 handler、清理和帧退出
    request.rs          内核向外提交的明确请求
    roots.rs            VM 状态的根枚举
  semantics/
    coercion.rs         ToPrimitive/ToNumeric 等阶段
    operators.rs        完整 Add、比较等操作
    properties.rs       属性算法的统一入口
    invocation.rs       callable 分类、构造与特殊调用
    protocol.rs         builtin/语义服务共用的请求与回复数据
    operation.rs        恢复状态、回复和 operation 栈
  object/               shape、槽与存储探测；不重复完整属性语义
  builtins/             按 JS 内置家族组织完整算法和可恢复状态
  realm/                intrinsic 与 realm 身份
  modules/              模块图、实例和链接/求值协议
  runtime/
    driver.rs           唯一外层执行驱动
    execution.rs        ExecutionRecord 和入口状态
    safepoint.rs        root 发布、GC、预算和调试协调
  api/                  宿主入口与 rooted public value
  jobs/                 Promise jobs、任务队列的规范语义
```

模块依赖以“谁调用谁”区分。runtime 驱动 vm 和 semantics；semantics 可使用对象/heap/内置算法，但返回调用请求，不递归调用解释器；vm 不调用 runtime driver；compiler 产生 code，不接触运行中的 realm；code 的不可变部分不持有运行时对象。

语义操作状态由 runtime 的 ExecutionRecord 持有，vm 状态不需要依赖每一种内置 continuation。builtins 自己拥有各算法的阶段类型，semantics 的操作分派只组合它们。这样增加 Array.map 的阶段不会迫使数值执行模块认识 Array。

builtins 依赖小型 semantics::protocol 数据类型，不依赖 operation 分派器；operation 分派器可以组合 builtin 阶段类型。protocol 不反向依赖这些阶段，从而避免接口循环。object 负责 shape/槽/存储事务；semantics::properties 拥有完整 JS 属性操作。一次属性操作的规范顺序只有一个实现 owner。

内部依赖仍是同 crate 的具体类型；优先 private/pub(super)，跨兄弟责任才使用 pub(crate)。不引入对外通用 VM trait，也不让独立的数值函数依赖某种 Runtime 实现。

### 各层的明确禁区

| 模块 | 可以做 | 不可以做 |
| --- | --- | --- |
| value::number | 输入输出纯数值，表达负零/NaN/溢出 | 查属性、分配 JS 错误、调用 JS |
| vm::interpreter | 操作已验证代码与槽，返回请求 | 任意访问 heap、执行宿主回调 |
| semantics | 执行完整规范操作，保存阶段 | 保存跨边界 Rust 借用、递归进入 VM |
| runtime::driver | 选择下一执行步骤，协调 roots/GC/host | 重写一份 Add/属性/Promise 语义 |
| heap | 管理存储、可达性与资源 | 直接执行 JS finalizer 或 jobs |
| compiler | 控制流和合法转换、生成 maps | 基于本轮观察到的对象类型假定语言语义 |

## 4. 核心状态与接口契约

下列 Rust 是接口草案，不是可编译实现。首个切片需要把借用和失败路径落实后再固定签名。

```rust
struct ExecutionRecord {
    vm: VmState,
    operations: OperationStack,
    pending: PendingBoundary,
    status: ExecutionStatus,
}
struct VmState {
    frames: Vec<Frame>,
    registers: RegisterArena,
}
struct Frame {
    code: CodeInstanceId,
    registers: RegisterRange,
    position: FramePosition,
    return_to: ReturnTarget,
    environment: EnvironmentId,
    arguments: ActualArguments,
    handler: HandlerState,
    this_value: VmWord,
    new_target: VmWord,
}
enum ReturnTarget {
    Register { frame: FrameId, dst: Reg },
    Operation { id: OperationId },
    Entry,
}
```

Promise capability 属于相应 async operation/执行的根状态，不必让所有普通 Frame 膨胀为所有异步协议的合集。Frame 的实际尺寸需测量；冷字段必要时放 side table，但先保持可读的权威模型，再根据 profile 拆分。

FrameId/OperationId/ExecutionId 是不同新类型。帧引用包含执行归属和复用验证；不能把任意 usize 在跨边界后重新解释为仍有效的帧。局部 Reg 与 arena 全局偏移也是不同类型，避免 base 加两次。

### 执行入口与状态机

```text
Ready → Running → Ready                 内部 safepoint 后继续同一执行
                → Waiting(token)        真正语义挂起
                → HostCallPending       进入已登记宿主调用
                → Completed(result)
                → Failed(thrown)
HostCallPending → Ready                 收到唯一合法回复
Waiting(token) → Ready                  收到匹配且未消费的恢复事件
```

Ready 只表示允许驱动，不等于允许队列插入另一项 JS。预算耗尽交还 embedder 时，要保留同一 job 的执行约束；同步 API 可以内部补充预算继续，不能静默变成异步 API。挂起、宿主返回、完成是不同情况。

执行正在 Running/HostCallPending 时，宿主重入创建独立的合法入口；禁止递归驱动同一个 ExecutionRecord。恢复 token 带身份和代次，只能消费一次。完成/失败的值在交给宿主 root 或 Promise/job owner 前仍属于根图。

### 边界协议

```rust
fn run_until_boundary(
    code: &VerifiedCode,
    frame: &mut ActiveFrame,
    registers: &mut [VmWord],
    budget: &mut Budget,
) -> KernelExit;

enum KernelExit {
    Request(Request),
    Return(VmWord),
    Throw(VmWord),
    Poll,
}
```

常见 Request 可按值保存在 pending 小 enum；较大 continuation 使用 operation arena。不要把每个数值 miss 都实现成一次 Box 分配。Request 的大小、move 成本及 native 栈帧需要测量。

KernelExit 的临时 Rust 局部在无分配/无 GC 的短交接中安装到 ExecutionRecord.pending，随后才能进入可能收集的路径；“返回一个 VmWord”本身不建立 root。先发布根、再执行外部操作是统一规则。

驱动器只做：取得可执行状态 → 借用内核窗口 → 收到出口并结束借用 → 登记根/位置 → 执行语义阶段、进入帧或交给宿主 → 安装回复 → 再次借用。不能在解释器窗口仍存活时扩容 arena、执行 GC 或回调。

## 5. 编译与发布的具体算法

### 5.1 绑定分析先于局部槽优化

为绑定分配稳定 BindingId，并记录声明种类、初始化、捕获、direct eval 可见性和映射 arguments。

- 普通非逃逸局部可提升为 SSA 值。
- 被捕获或动态可见的绑定使用 Cell/Environment 操作；只读权限属于访问视图，不通过复制 cell 伪造身份。
- per-iteration lexical environment 保留每轮身份。
- TDZ 与“存在一个 SSA 值”分离；不能以 Undefined 代替未初始化。
- eval-capable 函数先保守物化环境，后续有证明再缩减。

交付物不是一组布尔标志散在发码分支里，而是一个 BindingStorage 分类，lowering 据此选择明确的 LoadRegister/LoadCell/CheckedBinding 行为。

### 5.2 一种语义 CFG

Block 参数表达合流值；操作带 source、按需要带异常边；终结操作表达分支/返回/异常/挂起。只保留一种权威操作语义定义。普通语法树可以复用，但不长期经过旧栈字节码再反编译为 CFG。

可提升变量使用 sealed-block SSA：循环头未封闭时建立不完整参数，回边补齐后删去平凡参数。异常边上的环境对应操作抛错时已完成的前缀；不能一律沿用块末尾状态。首版可以保守把可抛操作设为明确块边界。

finally 用显式 completion 描述 kind/payload/target，在 handler 执行后恢复或被新 completion 覆盖。非正常边需要自己的活跃值，不能只对普通 successor 做 liveness。

### 5.3 Pass 次序与合法性

固定首版次序：CFG 验证 → 初始化/绑定验证 → 常量折叠 → copy propagation → 死纯值消除 → 简单分支化简 → liveness → allocation → parallel moves → 编码 → 发布验证。

每个 pass：输入/输出契约、会改变的元数据、一个可读 dump 入口、相关反例测试。开发/诊断配置可在每个 pass 后验证；release 不为调试反复验证相同不可变产物。

只有已知纯且不会抛错的操作允许删除或重排。Generic Add/比较会转换对象；属性访问可执行 getter；浮点不允许重结合。Numeric Increment 独立于 Add 1，保留 BigInt 和字符串输入差别。

### 5.4 寄存器分配

首版采用保守 linear scan；区间覆盖正常、异常、恢复边和 operation 使用。优先实现正确的区间，之后再比较区间分裂与槽压力收益。

将形式参数、需要保留的实际参数、临时值和 cells 的引用布局写入 FrameLayout。所有 actual args 都可按需供 arguments/rest 使用；不能仅按 formal count 丢弃多余参数。不同函数模式可采用不同明确布局，但不对每个访问反复分类。

block 参数消除为边上的并行 move；critical edge 先拆分。算法先发出目标未被剩余源引用的 move；只剩环时将一个旧值保存到临时寄存器，替换相应源，再继续，直到列表为空。附交换、多节点环、重复源和自复制反例。

### 5.5 指令与 maps

按家族定义操作：常量/移动、数值/比较、分支、绑定、语义请求、调用/构造、completion、暂停。初版不增加只对某个 microbench 有用的几十种组合 opcode。

单一指令规格描述操作数种类、读写槽、是否可能抛错/调用/分配/挂起。可生成编解码和打印的机械部分，语义处理器仍是可读的 Rust。liveness 可保守使用效果分类；分类遗漏必须有验证和针对性反例，不能以元数据表替代语义测试。

发布验证检查全部字节，包括不可达区域；验证跳转到指令头、Wide、寄存器和窗口、handler/resume/source/live maps 配对。源位置使用独立 InstrId→source 表，不能依赖优化前偏移。

VerifiedCode 的构造受限，发布后不可变。CodeInstance 在当前 runtime/realm 解析常量、持有 roots 和可变 IC；code 的身份与 code instance 的身份分开。先保证这种分离，不将后台编译或跨 runtime 共享作为本轮交付前提。

## 6. 执行、转换、调用与异常

### 6.1 热循环

局部缓存当前 frame、code、寄存器窗口和 PC。匹配 opcode 后读取全部源，再写目标，允许 dst 与源重合；成功的纯原语不出循环、不发布整个帧、不 retain/release。

Number helper 共享一套负零、NaN、ToInt32、溢出和 pow 规则。首版只对确切 Int/Number 做可读分支；不能把 Float→Int 的 Rust cast 当成 JS 转换。旧 helper 可以移植语义和测试，不要求沿用旧 owning Value 接口。

循环只在调用/语义请求、异常、返回、预算/调试 poll 处回写必要状态。poll 在回边及有界直线指令区间设置，避免一个无回边的大函数无限延迟中断；区间上限由预算精度和测量决定。融合与专化仍按约定计算逻辑 fuel，不任意降低计数。

### 6.2 一个完整例子：对象转换后做加法

```js
function addOne(x) { return x + 1; }
const x = { valueOf() { return 41; } };
addOne(x);
```

1. 通用 Add 读完 x 和 1，发现不能直接执行 Number 加法；dst 不变。
2. 提交 AddOperation，保存输入、目标、fault/resume 位置和阶段，发布根。
3. coercion 根据规范取 @@toPrimitive 或 valueOf/toString；属性访问本身也可能再次调用 JS。
4. operation 保存已完成阶段，提交 InvokeJs；driver push 新 frame，返回目标是该 OperationId。
5. 回调返回 41，安装到 operation 根状态，继续右侧转换及 Add 的 String/Number/BigInt 分类；不重复左侧转换。
6. 运算成功写目标并推进到 resume PC；抛错则按原 Add 的 fault PC 回到所属 frame 的 handler 流程。

AddOperation、ToPrimitiveOperation 各自拥有自己的语义，不给所有操作共享一组含义随 opcode 变化的 temp0/temp1/phase 整数。operation enum 的每个状态用命名字段表达其需要保存的最小值。

### 6.3 调用事务

callee/this/arguments 按语言顺序求值。进入帧前校验布局、容量和 callable，准备可能失败的分配，保持输入有根；然后一次提交 frame 和返回目标。分配失败不能留下半初始化帧或错误的当前 realm。引擎可捕获资源限制与平台 fatal OOM 的契约需明确，不能承诺任意进程 OOM 都可恢复。

参数浅复制到 callee 的独立区域，保持参数写入不别名 caller 变量；新增 this/new.target、actual arguments 和环境 roots。默认不做零复制参数传递。

普通调用由 driver 切换帧，返回先安装结果到 caller/operation/entry root，再清除并释放 callee 区域。构造、bound/proxy callable、derived constructor 等扩展属于 invocation 的明确分支，不进入每个算术 handler。

### 6.4 异常和原生清理

FramePosition 显式区分当前 fault 和继续位置；只有成功完成才推进语义状态。回溯使用稳定 source map，不从 next_pc - 1 反推。

operation 也可能需要异常清理，如 IteratorClose。不能遇到 Throw 就直接丢弃全部 operation：先按规定把异常交给相关 operation 的错误分支，执行必要清理，再沿目标 frame 的 handler/finally 展开。清理的新异常是否替换旧 completion 由该规范操作决定，不能使用一个无条件覆盖规则。

每个被移除 frame/operation 的根清理只做一次。finally 保留 pending completion，执行内嵌 return/throw 后按规范替换；catch 不应捕获内部挂起/预算控制信号。

### 6.5 内置算法与宿主

以 Array.map 为首个复杂内置：状态字段为 source、length、index、callback、thisArg、result、phase；按步骤处理 species、HasProperty、Get、callback 和目标发布。不能以 dense 专用循环替代完整语义切片。以后 dense 优化只在可证明区间复用同一个完成协议。

同步 host 函数仍可用同步宿主接口。driver 在调用前登记根并释放内部借用；host 可重入新的 execution；返回后验证原入口状态再恢复。不可挂起的 host 调用不得保存 Rust 栈并声称可暂停；支持异步的宿主需要显式 wait token/完成接口。

## 7. 值、根与 GC 的落地边界

### 7.1 首个 heap 不假装与旧 heap 自动兼容

首个完整切片优先使用目标 heap/roots，覆盖所需对象、函数、环境和字符串；可以移植现有 payload 算法，但每个 JS 对象只有一个身份和一个生命周期 owner。

若使用旧 heap 的临时适配，必须让每个导入 handle 对应有效的 owning root，明确跨边界值身份、回调和释放。不能复制一个 Object 到另一 heap 后宣称语义等价，也不能在 RC heap 上直接复制无 root 句柄。桥接不进入性能结论；其退出条件是切片值和对象均由目标 heap 管理。

### 7.2 根图

roots 包含：当前运行入口、宿主 persistent roots、realm/code instance、jobs、所有可达 execution 的帧/寄存器/operation/pending，以及外部登记资源中的 JS 引用。挂起 execution 由可达 generator、异步等待关系或宿主 token 持有，不无条件永久注册。

第一版扫描已初始化活动槽，死亡/返回时清空；后续按 safepoint live map 扫描。寄存器之外 this/new.target、actual args、cell、completion、代码绑定与宿主临时值都要有显式 trace 实现。

新增加含 VmWord/HeapId 的状态类型，必须同时说明其 owner 和 trace 路径。测试在分配点强制 GC，覆盖“最后一个引用正在 pending/return/host reply 中”的情况。

### 7.3 收集过程

暂停可收集运行 → 规范状态发布 → 强根标记 → heap 强边遍历 → weak/ephemeron 固定点和 kept-alive 处理 → 回收与资源记账 → 将适当的 finalization 工作交给 jobs → 恢复。不得在收集器内部执行用户 JS。

首版非移动 collector 简化地址变化，但句柄仍必须验证归属和复用。generation 耗尽时退休槽或采用明确安全策略，不能静默绕回。NaN-box 的 handle 位宽压缩等待布局实验，先保持正确身份。

保留内存、GC 暂停和 job 内 WeakRef 行为是验收的一部分。减少 retain/release 不自动证明整体内存管理更好。

## 8. 统一暂停的范围

generator/async 共用可保存的执行状态和根枚举，分别保留 yield、return、throw、await、Promise reaction 的语义。async 函数同步前缀、await 后续 job 顺序以及 generator 重入错误必须明确验证。

连续 arena 下，首版为可独立挂起的执行建立独立存储；如果选择迁移后缀，需修改所有基于偏移的 frame/arguments/live map 引用并验证没有逃逸 Rust 借用。不要同时实现两种存储再让所有调用者承担策略分支。

预算 poll、generator yield、await 和未来 task cancellation 是不同原因。执行状态可以统一；触发与恢复政策不能混成一个随意 yield。完整 Task/Fiber API、取消传播、结构化资源域留给[issue #20](https://github.com/pocket-stack/quickjs-oxide/issues/20)，本计划只交付它需要的明确状态和边界。

## 9. 交付拆分、依赖与完成条件

每个交付项包含代码、相应反例测试、可读诊断和一段最终契约说明。以下不是工期承诺。P0–P2 是目标架构切片，P3 是比较决策，P4–P7 扩展语义和生产入口，P8–P9 增加可选优化并完成清理。

| 项 | 前置 | 可运行结果 | 完成门槛 |
| --- | --- | --- | --- |
| P0 基线与契约 | 无 | 固定当前/QuickJS 语义和性能入口、实际字节码 dump | ELF/负载可复现；现有/新增证据分开；列明宿主/二进制契约 |
| P1 纯数值端到端 | P0 | JS 源码→CFG→寄存器码→数值结果 | 分支/循环/比较/更新/溢出/负零；CFG 与最终码可读 |
| P2 完整架构切片 | P1 | 嵌套调用、对象转换、catch、分配/GC | 本文 addOne 流程可运行；默认普通 JS 不递归 Rust；根交接成立 |
| P3 架构与布局决策 | P2 | 同语义底座 S/R 对比、word/编码实验 | 不靠缺失语义取胜；记录选择、退化和退出方案；删除失败原型 |
| P4 绑定与异常闭环 | P3 | closure/cell/eval/arguments/构造/finally | 所有相关语义回归；返回/throw/清理跨帧完整 |
| P5 对象与内置闭环 | P4 | 属性/Proxy/species/迭代和完整内置 | 内置回调不依赖旧 VM；weak/资源生命周期完整；Array.map 作为首个验收例 |
| P6 可恢复与宿主闭环 | P5 | generator/async/jobs/重入/模块恢复 | 标准 job 顺序、挂起 roots、重复恢复和 host 边界通过 |
| P7 新核心生产切换 | P6 | 所有生产入口使用新 VM | 新入口完整回归/Test262、50+8/原始 V8、native/web/WASM、binary/API 对照通过后切换 |
| P8 有界专化 | P7 且 profile 支持 | 同一新核心可开关的类型块 | side exit/根/效果正确；冷热、类型抖动与真实负载有可信净收益 |
| P9 退出与最终性能 | P7；若做 P8 则在其后 | 旧生产执行器/桥接退出，最终矩阵 | 完整 50+8、原始 V8、默认栈与小栈验证；无测试转旧引擎 |

P5/P6 的算法可以在 P4 期间做独立设计，但应按已完成的 owner/调用协议接入。模块和 async 初始化等共享入口需要在 P6 汇合，不能把“普通函数可用”写成完整引擎。

### P0 必须冻结的内容

- 当前 PR19 的普通 ELF、工具链、负载、输出和 profile；实现开始时记录新的仓库 HEAD 与基线差异，不能自动把旧数字当新 HEAD 现状。
- 当前公开 Runtime/Context/Value、host callback、GC/资源限制、binary-object 格式契约清单；标记必须兼容与允许显式版本变化的项目。
- final opcode/CFG/source 的诊断入口；不从 lowering 的发码顺序推断实际分派量。
- 语义 corpus：数值边界、转换顺序、调用、异常、根、宿主重入。复用有效 oracle/反例，避免镜像实现的测试。

### P1 的内部顺序

绑定基础与 CFG builder → 验证/打印 → 无优化寄存器分配与编码 → 纯数值执行 → 合法优化和 live map。先让输出可核查再优化，不能第一版就让 SSA、编码、融合、NaN-box 和专化同时出问题。

P1 没有 heap/callback 覆盖，结果只能证明纯数值切片。它不能触发生产切换或架构最终定案。

### P2 的内部顺序

目标 heap/root 最小模型 → 普通帧 push/return → pending 根交接 → ToPrimitive operation → JS 回调返回 → catch/unwind → 强制 GC 与宿主根测试。

入口包含数字循环和对象反例，至少证明：callee 参数独立、返回值在回收窗口内存活、左转换不重复、异常位置正确、最后引用覆盖不会破坏 heap。完成后才开始比较整体底座成本。

### 第一批可直接领取的工作项

| 工作项 | 责任与依赖 | 具体交付 | 独立验收 |
| --- | --- | --- | --- |
| A 基线与诊断 | P0 | 固定 receipts、真实发布码打印、语义 corpus 入口 | 同一输入可定位到最终指令与 source；旧输出不被重写 |
| B 最小 CFG | compiler，依赖 A | BindingId、block 参数、分支/循环、验证与 dump | 循环合流和 TDZ 分类反例；dump 能解释每条边 |
| C 字节码发布 | code/backend，依赖 B | 基础操作、寄存器分配、编码/解码、不可变发布 | move 环、Wide、非法跳转和不可达坏码拒绝 |
| D 数值执行 | vm/value，依赖 C | 纯数值循环、统一 Number helper、预算出口 | i32 溢出、负零、移位、循环输出和 source 正确 |
| E 目标根与调用 | heap/vm/runtime，依赖 D | root 登记、最小对象/函数、显式 frame、返回事务 | 参数不别名；强制 GC 下最后引用和返回值存活 |
| F 可恢复转换 | semantics/runtime，依赖 E | Add/ToPrimitive 阶段、InvokeJs/回复、pending roots | 左右转换顺序、返回非原语、getter/回调抛错 |
| G 异常与生命周期闭环 | vm/semantics，依赖 F | catch 路由、operation 异常回复、资源清理 | fault source 正确；强制 GC/失败时无遗漏根或重复恢复 |
| H 切片比较报告 | 测量工具，依赖 G | 数值、调用、对象 miss 的交错计时和计数 | 支持范围明确；新入口独立运行；进入 P3 的问题列表 |

A–H 是领取单位，不要求一项等于一个巨大提交。C/D 可以围绕手工构造的已验证码分别开发，但集成验收必须走源码端到端。E 的对象支持是该切片的最低需求，完整 Proxy/species 等在 P5 收敛；未支持输入不能静默降级。

F/G 至少包含以下可观察行为（示意测试，不是本轮已执行结果）：

```js
let events = "";
const left = { valueOf() { events += "L"; return 40; } };
const right = { valueOf() { events += "R"; return 2; } };
const answer = left + right;
// answer === 42，events === "LR"；恢复不得变成 "LLR"。

let caught;
try {
  left + { valueOf() { throw 7; } };
} catch (error) {
  caught = error;
}
// caught === 7，events === "LRL"。

function change(x) { x = 9; return x; }
let original = 3;
const changed = change(original);
// changed === 9，original === 3。
```

同一组用例在正常收集和“每个分配点尝试收集”的诊断模式下运行。GC hook 属于测试/诊断接口，不给标准 JS 偷加语义；小切片阶段不以完整 Test262 通过作为虚假完成条件。


### P3 不做永久实验框架

优化栈 VM 仅实现共同切片并复用值/heap/语义算法，不扩展成第二个完整长期引擎。编码与 word 实验可用隔离分支/构建，记录固定 commit；不为所有组合增加永久 feature 矩阵。

选择依据包括速度、编译延迟、代码/状态大小、GC/内存及实现可解释性。有限负载的胜出是进入 P4 的依据，不是完整 JS 的最终性能声明。没有可信差异时，优先维护成本较低的实现。

### P7/P9 的迁移退出

旧引擎允许在迁移期作为单独命令/构建的差分 oracle；不允许对某个 opcode、测试名或运行失败自动切旧执行器。未经支持的原型输入明确报未支持。

语义服务迁移一项，就建立唯一新 owner，并登记旧入口的删除条件。新核心切换前完成语义和性能门槛；切换后再删除旧 operand stack/VmHost local 协议、递归普通调用、重复数值语义及桥接。保留历史结果与可重建旧 commit 即可，不要求永久编译旧引擎。P9 根据切换后实际改动补做受影响验证；已有通过结果不无理由重复，最终 receipt 必须准确对应最终源码/ELF。

职责文档、架构检查器、模块 README 和有效 canary 随实际切换更新；不能只改扫描路径或源码指纹，使旧规则看起来通过。公开 API 如需兼容薄封装，封装只转换边界，不复制执行语义。

## 10. 类型块的具体后续计划

P8 不进入第一批重写。先给 R baseline 加 runtime-local 的有界热度/tag 统计；仅为热点直线数值区间建立版本。计划描述输入 guards、操作序列、出口和 baseline 状态映射。

第一版只跨无回调、无 heap 分配、无可见效果的计算区间消除重复标签检查。checked Int 溢出直接保持正确 Float 结果或在该操作之前退出；已经完成的效果不能重放。

每个出口需要 baseline InstrId、寄存器来源和完整根物化规则；call/GC/debug/suspend 前恢复规范状态。计划版本有容量上限，活动执行持有有效版本身份，不被 cache 回收破坏。

不先做 OSR、跨函数内联或 JIT。若“块执行”仍逐个解释一套微操作而没有减少真实机器指令，就保留 baseline 并移除这层。算法与 Number 语义仍是同一份，不能在类型块中再维护一套近似浮点规则。

## 11. 如何新增功能而不破坏结构

### 新增一条纯数值 opcode

定义操作数/效果规格 → 在共同 number 算法实现语义 → 接入 vm 数值分派 → 需要时接入 compiler lowering → 验证编码/解码及相关语义反例 → 测量是否值得单独 opcode。无需改 runtime driver、GC、宿主接口或每种 continuation。

### 新增一个可能调用 JS 的内置

在 builtin 家族中定义命名阶段和 trace → 通过 semantics 返回 InvokeJs/属性等请求 → operation 接收成功或异常回复 → 使用共同 driver。必须给出正常、回调抛错、清理、强制 GC 的例子。无需增加一套执行循环。

### 新增优化 pass

注明保持的语义、输入/输出不变量和 maps 更新 → 接入固定 pass 序列 → 输出可读前后 IR → 用会失败的反例和差分执行验收。无需修改 Value 或根表，除非明确引入新的运行时表示。

### 新增暂停来源

定义等待 owner、恢复 token、成功/异常/丢弃路径及 roots → 接入 runtime 的已有状态转换 → 单独定义 job/调度政策。不能直接从任意 Rust helper 返回一个 yield 标志就假定所有调用者可恢复。

## 12. 验收：正确性、性能与维护性

**最终完成条件是两张清单同时通过**：第 1 节的必做架构能力全部落实到生产路径，第 1 节的问题清单逐项提供结果和证据。任何尚未解决的问题、缺失的架构能力都要明确保留为未完成。第 10 项独立归因未支持性能假设时，应以诊断证据结论记录，不虚报优化收益；其状态/观察契约仍须完成。

阶段记录分列“架构能力完成情况”和“问题解决情况”，避免用总体进度掩盖任一侧缺口。

**正确性**：每阶段跑相关真实语义反例；新核心生产切换前跑完整回归、QuickJS oracle 和 Test262，对照冻结基线的逐项结果。先确认原型入口实际使用新 VM，不能以旧入口通过作证明。GC 每个分配点、异常/恢复每个阶段、专化每个 side exit 都需要可强制触发的诊断方式。

**性能**：以相同工具链/构建、固定输出，至少五轮交错；敏感/退化项追加十轮及独立 instructions/cycles。数值循环、调用、Crypto、Navier-Stokes 是重点，同时保留字符串/BigInt/对象转换、冷启动、分配大对象和挂起状态的控制负载。完整 50+8 和原始 harness 在语义覆盖完成后验收；超时、栈溢出、未支持和错误输出分开记录。

**资源**：测 native frame、字节码/plan/.text 大小、分配与复制次数、峰值/保留内存、GC 暂停、编译时间。时间收益伴随无法解释的内存暴涨不算通过；不预设未经数据支持的加速倍数。

**维护性**：在实现评审中实际走一遍第 11 节的四种修改。检查是否存在万能 Runtime 参数、双份 Number 算法、无 owner 的 VmWord、循环模块依赖、匿名阶段数字、永久 legacy 回退或为实验暴增的配置组合。源码结构变更需让这四条修改路径更清晰。

每个阶段结束留下短决策记录：最终接口、保留的不变量、真实运行命令与 receipt、未解决的问题、下一阶段前置条件。实验废案从生产代码删除，解释保留在记录中。

## 13. 第一批建议范围

第一批围绕第 5/9 项的原语与局部执行、第 1/7 项的调用与帧建立诊断和方案；第 10 项先量化。P0–P2 验证必做架构的端到端切片，P3 选择具体实现与后续迁移方式，不决定是否放弃架构目标。第一批同时触及 compiler、code、value/heap、vm、semantics 和 driver，这是有意选择完整流程；但只承诺明确的语言切片，通过同一个 PR 内的独立 commit 保持变更可审查；最终 PR 完成全部架构迁移与问题验收。

提交应沿可验证增量组织：诊断/契约，纯数值端到端，heap/root 与调用，转换/异常恢复，完整切片证据。中间提交允许处于实验入口，但每次有自己的运行结果和清楚的支持范围。目标执行器的生产默认入口在 P7 对应的 C30 切换；能独立修复问题的变更可以提前作为本 PR 内的 commit 验证，但不单独合并或替代最终双重验收。

最终交付必须同时具备完整的目标执行架构和 issue #16 本轮问题的解决证据。当前文件和接口的复用只有在帮助达到这两个目标时才有价值。

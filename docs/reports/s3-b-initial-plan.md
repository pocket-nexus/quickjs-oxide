# S3-B 首批实施计划：只读 QuickOp 与基础派发

> 状态：2026-09-22 已实现 B1a 测试模块与 B0 编码约定；B0 的性能输入交接尚未关闭。
> 本轮实现与验证见 [C/B 开头实施记录](s3-c-b-opening.md)；B1b–B1e 尚未接入或验收。
> 与 [S3-C 计划](s3-c-plan.md) 配套，依据
> [performance-architecture.md](performance-architecture.md) §5、§7、§11。
> 本文把 §5.4 第一步“发布译码＋切换派发”拆成可与 C 并行的工作包。
> 初读源码为 `60d1a9ee`，收尾复核为外部工作推进后的 `d4f78697`。
> QuickOp 当前仅在测试配置编译，生产发布和派发仍按后续工作包推进。

## 0. 首批范围与交付边界

首批交付一个**只读、已认证、按规范 PC 索引的 8B QuickOp 执行投影**，
并在通过门禁后让选定热指令直接从该投影执行。复杂指令继续复用现有规范
handler。目标是建立 B 后续特化所需的执行载体，验证译码与派发成本。

首批不实现自适应重写、warm-up counter、AddInt/guard/deopt 状态机、
直接 IC 槽访问、call-site specialization、压缩 fusion 跨度或新的派发机制。
这些属于 B 后续批次；本文的“首批完成”不等于方案 B 全部完成。

执行顺序：

**B0 接口与编码冻结 → B1a 纯译码与验证 → B1b 发布只读投影 →
B1c 共享热 handler → B1d QuickOp 基础派发 → B1e 默认切换裁决。**

- B0/B1a 可立即与 C 独立并行。
- B1b 可在独立改动中开发；与 C4 同改发布/fusion 文件时排队集成。
- B1c/B1d 必须等待 C 已接受或已回退的栈接口冻结，顺序修改 `run/stack`。
- B1e 以 C1–C4 接受的 match 快照为隔离分母，以 E 后 saved 为后续路线累计分母；
  C5 若采用再加真实 C-final 保护比较，详见 §6。沿用 C 的无 PGO 发布配置。
  C 设计准备与 B0/B1a 不因测量前置缺证据而停下。

## 1. 与 C 的分工和并行接口

| 事项 | C 负责 | B 首批负责 | 集成约束 |
| --- | --- | --- | --- |
| 值与操作数 | TOS、consume/keep store、pending owner、flush | 只调用 C 的 facade | B 不直接操作 `store.slots`、depth、TOS 字段 |
| 规范指令 | 保持 `Instruction`/BC5 不变 | 派生 QuickOp | 不新增序列化 opcode |
| 静态 fusion | 候选选型、跨度认证、handler 语义 | 保留并复用已接受跨度 | 首批不压实、不重新定义 span |
| PC | fault/resume、观察边界、挂起保持规范 PC | identity mapping | 不增加第二种外部 PC |
| release/GC | 沿原契约并由 C 补齐缓存出口 | QuickOp 不持有堆 owner | 不新增 quick 专属 release/deferred 路径 |
| 派发 | match 基线；C5 可选实验 | 紧凑 word 的热/冷分类 | 首批不混入 C5 函数指针实验 |
| 测量 | 认证 baseline、矩阵与噪声 | 增加译码/IR/冷启动证据 | 同机串行计时，不因开发并行而并行跑 benchmark |

交接给 B 的 C API 至少包含逻辑 `depth/peek/push/pop`、直接 binding store、
Number 事务、canonicalize、RunExit、ProgramCounter 和 fusion handler 行为。
C 不要求启用 TOS 才能交接：若 C2/C3 失败回退，B 使用相同 facade 的 canonical
实现。B 的成功不能建立在“再绕过一次 C 的检查”上。

## 2. 只读执行投影设计

### 2.1 固定 8B word，有限热集合

建议新增 `src/engine/code/quick.rs`，类型均为 crate 内部：

```rust
#[repr(transparent)]
struct QuickOp(u64);

// bits  0..=7  : QuickTag
// bits  8..=15 : flags
// bits 16..=31 : aux
// bits 32..=63 : operand
```

加入 `size_of::<QuickOp>() == 8` 静态断言。所有 pack/unpack 使用整数掩码、
移位与 checked conversion，不使用 union、transmute、裸指针或 unsafe。
QuickOp 仅含数字元数据，没有 `JsValue`、Runtime、atom root 或其它 owning edge。

首批字段规则：

| 字段/情况 | 规则 |
| --- | --- |
| QuickTag | 私有稳定枚举，显式数值；只编码选定热集合与 `GenericCanonical`，不绑定 BC5 wire tag |
| flags / aux | 首批未使用位必须为零，验证器拒绝不合法组合；不携带未启用的 feedback |
| PushI32 | operand 保留完整 i32 位模式，解码重建相同有符号数 |
| 常量/规范分支目标 | 若进入热集合，用完整 u32；branch target 仍为 canonical PC |
| local/arg 等 u16 | 若后续首批接入，检查原始宽度、零扩展；不以截断代替验证 |
| 多操作数/复杂结构 | `GenericCanonical`，从当前 canonical PC 查原指令，不复制到宽侧表 |
| Generic 的 PC | 不编码在 operand 中，由上下文的 usize PC 给出，避免引入新的代码长度限制 |
| 任何放不下的扩展字段 | 译成 Generic，不能拒绝原本合法的函数、截断、或伪装成分配失败 |

aux 暂不承诺长期用途。以后嵌入 u16 IC index 时必须有 sentinel 与溢出回退，
且同时覆盖 read/write site；此能力不在首批启用。允许未来扩展编码版本，但
不承诺全部规范 opcode 加所有 specialization 永久装进 u8。

### 2.2 一条规范指令对应一个 word

首批在 `Words` 模式固定以下关系；`CanonicalOnly` 是 §2.4 的函数级认证模式，
不持有 quick 数组，进入函数时直接选择 canonical 执行：

```text
quick.len() == canonical.code.len()
quick_pc == canonical_pc
```

不分配双向 PC 映射数组，不压缩或删除 fusion interior word。
跨度命中时按照原 `FusionPlan` 长度推进 `next_pc/resume_pc`，跳过保留的 word；
失配仍可从正确的规范位置执行。所有 branch target、fault/resume、pc2line、
Gosub/Ret 的整数返回地址、IC site 和挂起恢复地址原样保留。

已有 fusion 入口若仍属 Generic，就直接沿原 handler 运行。不要新增一种
“interior word 永远不可执行”的格式：规范语义与测试仍需独立处理对应 PC。
以后把融合跨度压成单 QuickOp 是另一项设计，届时才增加规范/执行 PC 映射门禁。

### 2.3 译码与验证必须复用规范契约

`Instruction` 仍是栈效果、控制流、操作数和潜在效果的权威来源。translator
显式覆盖当前 enum variants，新增 variant 应触发编译期审查，不用无条件 `_`
把所有未来 opcode 静默纳入某个热 handler。

纯译码输入是已验证 code 及必要的不可变 metadata；输出每个 PC 的 QuickOp。
验证至少检查长度、合法 tag/位、该 PC 允许的只读 tag、参数位保真、分支目标
一致、Generic 的规范来源，以及首批热 handler 的栈/控制效果与规范一致。
用 `stack_contract`、`control_effect`、`operand_contract`、`potential_effects`
等现有单项契约，不依赖仅 `profiling` 构建才存在的 `Instruction::info()`。

执行层不能从不可信任的字节流直接构造 QuickOp；生产构造器私有并限于认证
publisher。测试可构造损坏 word 验证拒绝。首批不保存“运行时允许重写集合”
侧表；后续真的开始 quickening 前，另行认证 Generic→specialized 的等价集合。

### 2.4 发布、共享、回滚

真实发布点位于 [heap/allocation.rs](../../src/engine/heap/allocation.rs) 的
`allocate_function_bytecode` 流程：metadata/payload/generic bytecode 验证完成，
随后重建 `FusionPlan`，再 reserve、retain edges、publish。

新增 quick projection 按下列顺序接线：

1. 清除 draft 携带的旧 executable/quick，禁止调用者把别的函数或旧 code 的
   quick 带入；与现有 fusion 派生投影同样从确切 payload 重新生成。
2. 在验证和 fusion 构建后、reserve/retain edges 前译码并验证 quick。
3. 编码不适合只选择 Generic；word buffer 的可恢复分配失败映射
   `HeapError::Allocation`，由现有 publisher 错误链清理 atoms、converted
   constants 和 child roots。不发布半成品，不把分配失败静默当 cold fallback；
   Rc 小控制块的全局 allocator OOM 策略与可恢复失败严格区分，见 §2.5。
4. 在 [heap/code_records.rs](../../src/engine/heap/code_records.rs) 的
   `FunctionBytecodeData` 保存只读 `QuickProgram`，在
   [code/executable.rs](../../src/engine/code/executable.rs) 的
   `PublishedFunctionData` 共享其数组；多个 closure/snapshot 只 clone Rc。
5. QuickProgram 无堆边，不改 GC edges。失败与正常销毁释放 Rust sidecar 即可。
   保持 root/domain authentication 的现有规则。

首批 `QuickProgram` 明确区分 `CanonicalOnly` 与 `Words(Rc<Vec<QuickOp>>)`：
若整个函数没有首批热 opcode，发布时可认证为 CanonicalOnly，避免全 Generic
函数多分配 8N。Words 一定覆盖全部 PC，不允许“缺数组/长度错时悄悄降级”。
draft 的未构造状态与正式 CanonicalOnly 证书必须可区分，防止遗漏发布接线。

`snapshot_function_bytecode_owned` 的 OnceCell 继续只建立共享 snapshot；
首批不把译码暗移到首次执行以美化 compile 数字。若后续评估 lazy 建表，必须
单独设计并计入 first-execution 代价。

测试中的 `empty_for_test` 及可变 synthetic snapshot 需要显式 finalize/rebuild
quick，或明确选择 canonical 测试模式。不得让生产 fallback 掩盖 stale fixture。
生产快照保持不可变，BC5 round-trip 仅序列化 canonical code；两条编译/BC5
发布链均经共用 publisher 获取同一投影。

### 2.5 总内存与错误处理的真实成本

保留 `Rc<[Instruction]>` 的同时增加 8N QuickOp，首批**总字节码内存增加**。
只能说热取指目标是 8B word，不能说总内存减半。预算必须包括 canonical code、
QuickOp、Rc 头/allocator、PublishedFunctionData/FunctionBytecodeData 大小、
fusion/IC 侧表及 translator 的峰值临时内存。

修改 [heap/profiling.rs](../../src/engine/heap/profiling.rs) 的存储计量，按
数组身份去重 quick bytes；多个 closure 不重复算也不复制数组。记录每函数
长度、每类 tag 数量、Generic 比例、发布时间与峰值。若给 FunctionBytecodeData
加字段使统一 arena 最大槽变大，也必须按实际 `ArenaSlot` 布局与全堆 RSS 计入。

translator/validator 应为 O(N) 时间与至多 O(N) 新存储，不能逐 PC 扫描剩余 code
或复制整个 descriptor。首批固定用 `Vec::try_reserve_exact(code.len())` 预留
word buffer，只填已预留容量，然后 `Rc::new(vec)` 共享同一 buffer。禁止
`shrink_to_fit`、转 boxed slice 再转 `Rc<[QuickOp]>` 等隐式大数组重分配。
Rc 内的 Vec 不暴露可变入口，发布后只读；记录 capacity×8、Vec header、Rc
控制块与 allocator 开销，不能只记 len×8。

`try_reserve_exact` 的失败可映射和测试回滚；`Rc::new` 的小控制块分配沿用
项目现有 std 全局 allocator 的 OOM 终止策略，不宣称能被 Result/rollback
捕获，也不以稳定 MSRV 未提供的 `try_new` 能力为前提。失败注入测试只模拟
可恢复边界；文档与报告不得把两类 OOM 合并成“所有分配失败均可恢复”。

## 3. 首次派发设计

### 3.1 仍然只有一个执行核心

`run` 仍创建同一个 `FrameTransaction`、短 `RunSlots` 与 `ProgramCounter`。
不复制两份 198 臂解释器、不新增递归解释器、不加入 fn-pointer threading。
实验选择在函数入口或独立构建级别完成，不给每条指令添加公开 runtime 模式判断。

Words 模式循环：读取 QuickOp → 小 `QuickTag` match → 热 handler 直接操作
C 的 facade → 共用 PC/profiling epilogue。只有 Generic 或仍无效果的 guard
decline 才取该 PC 的 `Instruction` 并进入现有 canonical 语义。

禁止所有 QuickOp 先还原完整 `Instruction` 再进原 giant match；该形态最多
用于最初验证脚手架，不能作为通过性能门禁的派发方案。热成功路径不应再读取
canonical instruction；profiling 的逻辑元数据按需要另读，普通构建不为此付费。

第一版热集合优先选不破坏既有融合入口的简单指令：`Nop`、`PushI32`、
Undefined/Null/Bool、`Goto` 等。`ReturnUndefined` 等出口只有复用完整既有
协议后才纳入。具体纳入集合由 C0/B0 profile 固定，不为了凑覆盖率增加 opcode。

`GetLocal`、`PushConst`、`GetField/GetField2`、compare-branch 等现有 fusion
入口首轮可保留 Generic。后续在首批内扩大 Number binary/branch 集合时，必须
先证明不会截断既有 AddStore/LocalAdd/borrowed-base/method span 的路径。

### 3.2 handler 共享与 fallback 规则

1. 先抽出选定热 body 的窄函数/宏，在 canonical 模式验证语义与 codegen。
   输入是已解码的小参数，操作数通过 C facade 读写，不重新 root/unroot。
2. 成功只经过一次 QuickTag 派发；Generic 的多一次分类成本单独测，不隐去。
3. 热类型 guard 在任何 mutation 前失败，可走同 PC 的 canonical handler；
   一旦消费输入或提交结果，不能以 `Generic` 为理由重跑该指令。
4. numeric 的 resident fallback 显式携带 `NumericKind` 或等价窄分类，复用
   原 completion。避免为了选 fallback 再还原完整 Instruction/重新匹配全部臂。
5. handled/declined/RunExit/Error 的 PC 推进与缓存恢复完全沿 C 协议。B 不能
   把 Declined 与“已消费输入、准备抛错”合成同一返回值。
6. 不因 Generic 边界无条件 spill TOS；只有 C 的 canonical-only helper 或
   观察边界要求 spill。一次额外 spill 也要进计数，防止 B 吞掉 C 收益。

### 3.3 现有融合与 IC 的保全清单

| 现有机制 | 首批保留方式 |
| --- | --- |
| UpdateLocal、CompareBranch | 仍读取对应 canonical span facts，成功跳原长度 |
| AddStore、LocalAdd、常量左 concat | 不更改 guard、实际 Add fault PC、失败时的规范输入/输出前缀 |
| GetField2＋纯参数＋CallMethod | 保留逐步提交与最终 CallMethod PC 出口 |
| borrowed-base field | 保留 run 中相邻指令识别、绑定持有的基值边与零临时 base owner |
| property IC | 首批仍按 canonical PC 调 `PropertyReadCacheTable::site/write_site`，不同时改 rank 查找 |
| C 新增 span | 按最终接受清单保留；未接受的实验不迁入 B |

非 profiling 与 profiling 两种构建都要验证；逻辑指令/span 权重、owner/GC
计数应在同一执行模式选择下等价，真实 dispatch 分类次数另计。

## 4. 分阶段工作包

| 阶段 | 具体产物 | 估算 | 可与 C 并行 | 关闭条件 |
| --- | --- | --- | --- | --- |
| B0 | 编码/热集合/PC/接口/测量 receipt | 0.5–1天 | 是 | C/B 共享契约冻结；前置证据状态明确 |
| B1a | `code/quick.rs` codec、translator、validator、独立测试 | 2–3天 | 是 | 所有 variant 有处理，边界/损坏编码测试通过 |
| B1b | verified publication、共享投影、回滚/内存计量 | 2–3天 | 开发可并行；发布点顺序集成 | 只建不执行的正确性、compile/RSS 达标 |
| B1c | 热 body 共享、canonical-only 构建 | 1–2天 | 否，等待 C 接口稳定 | 单纯抽取不改变语义、不引入显著回退 |
| B1d | compact hot dispatch＋Generic fallback | 2–3天 | 否 | 差分、PC、fusion/C收益保持，全量门禁通过 |
| B1e | 三档比较、默认切换或关闭结论 | 1–2天加测量时间 | 测量串行 | 满足 §6 或明确不启用/回退 |

### B0：契约和输入冻结

1. 接收 C0 的 E/残余/A4 决策及 input manifests，并接收 C 交付的
   `pre_a_release` 源码/构建/输入 receipts 与未关闭回退台账及归属；不自行改
   值表示或测量协议。
2. 固定首批热集合、Generic 覆盖清单、word 位格式、构造可见性、错误类型。
3. 冻结 C facade 与规范 PC 协议；列出 `run.rs` 里首次接入会碰到的 fusion
   入口、fallback 和 exit。碰共享文件的工作由同一集成者排队。
4. 为每个函数记录 tag 分布，估算 8N 与新增 arena 槽大小预算。决定哪些全冷
   函数进入经过认证的 CanonicalOnly，不以“字段缺失”作为判断。
5. 冻结 §6 的三档对照与成本上限，建立 `target/s3-b-initial/<run-id>/`。

### B1a：纯模块，可立即并行

1. 新增 QuickOp/QuickTag/QuickProgram 与 pack/unpack；单测先覆盖数值边界、
   保留位、未知 tag、长度不符、错误 PC 参数、完整 enum 分类。
2. translator 只依赖 code/metadata，不引用 VM、Runtime 或 heap owner。
3. 从真实编译结果取 canonical code 做逐 PC 验证；测试所有复杂形落 Generic，
   小字段溢出不截断，source contract 未改变。
4. 将性能版本与测试 scaffold 分开；任何临时 `QuickOp→Instruction` 还原
   只供测试，不进入正式热循环。
5. 证明时间/内存线性增长，并检查 fallible allocation 的所有转换点。

此阶段可先独立交付，C 尚未完成时不接入 run。生产尚不使用的模块不靠
全局 `allow(dead_code)` 消除检查；未接线的实验保留在测试配置或独立工作包。

### B1b：发布建表，仍执行 canonical

1. 接线 `FunctionBytecodeData` 与发布认证顺序；所有构造器/默认 draft/tests
   明确新字段状态，不能复用旧 cache。
2. snapshot 共享同一 Rc；增加同函数多 closure/snapshot 的 ptr identity 和
   strong-edge 不增长测试，外 runtime 被拒绝。
3. 对失败注入验证 reserve/retain 前后的清理。既有无注入设施时先增加窄测试
   hook，不让测试实际耗尽主机内存。认证失败不留下可执行投影或 atom 泄漏。
4. synthetic fixture 在 code 最终确定后重建投影；BC5 两条发布路线共享测试。
5. 增加 profiling 的译码阶段计时、IR 常驻/峰值/共享去重。公开报告 schema
   若有新增字段，按现有兼容规则更新 parser/CLI 测试。
6. 运行“只建不执行”对照。若建表成本越界，先收窄覆盖、避免冗余分配；
   不通过 lazy 移动计时边界或删除 canonical code 掩盖成本。

### B1c：共享 body，先测重构本身

1. 在 C 最终 accepted tree 上抽选定 hot handlers，canonical dispatch 仍默认。
2. 保留内联与宽 Error 的冷路径边界，调用 C helpers，不整片抽空 giant match。
3. 核对编译后的 `run` 热成功路径、错误路径与 `.text`；通过 C 完整正确性门禁。
4. 相同源码/flags 配对测量，重构本身若显著回退，先解决或撤掉，不能留给
   QuickOp 的预计收益冲抵。

### B1d：切取指和热分类

1. 内部测试入口或隔离构建选择 canonical/quick；默认生产暂保留 canonical。
2. 实现 §3 的小 tag match、热 handler、Generic 路径及共用 epilogue。
3. 一次只扩一组热 opcode；generic ratio、二次取 canonical 的次数、TOS spill
   与现有 fusion 命中率同时入账。不要仅看 dispatch 数变少。
4. 在新 runtime 上运行 canonical/quick 差分，随后完整 Test262、oracle、
   profiling、BC5、host/suspend 边界。共享 IC 或 profile 状态不能从一边带到另一边。
   必须包括 hot→Generic→hot、Generic fusion 跳过包含热 word 的跨度
   （例如 method-call 的 PushI32 参数），以及错误/挂起出口的 C 缓存恢复。
5. 与最终 C baseline 比较，若 C 的基准收益消失，按 Generic flush、融合覆盖、
   helper 内联顺序归因，最多两轮有证据的局部调整。

### B1e：首批裁决与后续交接

1. 冻结最终快路径集合，清理临时开关和测试 scaffold，测真实默认候选。
2. 完成 §6 全部对照与内存/首次执行门禁，记录启用、保持 canonical 或回退。
3. 默认启用需 correctness＋成本门槛全部通过；首批允许没有显著净加速，
   但必须明确它只是后续特化的地基，不拿预期 B 的10–25%填表。
4. 若执行或内存成本不通过，不启用 QuickOp，不给生产默认留下无消费者的
   eager sidecar；可保留纯 codec/验证测试与设计，停用发布分配路径。
5. 向 B 后续批交付编码契约、canonical PC 身份、发布证书、热/Generic 分布、
   失败分类与全部 receipts。下一批才设计重写集合、反馈状态、guard/deopt
   和直接 IC 槽，分别实施并测量。

## 5. 测试计划与可执行检查

### 5.1 新增测试模块（以下名称为实施时新增，不是已有命令）

统一使用 `quick_` 测试名前缀，建议独立模块：

| 模块 | 必测内容 |
| --- | --- |
| `code/quick` codec tests | i32 MIN/MAX、u16/u32 边界、全保留位、非法 tag、Generic 复杂参数、不同端序下纯整数语义 |
| translator/validation | 每个 canonical variant 分类；长度/参数/tag 对不上拒绝；规范 effect 一致；未来 enum 新成员触发审查 |
| publication | stale draft quick 被重建；分配/认证失败回滚；同函数 snapshot/closure 共享；跨 runtime 拒绝 |
| synthetic/BC5 | test fixture finalize；两条发布链投影等价；序列化字节/版本不变；read→publish 后重建 quick |
| execution differential | arithmetic/compare/branch/local/arg、Generic、所有选定热 opcode 的 fallback、所有 RunExit |
| canonical PC | try/catch/finally、Gosub/Ret、异常行号、span 子 PC、yield/await 的 next/throw/return/resume |
| semantic ownership | getter/Proxy/coercion 只执行一次、TDZ/const/capture/mapped arguments、host reentry/tail call、最后 owner release |
| resource shape | 只编译不执行、多小函数、大函数、相同 bytecode 多 closure、反复发布/释放、译码线性增长 |

复用 [C §5](s3-c-plan.md#5-正确性验证矩阵) 的真实测试与全部门禁，特别是
profiling 配置下的融合测试、部分 numeric 输出失败与 PC/unwind 测试。
不是只测“算术结果相同”就可以切换执行入口。

### 5.2 每个工作包的命令

先按 C §8.1 设置可认证源码、同协议构建与唯一证据目录；B 使用自己的目录，
不覆盖 C 的结果。实现新增 `quick_` 测试后运行：

```bash
set -euo pipefail
cargo fmt --all -- --check
python3 scripts/checks/check-source-layout.py
cargo test --locked -p quickjs-oxide --lib quick_
cargo test --locked -p quickjs-oxide --lib --features profiling quick_
cargo test --locked -p quickjs-oxide --lib --features profiling fusion
```

B1b/B1c/B1d 每个被接受单元运行 C §8.2 完整门禁与 current-source Test262。
B1e 再跑全部固定测量。测试日志须核实实际测试数非零，不能让不存在的过滤器
返回“0 passed”充当通过。B0 只有设计时不执行上面的未来测试过滤器。

BC5 source pin、默认/profiling/host 配置、外部输入缺失、source stale 的处理
全部复用 C 文档，不另设较松门禁，也不更改 `current.conf`。

## 6. 测量设计与接受门槛

### 6.1 三档逐项分离，避免把成本藏起来

在 C1–C4 最终接受的 **match 派发快照**上冻结三档实验构建，toolchain/flags/
输入完全相同。若 C5 也被接受，另存真实 C-final 为额外保护分母，M2/C-final
也须满足执行保护门槛；不能通过放弃 C5 的收益掩盖整合回退。

| 档位 | 构建与执行 | 与哪一档比较 | 说明 |
| --- | --- | --- | --- |
| M0 | canonical；不生成 quick | E 后 saved 另作后续路线累计分母 | B 首批真正起点 |
| M1 | eager 生成/共享只读 quick，执行仍 canonical | M1/M0 | 纯建表、布局与内存成本 |
| M2 | 同一 eager quick，启用热 QuickOp dispatch | M2/M1 与 M2/M0 | 派发变化及首批净结果 |

模式选择只存在于内部实验，不能给正式每条指令增加开关判断。每档有源码
manifest、二进制 hash、flags/模式 receipt；最后再测移除实验选择后的默认构建。

与 C 同时评估时，核心语义测试保留 C canonical/cache × B canonical/quick
四组合；若某 C 缓存方案已撤销，就记录该组合不适用，不为凑矩阵恢复失败实现。
性能主分母是上述 accepted C match 快照；另有 C5 时同时报告真实 C-final。
没有相应快照之前只能作方向性实验。

另保留 C 交付的 `pre_a_release` 对照：双方 fat LTO、CGU=1、无 PGO、无
profiling，用于追踪 A 的历史回退。它不替代 M0/M1/M2 和实际 C-final 的既定
门禁；E 后 saved 只表示后续路线累计收益，不能单独证明 A 的回退已追回。

### 6.2 数据集合

完整复用 C 的86 scaling、58 fixed×10轮、67 compile×10轮、9 original×5轮、
property、microbench、V8 与 BigInt 固定工作量。运行 C §8.3–8.6 时将
`C_BEFORE/C_AFTER/C_OUT` 等参数绑定到本轮 M0/M1/M2，目录放在
`target/s3-b-initial/<run-id>/m1-vs-m0`、`m2-vs-m1`、`m2-vs-m0`，每组唯一。
重复 suite 测量按噪声与门禁需要安排，不能并行跑三档以节省时间。

B 额外冻结以下 workload/指标：

- 仅 compile、不 execute；冷 eval 的首次发布与首次执行；已发布函数热重复调用。
- 大量短函数、一个大函数、深层嵌套函数、同一 code 创建很多 closure；确认
  sidecar 共享且不会按调用次数复制。
- 反复 eval/释放、GC 后 live code/quick bytes；sidecar 与旧 code 一起回收。
- 译码时间、peak scratch、quick bytes、CanonicalOnly 比例、Generic 执行比例、
  间接/直接派发、C 的 spill/owner/fusion 计数和实际 `.text`。

compile probe 只证明其公共 compile API 时间区间；首次执行不能因此省略。
计时普通 release、双方 fat LTO/CGU1/无PGO；PGO 复核双边重训、单独表格。
历史输入仍无法认证时，与 C 同样不关闭正式完整验收。

### 6.3 首批专用门槛

本节是新增实施门槛，不是实测。C 的“至少目标2%加速”规则不直接套在 B1a/b
的基础设施上；改用受控成本，但派发候选不能无限制回退。

| 项目 | 门槛 |
| --- | --- |
| 语义/PC/GC/BC5 | 零回归，任何失败立即停止；禁止以性能补偿 |
| 执行保护 | M2/M0 每个执行时间矩阵 geomean≤1.01；Score 用 M0/M2≤1.01；每项>3%复核，重复仍>5%则不启用，3–5%需归因修复 |
| 建表/compile | M1/M0 compile geomean≤1.02；冷首次执行/发布单列，同样审查>3%的单项，重复>5%不通过 |
| 派发本身 | 单列 M2/M1，instructions、canonical 二次读取、Generic 税与 C 流量证据能够解释方向；没有收益不包装成 quickening 效果 |
| 常驻内存 | 记录理论8N＋结构体/allocator增量与实际 RSS；超过 C 的 `max(3%,1MiB)` 复核后须减小或收窄；持续增长/closure重复分配立即失败 |
| 实现复杂度 | 热 body 不复制语义，不绕过 C facade；不为首批加入通用 deopt 引擎/双映射/宽 per-PC descriptor |
| 规模 | 译码/验证时间和新增存储随 N 线性；无额外 owners；全 Generic 函数不额外保存无消费者的数组 |
| 证据不确定 | A/A 噪声或输入缺失按 C 规则处理；不确定结果不能用于切默认 |

最多两轮有 profile 依据的接线调整。若仍不能达到成本上限，停止 B1d 默认
接入，保留规范执行；纯译码/测试可保留。需要更大内存预算或更激进 lazy/压缩
设计时，另写裁决，不在本文首批里静默放宽。

## 7. 开工与交付清单

现在可并行开始的任务顺序：

1. B 实施者完成 B0/B1a；C 实施者继续 C0/C1。两边共享只读的源码/测量身份，
   不同时改 `run.rs` 或 `stack/window.rs`。
2. B1a 通过后开发 B1b，明确每个 draft/BC5/fixture 构造点；发布集成点按 C4
   时序排队。C 尚未稳定时 run 保持 canonical。
3. C1–C4 的接受/回退与 facade 冻结后，先 B1c，再 B1d；以该 match 快照重建 M0，
   另保存实际采用 C5 时的 C-final。
4. B1e 按三档与 saved 分母完成验证，并保留 `pre_a_release` 累计台账；
   记录“默认启用”或“未启用、原因”。
5. 两份计划互相更新实际接口/阶段状态，不改历史实测数字。C5 派发实验若采用，
   先恢复统一 match 分母独立测 B，组合收益另做一轮，不混入首批归因。

首批交付包至少包括：

- 编码规范与 validator、只读发布/共享实现、hot/Generic 处理清单。
- canonical PC/owner/fusion 的差分测试与完整一致性 receipt。
- M0/M1/M2 源码与二进制身份、执行/compile/首次执行/内存结果、失败样本。
- `pre_a_release` 同协议 receipts、历史回退累计结果与未关闭项的后续归属。
- 启用/回退决定、剩余成本和下一批入口，不宣称已经拥有 adaptive quickening。

后续 B2 的入口要求是在首批稳定基线上设计运行时允许重写集合、反馈所有者、
guard 失败后的单指令回退、状态 reset/backoff、IC slot 溢出与错误 PC。
这些设计不反向阻塞本批的独立译码工作，也不能提前以未验证的 `Cell<QuickOp>`
或 feedback 字段进入只读首批。

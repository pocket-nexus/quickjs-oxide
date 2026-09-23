# S3-B/C 实现交付记录

> **2026-09-23 撤回落：** 阶段 C（C1–C4）已按 benchmark/profile 负结果整体
> 撤销，B 首批因与 C 实现耦合一并回退，代码树回到 B/C 起点 `bcfb4fe5`。
> 撤销依据与“为什么 C 没有用”的归因见
> [阶段 C 负结果与撤回落](s3-c-negative-result.md)。本文以下内容保留为实现与
> 验证的历史记录；文中源码链接指向已删除的实现，不再代表当前代码树。

> 日期：2026-09-22。C1–C4 与 B1a–B1d 已实现，§1–§2 描述修复后的接口与接线。
> **实现完成、测试通过、性能接受是三个不同结论。**默认未启用实验路径；
> §4 为旧 C1 数据，§5 为修复前 `d1b52c9a` 的集中正确性验证，§7 为该版本的
> profile，二者归档于 `f3f152d8`。这些历史数值与结论保留，不能代表修复后源码。
> 修复后的验证与性能结果待新的 §9 补录；这里不预先声明收益或性能验收。

本文记录 [C 计划](s3-c-plan.md) 和 [B 首批计划](s3-b-initial-plan.md) 的实际交付，
补充此前 [开头记录](s3-c-b-opening.md) 与 [第二批记录](s3-c-b-next.md)。
旧文档中“C2 未实现”“B1c/B1d 未启动”属于此前状态，不代表当前工作区。

## 1. C1–C4 的具体实现

| 阶段 | 已实现接口与行为 | 主要源码 |
| --- | --- | --- |
| C1 | `store_local_from_top`、`store_parameter_from_top` 使用 `StoreMode::{Consume, Keep}`，返回 `Result<Option<JsValue>, Error>`。Consume 将 top owner 移入已认证 Direct 的值成员；Keep 先复制独立 owner；返回 displaced `JsValue`，由原 handler 决定释放。目标、容量、TDZ/captured/mapped 边界保持原路径。 | [stack/store.rs](../../src/engine/vm/stack/store.rs)、[stack/window.rs](../../src/engine/vm/stack/window.rs)、[run.rs](../../src/engine/vm/run.rs) |
| C2 | `FrameTransaction` 拥有单个 TOS 和已认证 backing hole；短 `RunSlots` 借用共享该状态。普通 push/pending 只缓存 Undefined/Null/Bool/Int/Float/ShortBigInt；peek/pop、Number binary/compare/local update、C1 store 复用同一逻辑深度。`canonical_slots` 保持整个 helper 借用规范化；事务 Drop 只做 move 恢复。 | [stack/tos.rs](../../src/engine/vm/stack/tos.rs)、[tos/number.rs](../../src/engine/vm/stack/tos/number.rs)、[run/tos.rs](../../src/engine/vm/run/tos.rs) |
| C3 | 专用 `FrameTransaction::cache_numeric_output(&mut Option<JsValue>) -> Result<bool, Error>` 仅接收 String/heap BigInt；失败保留 pending。`has_owned_numeric_output()` 用于 Keep 的 owning-cache 防护及调试/测试检查，run 入口按 opcode 选择 facade，不再逐步查询 owner。仅 numeric 的 `previous=None` 且紧接 PutLocal/PutArg 时接入缓存；Consume 移动原 owner，Keep 遇 owning cache 先恢复再走 canonical store。 | [run/numeric.rs](../../src/engine/vm/run/numeric.rs)、[tos/store.rs](../../src/engine/vm/stack/tos/store.rs)、[stack/window.rs](../../src/engine/vm/stack/window.rs) |
| C4 | `FusionKind` 显式解码既有 u8 tag；新增 `StoreDrop::{Local, Argument}`，编码 96/97。只认证相邻 SetLocal/SetArg + Drop，拒绝内部控制流入口。执行时 source/displaced 均须为六类无堆边标量，使用 Consume 消除临时 Keep owner；guard decline 从原 PC 走原 handler，错误保持 fault PC，成功跳过两条规范指令。 | [code/fusion.rs](../../src/engine/code/fusion.rs)、[run/fusion.rs](../../src/engine/vm/run/fusion.rs) |

C3 同时修复 resident numeric 的 pending cleanup：第二次输出失败时，仅释放尚未
提交的 previous/value，保留 postfix 已提交前缀。独立红/绿验证记录在
[numeric-cleanup-validation.json](../../target/s3-bc-completion/numeric-cleanup-validation.json)；
这是错误路径修复，不能计作缓存性能收益。

本轮 C3 有意保持窄准入。Object/Symbol 使用 canonical 路径，不新增 Runtime owner、
值 Drop release 或 deferred-release 批处理；快属性输出接缓存未扩展。
逻辑 depth/live slots 始终包含缓存值，缓存不增加容量；release、publication、driver
交接与退出前恢复 backing，unwind 的 Drop 不调用 Runtime 或 profiling。

profiling 分别记录 `tos.hit/miss/spill`、spill 原因、backing 读写、
`tos.owned_numeric_output`、`dispatch` 和 fusion 命中/候选/拒绝。
`dispatch` 计 run 主循环迭代，不是硬件间接分支数；Quick 的 hot、Generic、
guard decline、numeric handoff 和 canonical fetch 另计，因此二次分类成本可见。
`record_span` 仍按原字节码记录逻辑指令权重，不用融合后较少的循环次数冒充逻辑指令数。

**C5：not-started。**函数指针派发是可选支线；本轮保留中央循环与 match，
没有递归 handler 链，也没有实施函数指针性能实验。

## 2. B1a–B1d：认证投影、共享与执行

| 阶段 | 已实现内容 |
| --- | --- |
| B1a | [code/quick.rs](../../src/engine/code/quick.rs) 与 [quick/translate.rs](../../src/engine/code/quick/translate.rs) 提供 8B `QuickOp` 编解码、规范 opcode 翻译及逐 PC 合同验证。保留位、非法 tag/operand、长度和语义不匹配均拒绝。 |
| B1b | 在 [heap/allocation.rs](../../src/engine/heap/allocation.rs) 的原认证发布链从确切 canonical code 建表；`QuickProgram` 的全冷 `CanonicalOnly` 状态不分配 word buffer，`Words` 共享不可变 `Rc<Vec<QuickOp>>`。snapshot/closure 共享投影，BC5 仍只保存规范字节码。发布失败清理与共享释放均有独立测试。 |
| B1c | [run/hot.rs](../../src/engine/vm/run/hot.rs) 提取 Number binary、标量 branch、direct scalar binding read/store；canonical 与 Quick 共用语义和 C 的槽位接口。 |
| B1d | run 入口通过 `QuickProgram::execution_words()` 借用已认证的不可变 word slice，直接读取 tag/operand；[run/quick.rs](../../src/engine/vm/run/quick.rs) 的 continuation macro 将成功、span 完成与 Numeric handoff 接到主循环。热成功推进规范 PC；Numeric 携带已知运算种类、保留输入与 fault PC 进入既有路径，其它 guard decline 回同一 canonical PC。认证 span 共用原 fusion body 并按原指令计数；全冷函数在 run 入口选择 canonical，borrowed-base 路径继续保留。 |

当前共有 **33 个 tag**，不是最初只读投影的 7 个：

| tag | 内容 |
| --- | --- |
| 0–6 | GenericCanonical、Nop、PushI32、Undefined、Null、Bool、Goto |
| 7–18 | Add、Sub、Mul、Div、Mod、Pow、Shl、Sar、Shr、BitAnd、BitOr、BitXor |
| 19–24 | Eq、Neq、Lt、Lte、Gt、Gte |
| 25–26 | IfTrue、IfFalse |
| 27–32 | GetLocal、PutLocal、SetLocal、GetArg、PutArg、SetArg |

word 的低 8 位为 tag，随后 flags/aux 必须为零，高 32 位为 operand。
一个 word 对应一个规范 PC，包括 Generic 与 fusion interior；外部异常、源码映射、
IC、挂起和恢复仍用规范 PC。尚未覆盖的 opcode 保留 canonical 执行。

[heap/profiling/quick.rs](../../src/engine/heap/profiling/quick.rs) 按 buffer identity
去重 word capacity × 8 和 Rc/Vec 控制块，后者按 Vec header + 两个 usize 估算，
不声称覆盖 allocator 开销。实验布局将冷 `parameter_environment` 间接存放并单独计量；
[heap/edges.rs](../../src/engine/heap/edges.rs) 的 64 位编译期保护要求普通 M0 恰为
440B、实验布局不超过 440B。该约束不能替代 RSS 验收；旧版 B1b 嵌套 cold RSS
超限记录仍保留在第二批报告，不能据新布局设计直接宣布已消除。

### 内部模式

普通生产构建不自动打开 C2/C3/C4/Quick 实验。内部 cfg 已分别声明：
`oxide_scalar_tos`、`oxide_owned_tos`、`oxide_store_drop_fusion`、
`oxide_quick_projection`、`oxide_quick_dispatch`；仅 projection 不改变派发，
dispatch 单独启用时也包含投影定义，owned TOS 单独启用时包含缓存结构。
这些不是公共 Cargo feature、CLI 参数或用户运行时配置。

[run/mode_tests.rs](../../src/engine/vm/run/mode_tests.rs) 使用仅测试可见、Runtime
局部的 override，覆盖 0=canonical、1=scalar TOS、4=Quick、5=scalar TOS+Quick、
2=owned TOS、8=StoreDrop、15=全部组合。前四项组成缓存与派发的独立 2×2 对照；
每次观察创建 fresh Runtime，经实际编译器/driver 执行，比较结果、作用顺序、
异常行号、pending jobs、GC 后状态与 Runtime 最终释放。

## 3. C0 与 A4 的事实边界

完整输入已恢复：58 fixed、67 compile、9 original 和 3 个固定 BigInt 位于
[s3-c-opening/inputs](../../target/s3-c-opening/inputs/)；86 scaling 的原文件及模块
sidecar 身份见 [c0-audit/inventory.json](../../target/s3-bc-completion/c0-audit/inventory.json)。
before-C1、C1、pre-A 的同协议构建 receipt 均存在；before-C1 是保存候选，
其身份可认证不等于 E、残余批和 A4 已完成正式裁决。

当前实现保留 16B `JsValue`。A4 的窄可行性测试位于
[representation_spike_tests.rs](../../src/engine/vm/bindings/representation_spike_tests.rs)
和 [heap/tests/representation_spike.rs](../../src/engine/heap/tests/representation_spike.rs)：

- 将真实 FrameBinding 的 Direct 成员反事实替换成 8B word，实际
  `Option<FrameBinding>` 槽仍为 32B；其它真实变体限制了整体缩槽。
- 当前 handle 的完整 u32 index + u32 generation 无法直接放入宽松的 52 位
  payload；真实槽复用测试要求 stale owner 不因截断或补回当前 generation 而复活。
- 实际 ShortBigInt 包括 2^54 与 i64 边界；改为该 payload 会引入额外表示成本。

这些是当前“无旁表、保留完整身份的直接编码”候选的可行性约束，**不是完整 8B VM
性能实验，也不是所有 NaN-box/index-only 方案的否证**。上述测试在本轮通过；
本批继续使用现有 16B 表示，不把窄编码失败当作整个 A4 性能比较已完成。现有
[C0-CLOSURE-DRAFT.md](../../target/s3-bc-completion/c0-audit/C0-CLOSURE-DRAFT.md)
是草案，不充当已验收决定。

## 4. 历史数据：已存 C1 全 58 项结果

这是此前独立 C1 候选的数据，不能代表当前 C2/C3/C4/Quick 整合结果。
原始目录：[c1-matrix](../../target/s3-bc-completion/c1-matrix/)，包含 protocol、
两轮 A/A、三引擎交错 A/B、samples 和 summary。固定 CPU 2，A/A 每轮 5 对，
A/B 每引擎每项 10 次；普通 release、fat LTO、CGU=1、无 PGO/profiling。
比率均为整进程时间 after/before，低于 1 表示该次观察较快；V8 固定脚本行也
使用时间，不是 original replay 的 Score。

| 比较 | 58 项时间几何均值 |
| --- | ---: |
| C1 / previous | 0.996413 |
| previous / pre-A | 1.046202 |
| C1 / pre-A | 1.042449 |

`previous` 为 before-C1 `6c3b132f050d691bf3ee8af3278ead7eee042ad40823f412f5eb8cda383d7e64`；
`c1` 为 opening candidate `a698722f2041fb7e1006d9afae658112aaee38e56a09568f5fad515e453ec822`；
`pre_a` 为 `17694ed44a98b67223b3e2d74f5da6d5364181766edf82b424a99215714fa358`。
完整构建路径与固定输入 hash 见该目录的 `protocol.json`，不用最新工作区替换标签。

以下逐项转录 [summary.json](../../target/s3-bc-completion/c1-matrix/summary.json)，
仅四舍五入到六位小数；原文件 SHA-256 为
`07a18c73b8c643bec2e9c6e41bcfbfd1d930f21d4f8c775c74e11c1844ebdd5b`。

| case | C1 / previous | previous / pre-A | C1 / pre-A |
| --- | ---: | ---: | ---: |
| empty_loop | 1.007225 | 0.960861 | 0.967803 |
| empty_down_loop | 0.998339 | 0.950819 | 0.949240 |
| prop_read | 0.985437 | 0.744264 | 0.733425 |
| prop_write | 1.012237 | 1.063040 | 1.076049 |
| prop_update | 1.020229 | 1.012992 | 1.033484 |
| prop_create | 0.987586 | 1.112699 | 1.098886 |
| prop_clone | 0.997976 | 0.718436 | 0.716982 |
| prop_delete | 1.008091 | 0.859178 | 0.866130 |
| array_read | 0.989676 | 1.192495 | 1.180184 |
| array_write | 1.059032 | 1.024178 | 1.084637 |
| array_update | 1.080358 | 1.009347 | 1.090456 |
| array_prop_create | 0.986970 | 1.053394 | 1.039668 |
| array_slice | 1.000676 | 0.888590 | 0.889191 |
| array_length_read | 1.003706 | 0.739108 | 0.741847 |
| array_length_decr | 1.004089 | 0.744397 | 0.747441 |
| array_push | 0.971413 | 1.141155 | 1.108533 |
| array_pop | 0.992198 | 1.096287 | 1.087734 |
| typed_array_read | 0.974894 | 1.188505 | 1.158667 |
| typed_array_write | 1.010882 | 1.254321 | 1.267971 |
| arguments_read | 0.985621 | 0.983344 | 0.969205 |
| arguments_strict_read | 0.982191 | 1.012436 | 0.994405 |
| global_read | 0.977668 | 0.902364 | 0.882213 |
| global_write | 1.002188 | 0.931289 | 0.933326 |
| local_destruct | 0.980513 | 1.088580 | 1.067367 |
| global_func_call | 0.962689 | 0.984760 | 0.948017 |
| func_call | 0.975679 | 1.140355 | 1.112621 |
| func_closure_call | 1.001639 | 1.086457 | 1.088238 |
| int_arith | 0.998774 | 0.933892 | 0.932747 |
| float_arith | 0.992389 | 0.944080 | 0.936895 |
| bigint64_arith | 1.023316 | 1.168056 | 1.195290 |
| map_set_string | 0.963189 | 1.142771 | 1.100704 |
| map_set_int | 0.998567 | 1.042157 | 1.040664 |
| map_delete | 0.987292 | 1.175870 | 1.160928 |
| weak_map_set | 1.000972 | 1.122809 | 1.123900 |
| array_for | 0.993976 | 1.132287 | 1.125466 |
| array_for_in | 0.947827 | 1.122981 | 1.064392 |
| array_for_of | 0.958980 | 1.173524 | 1.125386 |
| math_min | 1.030863 | 1.083706 | 1.117153 |
| regexp_ascii | 0.978356 | 1.101608 | 1.077765 |
| regexp_utf16 | 0.982906 | 1.102343 | 1.083500 |
| regexp_replace | 0.989389 | 1.028730 | 1.017814 |
| string_length | 0.961611 | 0.802697 | 0.771882 |
| string_build1 | 0.996432 | 1.364111 | 1.359244 |
| string_build3 | 1.006172 | 1.329180 | 1.337383 |
| string_build_large1 | 0.982430 | 1.357259 | 1.333411 |
| string_build_large2 | 0.997030 | 1.272382 | 1.268604 |
| int_to_string | 1.032452 | 1.322104 | 1.365009 |
| float_to_string | 1.022549 | 1.077680 | 1.101980 |
| string_to_int | 1.020971 | 1.033294 | 1.054963 |
| string_to_float | 1.001020 | 1.015084 | 1.016119 |
| v8-richards | 0.988464 | 1.024224 | 1.012408 |
| v8-deltablue | 0.993921 | 1.080531 | 1.073963 |
| v8-crypto | 0.971453 | 1.061618 | 1.031312 |
| v8-raytrace | 0.997346 | 1.042155 | 1.039389 |
| v8-earley-boyer | 1.007953 | 1.074889 | 1.083438 |
| v8-regexp | 1.003330 | 1.107529 | 1.111217 |
| v8-splay | 0.988844 | 1.013772 | 1.002463 |
| v8-navier-stokes | 1.028803 | 1.142937 | 1.175858 |

限制：array_write、array_update、math_min、int_to_string 的 C1/previous 分别为
1.059032、1.080358、1.030863、1.032452，超过原计划的单项复核线；不能用总均值
掩盖。两轮 A/A 合并的线性插值 P95 在 array_read 达 12.1974%；上述四项依次为
4.0027%、2.0962%、3.5449%、2.0538%。噪声、独立复现、单项归因及其它完整矩阵
尚不能由这张表关闭，所以不宣称 C1 稳定净收益或整合版本性能接受。
[noise.json](../../target/s3-bc-completion/c1-matrix/noise.json) SHA-256：
`6c7255470bc74ad6aff15752ac6edc75dcaacba995e316d3e56c62484473b34a`。

## 5. 历史验证：修复前的集中正确性验收与交付状态

本节固定记录 `d1b52c9a` 的验证，随 `f3f152d8` 归档；不覆盖之后的性能修复。
修复后验证待 §9 补录，下列命令、计数、耗时和 receipt 均保留原版本身份。

本轮先完成核心接线，再集中验证；未为各个修正单独重跑 benchmark。
新的测试覆盖 C2 状态机、Materialize 和 PC/缓存联合 unwind，C3 String/BigInt
last-owner、Borrowed/Deferred 预检与 Symbol canonical 回退，C4 认证/执行 guard、
B 编码/发布/共享/回滚，以及七模式的真实编译器/driver 差分。

本节验证对象的代码提交：

- `3e116f75`：缓存事务、数值 owner 准入、边界恢复及 MSRV 接线。
- `d1b52c9a`：共享热 handler、QuickOp 取指、StoreDrop、真实执行差分与诊断。

所有 Rust 检查使用 Rust 1.88.0；构建、Rust 测试和 Test262 串行。
源码文件摘要为 `96725f8888e932707d7a25bc645e59e03355f88499c65d4ec0db142b987a7f27`，
[verified-source.json](../../target/s3-bc-completion/integration/verified-source.json)
逐文件认证与 `d1b52c9a82c627601151c2f82cdaf02ccdc0215a` 一致。
命令、配置、退出码及耗时见
[validation.json](../../target/s3-bc-completion/integration/validation.json)。

| 检查 | 实际结果 |
| --- | --- |
| 默认 workspace all-targets，含 pinned QuickJS oracle | 3533 passed、0 failed、1 个既有 ignored；其中 oracle 907 passed |
| profiling lib | 2644 passed、0 failed；包括四组合逻辑指令计数一致和实际 Quick/fusion/cache 命中 |
| 全部实验 cfg + profiling + test262-host，workspace all-targets | 3800 passed、0 failed、1 个既有 ignored；其中库 2704 passed，包含宿主 GC/重入 |
| workspace doc | 3 passed |
| Rust 1.88 Clippy workspace all-targets，`-D warnings` | 默认 cfg 和全部实验 cfg 均通过；两者均检查 profiling/test262-host |
| 独立配置 | scalar TOS、owned TOS、StoreDrop、Quick dispatch、projection 五个独立 cfg 的 workspace lib/bins + profiling 均构建通过 |
| fmt / source-layout / rust-only | 全部通过 |
| benchmark 工具单测 | 49 passed；这是工具正确性，不是计时实验 |
| BC5 | opcode/atom 门禁 self-test 与 pinned QuickJS 2026-06-04 两份真实 header 对照全部通过 |
| 完整 Test262 | 最终组合逐项匹配冻结向量：79982 pass / 80032 eligible / 102037 total；50 项既有失败不变 |

组合 cfg 为 `oxide_scalar_tos`、`oxide_owned_tos`、`oxide_store_drop_fusion`、
`oxide_quick_dispatch`。Quick dispatch 同时包含发布投影；不需额外 projection cfg。
Test262 使用相同组合、普通 release、fat LTO、CGU=1、无 profiling/PGO；
保留原 `dev-support/test262/current.conf`，不重基线。
完整执行耗时 697.14 秒，当时的 engine fingerprint 为
`5f66e16f48837b2e1e8483b5c7da959730e0692b2e382079edddf9cbae297ac8`；
runner SHA-256 为 `c7ef3312d6687c801c09ce9af4c1c780ebab6ee7bbeb409734d15993ac18d7f5`。
[完整 receipt](../../target/s3-bc-completion/integration/test262/receipt.json)
归档命令、flags、退出码与 TSV/JSONL/log 哈希；不是对旧 receipt 运行 `--check`。

## 6. 当前决定与未关闭项

| 项目 | 决定 |
| --- | --- |
| C1 | 现有直接存储实现保留；§4 数据仍不构成稳定净加速或完整性能接受 |
| C2/C3/C4 | 已实现，修复前集中 Rust 正确性检查见 §5，修复后验证待 §9；以内部 cfg 保留，默认不启用，性能接受未关闭 |
| C5 | not-started；尚无证据将当前成本归因于中央间接跳转，不投入函数指针实验 |
| B1a–B1d | 编码、发布、共享 handler 与执行入口均已实现；§5 历史正确性覆盖包含七模式及组合生产构建，修复后验证待 §9 |
| B1e | 保持默认 canonical、不生成 eager sidecar；尚未通过 M0/M1/M2 完整成本门槛，不宣称默认启用或 quickening 收益 |
| C0/C6 | 输入/源码身份、接口与本轮一致性记录已交付；前置 E/残余/A4 正式裁决、完整性能/内存接受仍未关闭 |

修复前定向 profile 见 §7，修复后的实测结果待 §9；尚无完整 RSS、compile 与性能矩阵的接受结果。
默认行为不依赖实验 cfg；单测通过或成功编译不替代速度、内存与启动成本证据。
后续 B 使用当前 TOS facade、已认证的规范 PC 与 fusion 列表，不重新定义 owner、
挂起或错误恢复协议。B2 的自适应重写、反馈状态、IC slot 和 deopt 不属于本首批。

## 7. 历史 profile：修复前实现的集中测量

本节全部数据及结论属于修复前 `d1b52c9a` 的冻结导出，归档于 `f3f152d8`。
下文“本批”均指这一历史测量；其中旧准入查询、解码与返回形状已经进入修复范围，
不能据此描述当前实现或判定修复后的收益。新结果待 §9 补录。

### 7.1 冻结版本与测量口径

本批使用 `d1b52c9a` 的完整只读导出。Rust 1.88.0、release、fat LTO、
CGU=1、无 PGO，三档均保留一级调试符号以定位 CPU 热点。在当前工作机
`eric-83am` 固定 CPU 2，governor 为 powersave；构建、计数、计时和 CPU
采样严格串行。此为本机诊断，不冒充计划指定 PocketLab 的正式验收。

| 本批名称 | 实际构建 | 比较的含义 |
| --- | --- | --- |
| M0 | 全部实验 cfg 关闭 | 当时的 canonical；已经包含 C1 和共享热 body，不是 before-C1 或 pre-A |
| C | scalar TOS + owned TOS + StoreDrop | C/M0 是 C2–C4 打包增量，不能拆成单个阶段的独立收益 |
| BC | C + Quick dispatch/发布投影 | BC/C 是 Quick 发布、派发及交互的净增量；没有 projection-only 档，不能当作纯派发收益 |

这组三档不是 B 计划中的 M0/M1/M2 正式成本矩阵。普通构建对八个完整输入
各运行五次，循环轮换三档顺序，保留每次结果；wall 是整个进程时间。
`perf stat` 另跑三次，记录用户态 instructions、cycles、branches、branch-misses；
`perf record` 再独立采集 cycles:u、499 Hz、DWARF 调用栈。

profiling 构建只对同算法的小输入记录计数，时间不用于加速比。
六个完整输入保持历史源码身份，另新增明确命名的 C3 heap BigInt→PutLocal
与 C4 local/arg StoreDrop 诊断输入；全部十六份 full/small 输入的精确结果
先由 Node 独立核验。三档八项的逻辑指令计数完全一致，C/BC 的 C3 owned
结果和 C4 两种 fusion 均达到预注册命中次数。

源码、构建、输入、命令与原始输出保存在
[profile 目录](../../target/s3-bc-profile/)，入口为
[protocol.json](../../target/s3-bc-profile/protocol.json)、
[builds.json](../../target/s3-bc-profile/builds.json) 和
[runs.json](../../target/s3-bc-profile/runs.json)。
本批用于定位实际成本，没有 A/A 噪声门禁、第二轮独立复现或完整内存/编译矩阵，
不将定向时间变化升级为 C6/B1e 性能接受。

### 7.2 时间与硬件指令：修复前组合没有净收益

以下为五次普通进程 wall 中位数（秒）及三次 `perf stat` 的用户态指令数
中位数比值；比值大于 1 表示成本增加。八项没有删除异常值或失败样本。

| 输入 | M0 秒 | C 秒 | BC 秒 | 时间 C/M0 | 时间 BC/M0 | 时间 BC/C | 指令 C/M0 | 指令 BC/M0 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| local-consume-scalar | 0.6279 | 0.9549 | 1.0535 | 1.521 | 1.678 | 1.103 | 1.494 | 1.585 |
| arg-consume-scalar | 0.6919 | 1.0773 | 1.0478 | 1.557 | 1.514 | 0.973 | 1.539 | 1.630 |
| mixed-boundaries | 0.6972 | 0.7883 | 0.8491 | 1.131 | 1.218 | 1.077 | 1.143 | 1.256 |
| bigint256 | 0.6833 | 0.8277 | 0.9248 | 1.211 | 1.354 | 1.117 | 1.226 | 1.395 |
| string_build1 | 0.2517 | 0.3174 | 0.3729 | 1.261 | 1.481 | 1.175 | 1.323 | 1.455 |
| prop_read | 0.4157 | 0.4235 | 0.5365 | 1.019 | 1.291 | 1.267 | 1.218 | 1.387 |
| c3-owned-bigint | 2.3996 | 3.0117 | 3.4743 | 1.255 | 1.448 | 1.154 | 1.240 | 1.375 |
| c4-store-drop | 0.5723 | 0.6128 | 0.6856 | 1.071 | 1.198 | 1.119 | 1.320 | 1.382 |

完整最小/最大值、中位数和 cycles/branches/branch-misses 见
[summary.json](../../target/s3-bc-profile/summary.json)，每样本见
[wall.json](../../target/s3-bc-profile/wall.json) 与
[stats.json](../../target/s3-bc-profile/stats.json)。72 次 perf stat 的四类事件均可用，
288 条事件记录的 running/enabled 比例均为 100%；没有把不可用计数填零。

最明显的 local/arg 回退同时伴随约 49%/54% 的机器指令增加，不能只解释成
时钟抖动。C 的 prop_read 时间比 1.019，尚无本批 A/A 支持其是否可分辨，但指令数仍增 21.8%；
这也说明指令与时间不可直接互换。BC 在 arg 上比 C 快约 2.7% 的单次批次观察
不能抵消它比 M0 慢约 51.4%，更不能单独称为稳定 Quick 收益。

### 7.3 优化路径命中，但节省被额外工作抵消

以下计数来自 small 输入，不能当作 full 输入的绝对事件数。

- **C2 驻留较短，重复判断仍多**：local/arg 分别提交缓存 110,009/130,011 次，
  spill 50,008/60,010 次，其中约 80%/83% 由下一次 push 驱逐引起。
  两项的 dispatch、逻辑 owner 移交与 value copy 数量相对 M0 不变；高 hit
  还包含准入和 handler 对同一 top 的重复 peek，不能直接解释为物理流量收益。
- **C3 真正命中**：定向 BigInt 输入在 C/BC 都有 20,000 次
  `tos.owned_numeric_output`；缓存并非未启用。但组合在完整输入上分别慢
  25.5%/44.8%，目前不能声称 owning 输出缓存已取得时间收益。
  历史 bigint256/string_build1 的该事件均为零，不能用专用输入代表其覆盖率。
- **C4 真正减少循环**：定向 StoreDrop 的 local/argument 各成功 10,000 次，
  dispatch 从 180,028 降到 160,028，逻辑指令仍为 220,029。
  prop_read 的 StoreDropLocal 成功 40,002 次，dispatch 从 260,304 降到
  220,302。两项的组合机器指令仍增加；没有单独 C4 档，不能把组合失败
  归咎于 C4 本身，也不能用 dispatch 减少冒充最终加速。
- **Quick 保留已有优化**：八项 C→BC 的 fusion、dispatch、spill 及其原因
  分项完全相同；string 的 borrowed local-add 和 prop 的 BorrowedBaseField
  仍命中。当时的新增成本并非这些输入中旧融合消失或额外 spill 所致。
- **Quick 二次分类明显**：回到 canonical 的执行比例在八项中为
  25.0%–72.5%；prop_read 为 45.5%，其中 Generic 仅 242 次，guard decline
  却有 100,006 次。C4 输入 Generic 仅 15 次，但 decline 有 40,001 次。
  只报告低 Generic 比例会漏掉主要退回成本。

八项均满足 `quick.canonical_fetch = generic + declined`。
mixed/旧 bigint/C3 的 BC 相对 C 分别多 10,002/20,010/5,000 次 backing-read
事件，同时 backing-write 与 spill 不变，符合额外守卫/重分类的方向。
事件不是机器 load：规范 M0 没有同覆盖范围的 TOS backing 计数，缺字段不能
当作零，因此本批不计算“相对 M0 减少了多少物理栈写入”。
逐项计数与源码解释见 [Quick 分析](../../target/s3-bc-profile/quick-analysis.md)。
缓存的全部事件、spill 原因与 facade 覆盖边界另见
[C2–C4 计数分析](../../target/s3-bc-profile/c-tos-analysis.md)。

### 7.4 静态代码成本

| 普通构建 | ELF `.text` 字节 | 相对 M0 | `run_with_modes` 符号字节 |
| --- | ---: | ---: | ---: |
| M0 | 6,918,025 | 1.00000 | 41,510 |
| C | 6,945,273 | 1.00394 | 47,227 |
| BC | 6,989,849 | 1.01038 | 两实例合计 79,258 |

BC 的全冷函数入口另选 canonical，留下两份 run 实例。整体 `.text` 增幅
约 1.04%，尚未越过计划 3% 复核线，但主循环实例总量增长约 91%；这只是
局部代码膨胀证据，不能由大小直接证明 I-cache miss。构建均含 `DEBUG=1`，
总文件中的调试节不能算作运行时代码，静态符号清单及 receipt 核对见
[code-size.json](../../target/s3-bc-profile/code-size.json)。

### 7.5 CPU 热点与具体归因

24 份普通 release 的 `perf record` 均无 lost samples。self 占比来自
`perf report --no-children`，各列分母为各自进程采样的总周期；不能将两个
占比直接相除当作时间比，也不能把内联子项再次加到所属符号上。
较短输入只有百余至数百样本，C3 三档的 header 缩写为 `1K`；适合定位
大热点，不用于声称小幅占比差异稳定。完整报告见
[raw](../../target/s3-bc-profile/raw/)。

**C 的额外工作集中在准入与缓存操作。**local 的 C 构建中，
`run_with_modes` self 为 23.77%，其中可辨认的 `tos::resident` 内联路径
占整个程序 7.30%，另有 `is_ok_and → resident` 1.88% 和
owning-output 查询链约 2.93%。`ScalarTos::install` self 为 9.43%，
`tos_store_binding` 为 5.01%。这些路径对应以下已存在的重复工作：

- 当时的 canonical 每条指令先选择 cache/canonical facade，并查询 owning cache。
- local/arg 准入读取 binding、检查标量；共享 handler 再读取/证明同一输入，
  store facade 继续认证目标和缓存状态。
- push 安装新缓存前恢复旧 top；单槽缓存使下一次 push 的驱逐成为主要 spill。

M0 本来就有安全检查和共享 handler，不能把整个 handler 的样本算作新增。
但新增准入/缓存调用与机器指令增长的方向一致，当时的失败不能仅归因于计时噪声。

**修复前 Quick 的取指仍执行认证后的解码。**
`d1b52c9a` 中的 `QuickProgram::operation` 每次从不可变投影取 word，
调用带错误结果的 `decode().expect(...)`。认证发布时已经验证过这份投影；
采样却显示这个运行期成功路径占据相当份额：

| BC 输入 | run 符号 self | operation/decode/expect 内联链 | quick::execute 内联链 |
| --- | ---: | ---: | ---: |
| local-consume-scalar | 58.80% | 23.65% | 20.20% |
| arg-consume-scalar | 64.96% | 30.26% | 20.55% |
| c3-owned-bigint | 43.08% | 16.55% | 11.45% |
| prop_read | 61.38% | 26.82% | 15.36% |
| string_build1 | 49.91% | 24.39% | 12.47% |
| c4-store-drop | 70.44% | 30.20% | 24.55% |

后两列是第一列中的内联路径，百分比仍以整个程序为分母。`expect` 出现在
成功解包路径的符号归属中，不代表执行 panic，也不能把这部分成本全归给
`expect` 语法。应针对认证后的解码/结果传递形状定位机器码，而非简单删除断言。
再结合 §7.3 的大量 guard decline，可见紧凑 8B 存储本身没有消除解码及
二次分类工作。完整逐项 CPU 解释见
[cpu-analysis.md](../../target/s3-bc-profile/cpu-analysis.md)。

### 7.6 修复前的批次结论

这批实现完成了所有权与执行接线，但 **C2–C4/Quick 的当时组合没有证明净收益，
定向数据反而显示明确成本增加**。C3/C4 并非未命中，旧融合也未丢失；应优先
减少 C 的重复准入/缓存状态判断，以及 Quick 的认证后解码和先探测再退回的工作。
这些是 profile 支持的下一轮修改目标，不是尚未测量的收益承诺。

当时决定保持默认 canonical，该批候选不进入默认启用。C5 的函数指针实验没有证据
能直接解决上述具体工作，继续不启动。C6/B1e 的完整性能/内存接受仍未完成，
本批不冒充整个 B/C 性能计划已验收，也不据组合结果单独否定 C3 或 C4。

本批共 256 次执行：Node 16、计数 24、wall 120、perf stat 72、perf record 24，
全部通过各自精确 stdout/退出码契约。源码/二进制身份、原始文件哈希、采样
有效性与协议边界见 [receipt.json](../../target/s3-bc-profile/receipt.json)。

## 8. Git 变更范围核对

以下统计固定到 `f3f152d8ebf8499b2563b54ac9e92b3b3672ace6`，
不包含后续性能修复；未执行 fetch，另经只读 `git ls-remote` 核对远端 `main`、`session/dd42dc95` 的 tip 与本地 tracking refs 一致。

| 比较基线 → 固定提交 | 提交数 | 文件数 | 新增 / 删除 |
| --- | ---: | ---: | ---: |
| `origin/main` 的 merge-base `49d1a299` | 84 | 537 | +60,063 / −22,278 |
| B/C 起点 `bcfb4fe5` | 11 | 77 | +11,106 / −422 |
| 当时 upstream `origin/session/dd42dc95`（`2b615277`） | 3 | 46 | +3,399 / −315 |

约六万行来自与 main 的累计差异。`49d1a299` 到 B/C 起点已有
73 个提交、+49,020 / −21,919，主要是前置 A 阶段的值表示与所有权迁移。
两个时段的 numstat 不能直接相加，后续修改会重写此前新增的行。
B/C 的 +11,106 行中，独立测试文件占 +5,088，源码（含内嵌测试）
占 +3,637，文档占 +1,415，非测试脚本占 +965，Cargo.toml 占 +1。

未发现误提交的 benchmark 结果、冻结源码/语料或生成物；`target` 均未跟踪，
B/C diff 无 JSON/JSONL/TSV/log/snapshot/generated、Cargo.lock 或 vendor/submodule 变化。
核对到的 41 个历史数据/生成物路径，其 blob 在 main merge-base、B/C 起点
与固定提交中完全一致，包括旧 Test262 ledger 和 Unicode 表；无需为缩小行数删除。
逐文件统计、提交来源与 blob 身份见 [git-audit.json](../../target/s3-bc-repair/git-audit.json)
及 [审计说明](../../target/s3-bc-repair/git-audit.md)。B/C 范围应按 `bcfb4fe5` 比较；
对 main 的审阅仍需包含其依赖的 A 阶段历史，当前 upstream 的较小差异是另一比较范围。

## 9. 性能回退修复：技术说明与测量协议

### 9.1 修复内容与保持的合同

修复针对 §7 暴露的重复准入、认证后解码及值传递成本。原实现将标量路径
接到通用 owner/错误处理接口，又在短借用之间重复分类；宽枚举与结果载体
还使局部抽象产生额外搬运。以下改动收窄已证明成功的路径，最终影响须由
同协议实测判断，不能从源码行数、内联提示或分支减少直接推导收益。

| 改动 | 技术目的与保持的边界 |
| --- | --- |
| `JsValue` 使用 `#[repr(u64)]` | 将 tag 与 payload 按 word 对齐，避免小 tag 枚举经 Option/Result 传递时形成重叠搬运。编译期仍要求 `JsValue` 和 `Option<JsValue>` 均为 16B；完整 handle generation、ShortBigInt 位宽与 owner 语义保留，不是 A4 的 8B 表示或序列化格式改造。 |
| 认证 Quick word 借用与 continuation macro | run 入口借用不可变 `execution_words()`；派发直接读取认证后的 tag/operand，成功和 span 完成直接进入主循环 continuation。发布时仍完整验证编码及规范 PC 合同；Numeric handoff 保留输入和 fault PC，其余 decline 回原 canonical handler，认证 fusion 继续复用共享 body。 |
| opcode 准入与固定 `cache_slots` | opcode 决定 facade，handler 负责具体值和 binding 的认证，避免两层重复查询。缓存事务的启用状态与缓存是否为空分开；`cache_slots` 仅在与缓存事务构造相同的 const 条件下借用。`canonical_slots` 仍保持整个观察/释放 helper 的借用规范化。 |
| 标量 Drop 与 displaced 标量直接丢弃 | 仅六类无堆边标量走 `pop`/`discard_scalar`，不再进入 Runtime 的通用释放路径。这些值在旧路径恒为 Ready，不处理 deferred 队列、不借 heap、不触发 GC；保留一次 `HotRelease(false)`。堆 owner 仍按原 readiness、publication 和释放协议处理。 |
| C1 只传递 Direct 的 `JsValue` | 两种 store 在认证 Direct 后只替换其值成员，返回 displaced `JsValue`；不再搬运完整 `FrameBinding` 或引入 captured/private 的 Drop 分派。目标和源认证、Keep 的 retain 均先于提交；Consume 移动原 owner，decline/错误保持原顺序。 |
| 分离 `copy_scalar` | 六类标量返回 `Option<JsValue>`，堆 retain 继续使用可失败的 `copy_reference`。标量成功只记录一次 copy，非标量 decline 不记 copy、不消费输入；避免标量读取承担通用错误结果载体。 |
| 空缓存的 owning push 复用 canonical | 普通 `tos_push` 遇非标量且缓存为空时直接使用 `push_current`，共享容量认证和提交，再记录相同安装事件。非空缓存仍先按原规则恢复；普通 push 不扩大 heap owner 的缓存准入，C3 专用 pending API 保持原失败合同。 |
| LocalAdd 复用当前 run 事务 | 已认证的局部字符串/primitive 加法直接调用共享 completion，不再每次退出 run、重建事务。保留 Materialize、同一 conversion identity、Add/Store fault PC、constant owner 清理和 span 逻辑计数；错误仍进入原 throw 路径。 |
| displaced owner 直接释放 | `release_displaced_value` 已拥有被替换值，保留 Ready 复核和 HotRelease 计数后直接释放，不再为调用槽位 API 把临时变量改写成 Undefined。 |
| 错误详情移出成功结果载体 | 私有 `ErrorData` 使用 Box；原有 kind/message/native payload/span、Clone/Eq/Debug/Display、线程 traits 和 const 方法保持。错误构造及 Clone 多一次详情分配，成功返回与错误传播不增加分配；不改变 JS owner、BC5 或公共值表示。原 80B 错误载体对热 Result 的影响及新布局由实际构建验证。 |

这些改动共同保持：逻辑 depth/live slots 包含缓存 owner；观察、释放、调用与
退出前恢复 backing；unwind 只移动恢复，不新增 Runtime release。外部异常、
IC、挂起和恢复使用规范 PC；span 按原指令记录逻辑计数，不能重复计入循环
尾部，也不能用 dispatch 减少替代逻辑工作量。具体正确性结果与源码身份另行验收。

### 9.2 冻结对照与测量口径

本轮候选源码已冻结于 [round7/source.json](../../target/s3-bc-repair/round7/source.json)。
各档从同一导出构建，receipt 记录源码清单、flags 和 binary SHA，并在构建前后
认证源码。修复前保存二进制与修复后候选在同一轮轮换运行，标签固定如下：

| 标签 | 二进制来源 | 用途 |
| --- | --- | --- |
| `baseline` | `s3-bc-profile/m0/plain` | 修复前保存 M0，已含 C1 与共享 body；检验共享代码的变化 |
| `old_bc` | `s3-bc-profile/bc/plain` | §7 的修复前保存 BC；比较组合修复前后 |
| `m0` | `s3-bc-repair/round7/m0/plain` | 修复后 canonical，全部实验 cfg 关闭 |
| `c` | `s3-bc-repair/round7/c/plain` | 修复后 scalar TOS + owned TOS + StoreDrop |
| `bc` | `s3-bc-repair/round7/bc/plain` | 修复后 C + Quick dispatch/发布投影 |

五档保持 Rust 1.88.0、普通 release、fat LTO、CGU=1、DEBUG=1、无 PGO，
不使用 profiling feature 的耗时作速度结果；运行固定 CPU 2。构建、正确性
验证与测量串行。复用 §7 冻结 full 输入及精确 expected stdout，不改变工作量；
运行前认证输入与 binary SHA，记录每次命令、退出码及原始 stdout/stderr。
普通执行要求精确 stdout、退出码 0、空 stderr；perf 自身诊断单独保留。

wall 测量整个进程；`perf stat` 独立运行 `instructions:u`、`cycles:u`、
`branches:u`、`branch-misses:u`，各轮旋转引擎顺序。实际重复次数、事件可用性
和 running/enabled 比例随本轮 protocol/raw 记录，失败样本不作为零耗时，
各模式的占比变化也不替代绝对时间比较。构建及测量入口分别为
[build_round.py](../../target/s3-bc-repair/build_round.py) 与
[evaluate.py](../../target/s3-bc-repair/evaluate.py)。

`m0/baseline` 检验共享代码，`bc/old_bc` 检验修复幅度，`c/m0` 与 `bc/c`
区分缓存组合和 Quick 增量；修复前后改善不自动等于相对 canonical 的净收益。
这组诊断也不代替完整保护矩阵、内存/编译成本及 C6/B1e 的接受门槛。

**本节最终性能与验证数据由后续验收补录。**

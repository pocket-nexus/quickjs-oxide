# 改善实施设计与切片

> 实施状态（2026-09-25）：A 的紧凑错误载体、B 的 P2／P3／P4 共 13 种跨度及其 matcher、存储原语和两个 `run` 入口已提交。四个完整真实内核的发布后 dump 在[收据](receipts/all-dense-6db6bfb0/README.md)中，共有 25 个已发布站点。以下保留原设计的职责和边界；代码状态以本段和实际提交为准。
> #41 在 R0 上的 cycles、错误分配和 RSS 回退见[复验收据](receipts/gates-2026-09-25/README.md)。按当前实施要求，未达到性能门禁只记录和归因，不停止完成既定代码。组合版的正式 benchmark／profiling 尚未运行。
> 调用与检查重排仍是条件项，不属于这 13 种跨度的已实现范围。

<a id="a-error"></a>
## A. 紧凑错误载体收尾

### A1. 恢复并审查已测候选

原报告的 `0cd4acee` 未能作为当前基线上的可复用候选取得；已在 R0 上独立恢复为 `04bb1a74`，并从零复验。当前集成分支包含该实现及后续语义测试；它不冒充原实验的测量身份。

审查范围限定为 `src/engine/api/error.rs` 的冷 payload 及直接受影响的测试。保留 `Debug`、错误 kind/message/span、CLI 输出和退出码；明确记录公共 `Error` 的表示变化、`const fn` API 和分配行为。不同时改 opcode、stack facade、内联注解或 release 协议。

### A2. 接纳前补测

按统一测量脚本在无人并发使用的主机上重测 #41 的 13 个固定负载，重点复核 `prop_write` 的 cycles。补错误密集场景的分配次数／字节、峰值 RSS、构造和传播成本；分配观测使用独立诊断构建或外部工具，不用插桩产物发正式 Score。

对公共 API 使用者测试 `kind`、`span`、`with_span`、Debug 格式、带 native message 与嵌套错误。对异常场景测试 TDZ、语法错误、getter throw、类型转换 throw、普通对象 throw、栈溢出／容量不足；对成功场景确认错误对象没有被预先构造。

反汇编复核实际保留的边界：`copy_value/copy_reference`、`push_current`、`local_current`、`parameter_current`、`replace_local_current`、`pop_current`、`array_immediate_read_current`、`property_ic_write_scalar_current`，以及 `run`、`numeric_local_add`、`numeric_local_field_add` 和比较／更新 handler。记录内联消失、返回寄存器／返回槽、调用者栈帧和 spill，不从 `size_of` 单独推导速度。

### A3. 决策与后续边界

原准入方案要求固定矩阵、真实 V8、错误分配／RSS与一致性全部通过才生成新工作基线 R1。实际 R0 复验发现 `prop_write` cycles 与错误路径分配／RSS 回退；本轮按用户要求继续实现并集成 #41，把这些结果如实记录，最终接纳裁决等待组合版串行测量。不据此批准全仓库错误通道重构。

候选接纳后重新归因。只有新产物仍保留、且在真实负载上足够热的调用边界，才允许追加窄成功路径。不能复用旧的 80B 模型推测剩余成本，也不要求给已经内联的 helper 添加无意义 facade。

<a id="b-arrays"></a>
## B. 数值／数组执行块：按冻结规格实施

本节原 B0–B7 的意向接口和开放选形态要求已被 [数值／数组跨度实施规格 v1](numeric-array-spans.md) 取代。该规格给出 13 种精确序列、每种 flag/长度/peak/delta、producer/store 白名单、所有新函数签名、生命周期、代码路径、单次提交和测试名称。**不存在待决定的 `NextPc`、泛型 Value 或省略参数 API。** 当前 13 种形态已按该契约接入生产 matcher 和 handler。

### B0. 取得真实指令与 site manifest，而非再选架构

运行 [发布后 dump 入口](probes/run_dump.py)，由其在临时 detached worktree 安装 [test-only Rust 探针](probes/dump_numeric_spans.rs)。编译完整 pin 的 Crypto 和 NavierStokes，不抽取／重编译内层函数。四个目标必须各出现一次，并保留原 PC、常量、参数、局部及闭包信息；随后以生产 matcher 生成 manifest。命令和回执契约见规格 §2。

已用 Rust 1.94.1 对 pin `2034d98` 的完整 Crypto 和 NavierStokes 运行发布后探针；四个目标函数各出现一次，25 个真实首 PC 和零覆盖形态见[完整 manifest](receipts/all-dense-6db6bfb0/README.md)。规格中的符号模板仍只是设计说明，真实 PC 以该 manifest 为准。

### B1–B3. 按编号切片，不建立第二个解释层

| 切片 | 冻结实现 | 交付边界 |
| --- | --- | --- |
| P2 / B1 | R0 普通读、R1 数值索引读、R2 后缀更新读、R3 前缀更新读 | flags 1–4；非拥有 base，成功只 push Number；索引更新先暂存、最后与输出一次提交 |
| P3 / B2 | R4 读取后数值运算；A0–A3 local 累加写回 | flags 5–9；复用现有 Number，局部写回不产生拥有式临时值 |
| P4 / B3 | W0 标量写、W1 数组复制、W2 计算写、W3 compound 数值写 | flags 10–13；只覆盖现有 dense 数值槽，包含 Insert3/PutArrayEl/Drop 契约并更新 property_generation |

精确序列见规格 §1.2。当前生产版本已在全部 handler 齐备后发布 13 个 flag；它没有留下已发布但未实现的形态。入口只位于 GetLocal/GetLocalCheck 与直接 GetArg 两个臂，既有 S1–S4 和 canonical helper 保留。build 优先最长**合法**候选，每个新 flag 都通过完整内部入口与 stack_contract 检查。

API 分为 `direct_value/numeric_span_room/try_commit_number`、Runtime 的 `peek_dense_number/try_write_dense_number`，以及 `try_numeric_span(...)->Option<usize>`。短 `&JsValue` 借用不跨提交；heap Ref/RefMut 不返回，唯一返回值是 Copy Number 或完成 PC。全部签名、re-export 和逐函数修改见规格 §3–§5。

### B4. 本版排除与覆盖缺口

本版明确排除 this、VarRef/global/captured producer、typed/arguments 对象、Float 最终下标、带副作用的 key 转换、跨调用块、多次 store 和 key-update 的数组写入。记录其实际拒绝数，不静默把它们纳入“通用 numeric block”。扩展需新的具体设计和独立 A/B；不在当前 P4 中安排未设计的捕获写。

### B5–B7. 提交证明、测试和回退

R2/R3 只有所有读取、类型和 canonical peak 容量检查通过后才提交索引；W 的最后可失败动作是已有数值槽写入，成功后只更新预先检查过的 property_generation 并返回。miss 必须保持 slots/depth/heap/owner/PC/generation 不变。Src==dst 先复制 Number 并结束共享借用，再借可变 heap，不能重排浮点运算。

规格 §7 的发布认证、栈容量、数值提交、alias／descriptor 与真实内核覆盖已有定向测试及收据；组合版 workspace 全目标测试和 Rust 1.88 check/clippy 已通过。full Test262 与正式串行 A/B 仍未完成，不能把源码完成或 25 个静态站点写成动态命中率、净收益或性能接纳。

<a id="c-deferred"></a>
## C. 条件项与明确不做

**调用路径：** #43 前置画像已完成，不再安排一次同样的广泛调研或通用 native 借用参数改造。仅在 A 后的统一八项画像仍支持 DeltaBlue 窄热点时，比较 frame 安装／清理的最小候选。真正的 callsite cache 须先测 per-PC callee 分布；必须守卫实际函数、realm、this、默认参数、arguments 与栈限制，不缓存跨闭包环境、不凭属性名识别 builtin、不持永久强 callee owner。native argv 已池化，不再设“去掉每次分配”目标。

**S3 检查顺序：** #44 的候选与复读变体均不接纳。重新实施必须用新基线的五类成本、真实分布和完整 helper 反汇编给出负的加权成本；4.8% 只是特定两类模型的阈值。没有新证据不重复布局彩票。

**B2.2、IC 冷却与 codegen：** 可在数组主线后作为局部实验，但旧 752→632 上界不构成倍数总分依据。优化 `run` 不变也不够，必须检查 outlined handler 的内联与 spill。源码位置变化不能直接诊断 I-cache／分支预测。

**不重新立项：** B1 全量 QuickOp 投影、单槽 TOS facade、无依据的统一 span 查询、NaN-box、已完成的 D 布局工程、通过延迟 RC 或修改 benchmark 来提高分数。目标是删除工作，同时守住真实程序与 safe Rust 的边界。

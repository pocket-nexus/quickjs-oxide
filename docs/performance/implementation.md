# 改善实施设计与切片

> 实施状态（2026-09-25）：A 的紧凑错误载体、B 的 P2／P3／P4 共 13 种跨度及其 matcher、存储原语和两个 `run` 入口已提交。四个完整真实内核的发布后 dump 在[收据](receipts/all-dense-6db6bfb0/README.md)中，共有 25 个已发布站点。以下保存当次候选的职责与边界，用于复核历史实现；后续优化可依据新证据调整内部表示和 API，代码状态以实际提交为准。
> #41 在 R0 上的 cycles、错误分配和 RSS 回退见[复验收据](receipts/gates-2026-09-25/README.md)。当次实施按用户要求完成既定代码，并记录、归因未达性能门禁的结果。[组合版的四方正式 benchmark、profile 和代码审查](receipts/fourway-2026-09-25/README.md)已完成。
> 调用与检查重排未纳入这 13 种跨度的已实现范围；当时列为条件项。

<a id="a-error"></a>
## A. 紧凑错误载体收尾

### A1. 恢复并审查已测候选

原报告的 `0cd4acee` 未能作为当前基线上的可复用候选取得；已在 R0 上独立恢复为 `04bb1a74`，并从零复验。当前集成分支包含该实现及后续语义测试；它不冒充原实验的测量身份。

当次审查范围限定为 `src/engine/api/error.rs` 的冷 payload 及直接受影响的测试。保留 `Debug`、错误 kind/message/span、CLI 输出和退出码；明确记录公共 `Error` 的表示变化、`const fn` API 和分配行为。当次没有同时改 opcode、stack facade、内联注解或 release 协议。

### A2. 当时拟定的接纳前补测

当时计划按统一测量脚本在无人并发使用的主机上重测 #41 的 13 个固定负载，重点复核 `prop_write` 的 cycles；还计划补错误密集场景的分配次数／字节、峰值 RSS、构造和传播成本。分配观测使用独立诊断构建或外部工具，不用插桩产物发正式 Score。已执行范围与结果以[复验收据](receipts/gates-2026-09-25/README.md)及[四方收据](receipts/fourway-2026-09-25/README.md)为准。

当时拟覆盖公共 API 的 `kind`、`span`、`with_span`、Debug 格式、带 native message 与嵌套错误；异常场景包括 TDZ、语法错误、getter throw、类型转换 throw、普通对象 throw、栈溢出／容量不足，成功场景检查错误对象是否被预先构造。具体已执行测试以源码和收据为准。

当时拟通过反汇编复核实际保留的边界：`copy_value/copy_reference`、`push_current`、`local_current`、`parameter_current`、`replace_local_current`、`pop_current`、`array_immediate_read_current`、`property_ic_write_scalar_current`，以及 `run`、`numeric_local_add`、`numeric_local_field_add` 和比较／更新 handler。复核需记录内联消失、返回寄存器／返回槽、调用者栈帧和 spill，不能从 `size_of` 单独推导速度。

### A3. 决策与后续边界

原准入方案要求固定矩阵、真实 V8、错误分配／RSS 与一致性全部通过才生成新工作基线 R1。实际 R0 复验发现 `prop_write` cycles 与错误路径分配／RSS 回退；本轮按用户要求继续实现并集成 #41，把这些结果如实记录。随后组合版串行测量已完成，结果见[四方收据](receipts/fourway-2026-09-25/README.md)。当次结果没有提供全仓库错误通道重构的依据。

当次方案要求基于新产物重新归因，再考虑真实负载上足够热的调用边界。旧 80B 模型不能直接推断当前剩余成本；已内联的 helper 也不需要仅为保持旧接口而增加 facade。后续候选应由新的生成代码与负载证据决定。

<a id="b-arrays"></a>
## B. 数值／数组执行块：已完成候选的实施记录

当次 B0–B7 的意向接口和开放选形态要求被 [数值／数组跨度实施规格 v1](numeric-array-spans.md) 收束为一个可实施候选。该规格记录 13 种精确序列、每种 flag/长度/peak/delta、producer/store 白名单、新函数签名、生命周期、代码路径、单次提交和测试名称。当次没有留下待决定的 `NextPc`、泛型 Value 或省略参数 API；当前 13 种形态已按该契约接入生产 matcher 和 handler。后续候选不受这些内部 API 和形态限制，但须重新证明语义并测量净成本。

### B0. 当次候选的真实指令与 site manifest

运行 [发布后 dump 入口](probes/run_dump.py)，由其在临时 detached worktree 安装 [test-only Rust 探针](probes/dump_numeric_spans.rs)。编译完整 pin 的 Crypto 和 NavierStokes，不抽取／重编译内层函数。四个目标必须各出现一次，并保留原 PC、常量、参数、局部及闭包信息；随后以生产 matcher 生成 manifest。命令和回执契约见规格 §2。

已用 Rust 1.94.1 对 pin `2034d98` 的完整 Crypto 和 NavierStokes 运行发布后探针；四个目标函数各出现一次，25 个真实首 PC 和零覆盖形态见[完整 manifest](receipts/all-dense-6db6bfb0/README.md)。规格中的符号模板仍只是设计说明，真实 PC 以该 manifest 为准。

### B1–B3. 按编号切片的当次实现

| 切片 | 当次实现 | 交付边界 |
| --- | --- | --- |
| P2 / B1 | R0 普通读、R1 数值索引读、R2 后缀更新读、R3 前缀更新读 | flags 1–4；非拥有 base，成功只 push Number；索引更新先暂存、最后与输出一次提交 |
| P3 / B2 | R4 读取后数值运算；A0–A3 local 累加写回 | flags 5–9；复用现有 Number，局部写回不产生拥有式临时值 |
| P4 / B3 | W0 标量写、W1 数组复制、W2 计算写、W3 compound 数值写 | flags 10–13；只覆盖现有 dense 数值槽，包含 Insert3/PutArrayEl/Drop 契约并更新 property_generation |

精确序列见规格 §1.2。当前生产版本已在全部 handler 齐备后发布 13 个 flag；它没有留下已发布但未实现的形态。入口只位于 GetLocal/GetLocalCheck 与直接 GetArg 两个臂，既有 S1–S4 和 canonical helper 保留。build 优先最长**合法**候选，每个新 flag 都通过完整内部入口与 stack_contract 检查。

API 分为 `direct_value/numeric_span_room/try_commit_number`、Runtime 的 `peek_dense_number/try_write_dense_number`，以及 `try_numeric_span(...)->Option<usize>`。短 `&JsValue` 借用不跨提交；heap Ref/RefMut 不返回，唯一返回值是 Copy Number 或完成 PC。全部签名、re-export 和逐函数修改见规格 §3–§5。

### B4. 本版排除与覆盖缺口

该候选排除了 this、VarRef/global/captured producer、typed/arguments 对象、Float 最终下标、带副作用的 key 转换、跨调用块、多次 store 和 key-update 的数组写入；拒绝数应结合当次收据解读，不能把它们计入已实现覆盖。后续若扩展这些形态，需新的具体设计、语义测试和独立 A/B；当次 P4 未安排捕获写。

### B5–B7. 提交证明、测试和回退

R2/R3 只有所有读取、类型和 canonical peak 容量检查通过后才提交索引；W 的最后可失败动作是已有数值槽写入，成功后只更新预先检查过的 property_generation 并返回。miss 必须保持 slots/depth/heap/owner/PC/generation 不变。Src==dst 先复制 Number 并结束共享借用，再借可变 heap，不能重排浮点运算。

规格 §7 的发布认证、栈容量、数值提交、alias／descriptor 与真实内核覆盖已有定向测试及收据；组合版 workspace 全目标测试和 Rust 1.88 check/clippy 已通过。full Test262 冻结向量匹配；同机串行的四方正式 V8 对照与定向 profile 见[收据](receipts/fourway-2026-09-25/README.md)。25 个静态站点本身不能替代动态命中率或净收益，故两者均另行实测。

<a id="c-deferred"></a>
## C. 当次未纳入的条件项

**调用路径：** #43 前置画像已完成，当次没有重复同一广泛调研或实施通用 native 借用参数改造。当时把 DeltaBlue 的窄热点列为可复评方向：若新画像仍支持，可比较 frame 安装／清理的候选。callsite cache 若进入新设计，须先测 per-PC callee 分布，并守卫实际函数、realm、this、默认参数、arguments 与栈限制；跨闭包环境缓存、凭属性名识别 builtin 和永久强 callee owner 不能由当次证据支持。native argv 已池化，因此“去掉每次分配”不是当次目标。

**S3 检查顺序：** #44 的候选与复读变体当次均未接纳。新的重排候选应重新测量基线、五类成本、真实分布和完整 helper 反汇编；4.8% 只是特定两类模型的阈值，不是长期常数。当次没有支持继续重排的证据。

**B2.2、IC 冷却与 codegen：** 当时保留为可另行验证的局部实验；旧 752→632 上界不构成倍数总分依据。后续分析需同时检查 `run` 与 outlined handler 的内联和 spill；源码位置变化不能直接诊断 I-cache／分支预测。

**当次未立项：** B1 全量 QuickOp 投影、单槽 TOS facade、无依据的统一 span 查询、NaN-box、已完成的 D 布局工程，以及通过延迟 RC 或修改 benchmark 来提高分数，均不在这次候选范围内。这不永久排除有证据支持的内部设计；新方案仍须说明删除了哪些工作，并以真实程序、safe Rust 和语义验证约束实现。修改 benchmark 来抬高分数不能作为性能证据。

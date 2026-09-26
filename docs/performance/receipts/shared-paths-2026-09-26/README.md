# 2026-09-26：继续删除公共执行路径成本

维护者指出：五项计划的候选验收不能作为整个项目优化工作的终点。本轮从已有画像继续处理普通属性读取、数组读取、调用安装与退出；不把历史接口、存储名称或实验边界视为永久约束。

## 实现及证明边界

| 改动 | 原先反复执行的工作 | 本轮处理 |
| --- | --- | --- |
| 普通自有属性读取 | IC 退化后已查到自有 Data 槽，但只接受立即值；Object/String 等又退 driver 重查 | 在原 heap borrow 内按既有协议 retain 本次读到的值，复用 native selection；不保存旧属性值 |
| materialized Array 读取 | 退化为普通属性存储后，自有 Number 也必须走完整属性流程 | 索引直接映射到已有自有 Data 槽，复制 Number；普通 GetArrayEl 和现有数值跨度共用 |
| 普通局部初始化 | 每次调用逐槽检查 lexical/function-name 并经过 fallible constructor | 随不可变 executable 保存 plain-local fact；适用函数一次填入 Undefined，其他函数沿用原来的 TDZ、命名 owner 与回滚路径 |
| 帧退出 | 每个标量槽都调用通用绑定释放，再在 value 层判定无需释放 | 直接清空无 owner 标量；有边的值、captured/private binding 仍按原槽序释放 |

静态局部事实首次建立于 executable snapshot 初始化，随该函数的不可变元数据复用；它不是当前值类型的缓存。命名函数位置由同一 FrameLayout 提供，删除了另一个可不一致的函数参数。测试用可变 synthetic snapshot 单独重算事实。

数组读取只需 **自有 Data(Number)**；数据属性是否可写、可枚举、可配置不影响读取，故不再重复检查默认 CWE。洞、继承、accessor、Proxy、非 Number、超出 immediate-index 编码的索引仍回落。写路径仍独立检查 dense 存储及写入条件，读快路不能授权对 frozen 数组写入。数值读取不创建 owner、不执行 JS；候选写失败前不提交局部槽、数组或 PC。

属性读取沿用消耗 base 前的 release-readiness 检查；最后一个 base owner、待清理队列、借用失败仍回落。保留 receiver 的读取只 retain 结果。Object/String/BigInt/Symbol 的提升在同一借用内完成，成功 retain 后没有新的可失败步骤。accessor、lazy、继承和 exotic 路径保留原 driver 协议。

## 身份与验收

产品受测源码 `ae81f090`，其后的 `fe3b58f6`、`e2cbe966` 仅修正测试夹具／旧快路预期及格式。分片受测源码：属性 `dc3d298a`；属性加数组 `af5b5902`（从最终源码撤回两个调用提交生成）；最终加调用 `ae81f090`。本轮起点是 `3c007f80`，产品代码与受测基线 `b280ec8b` 相同。

累计 main 对照的二进制来自 R0 `f531f605`；远端 main `2ed79f46` 与它的引擎代码相同，仅文档变化。不会把它写成从最新 main SHA 构建的二进制。全部 release 二进制使用同一 Rust 1.96.0 / Apple Silicon / fat LTO / CGU=1；诊断 profiling 构建另存。

外部原始证据：`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-shared-paths`。V8 复用此前冻结的源码、body、运行次数、顺序和输出断言，含八项 isolated 及 combined；每侧 A/A、A/B 各两次，每批 72 样本，采样截止 600 秒。固定工作量包括 Setup 与退出，**不是原版 adaptive V8 Score**。同机干扰允许存在，计时仍标为受干扰；本任务的构建、测试和各测量进程分开运行。

## 结果

四组 V8 配对均完成 **72/72** 有效样本，共 288 个进程；每组实际耗时 119–135 秒，四组串行合计约 8 分 16 秒。三组 20 项固定矩阵共 480 个有效样本，新增属性探针 32 个。八项诊断共 23 秒。构建和正确性测试用时另计。

下表为 **候选相对各自基线的退休指令变化**，负值表示减少。三片基线依次是 `b280ec8b → dc3d298a → af5b5902 → ae81f090`；累计列是直接测量 `R0 → ae81f090`，不把不同批次的百分比相加。

| 负载 | 属性片 | 数组片 | 调用两项合计 | 累计相对 main 同代码基线 |
| --- | ---: | ---: | ---: | ---: |
| richards | -2.52% | +0.62% | +0.52% | -8.59% |
| deltablue | -8.00% | +0.10% | +0.03% | -12.87% |
| crypto | -0.86% | -12.79% | -0.41% | -45.45% |
| raytrace | -0.17% | +0.02% | -0.18% | -5.72% |
| earley-boyer | +0.08% | +0.00% | -0.16% | -2.00% |
| regexp | -0.02% | +0.00% | +0.03% | -1.21% |
| splay | -5.37% | +0.17% | -0.22% | -8.78% |
| navier-stokes | +0.00% | +0.46% | +0.00% | -36.21% |
| combined | -1.36% | -1.54% | -0.10% | -13.24% |

累计 combined cycles 观察值为 **−14.20%**，wall 为 **−13.67%**（基线／候选约 1.158×）。同机干扰和短轮分辨率限制仍在，这些数值不构成原版 V8 Score 或精确的整体耗时承诺。RegExp 累计指令仅 −1.21%，wall 却 −15.01%，同二进制 A/A wall 跨度为 15.07%；这项时间差不能作为 RegExp 优化成果。各原始中位数、A/A、散布、二进制与负载身份见 [V8 数据](v8-summary.json)。

**属性读的交换成本：** 三形状、IC 退化的 Object 属性探针指令 −12.74%，cycles 观察值 −12.72%；但 Number/Object/String 命中探针的指令分别 +1.22%/+0.44%/+0.20%，与 promoter 新增 outlined 调用相符。这不是零代价快路，不能只报退化路径的收益。真实 DeltaBlue、Splay 分别减少 8.00%、5.37% 指令；保留此候选，并记录命中路径的代码生成成本。[属性及首批固定矩阵](initial-gates.json)。

**数组读：** Crypto 在属性片之后再减少 12.79% 指令；已是 dense 的 NavierStokes 增加 0.46%。未命中／不适用路径的代价没有消失，已列账。本轮累计 20 项固定矩阵最大指令增加为 array_read +0.93%，fusion_dynamic_miss +0.52%，call0 +0.30%；Object 搬运 +0.01%。RSS 中位数最大增加为 0.18 MiB。[完整固定矩阵](fixed-summary.json)。

**调用改动：** 机器码确认删除了普通 local 的逐槽静态分类及标量退出的通用 release 调用，但真实 combined 指令仅 −0.10%，cycles +0.57%、wall +2.71%；没有证明调用机制获得可分辨的整体加速。Richards 指令 +0.52% 也保留在账内。当前作为执行事实传递和共享清理的候选保留在性能分支，不能把它写成调用专项完成，也不能据此关闭调用方向。

调用片首轮 RayTrace wall +12.38%、cycles +3.16%，超出同项 A/A，故另作一次仅该负载的窄复核：复用完全相同的 frozen JS，A/A、A/B 各 4 次／侧，共 16 个进程、约 9 秒；A/B 指令 −0.216%、cycles −0.747%、wall −0.443%。首轮时间回退未复现，两组结果同时保留，不把复测的微小变化写成加速。[复核数据](raytrace-summary.json)。

V8 combined 的 RSS 波动很大：四组同二进制 A/A 范围约 34–107 MiB，内存收益仍未裁决；不能根据候选某次 RSS 较低声称节省内存。上文 0.18 MiB 仅指 20 项固定微负载矩阵。

### 当前覆盖与剩余工作

以下是固定一次诊断的逻辑事件，不能相加成时间占比，也不是收益上界：

| 负载 | uncached own-field 完成 | materialized 自有 Number 读取 |
| --- | ---: | ---: |
| richards | 65,687 | 0 |
| deltablue | 96,363 | 0 |
| crypto | 106,796 | 694,085 |
| raytrace | 387,191 | 0 |
| earley-boyer | 0 | 0 |
| regexp | 33,469 | 0 |
| splay | 1,187,398 | 0 |
| navier-stokes | 1 | 0 |

`uncached_field` 包括原来就可完成的立即值，不能把整列视作本轮新增 Object/String 命中。materialized 读取会同时被其他上层事件统计，不能重复计算。当前没有为 plain-local fact 或标量退出单独新增覆盖计数，因此它们的精确动态命中率仍未测；代码生成与调用片配对已经测量。[诊断事件与来源](profile-events.json)。

继续调查有具体依据：Crypto 仍有 423,956 次 `run_exit.SetProperty`；Splay 有 594,699 次 `lazy_frame_materialized`、9,209,530 次 `slot_authentication`；EarleyBoyer 有 3,123,581 次查询派发；RegExp 有 4,440,153 次查询派发，其中 `regexp_split` owner 1,723,420 次。它们指向写入提交、帧状态／验证和查询流程的基础成本，仍需结合真实时间与生成代码决定替换哪些内部结构。

### 正确性与证据

- Rust 1.88.0，最终 `e2cbe966` 的 profiling 库测试：**2,167 passed，0 failed**。涵盖新属性 owner、materialized/frozen 数组、原型回落、TDZ 与混合帧清理用例。
- `cargo fmt --all -- --check` 与 source-layout（626 个 Rust 文件）通过。最终 Clippy（workspace/all-targets/profiling，`-D warnings`）通过；远端完整 fast/focused Test262 状态单独报告，不继承上一提交的通过结果。
- 本轮没有重跑完整 Test262 或原版自适应 V8 Score。构建日志、失败夹具初次日志和最终通过日志均保留，见 [证据索引](evidence-index.json) 和 [命令](commands.json)。
- 基线二进制复用此前同编译器、同构建配置的干净构建；新候选均从独立干净源码构建。当前工作目录的其他文档编辑不进入受测产品。

## 机器码与验收中发现的问题

- 自有属性 fallback 的生成代码已能直接提升 Object/String，避免 driver 第二次查找。但共享 promoter 被 outlined，cache hit 新增一次调用。新加的对象、字符串、数值命中与三形状退化探针用于测量这项代价；原来的数值 prop_read 会走融合，覆盖不足。
- 普通局部初始化的真分支四槽一组写 Undefined，循环内没有每槽 lexical/name 分类；假分支保留原逻辑。install 942→988 条静态 ARM64 指令、栈 464→480 B，不能把代码变大直接解释为变慢。
- clear_frame 标量分支跳到下一槽；其他绑定仍调用相同的 release_frame_binding。clear_frame 143→148 条静态指令，栈保持 208 B；通用释放函数的 77 条指令字相同。
- 新数组测试最初漏保留 canonical 读取所需的 base owner；原实现正确回落。另两处旧断言把 frozen 数据读取必须回落作为要求，已改为验证读值正确且写入仍被拒绝。调用测试的父帧最初误用了有两个 local 的子帧布局，已修复夹具。这些初次失败日志保留，不算通过。

## 后续仍可改变的边界

本轮覆盖公共路径，并不表示其他优化点已调查完毕。调用的非标量原始 argv 保留仍有成本，但它关联参数覆盖后原值存活、arguments、挂起和 owner 释放顺序，不能靠重复添加缓存处理。RegExp、字符串与分配/回收等未覆盖成本需继续按当前证据排序。历史 `Dense` 名称不要求读路径永远只接受 dense 存储；本轮局部初始化 fact 也不是禁止以后调整帧表示的规则。

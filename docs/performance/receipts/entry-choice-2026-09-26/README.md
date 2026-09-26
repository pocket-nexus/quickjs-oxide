# 2026-09-26：测量、真实覆盖与融合入口实验

本轮把三个讨论合并成三项可核验工作：修正测量与文档、补齐当前真实负载的成本/覆盖诊断、按证据试验执行入口。数组表示与调用架构重写属于后续候选；本轮只调查其实际成本。

## 本轮结论与交付范围

| 任务 | 已交付 | 证据与限制 |
| --- | --- | --- |
| 1. 测量与文档 | 干净源码构建回执、源码／工具链／二进制／输入哈希、20 项固定矩阵、交错 A/A 与 A/B、macOS 计数器；取消指定唯一测量主机和永久冻结候选 API 的文档约束 | 固定矩阵的全部 3,296 个样本输出有效，分别保留逐片与 Parent／B37／R0 累计结果。按维护者最终要求，原版 V8 在 RegExp A/A 后停止，未采集候选正式 Score 对照。 |
| 2. 真实执行诊断 | profiling 独立构建中的函数／canonical PC、候选与非候选访问、命中／失败原因、有界调用身份分布；修正失败标签优先级 | 八个原版 V8 子项的固定工作量诊断及修正后回放都完成。Crypto 数组 materialize 机制有源码依据，但逐对象转换因果未记录；callsite 身份分布不是机制时间上界。 |
| 3. 执行入口实验 | 互斥融合候选单次选择、flag=0 显式退出、普通写入分类内联；逐候选出口、owner、readiness 与 PC 契约 | ARM64 准入分支和 outlined 调用确有减少。真正 no-plan 的独立入口切片仍增加约 0.66% 指令；对象时间／周期的不利观测也保留。候选留在实验分支，时间性能准入未裁决。 |

当前证据没有证明 v8-v7 总分达到 B37 的 3–4 倍，也没有实现所有热路径的零重复验证／零额外派发。后续源码结构可以继续由新证据调整；本轮没有新建数组表示或调用缓存框架来填补这些未知项。

## 测量身份

这是 macOS / Apple M1 / Rust 1.96.0 的独立系列，不能与旧 Linux / Rust 1.94.1 的绝对数值相除。外部 v8-v7 pin 为 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。所有源码 worktree、独立 target、构建日志、原始样本和 trace 放在本机：

`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3`

| 身份 | 源码 |
| --- | --- |
| B37 | `3341ac456ea2719858fd6173e8dcd9123ad9e660` |
| R0 | `f531f6052cb497ce4707f01c276e8642e5e26788` |
| Parent（普通 Number 写入前） | `29560e08e37e7e3465fb355be5641b26da374576` |
| Current（本轮起点） | `b42652f8a31240feebd247b8a730b58a6b9a4921` |
| Entry choice 初始候选 | `3f17738c9bfa461758451bf3ca2724888c077424` |
| Entry choice 固定负载受测产物 | `315035b5dba805b033dad5187d187b360e39ceff` |
| 本轮集成版 plain 构建 | `d6a168e2` |
| 集成版加普通写入分类内联的受测候选 | `8c38bc32a316aa6eb4fdf6540b448d0163770671` |

两份不在 PR 主历史中的受测提交另保留远端实验引用，避免只剩本机 worktree：[`codex/measured-entry-choice-20260926`](https://github.com/pocket-nexus/quickjs-oxide/tree/codex/measured-entry-choice-20260926) 精确指向 `315035b5`，[`codex/measured-inline-20260926`](https://github.com/pocket-nexus/quickjs-oxide/tree/codex/measured-inline-20260926) 精确指向 `8c38bc32`。它们保存受测源码；不是已接纳性能候选的声明。

普通计时构建为 release、fat LTO、CGU=1，无 PGO/profiling。诊断构建另存，逻辑事件不作为普通版耗时或 Score。macOS 原生 `/usr/bin/time -l` 在此主机提供整进程退休指令、cycles、最大 RSS 和 peak footprint；不是 Linux `perf ...:u` 的用户态专用口径。Instruments CPU Counters 的初步能力探针因时间限制终止、目标收到 SIGKILL，仅作为工具能力记录；它不能支持任何缓存、分支预测或瓶颈比例结论。

Current 和 Parent 构建启动于工具完善期间，完整 Cargo verbose 日志保留了实际 rustc 命令和参数，但其旧版 receipt 中的 tooling hash 是构建结束时观察，不能证明进程加载的脚本版本。后续构建工具在启动时冻结脚本快照并在结束时校验；这项边界不改变前两份干净源码、完整编译命令与二进制的身份。

本轮发现其他同机工作区仍在编译／测试后，维护者明确要求“继续采样，耗时结果标为受干扰”。因此后续轮次的 wall、cycles 与原版 V8 Score 均标为**受同机并发任务干扰**，不冒充空载门禁或凭小幅中位数变化裁决收益。自己的构建／正确性测试与性能批次仍分开运行；阶段边界另存活跃编译／测试进程和 load average。固定输入的退休指令、A/A 波动、机器码与语义结果分别报告。

原版 V8 的初始外层超时设为 180 秒；A/A 已观察到 Crypto、RayTrace、EarleyBoyer 单次较长，其隔离耗时合计已超过这个 combined 上限。在任何五版本候选 V8 样本开始前，将后续统一上限改为 900 秒，原版 body、warmup、最低迭代次数、Score、重复次数和顺序均未改。前四项完整 A/A 共 32 个样本保留；已完成的 4 个 EarleyBoyer 样本与 1 个主动中止进程也保留为资源修订记录，计划将该未完成组及其余四项另在新目录完整运行 40 个样本。原始文件不覆盖、不按分数选择样本。预先记录的计划、命令和修订身份见 [v8-plan.json](v8-plan.json)。

上述 40 个续跑样本是当时计划；维护者随后要求“先把 RegExp 跑了就不要跑这个 benchmark”。实际在 EarleyBoyer／RegExp 各 8 次、共 16 次续跑样本完成后停止，并终止后续队列。有效 A/A 汇总覆盖六个子项、48 个样本；原 EarleyBoyer 的 4 个额外样本、资源修订和队列切换时主动中止的进程均单列保留。Splay、NavierStokes、combined A/A 与五版本正式对照按该要求取消，不能填成零分或已通过。停止记录和分布见 [v8-summary.json](v8-summary.json)。八项固定工作量逻辑 profile 已在此前完成，不受这次停止影响。

这六项同一二进制的 A/A 中位 Score 假变化为 −8.92%～+44.22%；RegExp 为 −8.92%，EarleyBoyer 为 +44.22%。这些数值记录本轮受干扰的时间波动，不能用来评价候选实现的收益。

## 初始机器码证据与候选边界

ARM64 普通版中，整个函数没有融合计划时已有一次跳转进入普通路径。存在其他融合站点、当前 GetLocal 的 flag 为零时，仍执行多个 selector 的条件分支。候选保留原 u8 sidecar、canonical 指令和 PC，仅把互斥的五类候选改成一次选择，并显式绕过零 flag；Number 未命中后的字符串 Add 桥和动态回落维持原有顺序。不能由源码 if 数量直接推导机器指令收益，后续以受测产物的反汇编核对。

实际反汇编中，计划存在且 GetLocal flag=0 的准入路径从 10 条条件分支加 1 条无条件分支变为 5 条条件分支；`run::run` 按下一个符号估计的布局范围为 20,680→20,704 字节，栈帧仍为 `0x780`。这不是零分支，也不是预测失败次数。Current 与加入诊断后的普通构建、Choice 与 `d6a168e2` 集成后的普通构建，各自比较的 `run` 及七个融合 helper 的地址与指令字均完全相同；后一对 `run` 为 5,176 条 ARM64 指令。该检查覆盖八个选定符号，不声称整二进制相同。原始反汇编与比较 JSON 位于上述外部目录的 `series/codegen-{current,instrumentation,choice,integrated}`。

### 事实的建立、使用与失效

| 阶段 | 当前证据／表示 | 使用范围与失效边界 |
| --- | --- | --- |
| Rust 构建 | 私有 `FusionEntry(u8)`、穷尽的 `LocalFusionChoice` 分支和 safe Rust 检查 | 保证实现能处理所有枚举分支；不证明某个 JS 值是 Number，也不保证内联提示一定生效。 |
| JS 函数发布 | 生产 matcher 按候选检查形态和内部控制流入口；Dense 13 类另有显式 `stack_contract` 核验，将一个候选 flag 与不可变 canonical code 一起发布 | 静态事实只属于该已发布函数与原 PC。没有把某次调用的值类型放进静态 flag；后续重新发布的函数使用自己的计划。 |
| 每次指令派发 | 从当前函数、当前 `pc.fault` 读取一次 entry，flag=0 退出选择，否则只进入一个候选分支 | `LocalFusionChoice` 是本次派发的临时选择，不是跨调用缓存。帧切换、重入或挂起后不能复用它。GetArg 的单一 Dense 入口没有增加同类选择层。 |
| 候选动态守卫 | 各 handler 按原契约检查当前槽绑定和类型；Dense 另查数组、索引和容量，写路径预检 `property_generation` | 借用只在当前 handler 内有效；不跨用户代码、释放、重入或挂起。各 handler 的 guard miss 按其原契约保持原槽、深度、heap、owner 和 PC，继续当前 canonical 指令。异常仍走原异常出口，不能冒充 miss 后再执行一次。 |
| 成功提交 | 复用原 handler 的提交边界；只有成功后才推进 `pc.resume` | Number 运算不新增拥有式临时值；数组写仍在完成可失败检查后提交。字符串 Add 桥、getter／Proxy／转换与释放顺序沿用原路径。 |
| 普通写入分类 | `direct_write_class` 判断当前旧绑定；内联实验只改注解 | 不延长 readiness 证明的作用域，不跨释放或 JS 执行复用结论。对象等 owner 仍走原来的 ready／boundary 路径。 |

这里删的是互斥候选的重复准入选择，没有删完 handler 内部的静态重验，也没有实现“所有静态不适用指令零额外派发”。原有 13 类跨度的详细提交证明仍见 [历史候选契约](../../numeric-array-spans.md)；它不是未来必须保留 u8、签名或 helper 边界的理由。

五类选择各自的 guard、Result／RunExit、fault／resume PC，以及普通写入四种分类的 owner 和 readiness 作用域，详见[当前执行契约](execution-contract.md)。该文档也给出 Crypto 数组转换的源码链及未覆盖的逐对象因果边界。

## 结果

### 固定工作量：分别审查执行成本与回退

下面初始四批中的 `fusion_no_plan` 名称不准确：原来的 `while(n > 0)` 发布了独立的比较融合计划，因此它不能作为“全函数没有计划”的证据。`b59331bc` 将该项及配对的 `fusion_flag0` 改为 `while(n)`，并通过真实编译／发布测试分别断言计划为 None／Some、热循环读取的 flag 均为零。旧样本保留原名称和哈希；修订后两项另测，不混算不同工作量。其余 18 项字节未变。

四批 Parent→Current 和 Current→Entry choice 的 20 项固定负载，每批每侧 8 次，合计每批 320/320 样本有效；首轮按 `abba-baab`，次轮按 `baab` 交错。以下均是每批左右两侧 **8 次中位数** 的百分比变化，负值代表该指标减少。`315035b5` 在初始候选 `3f17738c` 后加入固定负载和一个 `#[test]`，没有改动非测试 VM 路径。受测二进制 SHA-256、每项每侧的 median/min/max/CV、完整 wall/cycles/instructions/RSS/peak-footprint 数据及四份原始 `results.json` 的绝对路径和 SHA-256，均在 [fixed-summary.json](fixed-summary.json)；外部逐项分析在 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/analysis-fixed/README.md`，其原始完整统计 JSON 的 SHA-256 为 `f32b1dc4db3573d5d56f0a97435aab3fd1f1e6ea7c505a09fe698fb688a9cd6c`。仓库精简统计 JSON 的 SHA-256 为 `ac4df592d0ba339b793339ac984b201de5a6b4bfddea8a03bd2f58bf6a5d0214`。

| 固定负载 | Parent→Current instructions，轮 1/2 | Current→Choice instructions，轮 1/2 | Current→Choice cycles，轮 1/2 | Current→Choice wall，轮 1/2 |
| --- | ---: | ---: | ---: | ---: |
| `fusion_hit` | −11.24% / −11.23% | +0.48% / +0.51% | +0.49% / +1.00% | +0.52% / +0.80% |
| `fusion_dynamic_miss` | −0.26% / −0.06% | +0.13% / −0.06% | +0.33% / +0.48% | +0.18% / +1.26% |
| `fusion_flag0` | −15.74% / −15.74% | −0.45% / −0.45% | −0.16% / +0.07% | +0.29% / −0.78% |
| `fusion_no_plan` | −15.73% / −15.73% | −0.46% / −0.45% | −0.04% / −0.24% | −0.72% / −2.95% |
| `bigint_32` | −1.37% / −1.37% | −0.38% / −0.34% | +1.67% / +1.53% | −0.05% / −0.64% |
| `bigint_64` | −0.72% / −0.71% | −0.18% / −0.19% | +0.28% / −1.20% | +1.57% / +0.87% |
| `bigint_256` | −0.58% / −0.63% | −0.15% / −0.13% | −0.17% / −0.08% | −0.49% / +0.52% |
| `prop_read` | −0.01% / +0.05% | −2.50% / −2.48% | −0.31% / +0.44% | −0.02% / +2.63% |
| `prop_write` | −0.00% / +0.04% | −2.19% / −2.23% | +0.31% / −0.02% | +2.08% / −1.24% |
| `array_read` | +0.01% / +0.03% | −2.75% / −2.72% | −2.32% / −2.03% | −1.14% / −2.52% |
| `array_write` | −0.05% / +0.01% | −3.14% / −3.12% | −1.99% / −1.41% | −1.26% / −8.86% |
| `call0` | −3.27% / −3.37% | −0.70% / −0.68% | −0.09% / −0.26% | −0.05% / +0.63% |
| `string_bridge` | +0.54% / +0.50% | −1.39% / −0.62% | +0.59% / +1.85% | −1.15% / −0.98% |
| `type_error` | −0.63% / −0.34% | −0.22% / −0.06% | −0.15% / −0.26% | +1.06% / −1.49% |
| `tdz` | −0.12% / −0.25% | −0.19% / −0.12% | −0.96% / −0.97% | −4.08% / −0.53% |
| `local_move` | −18.26% / −18.38% | −3.18% / −3.17% | +0.38% / +0.55% | +1.07% / +6.11% |
| `argument_move` | −15.65% / −15.73% | −3.75% / −3.73% | −0.20% / +0.04% | +0.38% / +2.92% |
| `number_owner_fallback` | +0.37% / +0.40% | −1.68% / −1.67% | +0.90% / +0.84% | +0.80% / −0.97% |
| `object_move` | +2.08% / +2.05% | −1.68% / −1.68% | +6.08% / +6.21% | +5.81% / +6.05% |
| `empty_loop` | −0.00% / −0.05% | −6.03% / −6.05% | −2.56% / −3.08% | −1.57% / −0.93% |

同一 Current 二进制的 A/A 对照中，20 项最大的 instructions 中位数差为 0.077%，cycles 为 1.073%；wall 单靠中位数曾误报 `fusion_no_plan` −2.26% 与 `array_write` −3.49%，不能把 2% wall 变化一律裁定为收益或回退。Parent→Current 的 `object_move` 退休指令 **+2.08%/+2.05%**，两轮重复且远大于该项 A/A instructions 差约 0.02%；Current→Choice 虽减少该项指令 **1.68%/1.68%**，cycles 与 wall 却两轮均上升约 6%。这要求另查生成代码及执行依赖。其余 Parent→Current 的 `prop_write` wall +7.3% 两轮同时伴随很高的 side CV（39.8%/21.3%），而 instructions 基本不变、cycles 略减，故不能按 wall 单项认定该路径回退。

RSS 复核门槛是左侧中位数的 `max(3%, 1 MiB)`，本组约 6 MiB 的工作负载全部由 1 MiB 主导；80 个批次/负载比较均未越线，最大中位数差 264 KiB。Peak footprint 是补充指标。Parent→Choice 的逐项累计乘积已记录在 JSON，但两步来自不同交错批次，**不是** Parent/Choice 直接配对 A/B，尤其 wall 的 Current 参考窗口漂移不能忽略。整进程 `/usr/bin/time -l` 计数包括启动、编译和退出，没有缓存或分支事件。

集成版 `d6a168e2` 的 plain 构建已与受测 Choice `315035b5` 对照：选定的 8 个 VM 符号（`run`、两个 numeric local helper、五个 Dense helper）的地址与 ARM64 指令字逐条相同，其中 `run` 比较了 5,176 条指令。可复核对照在 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/series/codegen-integrated/comparison-choice.json`（SHA-256 `733ef288f79cd228285ae4dd583309991bc5aadc6a7243602f3fc7d80de837f4`）及同目录反汇编；它只证明这些选定符号相同，**不证明整个二进制相同**。

### 修订探针与普通写入分类内联

修订后的两个探针分别完成 A/A 和两轮交错 A/B，共 96/96 个有效样本，完整分布及原始哈希见 [revised-summary.json](revised-summary.json)。Current→集成入口的 flag=0 指令数为 −0.4718%／−0.4930%；**真正无计划函数则为 +0.6539%／+0.6718%**，对应同二进制 A/A 仅 +0.0055%。该独立回退保留，不能用旧同名探针的下降或后续切片的收益抵消。

现有反汇编显示，no-plan 的 GetLocal 在两版都通过同样形态的四条指令直达 canonical `RunSlots::local`，不会执行 flag load 或 choice/helper；canonical 起始段也相同。每 300 万轮多退休约 2,400 万条指令的来源仍未定位，不能称为“布局噪声”。后续需要覆盖整个循环的动态机器 PC 归因，而不是继续只数该入口的静态分支。

对象普通写回的生成代码发现了具体额外工作：Current 的 `direct_write_class` 被 outlined，另建栈帧、调用 readiness，再包装和解码分类结果。`8c38bc32` 将该 helper 改为 `#[inline(always)]`；受测 ARM64 产物已没有该独立符号及其调用，仍保留 Number 分类和必要 readiness 检查。它是本次 codegen 选择，不是永久 API 契约。该单行候选已以 `9df1969f` 接入当前实验分支。

两轮 Integrated→Inline 的对象负载指令数下降 1.45%／1.42%，没有其他项出现重复且大于 0.1% 的指令增长。直接 Parent→Inline 中，局部变量搬运 −22.46%、参数搬运 −20.13%、对象搬运 −1.06%。但对象搬运的 **cycles +6.81%、wall +5.03%** 仍是需保留的受干扰不利观测，不能宣布时间回退已修复。三批共 960/960 样本输出有效，RSS 最大中位数差 264 KiB，未越过 1 MiB 复核线。完整 20 项、五种指标、每侧 8 样本的分布与身份见 [inline-summary.json](inline-summary.json)。

另用相同构建配置直接重建 B37、R0，与 Inline 分别交错测量 20 项；每批 320/320 样本有效。数组读取退休指令相对两个历史分母分别 −63.142%／−63.119%，局部变量搬运 −26.779%／−26.682%，参数搬运 −24.824%／−24.831%。这些是整个版本跨度的累计结果，不能全归给本轮入口或单行内联注解。`type_error`、`tdz` 有小于 0.36% 的正差；两组历史对照加两轮 Inline 对照的 80 个项目中，无指令增长超过 2% 或 RSS 越过 `max(3%, 1 MiB)` 的项目。完整统计见 [cumulative-summary.json](cumulative-summary.json)；这只核对固定工作量的指令与 RSS 门槛，不能替代时间／V8 准入。

### 真实 V8 逻辑覆盖：找出该删的工作

另用外部 pinned v8-v7 源码原始 body，为八个子项分别运行一次固定 Setup/run/TearDown 并保存 `-d --profile-json`；这是执行覆盖诊断，**不是** adaptive Score 或普通版耗时。完整各 suite 的静态站点、动态 PC、融合各类尝试/命中/失败、callsite、omission、原始路径与 SHA-256 在 [profile-summary.json](profile-summary.json)；外部分析在 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/analysis-profile/README.md`。profile 原始结果 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/v8-fixed-profile/results.json` 的 SHA-256 为 `3c4d0409f07d1af116805a4c58de8f696f84dc9045ffa85a9d3ca011f17c3746`；仓库 summary 的 SHA-256 为 `8b9d6bf3ff3ba87bfe3575d4461f1eb8555174817d99485edd7ccf1598759a74`。

| 子项 | 静态未融合 direct-read 站点 / 全部 | 动态 flag=0 访问 / 全部 | Dense 命中 / 尝试 | 被观察 callsite 中单一 Object 身份的调用 / 全部 |
| --- | ---: | ---: | ---: | ---: |
| Richards | 132 / 140 | 57,018 / 57,962 | 0 / 3 | 33,899 / 40,472 |
| DeltaBlue | 257 / 285 | 93,992 / 110,190 | 0 / 3 | 95,965 / 115,452 |
| Crypto | 828 / 971 | 14,510,407 / 17,057,925 | 929,995 / 2,421,003 | 85,334 / 85,340 |
| RayTrace | 366 / 385 | 1,001,140 / 1,024,517 | 0 / 41 | 151,938 / 158,994 |
| EarleyBoyer | 813 / 913 | 4,120,766 / 4,227,266 | 1,785 / 97,775 | 503,772 / 512,379 |
| RegExp | 345 / 417 | 504,529 / 675,779 | 0 / 3 | 379,476 / 379,476 |
| Splay | 131 / 143 | 4,203,463 / 4,235,548 | 15,998 / 16,001 | 626,943 / 626,943 |
| NavierStokes | 374 / 472 | 6,331,197 / 10,777,430 | 3,316,344 / 3,316,347 | 476 / 476 |

静态未融合站点在各子项占 79.2%～95.1%；动态 flag=0 访问占 58.7%～99.2%，说明“静态不适用也反复探测”的成本具有真实覆盖，但尚不能从覆盖次数直接推出耗时收益。Crypto Dense 的 1,491,008 次未命中中，`array_materialized` 为 1,490,963 次（99.997%），另有越界 41、非 Number 4；这与外部 Crypto 源码由高位向低位写数组的路径一致，仍需定位导致 materialize 的具体状态转换。NavierStokes 则 3,316,344 / 3,316,347 次命中，说明 Dense 路径也必须看**命中后每次执行的成本**。八项被观察的 callsite 共 1,919,532 次调用，按调用加权 97.83% 落在单一 Object 身份的站点；身份变化并不等于 bytecode 实现变化，新建 closure 也会换身份。EarleyBoyer 有 12 个站点、8,597 次调用触及“超过四种身份”记录上限，至少五种身份，不能把它们误记成恰好五种。

融合事件的 omission 均为零；通用 VM phase 样本有非零 omission，按 suite 逐项保存在 summary。上述结果支持下一轮调查普通路径、Crypto 数组表示和调用点成本；不构成任何一种候选的收益上界。原版 V8 时间采样已按维护者要求停止，本文件不作候选正式 Score、时间准入或整体加速声明。

诊断标签优先级在 `b4a5f446` 修正后另行构建、重跑八项固定 V8。去除元数据中的提交／文件路径／时间后，八项 `functions`、`dispatch`、`sites`、`callsites` 的完整记录以及 omission 均逐项一致，包括 Crypto 的上述失败分布。复核结果及 v2 原始 SHA 见 [profile-v2-summary.json](profile-v2-summary.json)。这是逻辑事件一致性，不代表普通构建的时间或 Score 相同。

Crypto 的静态原因链也已核对：BigInteger 的 fresh Array 从空 dense 存储开始，`bnpSquareTo` 的倒序清零可首先写入大于当前 dense 长度的索引；普通 indexed definition 随即通过 `materialize_dense_array` 将 dense 值搬入通用属性槽，并将 dense 置为 None。当前实现不会自动恢复 dense 表示。该规则足以解释这种首次高位写为何破坏后续快路径；但 profile 没有 Array 身份与转换 PC，其他写入和目标复用也可能参与，不能将全部 1,490,963 次失败归因于同一处代码。`bnpSquareTo` PC 24 到具体源行的对应仍待该函数的发布后 opcode 表确认。

## 正确性与工具验证

完整命令、源码身份、退出状态、日志与报告 SHA-256 见 [validation-summary.json](validation-summary.json)。这些验证属于不同冻结点，不能合称为最新 HEAD 的一次全量通过：

| 源码身份 | 验证 | 结果 |
| --- | --- | --- |
| 冻结的集成执行版 `d6a168e2` | `TEST262_WORKERS=2 CARGO_BUILD_JOBS=2 ./scripts/test262/test-test262.sh --full` | 102,037 个变体，80,060 eligible，80,010 pass；完整结果向量与冻结基准逐项匹配，命令退出 0。其余结果保留在原始报告中，不计作通过。 |
| 集成版加固定探针修订 `b59331bc` | profiling 库测试；CLI profiling 测试 | 分别 2,134/2,134 和 6/6 通过。新探针的发布器测试另确认 `fusion_no_plan` 无计划、`fusion_flag0` 的热循环局部读取 flag 为零。 |
| `b59331bc` | Rust 1.88 workspace default 与 profiling 的全 targets Clippy `-D warnings`、`cargo +1.88.0 fmt --all -- --check` | 均退出 0。 |
| profiling 标签修正 `b4a5f446` | `dense_diagnostic_priority_` 两项定向测试 | 初次与保存日志的回放均为 2/2 通过。此前的 2,134 项全量测试没有覆盖这次后续修正。 |
| 工具检查 | benchmark Python 单测、源码布局检查 | 分别 37/37 通过、625 个可达 Rust 文件通过。Python 日志未自证执行时的精确 Git 提交。 |
| PR head `945d485d`，CI checkout `4bb6e870` | 远端 fast 与 test262-focused | 均通过。核对两个提交的 Git tree 均为 `5e0e5e46a21d882e5a3d5e27ab2687f4bc398654`，因此该 CI 覆盖已集成的内联注解及诊断修正。fast 包括 workspace 全 targets、CLI profiling、部分 profiling 库测试、doc／host 测试、Clippy、格式与架构／工具检查；不冒充全量 profiling 库回放。 |

另行构建的 inline 候选 `8c38bc32` 仅改变一个内联注解，已以 `9df1969f` 并入当前实验分支；较早的 `d6a168e2` 完整 Test262 与 `b59331bc` 全量 profiling 库测试不覆盖它。后续探针、诊断标签及内联注解提交不在该完整 Test262 结果中。受测候选的固定矩阵输出校验另见各测量收据，不冒充最新 HEAD 的全量 Test262。

后续 [CI run 36214372192](https://github.com/pocket-nexus/quickjs-oxide/actions/runs/36214372192) 覆盖了包含该注解的 PR tree。该次 `test262-full` 与 `quickjs-differential` 按 workflow 条件跳过，不计为通过；完整日志、实际 checkout 提交与 tree 对照已留入 validation summary。

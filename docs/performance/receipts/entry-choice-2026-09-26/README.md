# 2026-09-26：测量、真实覆盖与融合入口实验

本轮把三个讨论合并成三项可核验工作：修正测量与文档、补齐当前真实负载的成本/覆盖诊断、按证据试验执行入口。数组表示与调用架构重写属于后续候选；本轮只调查其实际成本。

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

普通计时构建为 release、fat LTO、CGU=1，无 PGO/profiling。诊断构建另存，逻辑事件不作为普通版耗时或 Score。macOS 原生 `/usr/bin/time -l` 在此主机提供整进程退休指令、cycles、最大 RSS 和 peak footprint；不是 Linux `perf ...:u` 的用户态专用口径。Instruments 的采样与 bottleneck 指标另行保留，不能把百分比当作机制收益上界。

Current 和 Parent 构建启动于工具完善期间，完整 Cargo verbose 日志保留了实际 rustc 命令和参数，但其旧版 receipt 中的 tooling hash 是构建结束时观察，不能证明进程加载的脚本版本。后续构建工具在启动时冻结脚本快照并在结束时校验；这项边界不改变前两份干净源码、完整编译命令与二进制的身份。

## 初始机器码证据与候选边界

ARM64 普通版中，整个函数没有融合计划时已有一次跳转进入普通路径。存在其他融合站点、当前 GetLocal 的 flag 为零时，仍执行多个 selector 的条件分支。候选保留原 u8 sidecar、canonical 指令和 PC，仅把互斥的五类候选改成一次选择，并显式绕过零 flag；Number 未命中后的字符串 Add 桥和动态回落维持原有顺序。不能由源码 if 数量直接推导机器指令收益，后续以受测产物的反汇编核对。

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

融合事件的 omission 均为零；通用 VM phase 样本有非零 omission，按 suite 逐项保存在 summary。上述结果支持下一轮调查普通路径、Crypto 数组表示和调用点成本；不构成任何一种候选的收益上界。正式 V8 时序测量仍需独立完成，本文件当前不作准入或整体性能声明。

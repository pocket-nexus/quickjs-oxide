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

普通计时构建为 release、fat LTO、CGU=1，无 PGO/profiling。诊断构建另存，逻辑事件不作为普通版耗时或 Score。macOS 原生 `/usr/bin/time -l` 在此主机提供整进程退休指令、cycles、最大 RSS 和 peak footprint；不是 Linux `perf ...:u` 的用户态专用口径。Instruments 的采样与 bottleneck 指标另行保留，不能把百分比当作机制收益上界。

Current 和 Parent 构建启动于工具完善期间，完整 Cargo verbose 日志保留了实际 rustc 命令和参数，但其旧版 receipt 中的 tooling hash 是构建结束时观察，不能证明进程加载的脚本版本。后续构建工具在启动时冻结脚本快照并在结束时校验；这项边界不改变前两份干净源码、完整编译命令与二进制的身份。

## 初始机器码证据与候选边界

ARM64 普通版中，整个函数没有融合计划时已有一次跳转进入普通路径。存在其他融合站点、当前 GetLocal 的 flag 为零时，仍执行多个 selector 的条件分支。候选保留原 u8 sidecar、canonical 指令和 PC，仅把互斥的五类候选改成一次选择，并显式绕过零 flag；Number 未命中后的字符串 Add 桥和动态回落维持原有顺序。不能由源码 if 数量直接推导机器指令收益，后续以受测产物的反汇编核对。

## 结果

测量进行中；本文件尚不构成准入或整体性能声明。

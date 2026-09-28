# Safe Rust 执行优化

以 PR #52 `996663f771afdabdc69d52c94bd4d2fb392e27b1` 为新起点：
发布后使用单一 `ExecCode` 字流，`vm/execute.rs` 是唯一指令循环。
PR #53 已实现 M1 数值累加及 M2 的直接目的地、数组更新和比较分支，
并发布执行字 continuation 与一个在 V8 v7 中实际执行的数组乘积更新来源；
其他工作流仍是设计提案。当前实现由
[架构文档](../architecture.md)描述。

## 当前阅读入口

- [优化与 Profile 原则](principles.md)：事实的建立阶段、有效作用域、所有权和证据要求。
- [后续路线与 M1/M2](roadmap.md)：三十项模式的去向、已交付的数值操作和后续里程碑。
- [测量协议](measurement.md)：新系列对照 #52 与 Parent，分开编译、执行、适应、内存与延迟。
- [诊断工具](../profiling.md)与[benchmark 工具](../../scripts/benchmark/README.md)：真实计数覆盖、运行入口与限制。
- [固定工作量配方](probes/README.md)：工作量及输出契约；历史 case 名称不代表 #52 的 opcode 分类。

优化先删除重复工作，再测其机器成本和真实覆盖。单一候选的结果可以改变形态、
接口及实施顺序；历史百分比门槛、旧 helper 边界和一次失败不限制新设计。
保留 safe Rust、JavaScript 可观察语义、完整代际身份、root 与释放责任。

## 历史收据索引

每项结果只属于其记录的源码、工具链、负载和二进制。日期是实验系列日期，
不是对当前 HEAD 的重新验证。#52 重写收据使用其记载的临时测量快照
`e0c2037970e168c0cfd2a606b1558ab780020b41`，不能仅凭本次文档修改把它
改称 #52 commit 的一次新测量。后续系列须重建明确的 #52 对照。

| 日期 | 收据 | 内容 |
| --- | --- | --- |
| 2026-09-28 | [执行字计划与覆盖](receipts/execution-ready-plans-2026-09-28/README.md) | 运行时直接消费发布事实、V8 执行站点选择、dense 单次认证与三版对照 |
| 2026-09-28 | [M2 数值操作扩展](receipts/m2-numeric-operations-2026-09-28/README.md) | 直接目的地、数组更新、比较分支及当前 #53/#52 独立对照 |
| 2026-09-28 | [M1 数值区域](receipts/m1-numeric-region-2026-09-28/README.md) | 数据流选择、单操作执行、语义验证及 Parent/#52 新构建对照 |
| 2026-09-27 | [VM 重写](receipts/vm-rewrite-2026-09-27/README.md) | 单流替换、语义验证、固定工作量与原版 Score |
| 2026-09-26 | [Shared paths](receipts/shared-paths-2026-09-26/README.md) | 自有属性、materialized 数组读、调用与帧清理 |
| 2026-09-26 | [Plan closure](receipts/plan-closure-2026-09-26/README.md) | 静态事实、内联、候选取舍与累计证据 |
| 2026-09-26 | [Cost follow-up](receipts/cost-follow-up-2026-09-26/README.md) | 存储恢复、callee 读取和短探针 |
| 2026-09-26 | [Follow-through](receipts/follow-through-2026-09-26/README.md) | dense 恢复、首操作数与调用校验 |
| 2026-09-26 | [Entry choice](receipts/entry-choice-2026-09-26/README.md) | 旧融合入口、非候选与自然 miss 成本 |
| 2026-09-26 | [Number writes](receipts/ordinary-number-writes-2026-09-26/README.md) | 普通数值写入探索 |
| 2026-09-25 | [Four-way](receipts/fourway-2026-09-25/README.md) | 错误载体/数组候选单独及组合 Score |
| 2026-09-25 | [Gates](receipts/gates-2026-09-25/README.md) | 当次 #41 复验与负结果 |
| 2026-09-25 | [Array manifest](receipts/all-dense-6db6bfb0/README.md) | 旧发布器的四函数、25 静态站点 |
| 2026-09-25 | [R0](receipts/r0-d6080b38/README.md) | 旧 R0 数组读取的编译/发布覆盖 |

[issues #41–#44 证据账本](evidence.md)、[历史报告](../reports/README.md)、
[前端编译测量](../compile-benchmark.md)和[原语 VM 历史结果](../primitive-vm.md)
保留各自来源。B37/R0/H0、旧 3–4 倍目标及实验门槛属于这些历史系列，
不作为 #52 新路线的准入要求，也不因清理而成为已完成的收益。

已删除的 performance slice、numeric span、verify/publication、lexer/parser
及 Test262 migration 计划可在[清理前固定快照](https://github.com/pocket-nexus/quickjs-oxide/tree/996663f771afdabdc69d52c94bd4d2fb392e27b1/docs)
查阅。现行语义与操作说明保存在当前指南；历史计划不再是实现指令。

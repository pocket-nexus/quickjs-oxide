# B1/C 负结果：历史摘要与原文

> 历史实验，不是当前实施计划。原裁决日期：2026-09-23。
> 本页于 2026-09-25 整理为摘要；完整实验、表格、限制和原始路径见
> [清理前的完整报告](https://github.com/pocket-nexus/quickjs-oxide/blob/f531f6052cb497ce4707f01c276e8642e5e26788/docs/reports/s3-c-negative-result.md)。
> 当前路线：[Safe Rust 性能改善](../performance/README.md)。

## 保留的裁决

C1–C4 与耦合的 B1a–B1d 整体撤回到 `bcfb4fe5`。当时的 QuickOp codec、只读发布投影、单槽标量／拥有式 TOS 及 StoreDrop 不构成当前的可复用实施基础。

在同协议全量第 1 轮中，V8 isolated 相对 pre-A 的 Score 比分别为默认 m0 **1.045**、C **1.017**、C+B **1.013**；combined 为 **1.035／1.000／1.000**。它们不支持采用 C/B 组合。

更直接的固定工作量与归因证据是：

| 机制 | 历史证据 |
| --- | --- |
| 单槽 TOS | local／arg 的 spill 中约 80%／83% 被下一次 push 驱逐；准入、安装、恢复难以摊薄 |
| 重复认证 facade | local／arg 组合退休指令分别约 +49%／+54%；其他输入也有明显增长 |
| QuickOp | 认证后解码链占采样 16.55–30.26%；canonical 回落比例 25.0–72.5% |
| 代码体积 | BC `.text` +1.04%；`run` 主循环实例总量 +91% |
| 只减 dispatch | StoreDrop 的 dispatch 减少没有转化为整体指令／时间改善 |

因此保留的教训是：**专用化必须删除真实 helper、拥有式中间值和重复证明；不能用更紧凑表示、更多命中或更少 dispatch 替代工作量与净收益证据。**

这些证据不证明所有 execution block、所有 quickening 或所有寄存器化思想无用。后续候选按 [当前原则](../performance/principles.md)说明实际删除的工作，并测量语义、命中与未命中路径、编译及空间成本；旧候选的门槛不自动沿用。

## 限制与不能重新解释的部分

当时是单机／单轮、powersave，单项时间噪声约 −12%～+6%；结论不能夸大成所有子项统计显著变慢。退休指令增长与新增热路径是主要机制证据，不能反过来声称已证明 I-cache 或分支预测是根因。

完整实现记录及其快照在 [历史入口](README.md) 的固定版本目录中。历史 #44 实验还发现，即使当时的 `run` 指令序列不变，outlined helper 的内联翻转仍可造成回退，见 [证据账本](../performance/evidence.md#e44)。

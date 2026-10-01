# 原语 VM 历史结果

本文保留 #52 之前原语执行核心重构的报告结果；不是当前执行架构或最新 HEAD
的验证。原文与当时结构描述见[清理前固定快照](https://github.com/pocket-nexus/quickjs-oxide/blob/996663f771afdabdc69d52c94bd4d2fb392e27b1/docs/primitive-vm.md)。
下列数值沿用原报告，其“重写前基线”不是 #52；本摘要未记录每个产物的完整
构建身份，不能拿这些倍率与新系列绝对数值混算，也不补造受测 commit。

当前代码责任和生命周期契约见[架构](architecture.md)，后续设计见
[优化路线](performance/roadmap.md)。#52 的单流替换有独立的
[测量收据](performance/receipts/vm-rewrite-2026-09-27/README.md)。

## 当时报告的验证

- Test262 冻结向量：`pass=80010 / eligible=80060 / total=102037`。
- workspace `--all-targets`、profiling lib、`--doc`、test262-host `--lib --bins`、oracle `test262_` 全部通过。
- pinned 1.88：clippy `-D warnings`、`cargo fmt --check`、source-layout、rust-only、oracle registry、QuickJS fixtures/c-oracles/dynamic-import 全部通过。

## 性能（相对重写前基线）

| 类别 | 数量 | 相对基线 | 相对 QuickJS |
| --- | ---: | --- | --- |
| fixed（wall） | 58 | 0.680× | 12.0× |
| probe（wall） | 33 | 0.721× | — |
| original（V8 Score） | 9 | 1.386× | 18.3× |

可比 78 例中 60 例比基线提速 >5%，仅 3 例慢 >5%。显著提速：`array_slice` −93%、数组写/更新/弹出 −70~−74%、`width-64/256` −79~−93%、`v8-navier-stokes` −60%、字符串族 −15~−49%；original `regexp` +150%、`navier-stokes` +156%。

## 当时报告的剩余差距

- `depth-proxy-0/32`（约 +11%）、`v8-earley-boyer`（+11%）、`richards` Score（−8%）仍慢于基线 >5%；前者属 proxy get 驱动的结构成本。
- 相对 QuickJS 仍有约一个数量级差距，集中在普通对象/数组属性访问与 V8 族基准。

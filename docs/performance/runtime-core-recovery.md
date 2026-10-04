# 阶段 B 恢复与迁移验收

阶段 A 已验收：PR #87 的 `e37b039` 与运行时 `e034f650` 相同。
PR #88 的 B40（`855fe93`，运行时 `f80b91ed`）保留为实验参照，尚未通过
阶段 B 的性能和架构验收。恢复栈从 A 分拆集成，每批同时交付直接消费者。

## 执行契约

共享语义算法不强制同步消费者建立整套挂起运输协议。同步完成与同步抛错
使用当前 RuntimeState、帧槽和短期 owner guard；实际 JS 子调用、挂起或
必要的状态访问交接才建立 Query、持久 progress 与 continuation。恢复消费
已完成的语义进展，沿用认证、发布、GC 安全点和失败清理契约。

诊断构建分别记录 Query 取得（包括池复用）、持久记录创建、真实等待或
交接，以及未经过这些效果便完成的次数。已迁移的同步消费者最后一项为零。
普通调用留在解释循环中仍属于真实子调用；共享 backing、宿主与迁移期
边界分别归因。计时与 callgrind 使用没有诊断 hooks 的普通 release。

## 批次

| 批次 | 原计划 | 交付 |
| --- | --- | --- |
| R0 | 验收基础 | 固定 A/B40 与工作量；现有重放支持 callgrind；先定位变化区间，修复开始后补齐前 32 个运行时提交的归因。 |
| R1 | B1、相关 B2/B3，提前部分 C3 | 分别提交 Set 同步契约、PutField、PutArrayEl/dense 值消费、数组方法消费者；支持 immediate 与 heap 值替换和追加。 |
| R2 | B1/B2/B3，提前部分 C1/C3 | primitive 键转换和 computed/dense 读取直接完成；实际 getter、Proxy、转换等待才发布恢复状态。 |
| R3 | 连续调用返回及 B 调用迁移 | 分开普通选择、安装与返回的热路径和冷回复；检查实际汇编、栈搬运和代码布局。 |
| R4 | B1–B3 收口 | 全子项累计无可信回退后，继续 B4，再删除 B5 旧协议并验收零残留指标。 |

## 日常证据

复用 scripts/benchmark/iterate_v8.py 已冻结的工作量和 fixed.py 重放。
Rust 1.88、同配置普通 release、无 PGO；复用 A/A 及 Boa 数据。

每个运行时 commit 对固定 Richards、DeltaBlue、NavierStokes 各收集一次
callgrind：Ir、Dr、Dw、I1mr、D1mr、D1mw、Bc、Bcm、Bi、Bim。固定
Valgrind 版本、缓存模型、源文件、二进制与工具哈希；校验原完整输出。
重放 CLI 使用 `--callgrind PATH --callgrind-cache-config FILE --repeat 1`，
FILE 含 I1/D1/LL 的 `[bytes, associativity, line_bytes]`。模拟耗时不进入
原生时间汇总；Valgrind 日志与 JS stderr 分开。必要时使用
`--callgrind-instructions` 收集指令位置。

历史先查调用/native 和读取关键节点及前后版本，再逐提交展开变化区间，
最后补齐其余提交。不同冻结迭代数的报告分别标识，函数移动必须连同
解释循环费用一起计算，不能相加 inclusive costs。调用使用实际 JS 次数
及现有 call0 probe；无法准确划界时不宣布独立的每次调用收益。

每批约 3–5 个运行时提交结束，八项及 Combined 对 A 做一个 ABBA 块：
`--repeat 2 --order abba`。局部效果对照前一个已验证版本。计时不并行
编译或测试；两对同向且超出既有 A/A 噪声的回退先修，临界或混合结果
仅复核相关项。短测指导集成，不作为原版 Score 或正式区间验收。

每提交运行相关正确性测试；每批 CI fast 与 focused Test262。阶段结束
执行完整正确性、原版 Score、置信区间和资源验收。代码体积增长上限
10%，固定 RSS 增长上限 5%，持续分配内存有界。B 的性能缺口在 B 内
解决，后续 C 单独归因。

## 实施记录

R0 已为 fixed.py 增加 callgrind 模式。计数按事件名解析最终 totals，拒绝
缺失、重复、负数、缓存模型不一致或不完整报告。错误语义输出即使有
有效计数也不能通过。当前工具改动只交付证据能力，不声称运行时提速。

### A/B40 固定工作量 callgrind

2026-10-05，复用既有冻结工作量，Richards 21、DeltaBlue 13、NavierStokes 2
次，warmup 0。各二进制每项一次，六个进程完整语义输出通过。Valgrind
3.25.1，I1/D1 均为 32768/8/64，LL 为 16777216/1/64；缓存为模拟模型。
普通 Rust 1.88 release 二进制、features 空、无 PGO，实际 compiler/target/
Cargo 配置与最终 qjs codegen 相同。A 构建回执的首次 rustup 安装 stderr
和工具链别名与 B40 不同，此处只核对实际编译器身份及生效构建参数，
原始回执没有改写。

| 工作量 | Ir | Dw | D1mw | I1mr | Bim |
| --- | ---: | ---: | ---: | ---: | ---: |
| Richards | +4.50% | +13.47% | +71.60% | +2.91% | +15.88% |
| DeltaBlue | +5.35% | +11.23% | +118.09% | +34.03% | +30.40% |
| NavierStokes | +5.76% | +16.89% | +23.05% | +120.11% | +10.85% |

这证明该配置下 B40 的写入和模拟缓存费用增加，不是原生耗时或原版
Score 验收。用户提供的 10/10/15 迭代报告保留为独立证据，不混合绝对
计数。回执位于 `/home/eric/.cache/oxide-runtime-core-20261003/
r0-a-b40-callgrind-20261005/comparison.json`，SHA256
`d06617245f7a3e4cbbb4054250440cbafd6bf08b4f91525dbfb087adf4029457`。

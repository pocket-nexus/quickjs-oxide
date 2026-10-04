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


### R1a：现有字段与 dense 写入消费者

恢复分支 `perf/runtime-core-recovery` 从 A 建立，正式 PR #89。运行时代码提交：
`7cbb1f31`（存储 owner 交换）、`3fe796c5`（VM 字段消费者）、
`800d0546`（VM dense 替换和追加）、`8e5eb207`（数字 payload 直接交换）。
接收者在帧中保持拥有，源 owner 转到 heap，旧目标 owner 回到源槽后由当前
State 释放；miss 不改变操作数。物理选择复用 ordinary Set 和 prototype
选择，getter、Proxy、冻结、hole、length 等仍按共享语义回退。
数字同类替换直接交换 payload，其他类型共用转换 leaf；不新增运行期模式。
旧的仅标量字段和 dense 写入实现已经删除。

此批完成时，新字段追加、Array 方法和其余通用 Set/读取消费者尚未迁移；本批不宣称
B1–B5 完成或架构零残留。最初候选的 DeltaBlue 同向回退由数字表示搬运
修复；以下只记录最终组合，不给基础 kernel 编造单独收益。

2026-10-05，Rust 1.88 普通 release、无 PGO，相同冻结工作量，一个 ABBA
块，A 对 `8e5eb207`；36/36 进程完整输出通过。数值为配对耗时变化的
中位数，负数表示更快；这是日常短测，非原版 Score / 正式置信区间。

| 子项 | 耗时变化 | 两对变化 |
| --- | ---: | --- |
| Richards | −20.05% | −19.78% / −20.31% |
| DeltaBlue | +0.39% | +0.88% / −0.09% |
| Crypto | −1.19% | +0.86% / −3.25% |
| RayTrace | +1.08% | +1.80% / +0.36% |
| EarleyBoyer | +3.06% | +2.11% / +4.01% |
| RegExp | +0.41% | +0.72% / +0.10% |
| Splay | −7.20% | −7.16% / −7.23% |
| NavierStokes | −0.78% | −1.08% / −0.49% |
| Combined | −2.67% | −2.99% / −2.36% |

EarleyBoyer 的初次同向回退触发单项复核：修复前后局部 ABBA 为
+1.99% / −1.94%；A 对最终候选复核为 −1.08% / +0.81%。此前回退未复现，
当前结论为未分辨，不能把首次 +3.06% 当作稳定回退或宣称该项已经提速。
RayTrace 处在既有 A/A 边缘且两对幅度不同，保留为未分辨。Combined 不能
代替这些单项结论。重新核对 A/A 回执后，manifest 与完整 workload 元数据
和本轮相同；迭代数是 R21/D13/C2/Ray2/EB2/Reg1/S1/NS2，Combined44。
复用这一噪声参照，不新增 A/A 或重新校准。

同二进制固定 callgrind（各项一次，模拟缓存）：

| 工作量 | Ir | Dw | I1mr | D1mw |
| --- | ---: | ---: | ---: | ---: |
| Richards | −16.48% | −19.91% | −68.31% | −7.94% |
| DeltaBlue | −0.96% | −1.15% | −0.12% | +0.75% |
| NavierStokes | +1.09% | +0.65% | −3.74% | −0.63% |

同一指令路径的缓存模拟仍受地址布局影响；没有把模拟缓存变化当作硬件
计数或把指令比例折算成耗时。三个运行时提交各自的 R/D/NS profile 和
未接入消费者的 kernel profile 都已保存。诊断 release（不用于计时）记录
Richards / DeltaBlue 在片段内字段写入 1,062,369 / 234,780 次，dense 写入
19,488 / 2,639 次；NavierStokes dense 写入 516,104 次。

正确性：最终运行时代码通过 workspace all-targets、CI fast 其余全部检查、
profiling 相关测试和 focused Test262，6844/6844 pass。ELF 的 text+RO
观察未接近 10% 上限；完整 RSS、停顿和阶段原版 Score 留到阶段验收。

回执目录统一为 `/home/eric/.cache/oxide-runtime-core-20261003/`：
- `recovery-r1-payload-vs-A-abba-20261005/{results,screen}.json`
- `recovery-r1-payload-vs-A-EB-check-20261005/{results,screen}.json`
- `recovery-r1-payload-local-EB-abba-20261005/results.json`
- `recovery-r1-callgrind-comparison.json` 与各 `recovery-callgrind-*` 原始文件
- `recovery-r1-ci-fast-gates.json`、`recovery-r1-final-workspace-tests.log`
- `recovery-r1-payload-focused.log`、`recovery-r1-mechanism-8e5eb207/`


### R1b：新增字段的共享发布与直接 owner 转移

运行时代码 `8e732550` 复用 missing-property 的 prototype 选择和 canonical
shape successor 发布；描述符与 VM 消费者共享同一 append 主体。VM 源槽
拥有的 JS 边直接转移到 heap，不先建立 SetOperands、Query 或 continuation。
setter、Proxy、不可扩展和需要独立 dictionary 重建的对象保持语义回退，
miss 不消费源 owner；失败回滚、atom 与 shape owner 沿用共享发布责任。

新增 shape 会消耗 GC 分配预算。`1c49652f` 将新增布局的收尾独立出来，
在 owner 转移和原有释放顺序完成后服务 GC；已有字段替换只执行直接清理。
最终汇编中，普通清理函数没有 GC 调用，新增布局收尾函数才检查预算。

初版相对上一验收运行时 `8e5eb207` 的局部 ABBA：Richards +3.53%、
DeltaBlue −3.16%、EarleyBoyer −6.50%。Richards 的同向回退触发收尾拆分；
修订后同一局部比较为 +1.35%、−1.36%、−7.65%。前两项幅度小，暂不作
独立收益结论。硬件 perf 仅用于解释初版：Richards 指令约 +0.093%，两对
user cycles +3.16% / +4.49%，branch-misses 变化混合；没有把模拟间接跳转
预测当成硬件结果，也没有断言已定位全部周期增加的原因。

最终组合 `1c49652f` 对阶段 A 的全九项日常 ABBA（2026-10-05），36/36
进程完整语义输出通过。相同 Rust 1.88 普通 release、无 PGO、冻结工作量。
以下为配对耗时变化中位数，非原版 Score 或正式置信区间。

| 子项 | 耗时变化 | 两对变化 |
| --- | ---: | --- |
| Richards | −18.64% | −16.50% / −20.79% |
| DeltaBlue | −0.01% | −0.42% / +0.39% |
| Crypto | −0.80% | −0.27% / −1.32% |
| RayTrace | −7.35% | −6.94% / −7.76% |
| EarleyBoyer | −9.07% | −11.12% / −7.02% |
| RegExp | +1.37% | +1.66% / +1.08% |
| Splay | −7.22% | −6.91% / −7.54% |
| NavierStokes | −0.37% | +0.06% / −0.80% |
| Combined | −4.11% | −4.06% / −4.16% |

小幅项保留为未分辨；Combined 不代替单项结论。新增字段的 heap/atom
转移、canonical shape 复用、setter/Proxy 回退、发布失败和发布后 GC 有
针对性测试。Array 方法及通用 Set 的内部所有权迁移仍未完成。

本批最终 workspace all-targets、CI fast 各检查及 focused Test262 已通过，
6844/6844 pass。两个运行时 commit 的 R/D/NS callgrind 均通过完整语义
输出校验。最终组合相对 A：Richards Ir −16.34%、Dw −19.97%；DeltaBlue
Ir −2.43%、Dw −2.65%；NavierStokes Ir +1.10%、Dw +0.68%。相对前一
运行时 DeltaBlue Ir / Dw 下降 1.49% / 1.52%，其时间变化仍未分辨。
模拟缓存变化不替代硬件或原生时间。完整资源和原版 Score 仍待阶段验收。

原始记录目录仍为
`/home/eric/.cache/oxide-runtime-core-20261003/`：
- `recovery-r1-owned-append-local-abba-20261005/`
- `recovery-r1-append-retirement-local-abba-20261005/`
- `recovery-r1-append-retirement-vs-A-abba-20261005/`
- `recovery-r1-owned-append-hardware-profile/results.json`
- `recovery-r1-append-retirement-asm.txt`
- `recovery-callgrind-8e732550/`、`recovery-callgrind-1c49652f/`
- `recovery-r1-append-callgrind-comparison.json`
- `recovery-r1-append-ci-fast-gates.json` 及四个 gate 日志
- `recovery-r1-append-focused.log`


### B1 状态基础：atom、捕获值与 Object opcode

三个运行时提交：`ffd22a92`（atom/对象/错误共享 State kernel）、`4ee964cb`
（捕获变量共享 State kernel）、`56ac336a`（Object opcode 与帧观察在当前
State 下完成）。构建/逐提交 profile 时的暂存提交分别为 `c16ae540`、
`693df72c`、`3baf6cd1`；逐层源码树相同，集成不改运行时代码或构建输入。
这批为后续 native 生命周期提供基础，不宣称每层都有独立时间收益。

最终相关 profiling 核心测试 2408/2408、workspace all-targets、CI fast 各项
和 focused Test262（6844/6844）均通过。每提交 R/D/NS 固定 callgrind 的
原完整输出通过。最终相对前一已验证运行时 `1c49652f`：Richards Ir +0.013%、
Dw −0.194%；DeltaBlue Ir −0.186%、Dw −0.381%；NavierStokes Ir −0.340%、
Dw −0.482%。这些计数不足以把时间变化归因到某一个 State kernel。

局部 ABBA 对 `1c49652f`：Richards −5.80%、DeltaBlue −6.21%、EarleyBoyer
−0.29%（混合）、NavierStokes +0.35%（混合）。另外 RegExp 单独局部为
+1.09%（+0.74% / +1.43%），未分辨。以下是同批对 A 的累计全九项 ABBA，
36/36 语义输出通过，仍不是原版 Score 或正式验收。

| 子项 | 耗时变化 | 两对变化 |
| --- | ---: | --- |
| Richards | −18.00% | −17.48% / −18.53% |
| DeltaBlue | −5.51% | −4.21% / −6.81% |
| Crypto | −6.70% | −7.40% / −6.00% |
| RayTrace | −7.46% | −6.58% / −8.34% |
| EarleyBoyer | −6.93% | −6.03% / −7.82% |
| RegExp | +2.24% | +2.02% / +2.46% |
| Splay | −8.12% | −6.71% / −9.54% |
| NavierStokes | +3.02% | +3.50% / +2.53% |
| Combined | −4.58% | −3.90% / −5.26% |

RegExp 的累计临界变化触发单项复核：+2.09%（+1.62% / +2.55%），既有
匹配 A/A 噪声参照约 2.07%，不是两对均超出参照的可信回退，也不能判为
无代价。NavierStokes 在约 3.26% 的既有参照内，局部变化混合；两项均保留
为未分辨并继续跟踪，阶段性能门槛尚未宣布通过。没有用 Combined 背书。

回执：`recovery-state-foundations-{vs-A,local}-abba-20261005/`、
`recovery-state-foundations-{vs-A-regexp-check,local-regexp-abba}-20261005/`、
`recovery-state-foundations-callgrind-{gates,comparison}.json`、
`recovery-state-foundations-ci-fast-gates.json`、
`recovery-state-foundations-focused.log` 与三个原始 `recovery-callgrind-*` 目录。

### 历史归因补充：B32 的瞬时 parent buffer 问题

B32 `aecf0703` 的 DeltaBlue profile 中，resident Query 回收函数自身约占
34.7% 指令：外层 activation 不从 spare pool 取回 parent buffer，退出却
持续放回，随后每次回收遍历并清空全部 spare。B33 `673e91ca` 复用取出的
buffer、在退休处清理语义 parents 并删除重复全池扫描，现有测试覆盖容量
有界、嵌套和失败路径。

同冻结工作量、普通 release 的 B33 对 B32：DeltaBlue Ir −34.59%、Dw
−20.48%；Richards 与 NavierStokes 几乎不变。B33 累计对 A 为 R Ir +2.14%、
Dw +5.60%；D Ir +4.10%、Dw +9.07%；NS Ir −1.74%、Dw −1.59%。因此
B32 的约 +59% 指令不能归因到 Date 算法或当作稳定 B 成本。
原始回执 `r0-history-callgrind-{aecf0703,673e91ca}/` 与
`r0-history-B32-B33-comparison.json`。其余历史区间仍需继续逐层补齐。

### B1 发布契约与构造事务

三个运行时提交：`b759e4d3` 将 global/descriptor 存储与发布失败处理移入
当前 State，保留 R1 的共享追加与 owner 转移；`419ca76e` 区分对象分配的
发布前拒绝和发布后失败，并在实际构造入口的临时 owner 清理前隔离状态；
`0b69fbd2` 将整个构造选择、分配和安装事务放到循环外。普通调用/返回
仍由当前执行循环推进，发布失败规则没有撤回。

错误注入覆盖 Symbol edge 发布后失败、prototype 清理失败、weak registry
失败和构造执行器的实际失败路径。workspace all-targets、CI fast 全部通过，
focused Test262 为 6844/6844；每个提交的 R/D/NS 固定 profile 语义输出通过。

分配迁移的首次短测发现 NavierStokes 局部 +18.08%。硬件 ABBA 确认
全进程用户态周期约 +18.95%，退休指令几乎不变，分支误预测结果混合；
另一个计数组合的前端停顿周期约 +87%，L1 指令缓存缺失却减少。它们支持
继续检查生成代码与执行吞吐，不能将模拟 cache 缺失换算为时间。
构造事务划界后，解释循环从 72,042 缩到 63,962 字节。相对分配迁移前版，
局部 R/D/NS 约 −7.32%/−6.54%/−4.44%；相对前一已验收源 `3baf6cd1`
（集成运行时 `56ac336a`）为 −5.57%/−5.64%/−4.84%，RegExp +0.96%
且两对混合。不能把这些跨实验变化相加，或全部归因到某一个硬件事件。

最终 `0b69fbd2` 对 A 的累计全九项 ABBA，36/36 原完整输出通过：

| 子项 | 耗时变化 | 两对变化 |
| --- | ---: | --- |
| Richards | -20.17% | -21.36% / -18.98% |
| DeltaBlue | -3.88% | -3.50% / -4.26% |
| Crypto | -7.22% | -6.64% / -7.80% |
| RayTrace | -8.40% | -8.60% / -8.19% |
| EarleyBoyer | -10.67% | -11.22% / -10.12% |
| RegExp | +1.75% | +2.55% / +0.95% |
| Splay | -8.35% | -8.65% / -8.06% |
| NavierStokes | +0.10% | +0.69% / -0.49% |
| Combined | -5.98% | -5.15% / -6.82% |

以上仍是两对日常短测，不是原版 Score 或正式置信区间。RegExp 的
两对跨过既有 A/A 参照，NavierStokes 两对混合；均保持未分辨，阶段 B
性能和架构门槛尚未宣布通过。最终 profile 对 A：R Ir −16.38%、Dw
−20.44%；D Ir −2.70%、Dw −3.48%；NS Ir +0.67%、Dw −0.10%。
普通 release 的 text＋rodata 相对 A 增长 0.26%；RSS、最长停顿与正式
资源验收仍待阶段收口。

原始回执：`recovery-layout-allocation-{vs-A,local}-abba-20261005/`、
`recovery-allocation-local-abba-20261005/`、
`recovery-constructor-outlined-local-abba-20261005/`、
`recovery-allocation-{hardware,frontend}/`、
`recovery-layout-allocation-gates.json`、
`recovery-layout-allocation-callgrind-vs-A.json`、
`recovery-layout-allocation-code-size.json` 及三个逐提交 profile 目录。

# V8 v7：实现简化试验验收（2026-10-02）

本轮从 `070d01bb666402ca74a306a2a4e33f357c41cc81` 并行探索三个独立方向，做了两次限定修订。最终只采纳 **RegExpSplit 复用已有 continuation Box**，生产与合同测试 commit 为 `a40fe61591cb97c285c9bdc1388c2b2f324278f4`。其余候选未合入；此前已丢弃的四个候选也不包含在结果中。

目标是降低完整程序执行时间，通过简化重复的认证、所有权交接与状态组织实现。硬约束包括 JavaScript 可观察顺序、完整 generation 认证、owner 与饱和行为、realm、异常清理和 GC 服务位置。没有新增通用执行框架或 benchmark 特判。

## 候选与决定

| 方向 / commit | 实际简化与覆盖 | 验收结果与决定 |
| --- | --- | --- |
| Call 执行窗口 / `afe9b335` | 在已有解释窗口内准备 Call；八项减少的 transaction 次数逐项等于 Call 次数。但返回容器 16→24 B，并引入准备状态及 ABI。 | 固定工作量 NavierStokes 耗时 **+4.44%**，该次 A/A 观察范围 1.14%；拒绝。 |
| Call 限定 fusion / `f1c358d3` | 内联唯一 prepare caller，删除该 ABI 与连续 staging copy；非 Call 的返回分类和解释器栈成本仍在。 | 限定 pilot NavierStokes **+1.98%**，A/A 1.33%；停止并拒绝，没有追加原版套件。 |
| StringSearch 本地阶段 / `06541b09` | primitive 路径不创建原有 88 B Box，真正等待回调时才创建；八个方法共用推进逻辑。 | 实际 V8 v7 命中集中在 RegExp **Setup 的 32,958 次 substring**，计时块没有该消费者；RegExp 固定耗时 **+1.70%**，A/A 1.20%。本轮不采纳。 |
| 普通属性读取 / `999034ce` | 一个 Runtime borrow 遍历普通 prototype 段，终点提升必要 owner；共享物理槽选择，保持 getter/Proxy 边界。 | EarleyBoyer 计时阶段新增 104,072 次借用跳步，但原版 Score **212→212**；RegExp **89.05→87.4（−1.85%）**，四个候选样本均低于全部基线样本。Combined 184→187.5（+1.90%）仍不足以消除该代价，未采纳。 |
| RegExpSplit resident Box / `a40fe615` | 一个初始 Box 贯穿 next/execute/captures 请求，替代循环中重复重建。独立于 StringSearch，RegExp 计时块有 34 个 `.split(re)` 源码调用点。 | 原版 RegExp **89.25→91.15（+2.13%）**，完整候选 Combined **182.5→188（+3.01%）**；采纳，同时保留 Splay **−0.53%** 的观察结果。 |

固定工作量耗时变化与原版 Score 变化是不同指标；以上候选独立测量，不叠加各自收益。A/A 范围是 `(max−min)/median`，用于描述观察波动，不是置信区间。

## 采纳的生产机制及交换

在一个诊断构建的完整 RegExp Setup/run/TearDown 中，初始 Box 为 **2,570** 个，next 复用 **843,320** 次，execute 复用 **843,340** 次，共避免 **1,686,660** 次循环 Box 重建。其他七项没有这个机制命中；该计数不代表原版自适应运行次数，也不代表峰值内存下降。

普通 release 汇编中，三个循环 helper 的 `malloc(352)`、`memcpy(352)` 与空旧 Box 的正常清理路径消失。初始分配、Phase/SplitState 移动、Phase Drop、请求搬运、回调和最终释放仍保留。每次 exec 仍重新选择方法，没有跨 JS 回调缓存动态事实。请求字段按原顺序逐项 take；原始 input/limit owner 的转移时点保持一致。

`.text` 增加 **160 B**。三个 helper 的局部栈分别减少 224/320/288 B，resume 增加 16 B；这些数字包含保存寄存器，不能当作完整活动链峰值。解释循环及主要 dispatcher 的归一化生成代码形状不变。本轮未测峰值 RSS、完整 release 栈峰值、编译时间或停顿分布，因此没有这些资源收益结论。

Richards、DeltaBlue、Crypto、NavierStokes 的分数变化只能记为**整个候选产物的观察结果**。它们没有 RegExpSplit 命中，不能把这些变化归因于循环 Box 删除；代码布局等相互作用尚未区分。

## 最终无 PGO 原版 Score 与 Boa

两种受测二进制各有 4 次完整 combined 输出，按 ABBA–BAAB 串行、CPU 2 执行。下表均取这组完整运行中的中位数，Score 越高越好。Boa 0.22.0 使用历史完整套件三轮的中位数，**没有重跑**。这不是相对会话开头或旧 main 的累计百分比。

| Benchmark | 同期基线 | 采纳后 | Score 变化 | 基线 min–max | 候选 min–max | Boa | 我们 / Boa |
| --- | ---: | ---: | ---: | --- | --- | ---: | ---: |
| Richards | 86.45 | 91.95 | +6.36% | 85.4–89.5 | 91.4–92.8 | 225 | 40.9% |
| DeltaBlue | 94.85 | 102.5 | +8.07% | 92.9–99.5 | 102–103 | 206 | 49.8% |
| Crypto | 194 | 201.5 | +3.87% | 190–195 | 200–204 | 242 | 83.3% |
| RayTrace | 156 | 157 | +0.64% | 156–158 | 157–158 | 398 | 39.4% |
| EarleyBoyer | 212 | 213 | +0.47% | 211–214 | 212–214 | 502 | 42.4% |
| RegExp | 89.25 | 91.15 | +2.13% | 88.9–89.6 | 90.7–91.8 | 67.6 | **134.8%** |
| Splay | 469 | 466.5 | **−0.53%** | 468–478 | 465–468 | 833 | 56.0% |
| NavierStokes | 558 | 581.5 | +4.21% | 555–560 | 580–583 | 520 | **111.8%** |
| **Combined** | **182.5** | **188** | **+3.01%** | **181–186** | **188–188** | **300** | **62.7%** |

每种二进制只有四个观察值，min–max 不是置信区间。RayTrace、EarleyBoyer 的小幅变化尚未分辨；Splay 四个候选均不高于基线最低值，应保留下降结果，不能宣称全项改善。当前仅 RegExp、NavierStokes 超过 Boa，**全部八项和 Combined 超过 Boa 的目标未达到**。

## 构建、语义验证与停止位置

真实性能产物：Rust **1.88.0 / LLVM 20.1.5**，显式 `x86_64-unknown-linux-gnu`，release opt-level 3 / fat LTO / CGU 1，features 为空。构建记录与实际 qjs rustc 参数确认 `optimization.kind=none`，**无 PGO**。诊断 profile 与合同测试使用另外的构建，未拿它们的时间当原版 Score。

- 最终 Split 合同测试 **6/6**，完整库测试 **2308/2308**；两者实际 exit 0，受测源前后不变。覆盖真实 VM 的 Species/Get/Set/Construct/Exec、重复 exec getter、capture 顺序、pending take/abandonment、owner 别名、稳定 Box、Unicode/空匹配/limit、throw/unwind 与 cross-realm。
- 初次合同测试 5/6 失败，旧同步 observer 的默认 species 留下 constructor owner；仅将 observer fixture 改为真实 JS species constructor，生产默认 species 路径另有测试。初冻 `3b044f9d` 到最终 `a40fe615` 无生产差异，失败日志保留。
- 本轮未重新执行完整 Test262 或整套 CI，不声称它们已通过。仓库 CI 的 full Test262 为 schedule/manual 阶段；本次局部采纳依据以上合同与完整库验证。
- Split 完整固定工作量矩阵 144/144 valid；其中 RegExp 耗时 −1.87%，A/A 2.38%，Combined −1.15%，A/A 2.09%，该矩阵本身未分辨。一次限定的 5-run RegExp 对照耗时 −1.66%，也接近 A/A 1.66%；最终收益结论来自随后完整原版输出。
- 用户指出重复采样成本过高后，收完已在运行的最后一个原版进程并停止；没有追加性能轮次、Boa、perf 或 RSS 测量。

## 可追溯记录

原始日志在本机归档根目录：`/home/eric/.cache/oxide-v8v7-boa-campaign/structural-simplification-20261002/`（下文相对此目录）。

| 记录 | 路径或身份 |
| --- | --- |
| 实验初始计划 / 独立初版矩阵 | `experiment.json` / `initial-matrix-summary.json` |
| Split 原版摘要与全部样本 | `split/original-summary.json` / `split/original/results.json`；results SHA-256 `d922da11066c8eaaa55a6cbbe5d201297599bbe9fac14d972b4e4fec3fbf0f7b` |
| 原版负载正文 SHA-256 | `777f2c2ea9fcf3b8c7db198f758f2e7bd2113c4f8a347ea7323e13c85b4d9946` |
| 基线二进制 SHA-256 | `7a5e91ad348f835202546487e2c6b095787283b883638435225d708c627705aa` |
| 采纳二进制 SHA-256 | `efa87f77dc38643e1042d496887cb8f2d41ce2d96a9d26fb71615c83dd557083` |
| Split 实际原版 stage / 构建 stage | `split/10-original-stage/receipt.json` / `split/06-fresh-plain-build/receipt.json` |
| Split 机制 / 源码 / 汇编 | `split/logical-summary.json` / `regexp-split/source-report.md` / `regexp-split/codegen/report.md` |
| Split 最终合同 / 完整库 receipt | `regexp-split/focused-tests-final/receipt.json` / `regexp-split/full-lib-tests/receipt.json` |
| Split 固定矩阵 / 限定对照 | `split/fixed-authenticated/results.json` / `split/reg-resolution/results.json` |
| 普通读取原版 / 归因 | `ordinary/original-summary.json` / `ordinary/codegen/attribution.md` |
| Call fusion 限定结果 / 汇编 | `fused/pilot-final/results.json` / `frame/fusion-generated-audit.md` |
| StringSearch 覆盖 | `string/coverage-review.md` |

Boa 历史记录：`/home/eric/.cache/oxide-history-three-engines-2026-10-01/boa-missing-combined/results.json`。历史 Boa 与本轮 oxide 的工具链不同；这份数据支持分数对照，不是同工具链的引擎因果实验。

失败的 fusion 脚本访问 `wall_ns`、Split 缓存构建缺少实际 rustc 参数以及 tooling 路径不符均保留在归档中；这些失败 stage 不纳入有效样本。正式结果只使用独立成功 stage，未把失败数据补成成功。

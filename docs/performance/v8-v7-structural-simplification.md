# V8 v7：RegExpSplit 简化验收（2026-10-02）

已采纳 **RegExpSplit 复用已有 continuation Box**，生产与合同测试 commit 为 `a40fe61591cb97c285c9bdc1388c2b2f324278f4`，配对基线为 `070d01bb666402ca74a306a2a4e33f357c41cc81`。

目标是通过简化循环中的状态容器重建，降低完整程序执行时间。JavaScript 可观察顺序、完整 generation 认证、owner 与饱和行为、realm、异常清理和 GC 服务位置保持原合同。

## 生产机制及交换

初始 352 B Box 贯穿 next/execute/captures 请求。RegExp 计时块有 **34 个 `.split(re)` 源码调用点**。一个诊断构建的完整 RegExp Setup/run/TearDown 中，初始 Box 为 **2,570** 个，next 复用 **843,320** 次，execute 复用 **843,340** 次，共避免 **1,686,660** 次循环 Box 重建。其他七项没有这个机制命中；该计数不代表原版自适应运行次数，也不代表峰值内存下降。

普通 release 汇编中，三个循环 helper 的 `malloc(352)`、`memcpy(352)` 与空旧 Box 的正常清理路径消失。初始分配、Phase/SplitState 移动、Phase Drop、请求搬运、回调和最终释放仍保留。每次 exec 仍重新选择方法，没有跨 JS 回调缓存动态事实。请求字段按原顺序逐项 take；原始 input/limit owner 的转移时点保持一致。

`.text` 增加 **160 B**。三个 helper 的局部栈分别减少 224/320/288 B，resume 增加 16 B；这些数字包含保存寄存器，不能当作完整活动链峰值。解释循环及主要 dispatcher 的归一化生成代码形状不变。本轮未测峰值 RSS、完整 release 栈峰值、编译时间或停顿分布，因此没有这些资源收益结论。

Richards、DeltaBlue、Crypto、NavierStokes 的分数变化只能记为**整个受测产物的观察结果**。它们没有 RegExpSplit 命中，不能把这些变化归因于循环 Box 删除；代码布局等相互作用尚未区分。

## 普通 release 原版 Score 与 Boa

两种受测二进制各有 4 次完整 combined 输出，按 ABBA–BAAB 串行、CPU 2 执行。下表均取这组完整运行中的中位数，Score 越高越好。Boa 0.22.0 使用历史完整套件三轮的中位数，没有重跑。这不是相对会话开头或旧 main 的累计百分比。

| Benchmark | 同期基线 | 采纳后 | Score 变化 | 基线 min–max | 采纳后 min–max | Boa | 我们 / Boa |
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

每种二进制只有四个观察值，min–max 不是置信区间。RayTrace、EarleyBoyer 的小幅变化尚未分辨；Splay 四个受测版本观察均不高于基线最低值，保留下降结果。当前仅 RegExp、NavierStokes 超过 Boa，**全部八项和 Combined 超过 Boa 的目标未达到**。

## 构建与语义验证

真实性能产物：Rust **1.88.0 / LLVM 20.1.5**，显式 `x86_64-unknown-linux-gnu`，release opt-level 3 / fat LTO / CGU 1，features 为空，构建记录为 `optimization.kind=none`。诊断 profile 与合同测试使用另外的构建，未拿它们的时间当原版 Score。

- Split 合同测试 **6/6**，完整库测试 **2308/2308**；两者实际 exit 0，受测源前后不变。覆盖真实 VM 的 Species/Get/Set/Construct/Exec、重复 exec getter、capture 顺序、pending take/abandonment、owner 别名、稳定 Box、Unicode/空匹配/limit、throw/unwind 与 cross-realm。
- 本轮未重新执行完整 Test262 或整套 CI，不声称它们已通过。
- 完整固定工作量矩阵 144/144 valid；RegExp 耗时 −1.87%，A/A 2.38%，Combined −1.15%，A/A 2.09%，该矩阵本身未分辨。一次限定的 5-run RegExp 对照耗时 −1.66%，接近 A/A 1.66%；收益结论来自随后完整原版输出。A/A 范围是 `(max−min)/median`，不是置信区间。

## 可追溯记录

原始日志根目录：`/home/eric/.cache/oxide-v8v7-boa-campaign/structural-simplification-20261002/`（下文相对此目录）。

| 记录 | 路径或身份 |
| --- | --- |
| 原版摘要与全部样本 | `split/original-summary.json` / `split/original/results.json`；results SHA-256 `d922da11066c8eaaa55a6cbbe5d201297599bbe9fac14d972b4e4fec3fbf0f7b` |
| 原版负载正文 SHA-256 | `777f2c2ea9fcf3b8c7db198f758f2e7bd2113c4f8a347ea7323e13c85b4d9946` |
| 基线二进制 SHA-256 | `7a5e91ad348f835202546487e2c6b095787283b883638435225d708c627705aa` |
| 采纳二进制 SHA-256 | `efa87f77dc38643e1042d496887cb8f2d41ce2d96a9d26fb71615c83dd557083` |
| 原版 stage / 构建 stage | `split/10-original-stage/receipt.json` / `split/06-fresh-plain-build/receipt.json` |
| 机制 / 源码 / 汇编 | `split/logical-summary.json` / `regexp-split/source-report.md` / `regexp-split/codegen/report.md` |
| 合同 / 完整库 receipt | `regexp-split/focused-tests-final/receipt.json` / `regexp-split/full-lib-tests/receipt.json` |
| 固定矩阵 / 限定对照 | `split/fixed-authenticated/results.json` / `split/reg-resolution/results.json` |

Boa 历史记录：`/home/eric/.cache/oxide-history-three-engines-2026-10-01/boa-missing-combined/results.json`。历史 Boa 与本轮 oxide 的工具链不同；这份数据支持分数对照，不是同工具链的引擎因果实验。

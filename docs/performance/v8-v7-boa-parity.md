# V8 v7：已采纳的优化

本会话的实现按独立提交保留。以下改动已进入当前工作分支；生产路径保持 JavaScript 效果顺序、完整 generation 认证、owner 与引用计数饱和规则、异常清理、realm、栈限制和 GC 服务边界。

## 已采纳的实现

| 提交 | 改动 | 执行机制 |
| --- | --- | --- |
| `ce30f34a` | 整数位运算转换保留表示 | 已有 Int(i32) 输入直接用于 ToInt32，避免先转换为浮点再恢复整数。 |
| `7f4722be` | binary64 整数位实现 ToInt32 | 从符号、指数和尾数计算低 32 位，替代浮点取模；保留 JavaScript 数值边界。 |
| `2d472ccd` | 普通 Base 构造直接安装 | 在已有构造入口完成适用的普通构造与帧安装，减少 query 交接；保留准入、失败和 active-frame 发布合同。 |
| `e7e4e091` | 普通 instanceof 同步完成 | 在既有内建入口完成普通原型链算法，必要的 exotic 和回调路径沿现有协议继续。 |
| `35ccc54e` | 捕获标量直接消费 | 当前解释窗口读取已持有 cell 的标量，减少 Binding 交接；保留生命周期、初始化与身份检查。 |
| `b7f87c32` | 普通写入一次借用提交 | 选槽、验证与提交共享一次 heap borrow，保留所有权与失败合同。 |
| `3c78a1f2` | 同步 public field 定义本地完成 | 在现有入口完成满足条件的定义，减少 query 进入；回调及一般属性语义沿现有协议执行。 |
| `737410e6` | 已有整数先执行乘法 | checked integer multiplication 满足负零等语义约束时保留整数表示，其他情况使用原浮点处理。 |
| `a40fe615` | RegExpSplit 复用 continuation Box | 初始 Box 贯穿 next/execute/captures 请求，减少循环分配和状态容器重建。 |
| `56b35a04` | 复用 spilled property slots 容量 | 已在 Vec 中的 slots 直接调用 `try_reserve(additional)`，容量充足时保留原缓冲区，增长时采用 Vec 的摊销扩容。 |
| `15f7bacc` | transactional retain 使用 Fx 哈希 | 超过两个 outgoing edges 的计数表使用现有 `FxBuildHasher`；完整 RawId 去重、次数溢出检查与全部 preflight 后 retain 的事务保持不变。 |

另有独立正确性修复 `2ef2f013`：CreateArray/Object/Variable 的 raw push 失败时，释放尚未提交的 retained edge，并返回原 push 错误。它不计为性能收益，合同与验证见 [factory publication 清理](factory-result-publication-cleanup.md)。

## 组合验证与归因

前八项的实验组合为 `eaf23ba8`，首次生产 head 为 `737410e6`；两者的生产源码、manifest、lockfile 与测试源码逐项一致。测量回执仍使用实际受测的源码身份。

该组合完成 focused Test262 **6,844/6,844**，完整 **102,037** 变体的冻结正文比较通过，结果为 **80,010 pass / 80,060 eligible**。普通 release 的库测试、逻辑 profile 和固定工作量比较也已记录。

`3d420407`→`eaf23ba8` 的组合固定负载完整进程耗时下降 **11.86%**。该矩阵使用默认 affinity，不与后续 CPU 2 的结果相减作单层归因。同一组合的完整进程资源诊断中，RSS 中位数增加 **1,286 KiB（0.37%）**，instructions 下降 **11.11%**；它不是原版 Score 或峰值分布结论。原始证据位于仓库外 `/home/eric/.cache/oxide-v8v7-boa-campaign/` 的 `cumulative-fixed-ledger.json` 与 `integrated-wave2-resources/`。

## 当前原版结果

最新原版计时源码为 `a40fe615`，包含前九项实现与 factory 正确性修复。完整原版 combined 输出的中位数为 **188**；RegExp 为 **91.15**，NavierStokes 为 **581.5**。该次测量中仅这两个子项超过历史 Boa，八项与 Combined 全部超过 Boa 的目标尚未达到。

八项分数、同期配对基线、观察范围、历史 Boa 对照及构建身份见 [RegExpSplit 验收](v8-v7-structural-simplification.md)。该表中的变化只归属于 RegExpSplit 改动的完整受测产物，不是本会话相对旧 main 的累计百分比。机制计数与原版时间分别验收，各项收益不相加。

## Slots 与 retain 修复验证（2026-10-03）

两个修复通过 **432/432** heap 测试（Rust 1.88.0，`profiling,test262-host`），包括四项新回归：重复预留时复用缓冲区、增长次数不随每次 append 增加、清空后容量复用、失败预留保留原数据，以及跨 arena/代际的完整身份、重复 edge 次数和饱和边界下的事务原子性。格式与 diff 检查通过，未新增依赖。

这两个修复尚未重新运行原版 benchmark；上面的 Score 不包含其收益。测试日志位于 `/home/eric/.cache/oxide-heap-growth-retain-fixes-20261003/heap-tests.log`。

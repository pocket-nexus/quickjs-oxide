# 内部执行所有权迁移：阶段 B

阶段 B 位于 PR #88，基于已验收的阶段 A（PR #87）。本页记录已提交的实现和对应验证；阶段 B 的完整架构、正确性和性能门槛仍未完成。

## 已采纳的实现

| 提交 | 实际变化 |
| --- | --- |
| `4bebb435` | atom interning、原始描述符验证与存储、普通对象和 Error message 构造共享 `RuntimeState` 实现。公共方法在借用结束后转换返回的 owner。 |
| `fed38b7b` | 捕获 cell 的创建、读取、写入、重置和元数据验证共享状态实现；实际 FrameBinding 读取使用该实现。保留 checked retain、TDZ、import view 和旧值释放顺序。 |
| `54de11b4` | `OP_object` 在解释器已持有的状态中完成分配与发布，结果进入帧槽后服务分配压力，再继续当前循环。 |
| `98ca0400` | Global descriptor 存储直接使用状态；slot、layout、dictionary 事务在实际发布处报告失败阶段，清理使用当前访问权。 |
| `5215326d` | 布局分配器携带实际对象发布状态；发布或破坏性清理失败时，在局部 guard 释放前隔离 runtime。发布前 checked retain 等拒绝仍可恢复。 |
| `3c8e5f21` | module callback 栈 guard 借用 Runtime，并读取维护中的活动帧计数；移除它的 Runtime clone 和状态重借用。 |
| `bbe1c3cf` | Array 字面量共享长度槽和 owned dense append 实现，在当前解释循环完成。失败时按拒绝元素、已发布前缀、未消费后缀的顺序清理。 |
| `08e80874`、`4ecedac9` | Error backtrace 的渲染和新 Error 的元数据捕获共享状态实现；对象、message 和 backtrace 字段发布失败先隔离，再停止后续 owner 清理。任意既有 Error 的通用属性语义保留。 |
| `375d9462` | 原生调用准备期间，参数继续由 guard 持有，直到 callable 的最后一次 checked retain 成功；修复该步骤失败时参数 owner 丢失。 |
| `b037075a` | 布局分配只保留一个必传隔离标志的入口；删除可漏传标志的旧入口及 Array 工厂的可选模式。Base 构造、Arguments、公共对象和公共 Array 分配都遵守此契约。 |
| `f23e9d68` | native receiver/argv 移出帧槽后，由局部 guard 持有直到 NativeActivation 接手；修复帧 materialization 可恢复失败时 owner 丢失。 |

这些改动沿用原有分配、描述符、帧发布和元素追加算法。状态访问权同时提供清理能力；内部结果直接交给持有 owner 的帧或调用存储。

## 已完成的验证

各实现提交运行受影响用例、Rust 1.88 编译与 Clippy、格式、源模块布局和 Rust-only 检查。普通配置与 profiling 配置分别验证；它们会重复执行同一用例，不能相加为独立测试数。

- 捕获 cell：15 个新见证，以及读取、别名、TDZ/import、失败回退和隔离测试。
- `OP_object`：6 个新见证；确认真实帧与 fault PC 发布、结果进入槽位后才执行 GC、提交拒绝释放新 owner。
- Global storage：18 个新见证；覆盖 checked 临时引用、删除/定义顺序、别名和事务发布失败。
- 布局发布与 module guard：6 个新见证；覆盖发布阶段、停止后缀清理、嵌套恢复及 unwind。
- Array：13 个新见证；profiling 见证确认一次执行入口和一次内部 Array 操作，直接完成，不产生 Runtime clone 或 deferred release。
- Error：7 个 backtrace 与 5 个发布失败新见证；最终普通配置和 profiling 配置各通过 62 个选定用例。
- 原生 argv：真实 callable retain overflow 见证确认活动帧、receiver 和参数正确清理，runtime 未隔离且仍可继续执行。
- 原生帧登记：Call/CallMethod 在 token 耗尽时释放独占 owner、保留有效别名与 lower slots，故障 PC 和活动帧恢复正确；25 条验证命令通过。
- 统一布局入口：公共对象/Array 和真实 Base 构造发布失败见证确认隔离发生在边界与执行存储清理前。

原始命令、受测文件摘要、失败尝试和验收回执保存在 `/home/eric/.cache/oxide-runtime-core-20261003`。采用的文件与通过验证的文件逐项核对；过滤器重叠和子进程结果不合并为独立总数。

## 性能归因与剩余验收

**阶段 B 尚无性能结论。** 最近一次完成整套验收的无 PGO 结果仍是[阶段 A](runtime-core-stage-a.md)：原版 Combined 中位分数从 191 到 215，配对收益 12.30%；历史 Boa Combined 为 300。阶段 A 结果不能替阶段 B 的新增提交背书。

内部 native、Proxy、模块/job、eval 和挂起路径仍有迁移工作。最终需要同时确认内部 Runtime 强 owner、状态重借用、deferred release/restore、公共 root 中间转换和迁移适配器全部为零，再执行完整 CI、native/wasm、冻结 Test262、QuickJS 差分和相对阶段 A 的全项性能验收。当前已删除的局部协议不代表这些全局指标已达成。

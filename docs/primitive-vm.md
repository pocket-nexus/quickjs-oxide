# 原语执行核心（Primitive VM）总览

状态：2026-09-16。本 PR 相对 base `perf/ordinary-property-kernel`（＝重写前基线 S0）完成 **S01–S22**：把解释器重写为统一的原语执行核心，并做性能收口。旧的 `dispatch` / `frame_execution` / `host_bridge` / `call_bridge` / `numeric_execution` / `unwind` 路径已退役，`root_call` 是唯一默认执行核心（`stack-vm` feature 开关已删除）。本文是这一系列工作的唯一汇总，不再保留逐阶段设计/账本。

## 最终架构

**编译**：完整前端保留；共享 IR/名字/绑定模型拆到 `compiler/model/{ir,scope,bindings}`，解析临时状态归 `parser/context`，完成产物由消费式 `finish` 移交。发布前有独立只读验证与事务化发布：`code/verify/*` 负责入口/参数/绑定/模块/操作数检查，`code/instruction.rs` 集中指令的栈效果、控制流与可捕获异常分类，`code/runtime.rs` 提交发布、链接 Atom 并在失败时回滚。

**调用帧**：`FrameStore` 持有不可变 executable 与冷状态（`FrameEntry` / `ColdFrame`），`SlotStore` 管理互斥的原始实参、可写形参、局部与操作数窗口。帧**惰性认证**：只有真正发生观察（错误、回调、回溯、GC 边界）时才物化 active frame；冷帧容量跨调用复用。活跃帧登记只保存 domain/identity 与诊断状态，不持有运行 Value。

**驱动**：普通字节码调用、native 调用、属性读写/描述符、Proxy 各内部方法、生成器/async 挂起恢复全部通过统一的 owned continuation 协议（`Step`/`Resume` + 池化 Box）在同一个显式帧栈上推进；JS 回调不再递归进入新的 Rust 解释器层，宿主重入仍受统一预算保护。

**驻留执行**：普通槽操作、Number 精确快路、字符串/BigInt 算术、比较/分支、局部更新（`++/--`、`+=`、比较融合）在 `run` 内驻留完成，不再返回冷分派。fault PC 在 span 入口本地写、resume PC 由 Drop guard 物化；Runtime 活跃帧 PC 只在观察边界发布。

**属性与 Proxy**：静态读/写/全局单元使用位置缓存（shape + layout revision + prototype epoch + realm/domain guard，entry 零 owner）；属性驱动对普通目标用快速回执，对 Proxy/getter 走 owned 帧。Proxy 陷阱选择缓存按 13 个内部方法索引，复用读 IC 的位置协议；ordinary-target 的 `[[Get]]` invariant 同步就地比对。

**其它**：转换内核去除多余拷贝并支持唯一存储原地追加，前插拼接走融合直存；驻留数值入口与 String/BigInt/标量释放按"不构造 JS 错误"豁免 PC 发布。堆侧 context 装箱、弱 shape 迁移、边/缓存元数据精简；regexp 借用匹配输入、复用已验证程序与结果布局；模块/动态 import/顶层 await/async 与 job 队列均接入 owned 驱动。

## 验证

- Test262 冻结向量逐位一致：`pass=79982 / eligible=80032 / total=102037`。
- workspace `--all-targets`、profiling lib、`--doc`、test262-host `--lib --bins`、oracle `test262_` 全部通过。
- pinned 1.88：clippy `-D warnings`、`cargo fmt --check`、source-layout、rust-only、oracle registry、BC5 pinned atoms/opcodes、QuickJS fixtures/c-oracles/dynamic-import 全部通过。

## 性能（相对 S0 / QuickJS）

单轮 ×3 取三轮中位（`taskset -c 6`）；S0 与 QuickJS 读取本地留存数据。

| 类别 | 数量 | 相对 S0 | 相对 QuickJS |
| --- | ---: | --- | --- |
| fixed（wall） | 58 | 0.680×（快 ~32%） | 12.0× 慢（S0 时为 17.6×） |
| probe（wall） | 33 | 0.721×（快 ~28%） | — |
| original（V8 Score） | 9 | 1.386× | 18.3× 慢（S0 时为 23.7×） |

可比 78 例中 60 例比 S0 提速 >5%，仅 3 例慢 >5%。显著提速：`array_slice` −93%、数组写/更新/弹出 −70~−74%、`width-64/256` −79~−93%、`v8-navier-stokes` −60%、字符串族 −15~−49%；original `regexp` +150%、`navier-stokes` +156%。

## 遗留

- `depth-proxy-0/32`（约 +11%）与 `v8-earley-boyer`（+11%）、`richards` Score（−8%）仍慢于 S0 >5%；前者属 proxy get 驱动的结构成本，S21.3/S22.3 经测量门控关闭。
- 相对 QuickJS 仍有约一个数量级差距，集中在普通对象/数组属性访问与 V8 族基准。
- 编译探针复测（R10）与峰值 RSS（R9）未在本轮执行。

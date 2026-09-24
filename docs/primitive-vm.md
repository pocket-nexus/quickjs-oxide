# 原语执行核心（Primitive VM）

本重写把解释器统一为一个原语执行核心：旧的 `dispatch` / `frame_execution` / `host_bridge` / `call_bridge` / `numeric_execution` / `unwind` 路径已退役，`root_call` 是唯一执行核心。

## 架构

**编译**：完整前端保留；共享 IR/名字/绑定模型在 `compiler/model/{ir,scope,bindings}`，解析临时状态归 `parser/context`，完成产物由消费式 `finish` 移交。发布由单一入口事务化完成：`code/bytecode_publish.rs` 负责常量与绑定链接，`code/instruction.rs` 集中指令的栈效果、控制流与可捕获异常分类，`code/runtime.rs` 提交发布、链接 Atom 并在失败时回滚。

**调用帧**：`FrameStore` 持有不可变 executable 与冷状态（`FrameEntry` / `ColdFrame`），`SlotStore` 管理互斥的原始实参、可写形参、局部与操作数窗口。帧惰性认证：只有真正发生观察（错误、回调、回溯、GC 边界）时才物化 active frame；冷帧容量跨调用复用。活跃帧登记只保存 domain/identity 与诊断状态，不持有运行 Value。

**驱动**：普通字节码调用、native 调用、属性读写/描述符、Proxy 各内部方法、生成器/async 挂起恢复全部通过统一的 owned continuation 协议（`Step`/`Resume` + 池化 Box）在同一个显式帧栈上推进；JS 回调不再递归进入新的 Rust 解释器层，宿主重入仍受统一预算保护。

**驻留执行**：普通槽操作、Number 精确快路、字符串/BigInt 算术、比较/分支、局部更新（`++/--`、`+=`、比较融合）在 `run` 内驻留完成，不再返回冷分派。fault PC 在 span 入口本地写、resume PC 由 Drop guard 物化；Runtime 活跃帧 PC 只在观察边界发布。

**属性与 Proxy**：静态读/写/全局单元使用位置缓存（shape + layout revision + prototype epoch + realm/domain guard，entry 零 owner）；属性驱动对普通目标用快速回执，对 Proxy/getter 走 owned 帧。Proxy 陷阱选择缓存按 13 个内部方法索引，复用读 IC 的位置协议；ordinary-target 的 `[[Get]]` invariant 同步就地比对。

**其它**：转换内核去除多余拷贝并支持唯一存储原地追加，前插拼接走融合直存；驻留数值入口与 String/BigInt/标量释放按"不构造 JS 错误"豁免 PC 发布。堆侧 context 装箱、弱 shape 迁移、边/缓存元数据精简；regexp 借用匹配输入、复用已验证程序与结果布局；模块/动态 import/顶层 await/async 与 job 队列均接入 owned 驱动。

## 验证

- Test262 冻结向量：`pass=79982 / eligible=80032 / total=102037`。
- workspace `--all-targets`、profiling lib、`--doc`、test262-host `--lib --bins`、oracle `test262_` 全部通过。
- pinned 1.88：clippy `-D warnings`、`cargo fmt --check`、source-layout、rust-only、oracle registry、QuickJS fixtures/c-oracles/dynamic-import 全部通过。

## 性能（相对重写前基线）

| 类别 | 数量 | 相对基线 | 相对 QuickJS |
| --- | ---: | --- | --- |
| fixed（wall） | 58 | 0.680× | 12.0× |
| probe（wall） | 33 | 0.721× | — |
| original（V8 Score） | 9 | 1.386× | 18.3× |

可比 78 例中 60 例比基线提速 >5%，仅 3 例慢 >5%。显著提速：`array_slice` −93%、数组写/更新/弹出 −70~−74%、`width-64/256` −79~−93%、`v8-navier-stokes` −60%、字符串族 −15~−49%；original `regexp` +150%、`navier-stokes` +156%。

## 遗留

- `depth-proxy-0/32`（约 +11%）、`v8-earley-boyer`（+11%）、`richards` Score（−8%）仍慢于基线 >5%；前者属 proxy get 驱动的结构成本。
- 相对 QuickJS 仍有约一个数量级差距，集中在普通对象/数组属性访问与 V8 族基准。

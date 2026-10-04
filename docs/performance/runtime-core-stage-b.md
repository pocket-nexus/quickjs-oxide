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
| `98b473c7` | 八类 AutoInit 和 fresh function 字段共享完整状态算法；保留原生表顺序、checked 临时引用、终结失败和 String 后置转换，删除五个失去消费者的旧 Runtime 工厂。 |
| `d637f6f8` | 所有 own-property 表示共享状态选择器：data/accessor/VarRef、String、dense 和 TypedArray；内部直接取得 raw-owned descriptor，共享 backing 只在真实 mutex 边界离开状态。 |
| `074f3898` | 字节码闭包创建、捕获发布和 function 字段初始化共享状态算法；`FClosure` 使用当前帧窗口和已解码下一条 PC，发布结果后服务分配压力，再继续解释循环。移除旧外层 driver 创建路径与失去消费者的 Runtime helper。 |
| `51c4257a` | native activation、调用准备、ABI 适配、帧登记与结束清理共享状态实现，内部存储持有原始 owner。四个 NumberPredicate selector、MathRandom 和 FunctionPrototype 在当前解释循环完成；等待路径共用一次 Query 生命周期登记，删除 activation 的 Runtime owner 和旧 operand guard。 |
| `089ab186` | 五类 primitive wrapper 的公共、内部和 String bootstrap 入口共用状态工厂；flat String 保留原 ID，rope 先规范化，length 使用共享原始描述符算法。对象布局沿用共享 atom visitor，统一保留 slot/payload/private-home 边一次，删除 Symbol 单独保留协议。 |
| `7ecbf50a` | 普通属性读取、primitive/String 读取及 linked own-read 共用完整状态算法。原型链使用当前借用，getter 仅取得 callee/receiver owner；TypedArray 数字终结缺失不再查原型，共享 backing 仅在实际 mutex 读取时离开状态。移除旧 Runtime walker，raw 结果携带已有 native 选择事实。 |
| `a66f422b` | 普通与 exotic 的 callback-free 属性定义共用状态算法；完整覆盖 Array index/长度提交与回滚、mapped Arguments、String virtual、Namespace 和 typed postconversion write。Define、selected Set、VM 与 public-field 消费者直接使用同一存储算法；实际 layout/owner 发布处报告失败阶段，删除旧 Runtime 定义与 Array 表示变更实现。 |
| `344ecc1f` | native preparation 直接返回借用 Runtime 的 guard，调用消费者直接使用它；删除生产路径的临时 Runtime 强 owner。只有确实跨越 Runtime binding 生命周期的独立测试使用显式 standalone 适配器。 |
| `2706caae` | 静态属性读取直接借用已发布代码持有的 linked Atom，删除每帧 owning key 缓存及由该缓存导致的返回退避。只有实际挂起的 Proxy 请求提升 key；选择结果的 guard 持续保护到 key 和 receiver 的 fallible handoff 完成。 |

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
- AutoInit：10 个新见证；普通配置与 profiling 配置各通过 91 个选定用例，包含 FunctionPrototype 的 checked retain/饱和边界、realm 释放、终结失败和发布隔离。
- Own-property：22 个新见证；普通配置通过 817 个选定顶层用例，profiling 配置通过 827 个，各自的 25 个成功子进程结果另列。覆盖 accessor 别名、checked overflow、TDZ、AutoInit、String/dense/typed/shared backing，以及清理失败先隔离。
- 闭包：10 个新见证；29 条验证命令通过，包含普通/诊断配置相关测试、严格 workspace/all-targets Clippy 和 host feature 检查。真实调用确认没有 Runtime clone/deferred release；覆盖 capture 元数据、输出拒绝、checked retain、generator prototype 和发布失败隔离。
- Native 状态执行：17 个新见证；完整 library 普通配置 2286 项、profiling 配置 2508 项通过，各自 40 个子进程运行另列。两种配置的严格 workspace/all-targets Clippy、host feature 构建及源检查通过。12 个真实 selector × Call/TailCall 场景各观测一次执行入口和内部 body，无 Runtime clone/deferred release。覆盖 standalone/nested/dead/状态忙碌时的 Query 放弃、登记失败、真实 iterator wait 安装失败、ABI 与清理隔离。
- Primitive wrapper：16 个新见证；完整 library 普通配置 2302 项、profiling 配置 2524 项通过。两种配置严格 workspace/all-targets Clippy 和 host feature 构建通过。覆盖 flat/rope String、短/heap BigInt、Symbol retain 顺序与 overflow、class/domain 拒绝、slot/payload/private-home 原子边、发布及回滚失败先隔离。
- 普通属性读取：14 个新见证；完整 library 普通配置 2316 项、profiling 配置 2538 项通过，两种配置严格 workspace/all-targets Clippy 及 host feature 检查通过。覆盖原型链借用、getter/receiver 别名及 checked overflow、未消费 setter、TypedArray 终结缺失与 shared BigInt、primitive receiver、AutoInit、既选 Proxy 与 native fact。完整测试后仅修正 cfg(profiling) 测试中的等价 absence 断言；复跑 14 个相关见证和最终编译检查，完整测试记录复用并明确记录该差异。
- 属性定义：37 个新见证通过；普通配置完整 2353 项复用已通过的记录，最终仅有一个测试等价 lint 写法变化，该 fixture 单独重验；最终诊断配置完整 2575 项通过。两种配置各有 40 个成功子进程运行另列。严格 workspace/all-targets Clippy 两配置、host feature 与源检查通过；964 项 Rust/Cargo 输入与采纳提交逐项相同。覆盖 TDZ/lazy 权限、public mapped checked owner 与 String/BigInt producer、Array partial publication/回滚停止、typed resize/shared growth、Proxy 输入和域/admission 优先级。
- Native preparation：3 个新生命周期与别名见证；普通配置 57 个、诊断配置 77 个不重叠的选定用例通过，两种配置严格 workspace/all-targets Clippy、host feature 和源检查通过。965 项 Rust/Cargo 输入与采纳提交逐项相同；没有重跑完整 library 套件。
- Linked key 消费：4 个新见证，覆盖七种 primitive 表示、静态 String index、MAX 借用、Proxy key 提升拒绝和 getter receiver 交接拒绝。普通配置 93 个、诊断配置 105 个不重叠的选定用例通过，严格 Clippy 两配置、host 和源检查通过；没有完整套件或性能运行。保留一个 cfg-only 过滤器错误及两个 fixture 入口修正记录，生产源码在验收中没有变化。
- 统一布局入口：公共对象/Array 和真实 Base 构造发布失败见证确认隔离发生在边界与执行存储清理前。

原始命令、受测文件摘要、失败尝试和验收回执保存在 `/home/eric/.cache/oxide-runtime-core-20261003`。采用的文件与通过验证的文件逐项核对；过滤器重叠和子进程结果不合并为独立总数。

## 本次内部契约调整

AutoInit 的旧内部入口会在工厂步骤中多次取得状态并选择性排空协调队列。现在先验证对象和 key，并确认 lazy slot，再由非 String 生产分支在入口排空一次协调队列，完整工厂直接使用当前状态。String 保留原有的无额外 operation 行为。

如果协调队列错误与 realm/prototype 的准备错误同时存在，现在由入口的协调错误优先；有针对性见证确认此时 lazy slot 未变、后续队列项未消费。没有 pending 工作时，原有 checked retain、终结失败和 String 转换顺序保留。

Own-property 内部迁移适配器先验证域，再统一完成一次 operation admission；公共入口保留原来的 operation-first 顺序。状态选择器不排空 Runtime 协调队列。此调整替代旧的虚拟值/TDZ 选择性 admission，并由优先级、无 pending 与清理失败用例验证；最终状态消费者将直接使用选择器，删除迁移适配器。

Native 的原始 activation 不携带 Runtime。生产 preparation 与消费者现在直接使用 borrowed guard；只有独立生命周期测试显式生成 rooted wrapper，没有公开的 prepared-call API。尚未迁移的 native body 通过显式边界临时转交同一 callee edge，不增加 retain 或重放算法。Query 在外部状态忙碌时仍保留原有协调队列兜底；resident 标量路径不创建 Query。其它 native family 和这些迁移边界仍需完成。

普通属性读取的状态内原型遍历不产生中间 prototype owner，getter 不再提升未消费的 setter；相应内部 MAX 拒绝随不必要的 owner 一起消失。实际输出和 getter/receiver owner 仍使用 checked retain。现有 getter/Proxy 消费者仍经过显式 rooted 适配器，后续 VM 与 Proxy 的 raw 消费迁移必须删除这些适配器，当前未声明全局 owner 为零。

静态读取的 key 由执行中的已发布代码持有，完成值与 getter 不需要独立 key owner。Proxy 挂起在已完成选择之后取得 checked key owner；该步骤和旧 receiver preservation 拒绝时，有限 guard 清理已选 callee/Proxy 与 raw receiver，并保留原帧输入。删除 key 缓存后，普通返回不再仅因缓存非空而退出内部返回路径；其它实际等待条件仍保留。

属性定义的内部 current descriptor 在同一状态访问与 receiver owner 保护下借用；删除它的独立 promotion，因此内部 mapped Arguments/Namespace 的 current MAX 拒绝不再发生。公共 mapped 路径保留 current/completion/cell 的 checked 角色和清理顺序，String/BigInt 的 cell 与只读 slot 仍使用独立 producer。Array 长度转换、typed numeric 转换和共享 backing mutex 保持真实边界，不重放已经完成的转换。迁移适配器退出后先确认 poison，再返回成功。

## 性能归因与剩余验收

**阶段 B 尚无性能结论。** 最近一次完成整套验收的无 PGO 结果仍是[阶段 A](runtime-core-stage-a.md)：原版 Combined 中位分数从 191 到 215，配对收益 12.30%；历史 Boa Combined 为 300。阶段 A 结果不能替阶段 B 的新增提交背书。

内部 native、Proxy、模块/job、eval 和挂起路径仍有迁移工作。最终需要同时确认内部 Runtime 强 owner、状态重借用、deferred release/restore、公共 root 中间转换和迁移适配器全部为零，再执行完整 CI、native/wasm、冻结 Test262、QuickJS 差分和相对阶段 A 的全项性能验收。当前已删除的局部协议不代表这些全局指标已达成。

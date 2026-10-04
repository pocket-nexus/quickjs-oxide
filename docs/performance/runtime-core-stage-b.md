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
| `2cb1eb5e` | Date 的旧 owned native 入口统一借用处理并显式退休 invocation；修复成功、抛错和 handler error 时遗漏原 receiver/newTarget owner。构造 helper 改为借用 invocation，仍复用既有算法与清理适配器。 |
| `38a50c79` | 28 个 read-only Date selector 的全部输入共用状态实现，真实 VM 调用在当前循环完成。时钟和时区直接借用 HostServices；checked brand 临时引用在原来的观察点释放，公共与 Query 消费者复用同一 body，删除旧 readonly Runtime 实现。 |
| `25c152d6` | ArrayBuffer、SharedArrayBuffer、DataView 的 owned native 入口复用既有 borrowed-handler 清理协议；所有成功、抛错和 handler error 路径都显式退休原 receiver/newTarget，十个内部 helper 改为借用 invocation。 |
| `e49eba17` | 内部调用在 checked callee 认证之前登记 receiver 和 argv owner，认证拒绝也经过原清理路径。持有可用状态时直接释放并保留当前边释放后服务 FIFO 的顺序；状态忙碌时仍用协调队列，破坏性失败停止后缀并返回隔离错误。 |
| `3b0ee61a` | GetField/GetField2、52 个 Numeric/Math selector、8 个 ScalarText selector 和 ToPrimitive/ToNumber/ToString 共用状态算法与原始 continuation；普通 getter/转换调用共用既有帧安装和 Query 消费，完成的选择及 PC 事实不重放。移除旧 Runtime conversion/body 实现，破坏性失败先隔离再停止 owner 后缀清理。 |
| `55c154a8` | 完整 Bound CALL 链共用状态内 payload promotion 与参数合并算法；四种实际调用 opcode、getter/转换 callback 和旧边界消费者共用一次规范化及既有帧安装。删除逐 Bound 的旧调用循环，实际 overflow Error 携带发布事实，清理失败停止剩余 owner。 |
| `d1e45524` | 18 个转换型 Date prototype selector 使用原始 continuation 与状态算法，普通/native 子调用共用既有 Query；ToObject 共用 checked prototype 准备和 primitive 工厂。getter 选择先进入 armed request，再退休输入；实际 boxing/Error 携带发布事实。 |
| `82e107ff` | GetArrayEl/2/3、dense miss 与 ToPropKey 共用完整状态内键转换和读取；保留原 key/receiver 的实际 keeper，复用 Query、callback 和帧发布。直接抛错通过同一 guarded 前缀提交释放原操作数，再发布 Error，修复 verified capacity 耗尽时错误被覆盖的问题。 |
| `620ecfe4` | 已迁移的 Number/BigInt/Index/String、Math/Numeric/Text、Date brand/ISO 与 constructor-only ABI 拒绝携带实际新建 Error/iterator 的发布事实；共同 Query 在 owner 发布后服务 GC。同步 Number 边界通过原 guarded finisher 消费事实，保留旧 Complete ABI。 |

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
- Date invocation：5 个新见证，两种配置各通过 68 个不重叠的选定用例；严格 workspace/all-targets Clippy、host 与源检查通过。967 项 Rust/Cargo 输入与采纳提交逐项相同。覆盖 receiver/newTarget 正常释放、constructor prototype 抛错，以及清理失败先隔离、停止后续 callee/argv/frame 清理。
- Date 状态执行：9 个新见证，普通配置 165 项、最终源码诊断配置 189 项相关用例通过；970 项 Rust/Cargo 输入与采纳提交逐项相同。两种配置严格 workspace/all-targets Clippy、host 与源检查通过。普通配置的 159 项复用通过记录，最终仅有一个测试 tuple 的等价 type alias lint 修正，六个使用该 fixture 的新见证重新编译验收。56 个内部 Call/TailCall 场景覆盖 28 个 selector，不计为独立测试；另覆盖真实 Query、忽略参数、backtrace、host panic 与发布后 GC。保留私有测试 API 和 type-complexity lint 的失败记录；生产实现未因这些失败改变。
- Binary buffer invocation：7 个新见证和 45 个受影响用例在两种配置各通过 52 项；两种配置严格 workspace/all-targets Clippy、host 与源检查通过。971 项 Rust/Cargo 输入与采纳提交逐项相同。覆盖方法成功/brand 抛错、species 独立返回 owner、DataView.buffer、三类构造成功/Proxy prototype 抛错，以及清理失败先隔离并停止后续 owner。首次两个失败发生在进入 body 前的测试 ABI，修正为尚未适配的 Call 输入后通过；生产实现未改变。没有完整套件或性能运行。
- 内部调用 admission：3 个新见证在普通和 profiling 配置各通过 3 项，覆盖 MAX 拒绝与别名、状态忙碌协调、真实破坏性清理后的后缀停止；严格 workspace/all-targets Clippy 两配置、host 和源检查通过。972 项 Rust/Cargo 输入与采纳提交逐项相同。保留第一次等价双重引用触发的 lint 失败；没有完整套件或性能运行。验收回执 SHA256 `188cd09d03ad024d51b15e1a15a63f9d6a57baa925687bb52828df74409ae988`。
- 命名读取与 Numeric/Text 纵切：完整普通 library 2423 项通过。完整 profiling 运行有 2639 项通过、6 项旧机制断言失败；这些用例的 JS 结果断言已通过。最终仅修改 cfg(profiling) 的实际消费/边界计数及机制断言，181 项相关最终源码测试通过，包含这 6 项和真正 unhinted callee 的 absence 见证。普通完整结果在三处已认证的等价 lint 改写下复用，最终普通配置另重验 175 项并通过严格 Clippy；最终 profiling 严格 workspace/all-targets Clippy、profiling+test262-host 及源检查通过。983 项 Rust/Cargo 输入与采纳提交逐项相同。回执 SHA256 `b3fd4cccc07e0b6477a192afa7a3a5ce3c0d27824f89cdb5ad3cb13b2823ed35`。原始 28 个语义失败促成共同 PC/throw 修复；后续 cfg 断言、计数归属及 launcher 解析失败记录全部保留。没有性能运行，子进程和交叠测试数不合并。
- Bound CALL：12 个新见证覆盖四种真实 opcode、各类 receiver/argv、普通/native/Proxy 回调、checked retain、overflow 与清理隔离。最终普通配置新编译的 433 项完整 VM 域通过；复用先前完整普通配置的 2434 项通过记录，该次唯一失败是新增 TailCall fixture 的指令生成假设。最终只改变该 fixture，以及经逐字变换证明等价的非迭代 loop→block；没有重跑完整普通套件。最终 profiling 完整 2657 项通过，严格 workspace/all-targets Clippy 两配置、host feature 和源检查通过。986 项 Rust/Cargo 输入与采纳提交逐项相同；回执 SHA256 `4ed18c08cfb33f0e70d41c53f8b2e1994ee4228d722184a817ca3f13f3ccc8c3`。首次生产借用检查、测试 API、fixture 与 lint 失败记录全部保留。没有性能运行，配置、交叠用例和子进程结果不相加。
- 统一布局入口：公共对象/Array 和真实 Base 构造发布失败见证确认隔离发生在边界与执行存储清理前。
- Date 转换与 ToObject：19 个新见证覆盖全部 16 个 setter、两种 hint、toJSON 的 primitive/Proxy、snapshot 与重读、host 调用顺序、PC/realm、checked retain、放弃及隔离。普通完整运行 2453 项通过，唯一失败是新增 ToObject 见证漏算已发布 shape 的 prototype 边；修正仅限该见证，最终新编译的 4 个 ToObject 用例通过，其余普通结果在逐项源码证明下复用。最终 profiling 完整 2676 项通过，严格 Clippy 两配置、host、源检查与 anti-special-casing 通过。990 项 Rust/Cargo 输入与采纳提交相同；回执 SHA256 `78072e2e82ed3ef8b4e10d413fc8854ea7712bd34ebbfb44543f2362f0a0aeca`。首次导入/可见性失败、原始见证失败均保留；没有性能运行。
- 计算读取与键转换：27 个新见证覆盖键表示、转换与 getter/Proxy 顺序、三种 opcode 的 keeper、别名、真实 slot budget、checked retain、fault PC、GC 与隔离。普通完整 2481 项通过；之后只有 cfg(test) 的等价 expect_err 改写和 cfg(profiling,test) 的旧机制断言更新，最终重新编译并验证清单，复用完整结果及 7 项 key-value 重验记录，没有再跑完整普通套件。完整 profiling 有 2702 项通过、1 项旧边界计数断言失败；最终编译的 28 项相关用例全部通过，复用其余完整通过记录。两配置严格 Clippy、host、格式与源检查通过，999 项 Rust/Cargo 输入与采纳提交相同；回执 SHA256 `8a64309033ab083ec0a19489583f0872c18492b2d4bb3bcb99b39752e7acc0ad`。原始编译、fixture、真实 throw 发布和机制断言失败均保留；没有性能运行。
- 分配事实：18 个新见证覆盖实际工厂与传播 throw 的区分、即时/恢复 iterator、真实 getter 的 fault PC、GC 时 raw owner 存活、压力只消费一次及 first-fatal 隔离。普通完整 2499 项通过；完整 profiling 有 2719 项通过，2 项旧机制用例发现同一个 Math 诊断事件漏记。最终只补回 cfg(profiling) 的该事件，原测试断言保持，20 项相关用例通过；在逐字源码/cfg 证明下复用其它完整结果，没有重跑完整普通或 profiling 套件。最终新编译、两配置严格 Clippy、host 与源检查通过，1000 项 Rust/Cargo 输入与采纳提交相同；回执 SHA256 `4ced6dacabbcf15d6f5d54fc8b8936e4a96285e0e4ba8450c95faef47e65298a`。保留最初旧 Number 同步 ABI 的真实回归、错误 BigInt 上限 fixture、私有测试 API 及计数断言修正后撤回的记录；没有性能运行。

原始命令、受测文件摘要、失败尝试和验收回执保存在 `/home/eric/.cache/oxide-runtime-core-20261003`。采用的文件与通过验证的文件逐项核对；过滤器重叠和子进程结果不合并为独立总数。

## 本次内部契约调整

AutoInit 的旧内部入口会在工厂步骤中多次取得状态并选择性排空协调队列。现在先验证对象和 key，并确认 lazy slot，再由非 String 生产分支在入口排空一次协调队列，完整工厂直接使用当前状态。String 保留原有的无额外 operation 行为。

如果协调队列错误与 realm/prototype 的准备错误同时存在，现在由入口的协调错误优先；有针对性见证确认此时 lazy slot 未变、后续队列项未消费。没有 pending 工作时，原有 checked retain、终结失败和 String 转换顺序保留。

Own-property 内部迁移适配器先验证域，再统一完成一次 operation admission；公共入口保留原来的 operation-first 顺序。状态选择器不排空 Runtime 协调队列。此调整替代旧的虚拟值/TDZ 选择性 admission，并由优先级、无 pending 与清理失败用例验证；最终状态消费者将直接使用选择器，删除迁移适配器。

Native 的原始 activation 不携带 Runtime。生产 preparation 与消费者现在直接使用 borrowed guard；只有独立生命周期测试显式生成 rooted wrapper，没有公开的 prepared-call API。尚未迁移的 native body 通过显式边界临时转交同一 callee edge，不增加 retain 或重放算法。Query 在外部状态忙碌时仍保留原有协调队列兜底；resident 标量路径不创建 Query。其它 native family 和这些迁移边界仍需完成。

Date 的无 JS 转换方法按固定 selector 选择状态实现，覆盖普通值、真实 Date、错误 brand 和忽略参数；不以输入种类决定是否迁移。保留 brand temporary 的 checked retain 与释放顺序。非重入 HostServices 直接借用，panic 标记同一 poison header。`d1e45524` 完成 setter、toJSON/toPrimitive 的转换协议；Constructor、Parse/Utc 仍需迁移。

普通属性读取的状态内原型遍历不产生中间 prototype owner，getter 不再提升未消费的 setter；相应内部 MAX 拒绝随不必要的 owner 一起消失。实际输出和 getter/receiver owner 仍使用 checked retain。命名 VM 读取和已迁移的转换消费者直接使用原始 owner 安装 getter；尚未迁移的公共读取、Proxy 与 legacy 外层消费者仍有显式 rooted 适配器。后续迁移必须删除这些适配器，当前未声明全局 owner 为零。

静态读取的 key 由执行中的已发布代码持有，完成值与 getter 不需要独立 key owner。Proxy 挂起在已完成选择之后取得 checked key owner；该步骤和旧 receiver preservation 拒绝时，有限 guard 清理已选 callee/Proxy 与 raw receiver，并保留原帧输入。删除 key 缓存后，普通返回不再仅因缓存非空而退出内部返回路径；其它实际等待条件仍保留。

属性定义的内部 current descriptor 在同一状态访问与 receiver owner 保护下借用；删除它的独立 promotion，因此内部 mapped Arguments/Namespace 的 current MAX 拒绝不再发生。公共 mapped 路径保留 current/completion/cell 的 checked 角色和清理顺序，String/BigInt 的 cell 与只读 slot 仍使用独立 producer。Array 长度转换、typed numeric 转换和共享 backing mutex 保持真实边界，不重放已经完成的转换。迁移适配器退出后先确认 poison，再返回成功。

## 已采纳的连续读取与转换

`3b0ee61a` 的 body 选择由固定 selector 决定，覆盖该 body 的全部输入；Math 对原始值借用转换，只在对象转换实际需要恢复时保存剩余参数。52 个新增 selector 包含五类 primitive 的 valueOf/toString、Symbol.description、四类 Number 格式、两个 BigInt.AsN 和 35 类 Math；8 个 ScalarText 包含 charAt/at、charCodeAt、codePointAt、concat、两种 well-formed 方法和 iterator。它们复用原数值/字符串 kernel。

普通 getter 和转换方法使用同一个已认证的 callback 安装器，getter/Proxy/native 选择在进入消费者后不重新查找。实际 Proxy、Shared mutex、General 调用、未迁移 body 和 legacy 外层回复仍是已标记的边界。Query 在真实等待或边界发布时才登记 Weak，完全 resident 的转换不生成 Runtime 强 owner 或 Query 生命周期登记。

Cold Call 的回复进入 resident 消费前只恢复一次下一条 PC，并转为携带 committed PC 的 continuation。Resident Call 使用已经携带的 PC；getter 抛错不提前推进，lower slots 和 fault PC 保留。ABI 适配错误及清理错误在剩余 receiver、argv、callee 和帧清理之前返回隔离错误。

机制断言按实际路径验收：20 次普通 getter 留在片段中，20 次 Proxy trap 复用原 Query 存储；三个混合用例分别确认 36/41/41 次 State Math body，以及 24/21/17 次无需参数存储的完成。第一个用例仍有一次 `Map.size` 的未迁移 native getter 边界，第三个有八次实际 Proxy 边界。边界原因计数记录 State dispatcher 返回的 effect，不等于解释循环退出次数；aggregate activation transport 也包含 helper scope，不能全部归给 Math。

## 已采纳的 Bound CALL 规范化

`55c154a8` 使用一套完整链算法：在当前状态中保留真正的 Bound target、receiver 和 argv owner，按 Bound 前缀后接调用参数的顺序合并，再把最终选择交给既有普通/native 安装器。Call、CallMethod、TailCall 和 TailCallMethod、getter 与转换回调均消费这套算法；旧 Runtime callback 和冷调用不再各自遍历 Bound 链。

实际 payload、callback callee 和帧发布所需的 checked retain 保留。只为旧公共 header 临时包装产生的 bytecode/global owner 与相应 MAX 拒绝删除；最终 callee 的 heap 边保护代码、closure 和 realm，帧安装在退休选择之前取得所需 owner。overflow 清理先处理 Bound argv，再处理调用 argv；真正新建的 Error 在 owner 发布后才服务分配压力，传播的旧 throw 不标记为新分配。

本提交保留真实 caller 的旧 materialization 时点，不创建 Bound 帧。Bound Construct、Function.call 的转发循环、Proxy、特殊 bytecode 与未迁移 native 的实际边界仍有后续工作；未声明所有调用成本消失或全局架构指标归零。

## 已采纳的 Date 转换与 ToObject

`d1e45524` 覆盖全部 18 个转换型 Date prototype selector。字段 setter 在转换前保存原 calendar/timezone snapshot，仍按声明窗口完整转换；setYear 按原语义在转换后重读 receiver。continuation 只保存 raw owner、phase、参数和进展，不持有 Runtime、公共 root 或 HostServices。普通、native 和 Bound 子调用使用既有 Query 与 callback 安装器。

ToObject 的公共与内部入口共用 prototype 准备和 primitive 工厂。公共入口保留 realm、checked prototype 与 operation admission 的优先级；内部使用当前状态。Existing、Boxed 和 Throw 明确区分实际生产结果。String wrapper 的空 shape 与 length successor 各有真正的 prototype 边，见证按实际已发布 shape 核对，不把它们误判成临时 retain 泄漏。

Date fresh Error 与真实 toJSON boxing 在 owner 发布后向共同 Query 传递分配事实。原始属性请求保存已选 ReadStep，再按顺序退休 receiver/request owner；致命失败停止后缀。只在真实外部忙碌状态的放弃边界使用协调队列。Constructor、Parse/Utc 以及其它尚未迁移的 native 仍有后续工作。

## 已采纳的计算读取与键转换

`82e107ff` 的固定 opcode 消费者覆盖全部输入表示。primitive key 共用 canonical atom 生产者；对象 key 按 String hint 完成转换，选择过的 getter、Proxy 或 native 直接交给共同 Query。原 receiver 与需保留的 key 在父帧仍为当前帧时，通过同一次 FramePush 事务发布，再安装子调用。没有新增解释循环、转换器或 Runtime 强 owner。

nullish receiver 的检查仍先于计算 key 的 JS 转换，GetArrayEl3 的类型错误顺序保持。故障见证发现直接 throw 时 base/key 已占满 verifier 允许的 operand 容量，追加 Error 会产生 Internal 错误。现在在 Error owner 受 guard 保护期间消费真实源前缀，再发布 throw；保留原故障 PC，未扩大栈容量。实际 fresh Error 的分配事实在结果或 pending owner 发布后消费，传播的旧 throw 不标记为新分配。

旧计算读取与键转换外层任务已删除。Shared backing、实际 Proxy/General 调用以及未迁移的 native family 仍经过现有明确边界；其它数值、字符串和 native 分配事实继续迁移，本提交不宣称所有生产者或全局架构指标已完成。

## 已采纳的实际分配事实传递

`620ecfe4` 在真正的 Error 或 iterator 工厂成功后标记发布；Number/BigInt/String leaf 以及传播的既有 throw 保持原结果形式。事实沿已有 typed reply、NativeStep 和 Query 消费者传递，实际输出、父 continuation 或帧槽先取得清理责任，再服务分配压力。同步边界复用既有 guarded finisher，普通 Math 成功仍保留即时完成。

完整普通测试发现旧 global 数值消费者把新的 NumberStep 标记当作挂起，错误地返回 Internal。Runtime 的同步 Number 入口现在通过原 finisher 消费事实再返回 Complete，State 入口仍携带事实；见证同时覆盖 global、mutation、slice 和 iterator 的真实调用。BigInt 上限见证改为产生真实的扩展符号 limb 后截断，保留 QuickJS 对 short -1 的原有提前返回行为。

Math 的无参数存储事件和 `core.internal_native_body` 是关联计数。补回工厂错误分支漏记的事件后，原断言直接通过；不能把这两项当作独立的 Query 数量。未迁移 native family 的实际边界仍保留已有 checkpoint，本提交不删除它们或宣称全部 GC 服务位置已迁移。

## 中途机制检查

在 runtime 提交 `25c152d6` 上，仅对冻结的 Richards/DeltaBlue 固定工作量各执行一次诊断运行。Rust 1.88、release、profiling feature、无 PGO；两项完成标志与冻结输入逐项核对。对照复用阶段 A 的相同诊断工作量。

| 计数 | Richards：A → 当前 B | DeltaBlue：A → 当前 B |
| --- | ---: | ---: |
| `core.runtime_clone` | 1,315,081 → 981,472（−25.37%） | 1,400,731 → 759,115（−45.81%） |
| 四类状态访问计数合计 | 4,764,530 → 4,354,394（−8.61%） | 4,649,509 → 3,508,032（−24.55%） |
| `core.frame_executor_exit` | 524,690 → 524,628 | 383,947 → 383,842 |

状态访问合计包含 `borrow`、`borrow_mut`、`try_borrow`、`try_borrow_mut`，表示被记录的访问次数。计数证明已有实现减少了部分 clone 与访问；解释循环退出尚未明显减少，属性读取和转换的实际 resident 消费仍需完成。计数范围不能证明全局架构指标归零，也不能换算成耗时收益。

回执：`/home/eric/.cache/oxide-runtime-core-20261003/b-interim-25c152d6-mechanism/summary.json`，SHA256 `4259a7d3bf701c74dc97e73351aa3b2fc859fc5c49d54d40013ce98a994bae94`。保存原始 JSONL、受测二进制和构建配置。第一次收集器读取了错误 schema 名；Richards 执行已成功，修正解析后复用其原始输出，仅继续 DeltaBlue，没有重跑 Richards。未运行原版 Score、A/A、Boa 或性能验收。

### 连续 native 消费的其他负载覆盖

在 runtime 提交 `3b0ee61a`、源码提交 `48332958` 上，以 Rust 1.88 release、profiling feature、无 PGO，对冻结的 Crypto 和 RayTrace 工作量各执行一次。二进制、构建参数、输入摘要和完成标志均核对；两项此前没有同范围的阶段 A 计数，因此不计算变化比例。

| 实际记录的事件 | Crypto | RayTrace |
| --- | ---: | ---: |
| 累计已迁移的 State native body | 4,961 | 21,262 |
| ScalarText State body | 3,441 | 未记录 |
| 仍进入旧路径的属性写入 | 417,047 | 394,739 |
| 仍进入旧路径的 arguments 对象创建（`OP_arguments`） | 未记录 | 133,190 |
| 仍进入旧路径的计算属性读取 | 21,854 | 23,064 |

这些事件确认真实负载使用了 State native 消费，也说明通用写入与 arguments 对象创建仍有较大迁移覆盖；arguments 对象创建和 CreateListFromArrayLike 的展开协议是不同消费者。State body 包含此前已迁移的 selector，不能全归给 `3b0ee61a`。未记录表示计数器没有该字段；边界次数不表示耗时占比。未重跑原版 Score、A/A 或 Boa。

回执：`/home/eric/.cache/oxide-runtime-core-20261003/b-interim-3b0ee61a-coverage/summary.json`，SHA256 `f32ae5815e20e71a7e462fe7510b98eb9ae54c21964fb8a24091cfe7f3241518`。保留独立的受测二进制、构建回执和原始输出。

## 性能归因与剩余验收

**阶段 B 尚无性能结论。** 最近一次完成整套验收的无 PGO 结果仍是[阶段 A](runtime-core-stage-a.md)：原版 Combined 中位分数从 191 到 215，配对收益 12.30%；历史 Boa Combined 为 300。阶段 A 结果不能替阶段 B 的新增提交背书。

内部 native、Proxy、模块/job、eval 和挂起路径仍有迁移工作。最终需要同时确认内部 Runtime 强 owner、状态重借用、deferred release/restore、公共 root 中间转换和迁移适配器全部为零，再执行完整 CI、native/wasm、冻结 Test262、QuickJS 差分和相对阶段 A 的全项性能验收。当前已删除的局部协议不代表这些全局指标已达成。

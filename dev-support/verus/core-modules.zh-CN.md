# quickjs-oxide-core 模块可验证性清单

基线为 `be767daec3b0632482a96347b058c354ae8b5672`，验证器为 Verus `0.2026.11.22.1ce6a59`，配套 Rust `1.100.0`。沿 crate 根的生产 `mod` 声明逐项核对，包括父模块自身、私有模块、宏生成的目录所在模块，以及 Linux 和 Wasm 条件编译路径。共 **98 项**：91 个生产源文件（含 crate 根）和 7 个内联模块。测试专用的 `gc_tests.rs`、内联测试模块、examples 及其他 workspace crate 均在此计数之外。编译器、正则后端和宿主作为依赖出现在具体目标中。[crate 入口](../../core/src/lib.rs) · [项目行为基准](../../README.md) · [构建配置](../../core/Cargo.toml)

本表记录立项时的判断与裸源码探测。十一项的后续实际结果、工具边界和未闭合目标见[独立模块验证报告](independent.zh-CN.md)。

## 如何读这张表

“能否验证”有两层含义。**工程判断**回答能否把右侧行为写成一个可验收的证明项目；**裸源码探测**记录固定版本 Verus 实际走到了哪里。入口能翻译、几个常量能通过、已有子程序定理和整个模块行为已经证明，分别对应不同的证据。

| 工程判断 | 项数 | 交付时需要的证据 |
| --- | ---: | --- |
| 独立 | 11 | 有清晰的输入/输出或局部状态模型，可以单独立项。仍须补表中列出的类型、库合同与证明。 |
| 组合 | 52 | 自身行为可以给规格；完整验收需要下层合同，以及模块之间真实调用的合成证明。 |
| 受阻 | 14 | 整 crate 的直接诊断位于此模块自己的源码中。当前工具无法按原类型翻译；需更换表示、扩展工具或给明示边界。后者只产生条件性结论。 |
| 外部 | 13 | 完整目标依赖数值、平台、并发或另一个 crate 的语义。core 一侧可分解；外部合同的正确性须独立交代。 |
| 随上层 | 7 | 主要提供位常量或记录；表中指出这些表示在哪条真实操作路径上获得行为含义。 |
| 部分已证 | 1 | URI 解码内核已有覆盖全部输入的行为证明；同文件其他入口仍有独立目标。其余模块已有的局部定理也在相应行说明。 |

**探测方法。** 对 94 个本机项，在原版 `core/src` 的临时副本中只引入 Verus prelude，并在相应模块的声明前加 `#[verifier::verify]`。保留真实函数体，不加载已有合同补丁。模块标记包含其子树，兄弟模块按 Verus 默认规则处理；crate 根使用 `--no-external-by-default`。各次使用 `--no-cheating`、锁定的 Cargo 依赖及全部 core 特性。表中记录第一条诊断，后续可能还有其他约束。“依赖”表示那一轮未纳入该类型或 trait；已有合同可改变结果。“宏展开的 for”指 Verus 对这次展开形式的诊断；需调整宏内验证标注。**5 个纯标志模块通过基础检查，89 项在该设置下止步。** 另 4 项以 `wasm32-unknown-unknown` 实测；Cargo 构建成功，Verus 因所装发行包缺目标版 vstd 未进入函数证明。父项如 `float` 的本机结果只对应本机分支。[Verus 对程序规格与工具支持范围的说明](https://verus-lang.github.io/verus/guide/overview.html)

**立项时的行为证据。** 加载仓库已有合同后，全特性作用域验证复跑为 `161 verified, 0 errors`；其中 URI 解码以任意真实 UTF-16 切片为输入，精确约束成功文本和错误种类，并有数学编码器到生产解码器的往返定理。全 crate 在相同特性下仍有 19 条翻译诊断，10 条函数指针、9 条动态回调，分布在 14 个源文件。两个结果的运行范围不同；全库通过率无法由 161 计算。[运行命令、已证合同与变异检查](README.md)

**行为规格口径。** 目标以仓库固定的 QuickJS 2026-06-04 为引擎行为基准；实现采用 ECMAScript 规则的部分可直接用数学或状态机规格。对可重入 JS 操作，同时比较返回/异常、状态变化和回调次序。区分引擎可返回的 OOM 与底层分配器终止。对于 GC、容器和 VM，让定理覆盖从合法初态出发的操作序列，并写明根纪律、内存容量、宿主进展与数值后端等假设。

## 核心入口、运行时与浮点（12 项）

| 模块 | 工程判断 | 行为验收目标 | 依赖及已有证明 | 裸源码探测 |
| --- | --- | --- | --- | --- |
| [crate](../../core/src/lib.rs) | 组合 | 从 Runtime 初始化、发布已验证字节码到执行完成或抛错，结果、宿主事件、仍可达的对象符合引擎规格。 | 本文件主要重导出；此项验收组合所有子系统，编译器属于根 crate。 | 类型：函数指针 |
| [value](../../core/src/value.rs) | 独立 | 任一 64 位表示的分类；每种构造与读取的往返、NaN 规范化、±0、EMPTY 与堆引用提取一致。 | 标签、Int32、布尔和四类堆 ID 已有合同；浮点构造、所有读取及使用方仍待证。 | 库：fmt 格式化 |
| [clock](../../core/src/clock.rs) | 外部 | 给定两次时钟读数，elapsed 等于纳秒差的饱和减法；倒退或缺省时钟得到零。 | 可以验证计时包装；读数的来源及单调性需由 host::Monotonic 提供。 | 依赖：<code>host::Monotonic</code> |
| [host](../../core/src/host.rs) | 受阻 | NoHost 的各项默认返回值确定；时钟、I/O、中断与拒绝通知只按所约定的参数和次数交给宿主。 | Monotonic 含函数指针；每个真实宿主需另证其能力合同。 | 类型：函数指针 |
| [embed](../../core/src/embed.rs) | 受阻 | 从 NativeCx 的属性、转换、调用及对象构造入口到返回值或异常，与同一 VM 操作一致；跨重入和分配仍保根。 | 动态宿主函数体是直接障碍；还需对象、异常、GC 和服务合同。 | 类型：动态回调 |
| [runtime](../../core/src/runtime.rs) | 受阻 | 创建并初始化 realm，发布单元、执行或构造、持有和释放句柄；各阶段的根、代码和异常状态一致。 | 编译钩子的函数指针；组合 Heap、Realm、CodeStore、VM 和宿主副作用。 | 类型：函数指针 |
| [shape](../../core/src/shape.rs) | 组合 | 任意合法的加属性、删改描述符、更换原型序列，树形和字典形态的 lookup、槽值、键顺序及缓存失效均符合抽象对象。 | 需要键、Arena、槽分配、字典和有效性单元；覆盖形态切换和原型环拒绝。 | 依赖：<code>heap::atom::PropertyKey</code> |
| [float](../../core/src/float.rs) | 外部 | 为各导出规定 binary64 特殊值及舍入；分别证明原生和 Wasm 后端满足约定，包括余数、小数部分和换底对数。 | 需固定 libm 版本与逐操作 IEEE 754 语义；超越函数需明确采用参考实现或误差界。 | 运算：f64 求余 |
| [float::acos](../../core/src/float/acos.rs) | 外部 | 在完整 binary64 域分区，证明端点、越域、NaN 以及各近似分支相对固定 acos 实现的输出或明确误差界。 | 生产入口限 Wasm；依赖 sqrt、系数和每一步舍入。 | 环境：缺 Wasm 版 vstd |
| [float::asin](../../core/src/float/asin.rs) | 外部 | 证明分段区间、符号对称、±0、±1 和越域处理，量化各区间舍入误差或建立参考实现等价。 | 生产入口限 Wasm；依赖 sqrt、位操作和舍入模型。 | 环境：缺 Wasm 版 vstd |
| [float::asinh](../../core/src/float/asinh.rs) | 外部 | 证明微小、中等和极大输入的区间选择、符号及特殊值，各输出满足固定数值规格。 | 生产入口限 Wasm；依赖 log、log1p、sqrt 的合同。 | 环境：缺 Wasm 版 vstd |
| [float::wasm](../../core/src/float.rs#L26) | 外部 | 证明 SIMD 的 splat、逐通道运算和取第零通道与标量 sqrt、ceil、floor、trunc 一致，pow 的捷径和回退满足其合同。 | 需 wasm32 的 Verus 库、SIMD 指令语义以及 libm 回退合同。 | 环境：缺 Wasm 版 vstd |

## 字节码（9 项）

| 模块 | 工程判断 | 行为验收目标 | 依赖及已有证明 | 裸源码探测 |
| --- | --- | --- | --- | --- |
| [bytecode](../../core/src/bytecode/mod.rs) | 组合 | 发布请求要么被拒绝，要么生成可检索的稳定单元 ID；索引、捕获、调试位置、处理器和单元树与发布内容一致。 | 组合 builder、validate、静态 CodeStore 和动态 Units；保留发布前后的可见状态。 | 类型：动态回调 |
| [bytecode::flags](../../core/src/bytecode/mod.rs#L137) | 随上层 | 函数标志被构造、验证和调用入口按同一含义解释，包含生成器、箭头、派生构造器与严格模式的约束。 | 位常量本身通过基础检查；行为归属于验证器与帧入口。 | 基础通过（14） |
| [bytecode::opcode](../../core/src/bytecode/opcode.rs) | 独立 | 每个合法操作码有唯一编码、名称、操作数格式和终止属性；所有消费者按同一个指令目录解释。 | 目录与 format 已用于比较表证明；名称、终止属性和消费者的一致性另证。 | 库：fmt 格式化 |
| [bytecode::opcode::global](../../core/src/bytecode/opcode.rs#L521) | 随上层 | 全局声明指令所带各位在声明冲突、可定义性和 eval 路径被正确解释。 | 要与指令发出方和解释器一起验证；单独验证位值无法证明全局声明的结果。 | 基础通过（4） |
| [bytecode::opcode::method](../../core/src/bytecode/opcode.rs#L535) | 随上层 | 方法、getter、setter 及可枚举位最终生成预期的属性描述符。 | 与方法定义指令、类构造和对象描述符组合。 | 基础通过（3） |
| [bytecode::builder](../../core/src/bytecode/builder.rs) | 独立 | 从任意合法发射、回填目标和 retarget 操作序列到 finish，指令、PC、异常区间、布局与缓存编号均与抽象组装结果一致。 | 需 Insn/Format、布局和每类缓存编号饱和规则；可独立于 JavaScript 源码编译。 | 依赖：<code>bytecode::Insn</code> |
| [bytecode::validate](../../core/src/bytecode/validate.rs) | 受阻 | 对任意单元及已发布子单元映射，接受当且仅当声明的结构规则全部成立；失败返回正确错误种类与索引。 | 直接障碍是 published 的 dyn Fn；需列全寄存器窗口、常量、跳转、spill、捕获、行号、缓存和处理器规则。 | 类型：动态回调 |
| [bytecode::call_plan](../../core/src/bytecode/call_plan.rs) | 组合 | 生成计划时，输入寄存器、别名和控制流条件足以保证快捷执行与原指令序列等价；无适用模式则返回 None。 | 模式识别可单证；等价性需与 vm::interp::call_plan 和原始解释语义组合。 | 依赖：<code>bytecode::opcode::Opcode</code> |
| [bytecode::disasm](../../core/src/bytecode/disasm.rs) | 受阻 | 在固定键名解析结果下，每个 PC 恰输出对应指令行，寄存器角色、常量、跳转和键名与输入一致。 | 键名格式化使用动态回调；另需字符串及 fmt 输出合同。 | 类型：动态回调 |

## 堆、分配与回收（16 项）

| 模块 | 工程判断 | 行为验收目标 | 依赖及已有证明 | 裸源码探测 |
| --- | --- | --- | --- | --- |
| [heap](../../core/src/heap/mod.rs) | 组合 | 每种分配从 safepoint 到发布新对象，成功则所有字段有效且保留调用方声明的根，失败则返回规定的 HeapError 并保持堆一致。 | 需组合 Room、Arena、ValueHeap、GC、对象和动态单元；逐入口审计临时根。 | 依赖：<code>core::any::Any</code> |
| [heap::arena](../../core/src/heap/arena.rs) | 独立 | 任意合法的 alloc、mark、sweep、free、再次 alloc 序列中存活记录内容不变；回收集合、回调次数、空位选择、计数和收缩策略精确。 | 参数化 Record、Room 和回调；分别验证普通复用与 gc-stress 延迟复用。 | 库：div_ceil |
| [heap::atom](../../core/src/heap/atom.rs) | 组合 | 在内容相等、哈希碰撞及回收下，驻留查找返回同一活动原子；删除和重建不混淆其他键。 | PropertyKey 的位级表示已有证明；闭环还需字符串内容、原子表与 Held/Pinned/Weak 生命周期。 | 依赖：<code>value::StringId</code> |
| [heap::bigint](../../core/src/heap/bigint.rs) | 随上层 | 从数学整数到堆记录再读回保持符号和小端 limbs；空槽与外部字节计数满足 Arena 合同。 | 本模块提供记录表示；规范化与运算在 heap 分配入口和 vm::bigint。 | 库：fmt 格式化 |
| [heap::buffer](../../core/src/heap/buffer.rs) | 组合 | 在创建、读写、重叠复制、增长及 detach 后，每个视图的有效长度、边界和可观察字节与字节序列模型一致。 | owned 路径可拆出；shared 路径还需 Arc、原子长度/字节、并发可见性和宿主协议。 | 库：fmt 格式化 |
| [heap::code](../../core/src/heap/code.rs) | 组合 | 发布动态单元后 pin、unpin、mark、sweep 只保留应存活的单元；存活 ID 可查、释放回调和内存账目正确。 | 需代码单元、Rc、缓存槽释放以及 GC 调用方。 | 依赖：<code>bytecode::CodeUnit</code> |
| [heap::gc](../../core/src/heap/gc.rs) | 组合 | 从一份合法堆图与根集执行完整 collect：按强边和 ephemeron 不动点保活，按协议清理弱引用、安排终结任务，再 sweep；幸存者内容和身份保持。 | 根栈与句柄的局部合同已证；须补齐各记录边、队列、缓存、固定根、阈值和预算。调度终结任务与实际执行分开验收。 | 依赖：<code>value::Value</code> |
| [heap::hash](../../core/src/heap/hash.rs) | 独立 | 给定初态及任意 Hasher 写入调用序列，finish 等于规定的分块、小端填充、旋转异或和模乘递推。 | 调用边界属于输入；hashbrown 表的碰撞解决需独立合同。 | 循环：宏展开的 for |
| [heap::object](../../core/src/heap/object.rs) | 组合 | 每种 ObjectKind 的头部、槽范围、payload 和标志一致；建立记录、取回、追踪、释放全过程遵守同一表示关系。 | 需各类 Arena、ValueHeap、InternalData；宿主不透明载荷另列合同。 | 依赖：<code>core::any::Any</code> |
| [heap::object::flags](../../core/src/heap/object.rs#L126) | 随上层 | 设置、读取、清除对象状态位时，各调用点看到一致的对象状态和适用的操作。 | 与 ObjectHeader、对象内建及 VM 快慢路径组合。 | 基础通过（8） |
| [heap::report](../../core/src/heap/report.rs) | 组合 | 从同一堆快照导出的各类报告和总额与容量、自由表、动态代码等分项相符，生成报告不触发收集。 | 以 Heap::bytes 及各存储区计量定义为基准；区分 reserved、used 和活动数量。 | 依赖：<code>heap::gc::GcStats</code> |
| [heap::string](../../core/src/heap/string.rs) | 独立 | 任意 UTF‑16 序列在单字节或双字节表示中逐码元保持；数组索引识别与十进制定义一致；同内容产生同哈希。 | 平坦表示可单证；rope 首次扁平化与缓存一次性写入需再接入 heap 和 OnceCell。 | 库：<code>core::cell::once::OnceCell</code> |
| [heap::symbol](../../core/src/heap/symbol.rs) | 随上层 | 分配到回收保持描述字符串及 private 标志；不同新符号身份不同，private 名不会作为公开 own key 枚举。 | 本文件是记录；身份由 Arena 给出，边追踪和键枚举分别由 GC 与上层负责。 | 库：fmt 格式化 |
| [heap::table](../../core/src/heap/table.rs) | 组合 | 对插入、更新、删除、清空、压缩及交错迭代，Map/Set 与有序映射模型逐步一致；碰撞和压缩不漏项、不重访。 | 需键相等与哈希、内部 slots、游标弱引用；弱集合的可达性归 GC。 | 库：<code>alloc::rc::Weak</code> |
| [heap::values](../../core/src/heap/values.rs) | 独立 | 任意合法 alloc、grow、free、coalesce 序列中活动范围不重叠；读取等于最后写入，增长保留前缀，复用位置已清空。 | 同时验证大小类、自由表、空闲运行区间、位图和记账；需 Room 与 Vec 分配模型。 | 依赖：<code>value::Value</code> |
| [heap::verify](../../core/src/heap/verify.rs) | 组合 | 独立枚举根、活动记录及侧表中的边；对所列边，目标失活时检查器报错，合法状态通过，并说明枚举是否完整。 | 检查器须与独立的边规格对照；随后才能用其证明加强收集器验收。 | 依赖：<code>value::Value</code> |

## 虚拟机（28 项）

| 模块 | 工程判断 | 行为验收目标 | 依赖及已有证明 | 裸源码探测 |
| --- | --- | --- | --- | --- |
| [vm](../../core/src/vm/mod.rs) | 受阻 | 统一 call、construct、异常展开与重入的结果；帧窗口、寄存器和根在返回或抛错后恢复到调用协议规定的状态。 | Services 持有函数指针；需分别建模幸存帧、宿主调用和中断，再连接解释器。 | 类型：动态回调 |
| [vm::frame_flags](../../core/src/vm/mod.rs#L59) | 随上层 | 进入、改写调用、挂起、恢复和退栈时，帧状态位驱动正确的接收者、恢复操作和回溯行为。 | 位常量通过基础检查；验收点在 interp、suspend 和 via 的实际消费位置。 | 基础通过（6） |
| [vm::arguments](../../core/src/vm/arguments.rs) | 组合 | 从形参/实参帧到 arguments、rest、apply 或 spread 窗口，顺序、缺省值、映射别名、解除映射和异常点均符合规格。 | 需寄存器、cell、数组式对象、迭代器和调用时保根合同。 | 依赖：<code>vm::Cx</code> |
| [vm::async_generator](../../core/src/vm/async_generator.rs) | 组合 | 任意 next、return、throw 和 await 交错下按请求队列顺序处理；每个完成的请求兑现或拒绝一次，结果及 done 状态正确。 | 与 suspend、Promise jobs 及调度合同组合；活性需单列队列驱动假设。 | 依赖：<code>vm::Cx</code> |
| [vm::bigint](../../core/src/vm/bigint.rs) | 独立 | 以数学整数解释符号和 limbs，验证规范化、比较、加减乘除余、移位、位运算和幂；每次返回规范表示或规定错误。 | 可先闭合纯整数算术；与浮点互转还需要逐位舍入模型，资源上限还需 qjs。 | 库：fmt 格式化 |
| [vm::bigint::qjs](../../core/src/vm/bigint.rs#L392) | 组合 | 从合法 Big 输入到 QuickJS 可观察的值或错误，验证带符号 limb 上限、零除、负指数及分配错误的优先级。 | 复用父模块的整数正确性；加入项目固定版本的结果大小规则。 | 依赖：<code>vm::bigint::BigError</code> |
| [vm::class](../../core/src/vm/class.rs) | 组合 | 从类创建到构造、super 和私有成员访问，品牌、接收者绑定、初始化顺序及非法访问异常与规格一致。 | 需帧、对象/原型、描述符、调用、异常和 GC。 | 依赖：<code>vm::Cx</code> |
| [vm::compile](../../core/src/vm/compile.rs) | 受阻 | 同一 eval、Function 或脚本请求选择正确作用域与编译种类；合法产物正确发布和执行，编译错误与临时根正确处理。 | 编译宿主与动态输出回调；源码解析/编译器在 core 之外，需另证或显式设合同。 | 类型：动态回调 |
| [vm::error](../../core/src/vm/error.rs) | 组合 | 从抛出点和帧快照得到规定的错误种类、消息、位置及栈；OOM 和不可捕获错误按引擎规定传播。 | 需源码位置、native 与经 call/apply 改写的帧、错误对象和堆。 | 库：fmt 格式化 |
| [vm::ic](../../core/src/vm/ic.rs) | 组合 | 缓存填充、命中、失效、剪枝和复用后，命中所得值或写入目标始终等同普通查找；过期条目只能 miss。 | 需形状/原型有效性、属性慢路径、GC；ic-stats 计数作为独立可观察结果。 | 库：fmt 格式化 |
| [vm::interp](../../core/src/vm/interp.rs) | 组合 | 对合法初态和已验字节码逐指令证明状态模拟；有限运行的返回、异常、挂起、中断和宿主事件与参考机一致，快慢路径可互换。 | 比较跳转表已证；尚需每种 opcode、帧/栈、GC、异常和内建合同。客体程序允许无限循环。 | 声明：static 需后置条件 |
| [vm::interp::call_plan](../../core/src/vm/interp/call_plan.rs) | 组合 | 对每个已生成计划，快捷完成与原指令结果、写入和异常相同；miss 进入普通执行时不重复可见副作用。 | 与 bytecode::call_plan、缓存 guards、调用窗口和轮询组合。 | 循环：宏展开的 for |
| [vm::iter](../../core/src/vm/iter.rs) | 组合 | 从 GetIterator 到 step、完成、提前退出或异常，产值与回调次序正确；IteratorClose 的调用条件和异常优先级一致。 | 需对象、调用和异常模型；数组快速路径要与协议路径等价。 | 依赖：<code>value::Value</code> |
| [vm::jobs](../../core/src/vm/jobs.rs) | 组合 | Promise 只能从 pending 转移一次；注册的 reaction 按规定入队执行，thenable、自解析、抛错及拒绝跟踪均有正确轨迹。 | 需队列、回调、宿主追踪和 GC 根；显式驱动作业时才能证明排队后的进展。 | 依赖：<code>value::ObjectId</code> |
| [vm::module](../../core/src/vm/module.rs) | 受阻 | 解析、加载、链接、求值到 namespace 或异常的状态机满足循环依赖、重导出的歧义检测、live binding、顶层 await 和错误复用规则。 | normalize/load 函数指针是直接障碍；需模块图、缓存、编译器和作业队列。 | 类型：函数指针 |
| [vm::native](../../core/src/vm/native.rs) | 受阻 | 一次宿主调用看到正确 this、argc、new.target 和捕获槽；重入及返回/抛出时恢复根和栈，副作用次数与调用轨迹一致。 | HostBody 动态回调；宿主实现须提供资源与重入合同。 | 类型：动态回调 |
| [vm::number](../../core/src/vm/number.rs) | 外部 | 给定 UTF‑16 或 binary64，验证 StringToNumber、ToInt32/Uint32、十进制输出及 Float16 双向转换，覆盖 NaN、±0、溢出及舍入。 | 整数与半精度位级部分可拆；全路径需浮点、文本解析和 radix 合同。 | 依赖：<code>value::Value</code> |
| [vm::object](../../core/src/vm/object.rs) | 组合 | 从内部对象操作到新对象状态、值或异常，验证描述符兼容、属性键顺序、可扩展性、原型约束及普通/特殊对象分派。 | 需 shape、property、代理、带索引对象、调用与堆；必须比较可见回调次序。 | 依赖：<code>value::Value</code> |
| [vm::ops](../../core/src/vm/ops.rs) | 组合 | 类型转换、算术/比较、相等与 instanceof 在各值类别上给出规定结果或异常；对象转换副作用顺序和根保持正确。 | 依赖 Value、BigInt、浮点、对象、调用和异常；规定 NaN、±0 与整数快速路径。 | 依赖：<code>value::StringId</code> |
| [vm::print](../../core/src/vm/print.rs) | 组合 | 从规定的堆和帧快照产生固定 QuickJS 的显示字节；对象、循环引用、字符串和异常输出过程中不执行用户 getter 或 trap。 | 需只读堆视图、字符串与数值格式；按实际深度/数量限制规定截断。 | 依赖：<code>value::Value</code> |
| [vm::property](../../core/src/vm/property.rs) | 组合 | 从键和接收者到读、写、删及枚举的结果；原型、访问器、稀疏/致密元素和缓存路径与参考属性操作的效果及错误一致。 | 依赖 shape、object、ic、arguments 和调用；同时证明缓存 miss 无额外效果。 | 依赖：<code>value::Value</code> |
| [vm::proxy](../../core/src/vm/proxy.rs) | 组合 | 对每种内部方法，未撤销时按规定查找并调用 trap 或回退；违反目标对象不变量及撤销后的访问在正确步骤抛错。 | 需要 target 描述符、可扩展性和可重入回调模型；覆盖 ownKeys、apply 与 construct。 | 依赖：<code>value::ObjectId</code> |
| [vm::radix](../../core/src/vm/radix.rs) | 外部 | 格式化与解析满足支持基数上的最短往返、舍入到最近偶数、指定精度及前缀消费；在 QuickJS 特殊位数限制处给出相同结果。 | BigInt、精确有理数及 binary64 舍入；输入解析与输出格式有各自的定义域。 | 依赖：<code>vm::bigint::Big</code> |
| [vm::regexp](../../core/src/vm/regexp.rs) | 外部 | 同一源码/标志复用活动程序，每次字面量求值产生不同对象；编译失败正确传播，标记/回收后无活动对象引用失效程序。 | 程序存储与 GC 可组合证明；正则编译和匹配正确性依赖 oxide-regexp。 | 依赖：<code>value::StringId</code> |
| [vm::suspend](../../core/src/vm/suspend.rs) | 组合 | 挂起时保存完整可恢复状态；next/throw/return 或作业恢复后，执行等同于在同一点注入对应动作，完成后的状态正确。 | 需帧/寄存器、堆 activation、解释器异常路径和 async 作业。 | 依赖：<code>value::Value</code> |
| [vm::typed](../../core/src/vm/typed.rs) | 组合 | 在元素类型、buffer 状态、索引和值给定时，读写字节、端序、数值转换与越界处理符合模型；快速和通用路径等价。 | 需 buffer、BigInt 和浮点；共享缓冲区另接并发协议。 | 依赖：<code>heap::Heap</code> |
| [vm::via](../../core/src/vm/via.rs) | 组合 | 改写 call/apply 链时记录、提交、放弃与退栈，最终回溯可见的虚拟帧顺序与未改写调用一致且无残留。 | 局部栈协议可单证；观测端在 interp 和 vm::error。 | 依赖：<code>value::Value</code> |
| [vm::waiter](../../core/src/vm/waiter.rs) | 受阻 | 对每一共享地址证明比较和登记的线性化、notify 的选择与计数、超时结果；非阻塞回调同步执行恰一次。 | 动态回调阻塞翻译；实际锁、队列与唤醒由宿主实现。进展性质需要调度假设。 | 类型：动态回调 |

## 内建对象与算法（33 项）

| 模块 | 工程判断 | 行为验收目标 | 依赖及已有证明 | 裸源码探测 |
| --- | --- | --- | --- | --- |
| [builtins](../../core/src/builtins/mod.rs) | 受阻 | 安装 realm 后每个内建名、属性、原型、长度、构造能力与 native ID 对应正确；分派调用选中同一实现。 | NativeFn 函数指针表直接受阻；还需 realm 初始化、描述符及 native ABI 合同。 | 类型：函数指针 |
| [builtins::agent](../../core/src/builtins/agent.rs) | 受阻 | 从 start、broadcast、receive、report 到 join，消息进入正确代理，回调在交付期间保根，停止/加入操作符合宿主协议。 | 脚本运行函数指针；线程、共享 backing 和可观察调度需 AgentHost 合同。 | 类型：函数指针 |
| [builtins::array](../../core/src/builtins/array.rs) | 组合 | 构造和各方法对空洞、长度、继承属性及回调顺序符合参考数组操作；逐方法证明致密快速路径与通用路径等价。 | 需 property、iter、调用和排序比较器；包括原地操作、复制方法及中途抛错后的状态。 | 依赖：<code>value::Value</code> |
| [builtins::atomics](../../core/src/builtins/atomics.rs) | 受阻 | 对并发读写、RMW、compareExchange 及 wait/notify，返回值和最终字节可线性化为允许的顺序，非法视图或操作数在规定步骤报错。 | 修改操作的函数指针；还需 backing 的原子模型和真实 Waiter 协议。 | 类型：函数指针 |
| [builtins::base64](../../core/src/builtins/base64.rs) | 独立 | 标准与 URL 字母表的生产编码→解码还原任意字节串；对任意 UTF‑16、容量及末块模式，解码所得字节、read 和错误三者精确；另证 hex 解码。 | 内核可纯建模；需覆盖空白、填充、剩余位、容量截断及项目的非 ASCII hex 行为。 | 库：<code>core::intrinsics::discriminant_value</code> |
| [builtins::bigint](../../core/src/builtins/bigint.rs) | 组合 | BigInt 调用、装箱接收者读取、基数输出及 asIntN/asUintN 给出正确数学结果或异常，保留规定的转换先后。 | 依赖 vm::bigint、输入转换和堆分配；调用与构造区分。 | 依赖：<code>vm::Cx</code> |
| [builtins::boolean](../../core/src/builtins/boolean.rs) | 组合 | 对任意值，Boolean 调用得到 ToBoolean；new 创建正确包装对象；valueOf/toString 对不同接收者返回正确值或抛错。 | 依赖 VM 转换和内部槽；适合作为轻量调用层验收。 | 依赖：<code>vm::Cx</code> |
| [builtins::buffer](../../core/src/builtins/buffer.rs) | 组合 | ArrayBuffer/SAB 与 DataView 的创建、resize/grow、slice、transfer、detach 和读写满足长度、别名、端序及错误语义。 | 需 heap::buffer、类型转换、分配与共享模型；可重入转换之后重新校验。 | 依赖：<code>vm::Cx</code> |
| [builtins::case](../../core/src/builtins/case.rs) | 外部 | 对完整 UTF‑16 串，上/小写转换符合固定 Unicode 数据，覆盖终结 sigma、多码元展开与长度超限返回。 | 主要算法和表在 oxide-regexp；core 可单证包装转换，全量语义需表和外部算法合同。 | 库：<code>oxide_regexp::unicode::CodeUnitBuf</code> |
| [builtins::collection](../../core/src/builtins/collection.rs) | 组合 | Map/Set 操作与迭代符合有序抽象；WeakMap/WeakSet、WeakRef、FinalizationRegistry 与可达性、kept objects 和排队终结规则一致。 | 普通集合需 heap::table；弱集合还需 GC 不动点和 jobs。 | 依赖：<code>value::ObjectId</code> |
| [builtins::date](../../core/src/builtins/date/mod.rs) | 组合 | 构造、now、parse、UTC、getter/setter、字符串化按固定 QuickJS 返回时间值/文本或异常，并在正确步骤调用时钟与时区。 | 需三个日期子模块、宿主读数和参数转换；显式区分本地与 UTC。 | 库：f64::is_nan |
| [builtins::date::calendar](../../core/src/builtins/date/calendar.rs) | 外部 | 给定时间值和时区函数，拆分/合成公历字段与历法定义一致；在有效 UTC 整毫秒域往返，涵盖闰日、负年份、边界及无效值。 | 整数历法可独立证明；完整 TimeClip、归一化与 FP 运算需舍入模型。 | 库：f64::is_nan |
| [builtins::date::format](../../core/src/builtins/date/format.rs) | 独立 | 对合法分解字段与方法选择，输出固定格式的完整文本；覆盖扩展年份、时区、locale 固定格式和无效日期异常。 | 需字段合法性与整数/字符串格式合同；不需真实时钟或时区数据库。 | 依赖：<code>builtins::date::DateStringMethod</code> |
| [builtins::date::parse](../../core/src/builtins/date/parse.rs) | 独立 | 任意 UTF‑16 输入映射到解析结果或失败，与固定 QuickJS 扫描语法等价，含 127 码元处理、ISO/旧式日期、时区和字段范围。 | 独立验收字段及本地/UTC 标志；转换为最终时间值在父模块。 | 模式：数组转切片 |
| [builtins::error](../../core/src/builtins/error.rs) | 组合 | 构造 Error、子类及 AggregateError 时消息、cause、errors、栈和原型正确；读取选项和迭代异常按顺序传播。 | 需 VM 错误、对象、迭代器、堆和 realm。 | 循环：宏展开的 for |
| [builtins::function](../../core/src/builtins/function.rs) | 组合 | call/apply/bind 对 this、参数和构造行为与原调用一致；动态 Function 的源码、类型和错误符合项目约定。 | 需帧、绑定函数、ToString、堆及编译宿主。 | 循环：宏展开的 for |
| [builtins::generator](../../core/src/builtins/generator.rs) | 组合 | next、return、throw 以及 async-from-sync 使迭代结果、完成状态、关闭与异常次序符合生成器协议。 | 需 suspend、async_generator、迭代器与 Promise jobs。 | 依赖：<code>vm::Cx</code> |
| [builtins::global](../../core/src/builtins/global.rs) | 组合 | parseInt/parseFloat、isNaN/isFinite 按转换规则返回；print 和 performance.now 按宿主协议产生输出与读数。 | 需 radix、ToNumber/ToString、打印与宿主读写边界。 | 依赖：<code>host::Host</code> |
| [builtins::iterator](../../core/src/builtins/iterator.rs) | 组合 | Array、TypedArray 与 String 迭代的产值、键值对、索引推进与完成状态正确；字符串代理对按迭代语义处理。 | 需对象/数组长度、typed 视图及字符串；覆盖迭代过程中的修改。 | 库：fmt 格式化 |
| [builtins::iterator_helpers](../../core/src/builtins/iterator_helpers.rs) | 组合 | map/filter/take/drop/flatMap/concat 等在任意拉取、关闭和抛错序列下产生规定值与回调轨迹；提前停止正确关闭。 | 需可重入迭代协议、闭包、嵌套迭代器和异常优先级。 | 依赖：<code>vm::Throw</code> |
| [builtins::json](../../core/src/builtins/json.rs) | 组合 | parse 先证明文本到值或语法错误，再纳入 reviver/context.source；stringify 验证属性顺序、转义、循环检测、replacer/toJSON 与 rawJSON。 | 要分别建立语法、对象图、数值和回调轨迹规格；往返定理限定到可无损表示的 JSON 子域。 | 依赖：<code>value::Value</code> |
| [builtins::math](../../core/src/builtins/math.rs) | 外部 | 逐方法规定 IEEE 特殊值和数值结果；整数/舍入、sumPrecise、给定种子的 random 转移各有独立目标。 | 超越函数、精确求和与随机状态分别需要浮点后端、数学误差/舍入和状态模型。 | 循环：宏展开的 for |
| [builtins::number](../../core/src/builtins/number.rs) | 组合 | 调用或构造、包装接收者检查、谓词与 toFixed/toExponential/toPrecision 等返回正确字符串、值或范围错误。 | 依赖 vm::number/radix、对象和 JS 参数转换。 | 循环：宏展开的 for |
| [builtins::object](../../core/src/builtins/object.rs) | 组合 | Object 静态与原型方法正确驱动内部方法；own keys、拷贝、定义/冻结/密封及 getter 副作用的结果与次序符合规格。 | 需 vm::object/property/proxy 和迭代器；包含特殊对象与目标异常。 | 依赖：<code>vm::Cx</code> |
| [builtins::promise](../../core/src/builtins/promise.rs) | 组合 | 构造、then/catch/finally 和四个组合器的结果、元素顺序、短路、拒绝与迭代关闭符合项目 Promise 语义。 | 依赖 vm::jobs、可调用对象、thenable 重入和显式 drain；按项目的 species 行为建模。 | 依赖：<code>vm::Cx</code> |
| [builtins::proxy](../../core/src/builtins/proxy.rs) | 组合 | 构造和 revocable 建立正确 target/handler；多次 revoke 幂等，撤销后各依赖代理的操作按规定失败。 | 状态入口在内建，完整观测需 vm::proxy 各内部方法。 | 依赖：<code>vm::Cx</code> |
| [builtins::reflect](../../core/src/builtins/reflect.rs) | 组合 | 每个 Reflect 操作与对应内部方法在 receiver、参数列表、键、布尔结果、异常和副作用次序上相符。 | 依赖 VM 对象、代理、构造/调用、数组式实参。 | 依赖：<code>vm::Cx</code> |
| [builtins::regexp](../../core/src/builtins/regexp.rs) | 外部 | exec、test、match/replace/search/split 等在匹配结果给定时正确构造 captures/groups/indices、推进 lastIndex 并处理自定义 exec。 | 对象与协议可组合证明；匹配语言语义需 oxide-regexp 的引擎合同。 | 依赖：<code>value::ShapeId</code> |
| [builtins::string](../../core/src/builtins/string.rs) | 受阻 | 逐组证明 UTF‑16 索引/切片/搜索、替换、大小写与协议委派；覆盖孤立代理项、回调顺序和长度错误。 | 命名捕获回调是直接障碍；Unicode 表、正则、对象转换需分别给合同。 | 类型：动态回调 |
| [builtins::symbol](../../core/src/builtins/symbol.rs) | 组合 | Symbol 每次创建新身份，for/keyFor 对注册表一致，已知符号稳定；描述、接收者和构造错误正确。 | 需 heap::symbol、驻留键、Realm 注册表与 GC。 | 循环：宏展开的 for |
| [builtins::test262](../../core/src/builtins/test262.rs) | 组合 | 对项目支持的 $262 能力，gc、evalScript、detachArrayBuffer 和 agent 的可见效果与合同一致；createRealm 按约定抛错。 | 这是生产编译的宿主对象；执行侧依赖 heap、compile、buffer、agent。 | 循环：宏展开的 for |
| [builtins::typed_array](../../core/src/builtins/typed_array.rs) | 组合 | 十二种 typed array 的构造、元素访问、复制、迭代及 codec 方法保持正确类型、字节和读写计数；detach/resize 后在正确时点重验。 | 需 heap::buffer、vm::typed、回调、数值和 base64；覆盖共享存储与重叠内存。 | 依赖：<code>heap::buffer::ElementKind</code> |
| [builtins::uri](../../core/src/builtins/uri.rs) | 部分已证 | decodeURI/decodeURIComponent 的解码内核对所有 UTF‑16 输入的成功和错误结果已证；补证生产编码器→生产解码器的有效域往返及 Annex B 两个入口。 | 现有往返使用数学编码器；JS 参数强制转换、分配和抛异常需 VM 合同。 | 依赖：<code>vm::Cx</code> |

## 可以单独验收的跨模块闭环

源码的分文件边界有时切在生产者与消费者之间。下表给出能由用户或调用方观察到的交付条件，并列出仍缺的证明。上述每个模块只计一次，下列组合共享前表的模块。

| 行为闭环 | 建议的外部观测与验收条件 | 需要连接的实现 |
| --- | --- | --- |
| 编码文本 → 字节 → 编码文本 | 任意字节序列在支持的 Base64 模式下能经生产编码器、生产解码器还原；对任意损坏输入或容量截断，已读长度、已写前缀、错误匹配独立规格。JS 层再核对目标 typed array 的实际写入。 | builtins::base64 → builtins::typed_array → vm::typed / heap::buffer |
| 分配 → 写入 → 扩容 → 释放 → 复用 | 任意合法操作序列后，所有活块互不重叠，值等于抽象映射；扩容保留内容，回收后再分配读不到旧值，空间账目守恒。 | heap::values、heap::arena、heap::gc::Room；两种分配器分别证明，再在 heap 入口组合 |
| 汇编 → 验证 → 发布 → 解释执行 | 检查失败不会发布；成功单元的所有静态操作数满足相应结构约束。另证每条指令保持动态帧和类型不变量，才可推出运行时访问正确。 | bytecode::builder / opcode / validate / CodeStore → vm::interp |
| 建立根与强弱边 → collect → 继续访问 | 任意合法对象图上的完整收集保留模型要求的可达对象；应清除的弱边失效，跟踪存活对象时绝不落入已复用槽。 | heap::gc / arena / values / object / code、作业根和 vm::regexp |
| 定义属性 → 缓存命中 → 原型变更 → 再读取 | 同一操作历史在有无缓存时产生相同结果、异常和可见副作用；形状切换、删除或原型变更后，旧命中不会读到过期值。 | shape → vm::property / ic → vm::object，包含对应内建入口 |
| 注册 Promise reaction → settle → drain | 在显式驱动作业和约定的回调行为下，reaction 的顺序、次数、结果及队列余项与参考状态机一致；覆盖抛错与重复 resolve。 | builtins::promise → vm::jobs / suspend → vm 调用与宿主拒绝追踪 |

若只选一个同时具有工程价值、又能先避开宿主回调的较大项目，建议从 **heap::values 的分配—复用闭环** 开始。它能以完整操作历史为输入，以所有活块的内容与空间账目为输出，直接检验 core 使用的存储实现。完整 GC 定理可随后消费该合同。

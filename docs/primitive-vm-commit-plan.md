# 原语 VM：单 PR 的逐 commit 计划

状态：实施计划，未创建下列代码提交，未取得新引擎收益。2026-09-12。

本文将[实施设计](primitive-vm-implementation-plan.md)落到一个 PR 内的提交顺序。架构目标与 issue #16 问题目标同时验收。原 P0–P9 是责任/验收分类，A–H 是早期切片划分；本文件的 C01–C32 是实际实施顺序，发生顺序冲突时以本文件为准。

## 1. 一个 PR 的完成范围

**一个 PR 完成目标架构、生产入口迁移、旧路径清理，以及 issue #16 清单第 1、5、7、9、10 项的结果验证。** 不将前半段合并为“架构准备”，再把实际问题解决和语义迁移留给下一个 PR。第 10 项的独立成本仍按证据报告，明确 PC/帧契约必须完成。

必须包含：

- 直接槽执行、显式调用帧、清晰的执行值/roots、共同异常/恢复协议、语义 CFG 与验证发布。
- 当前生产支持的 JS 能力、内置、模块和宿主入口迁移到新核心；没有按用例、opcode 或失败情况回旧 VM 的路径。
- 完整语义/平台回归、默认原始 Earley-Boyer 和组合、固定 50+8 benchmark、最终 profile 与问题对应报告。
- 旧 VmHost local/argument 协议、重复执行状态和重复生产语义退出；模块文档与架构检查对应真实新结构。

本 PR 的原语 VM 范围不额外包括 issue #16 的全部 Array/RegExp/shape 优化，或 #20 完整 Task/Fiber/取消 API。为新架构兼容而迁移这些内置和暂停状态是必做；新增其专项性能或调度功能不自动算入。类型块保持原计划中的有条件优化，在本 PR 内完成进入/不进入的决策。

### 提交约定

- C01–C32 是 32 个主提交边界。每个代码 commit 包含对应测试和必要契约文档，不把测试集中拖到最后。
- 提交保持构建和本阶段已支持能力的测试通过。实验入口明确拒绝尚未支持输入，不返回伪结果，也不转旧 VM。
- 最终只有一个 PR；可以逐 commit 推送供审查，但满足全部合并门槛前保持未合并。PR 中途绿色不等于任务完成。
- 发现缺陷时，在发现处加入具名修复 commit，再继续后续门槛；32 不是禁止必要修复的硬上限。不能将“修复所有回归”预埋成一个无范围的大提交。
- 中途接口调整可以整理尚未依赖的提交边界，保证最终历史可解释；每份 receipt 绑定实际源码和 ELF，整理历史后不能沿用失配的 commit 声明。
- 全部在同一任务分支推进，建议名称 `vm/primitive-execution-core`；当前仅规划，不在此文档变更中创建分支或 PR。

## 2. 首版实现基准

为避免每个 commit 都重新选择架构，先按寄存器 baseline、tagged Copy word、有身份句柄、非移动 STW tracing、独立执行的连续 arena 实施。Number 语义保持唯一实现；从现有算法移植必要规则和反例。安全 Rust 是起点。

这是明确的实施基准，不是宣称其性能已经获证。C14 用共同切片比较寄存器/优化栈以及关键布局。若改变具体实现，更新后续提交细节并保留同样架构能力，不回到旧 VmHost 或取消整体迁移。NaN-box、复杂 GC、分段 arena 不作为默认同时实现的三条生产路径。

所有提到的模块是实施设计中的责任，不要求沿用当前同名目录。确有帮助的现有 payload、parser 和独立 regexp 算法可以移植；完整 JS 语义的 owner 必须唯一。

## 3. C01–C06：建立可运行的数值执行流程

### C01 · `docs(vm): define architecture and issue acceptance contracts`

- **改动**：固定双重目标、接口兼容清单、模块依赖和所有权；登记当前 Runtime/Context/Value、host callback、binary、资源限制与平台契约。为当前语法/内置/入口建立迁移清单，每项有最终 owner 和验收入口。
- **产物**：本文及关联设计成为可追踪计划；语义能力清单用于 C19/C24/C29 核对遗漏，不是允许删减能力的名单。
- **验收**：六项架构能力和五项问题有明确完成证据；不将 PR19 历史数据标成当前新测量。
- **依赖**：无。此项属于计划文档提交，不虚报引擎实现。

### C02 · `test(vm): capture baseline and execution diagnostics`

- **改动**：记录实际起点 HEAD、PR19 参考及差异；保留普通 ELF/工具链/负载哈希。增加实际发布字节码/source 的可读 dump，以及受控诊断模式的 dispatch、复制/保活、调用分配、native 栈和 PC 管理观测。
- **算法边界**：诊断可插桩；正式计时二进制关闭插桩。不得用 lowering 发码顺序或 execute_inner 总占比替代最终 opcode/PC 归因。
- **验收**：复现默认 Earley-Boyer 的真实当前状态；冻结相关反例和输出；诊断构建与普通构建身份可核对。
- **依赖**：C01；对应全部问题的测量基础。

### C03 · `refactor(value): define execution words and shared number semantics`

- **改动**：增加内部 VmWord、类型化 heap identity 与 slot index；宿主 owning handle 与内部 word 分开。建立唯一 Number 算法入口；迁移 Int 溢出、负零、NaN、ToInt32/ToUint32、移位和 pow 规则。
- **边界**：此时只有纯数值入口可运行；heap word 类型不是已存在的 root，尚未接好 heap 的路径不可执行。临时旧 API 适配调用共同算法，不复制一份近似实现。
- **验收**：数值边界差分、实际 enum 大小/对齐；不把 Float cast 或 wrapping 加法当 JS 语义。
- **依赖**：C02；架构的值边界，第 5 项基础。

### C04 · `feat(code): publish verified register instructions`

- **改动**：指令/操作数类型、编码解码、Wide、不可变代码 owner、FrameLayout、source map 和验证器。初始支持常量、移动、数值、分支、返回；后续 opcode 随其语义 commit 一起加入。
- **算法**：验证指令头、所有字节（含不可达区）、寄存器边界、跳转和元数据配对；32-bit 基础编码配扩展字，解码规格与打印共用机械定义。
- **验收**：坏分支、截断/Wide 溢出、坏寄存器、owner/map 错配被拒绝；手工构造的合法代码可打印和解码。
- **依赖**：C03；发布边界。

### C05 · `feat(compiler): lower numeric control flow to semantic cfg`

- **改动**：复用 parser 能力，建立 BindingId、语义 CFG、block 参数及最小源码 lowering；循环、条件、普通非逃逸局部、数值表达式。保守分配寄存器，处理合流 move 和 source 位置。
- **算法**：sealed-block SSA、初始化状态独立于 Undefined；critical edge 拆分、并行 move 解环。先生成正确代码，优化在 C13。
- **验收**：源码→CFG→已验证码完整；循环合流、交换环、分支和未初始化访问有反例；未支持语法明确拒绝。
- **依赖**：C04。

### C06 · `feat(vm): execute primitive instructions in a local register loop`

- **改动**：执行槽/活动帧、match 分派、局部 PC、直接数值/布尔操作和基础预算出口。读完源后写目标，允许 dst 与源别名；原语区间不访问万能 Runtime。
- **算法**：确切 Int/Number 比较、算术、位运算和 Numeric Increment 复用 C03；回边及有界直线段 poll；出口发布状态。此时 Number miss 明确未支持，C10 补完整语义。
- **验收**：数值源码端到端；前后缀、溢出、负零和长直线预算测试；运行 empty/int/float 子集的普通 ELF 诊断，明确尚非完整 JS 引擎。
- **依赖**：C05；直接处理第 5 项和第 9 项的一部分。

## 4. C07–C14：完成根、调用、转换和架构定稿

### C07 · `feat(heap): add rooted execution storage and tracing collection`

- **改动**：目标对象表、root scope/persistent roots、非移动 STW 强边标记回收、分配与外部 backing 记账；ExecutionRecord、pending、frame/槽的 trace 路径。
- **算法**：根交接先于任何可 GC 操作；死亡槽清空；句柄归属/代次验证及耗尽策略。所有已支持对象类型有 trace；weak 相关类型暂不开放，C20 接入完整语义。
- **验收**：纯循环仍通过；小型目标 heap 的循环引用、最后 root、槽覆盖、分配失败契约和强制 GC 测试通过。
- **依赖**：C06；不在旧 RC heap 上省略保活。

### C08 · `feat(object): bind code and basic objects to the target heap`

- **改动**：运行时 CodeInstance、常量绑定、最小 realm、String/BigInt/Symbol/普通对象和字节码函数的目标 payload；最小自有 data/accessor 属性存储，用于普通函数和转换。
- **边界**：对象身份只有一个 owner；pure code 不嵌入 live runtime object。现有对象算法移植或适配必须明确根和身份，不复制 JS 对象跨 heap。
- **验收**：函数/对象字面量、常量绑定、基本属性身份、跨实例隔离、编译/绑定失败清理；强制 GC 下 payload 可达性正确。
- **依赖**：C07；完整 exotic/property 语义在 C16。

### C09 · `feat(vm): drive ordinary calls with explicit frames`

- **改动**：driver、ExecutionId/FrameId、ReturnTarget、参数和 actual args 布局、push/return 事务。普通 JS 调用不再递归进入 Rust 解释器。
- **算法**：准备容量和根后提交帧；参数复制到独立 callee 区域；返回值先安装到有效 root 再撤掉 callee。保留 this/new.target 和 code/realm 身份。
- **验收**：嵌套调用、参数不别名、额外实参、调用中 GC、返回最后引用；深度增长不会线性叠加普通解释器 native 帧。完整默认 Earley 在 C29 验收。
- **依赖**：C08；第 1/7 项。

### C10 · `feat(semantics): resume coercions through explicit operations`

- **改动**：小型 request/reply 协议、operation 栈、ToPrimitive/ToNumeric、通用 Add/关系/位运算。语义服务返回 InvokeJs，driver 将回调返回给 OperationId。
- **算法**：阶段字段有名字；保存已完成左转换，不因右侧回调或回复重做；String/Number/BigInt 分类在规范时点进行；pending/reply 全部有根。
- **验收**：左右转换顺序、@@toPrimitive、返回非原语、valueOf/toString、String/BigInt/Symbol 混合；每个分配点收集仍正确。
- **依赖**：C09；数值慢语义与第 5 项完整性。

### C11 · `feat(vm): unwind exceptions and pending completions explicitly`

- **改动**：CFG/字节码 handler 边、fault/resume 位置、catch/finally、return/throw/break/continue completion，operation 的异常回复和必要清理。
- **算法**：按已完成前缀恢复；先执行 operation 需要的错误处理，不能直接丢弃全部状态；finally 和清理的新 completion 按各自规范处理。
- **验收**：转换抛错准确回到调用点、嵌套 finally 覆盖、跨函数 throw、异常路径 root 释放一次。暂停控制信号不被 JS catch 捕获。
- **依赖**：C10。

### C12 · `feat(compiler): preserve captured and dynamic binding identity`

- **改动**：闭包 Cell/Environment、TDZ/const、每轮词法环境、mapped/unmapped arguments、重复参数、direct/indirect eval 和动态 Function 的新编译入口。
- **算法**：BindingStorage 决定明确 opcode；可提升局部与动态可见 cell 分离；写权限属于绑定视图；eval-capable 首版保守物化。
- **验收**：closure/eval/arguments 相互影响、per-iteration 捕获、默认参数作用域、严格模式和错误 realm。代码缓存不混用不同环境。
- **依赖**：C11；第 7/9 项语义保障。

### C13 · `perf(compiler): eliminate redundant local moves and updates`

- **改动**：完整 liveness（含异常边）、保守 linear scan、copy propagation、死纯值消除、常量/分支化简及局部更新结果复用。
- **算法**：Numeric Increment 与 Add 1 分开；不能删除会转换/抛错的运算，不能重结合浮点；前后缀、未使用结果、cell/arguments 别名保持正确。
- **验收**：实际最终码减少重复读写/dispatch，loop/int/float 指令数和耗时交错比较；C12 语义反例不退化。
- **依赖**：C12；第 9 项的主要优化提交。

### C14 · `perf(vm): settle the baseline layout using a shared semantic slice`

- **改动**：用数值、普通调用、对象转换、异常和 GC 的共同切片比较 S/R；对关键编码/word/arena 成本作独立实验，确定后续唯一生产方案并提交决策及相关最终布局。
- **边界**：比较方案复用相同值/GC/调用/语义底座；不把 8 字节表示、GC 和后端同时变化都归功于寄存器。实验原型不成为永久 feature 矩阵；有收益的具体调整随本 commit 保留。
- **验收**：报告真实支持范围、普通 ELF 计时/计数、编译延迟、代码和保留内存；记录未决成本。不因局部收益已足够而取消后续架构能力。
- **依赖**：C13；原 P3 决策门槛。结果改变实现时，在继续前更新后续 commit 细节。

## 5. C15–C20：迁移完整对象与内置语义

### C15 · `feat(semantics): support constructors and class execution`

- **改动**：普通/derived constructor、super、new.target、bound callable、class 字段与 private brand；相应源码 lowering 和初始化 continuation。
- **算法**：构造返回规则、this 初始化、字段顺序和异常 cleanup 明确；复用普通帧/operation 驱动，不另建 class VM。
- **验收**：继承、私有字段访问、字段初始化抛错、构造返回对象/原语、跨 realm；在尚不支持 Proxy 时明确拒绝，C16 接入。
- **依赖**：C14。

### C16 · `refactor(object): route complete property semantics through resumable operations`

- **改动**：完整 Get/Set/Define/Delete、receiver、原型、描述符和 Proxy；shape/槽事务归 object，完整规范流程归 semantics；接入 callable Proxy。
- **算法**：每次 getter/setter/trap 都通过共同 InvokeJs；回调后重新获取会变化的存储状态；异常时保留已完成副作用。
- **验收**：Proxy 不变量、原型访问器、不同 receiver、冻结/删除/顺序、重入修改 shape；当前属性优化能力迁移且不能引入旧 VM 回退。
- **依赖**：C15。

### C17 · `refactor(builtins): migrate array and iterator continuations`

- **改动**：Array 构造/方法、arguments exotic、同步 Iterator 协议和 IteratorClose；先以 map 完整流程建立可读阶段，再迁移相应家族。
- **算法**：species、holes、HasProperty/Get、callback、sort comparator、spread/destructuring 和中途异常共享规范协议；dense 存储复用已验证事务。
- **验收**：数组/迭代家族 oracle、species/Proxy 回调、close 异常优先级、每阶段 GC；原数组回退负载保持正确。
- **依赖**：C16；这里迁移架构，不重复申报 Array 专项优化。

### C18 · `refactor(builtins): migrate string regexp and buffer execution`

- **改动**：String/RegExp、replacement callback、ArrayBuffer/TypedArray/DataView/SharedArrayBuffer/Atomics 的当前已支持入口接到目标值、heap、请求和中断协议。
- **算法**：独立 regexp executor 可复用；JS 转换与回调归内置阶段。resize/detach 后重新取得 view；Latin1/UTF-16/rope、surrogate 和 BigInt 语义保持。
- **验收**：对应完整回归、replacement 顺序/抛错、共享/可变 buffer、TypedArray 转换回调；RegExp 与 TypedArray benchmark 控制项无未解释退化。
- **依赖**：C17。

### C19 · `refactor(builtins): complete synchronous language and intrinsic migration`

- **改动**：按 C01 清单迁移剩余同步能力：Object/Reflect/Function、Map/Set、Number/BigInt/Boolean/Symbol、Math/Date/JSON、Error 及仓库当前其他 intrinsic；补齐剩余同步语法 lowering 和 realm 初始化。
- **边界**：此提交只承接清单中可明确枚举的剩余同步家族；实施 C01 时把实际清单附到本项。若 diff 过大按家族拆连续子提交，不能以“misc fixes”隐藏工作。Weak、Promise、模块等有后续指定 owner。
- **验收**：每个迁移家族独立 oracle/回归；JSON toJSON/replacer、Map/Set 迭代修改、Error source/realm 等反例；同步覆盖清单无无主条目。
- **依赖**：C18。

### C20 · `feat(heap): complete weak references and collection lifecycle`

- **改动**：WeakMap/WeakSet、WeakRef/FinalizationRegistry、ephemeron、job kept-alive 与 finalization 队列接口；补齐所有已迁移 payload、realm/code/host/execution 边。
- **算法**：strong mark 后 weak 固定点；collect 不运行 JS；jobs 在允许的时机消费清理工作。不可达挂起记录也不能被永久 root 掩盖，后续挂起类型新增时带 trace 接入。
- **验收**：循环/weak 链、kept-alive、外部 backing 回收、句柄复用、最后根交接和强制 GC；最终 job 集成随 C22 验收。
- **依赖**：C19。

## 6. C21–C26：恢复、模块与宿主兼容

### C21 · `feat(vm): suspend and resume generator execution state`

- **改动**：generator 创建与独立执行存储、yield/yield*、next/return/throw、resume 边与值传递；统一 token/状态验证和 trace。
- **算法**：保存帧/operation/completion，暂停时无活跃 Rust borrow；恢复一次；yield* 的委托和清理保留规范行为。
- **验收**：重复/非法重入、finally 中 yield、throw/return、GC 后恢复和不可达 generator 回收。
- **依赖**：C20。

### C22 · `feat(jobs): resume async functions through promise jobs`

- **改动**：Promise 构造/组合、thenable assimilation、reaction jobs、async 函数/await、host job 驱动；接入 weak kept-alive 清理边界。
- **算法**：同步前缀直接执行；await 通过规定 job 恢复；Promise resolve 只生效一次；等待关系和 execution 可达性显式。预算 poll 不变成插入其他 JS 的调度点。
- **验收**：微任务顺序、恶意 thenable、异常传播、等待/回复 GC、async 返回 Promise 的归属和宿主 queue 契约。
- **依赖**：C21。

### C23 · `feat(vm): unify async generators and asynchronous iterator cleanup`

- **改动**：async generator 请求队列、await/yield 交互、for-await、AsyncIteratorClose 和已有异步迭代能力。
- **算法**：不同恢复原因保留独立语义，共用规范执行状态；排队请求只完成一次，cleanup 异常按规范传播。
- **验收**：交错 next/throw/return、finally await、异步迭代中断、取消所有者引用后的 roots；不新增 #20 Task 取消行为。
- **依赖**：C22。

### C24 · `refactor(modules): evaluate modules with the new execution core`

- **改动**：模块绑定/链接/求值、live imports、循环、top-level await、动态 import 和 loader 回复，统一新 compiler/code instance/execution。
- **算法**：模块状态和挂起依赖图保持规范；失败值保留原值/realm，不能字符串化；所有入口创建新格式代码。
- **验收**：同步和异步循环、重复导入、link/evaluation failure、TLA、loader 重入/失败；C01 的语法/模块清单完成。
- **依赖**：C23。

### C25 · `refactor(api): preserve host roots and reentry on the new runtime`

- **改动**：公开 Runtime/Context/Value 接口、persistent/temporary roots、native callback、资源预算，以及 native/web/WASM 适配接新 driver。
- **算法**：调用 host 前登记根并释放借用；同步重入建立独立 execution；禁止重驱动同一 active execution；等待 token/回复身份有效。同步 API 不静默改成异步。
- **验收**：JS→native→JS、跨 realm、宿主保留/释放值、GC/异常、有限/无限递归、小栈线程和平台构建。保留真实 host native 栈限制。
- **依赖**：C24；第 1/7/10 项对外契约。

### C26 · `refactor(code): preserve supported binary formats through verified translation`

- **改动**：所有受支持 binary-object/QuickJS 输入输出和嵌入入口适配到新 IR/验证发布；内部新编码与外部格式分开。
- **算法**：外部旧栈格式如需解释其控制流，使用独立格式翻译，涵盖异常、绑定和隐藏 completion；不得调用旧执行器。输出兼容按 C01 契约逐项证明。
- **验收**：现有认证 fixture、round-trip 和外部兼容对照、畸形输入、owner/root 失败清理；不能通过关闭 binary 测试完成迁移。
- **依赖**：C25。

## 7. C27–C32：优化落实、单 PR 切换与最终证据

### C27 · `perf(vm): reduce call storage and root bookkeeping overhead`

- **改动**：根据已完整语义的调用 profile，定稿 frame 冷热布局、连续窗口容量复用、元数据绑定缓存、actual args 按布局保留和 safepoint live roots。删除已经无必要的桥接保活/复制。
- **算法**：复用死亡窗口而不别名 caller 活变量；捕获/arguments/挂起值有独立 owner。返回值安装在根中后才释放帧；代码 owner 不能被 metadata cache 弱化成悬空引用。
- **验收**：func_call/closure 分配、复制、指令数和耗时下降；大对象保留/挂起内存控制，eval/arguments/重入反例全部通过。
- **依赖**：C26；第 7 项主要最终优化；第 1 项 native frame 同时复核。

### C28 · `perf(vm): publish execution positions only at defined observation boundaries`

- **改动**：独立量化 PC/帧管理后定稿 fault/resume/last-completed 位置和窗口协议；在可证明区间取消重复 active-frame 查找/发布，验证所有观察边界。
- **算法**：每个可能抛错/调用/分配/暂停/调试观察点有准确位置；局部 PC 在无观察区间更新。分支不能用 next_pc - 1 还原源位置；预算和中断保证有界。
- **验收**：错误 source、回溯、重入、generator/async、调试/中断反例；报告独立计数与性能。若重复开销已被 C06/C09 架构消除，提交最终契约与残余成本证据，不制造第二次“优化收益”。
- **依赖**：C27；第 10 项。

### C29 · `test(vm): qualify the new core against conformance and benchmark gates`

- **改动**：用独立新核心入口完成全量回归、Test262、oracle、平台/API/binary 以及冻结 50+8 和原始 V8；提交可重现 receipt/结果索引和按五项问题填写的证据表。
- **门槛**：默认预算 Earley-Boyer 独立和完整组合有有效结果；第 5/7/9 项有可信改善和反例验证；第 10 项如实归因；控制负载无未解释退化。任何失败都在此处停住，追加对应修复并重验受影响部分。
- **边界**：这是新入口的切换资格，尚不是仅靠旧默认入口得到的成绩；不新增 skip、不改冻结预期、不删 benchmark。此提交不能写成预期将通过的 receipt。
- **依赖**：C28，以及实际需要的修复/有条件优化。

### C30 · `refactor(runtime): make the new execution core the default`

- **改动**：CLI、API、模块、eval、dynamic Function、host、binary 和 conformance 默认入口统一切新核心。删除实验路由选择，不按输入或测试选择引擎。
- **验收**：默认入口实测身份；完整语义门槛在默认新核心上确认，平台集成通过；正式性能确认没有入口开销或配置差异。区分 C29 新核心 receipt 与本次默认入口 receipt。
- **依赖**：C29 通过；禁止先切换再留下未完成语义作为后续 PR。

### C31 · `refactor(engine): remove legacy execution and obsolete ownership paths`

- **改动**：删除旧普通递归执行器、VmHost local/argument 协议、重复 operand/activation、迁移桥接和旧专用测试路由；旧内部栈 compiler 路径退出。外部格式所需的独立翻译按 C26 保留。
- **维护**：更新责任文档、模块 README、架构检查和有效 canary。检查残余 owning/raw 值、无 owner root 和重复 Number 实现；历史 baseline 可由 commit 重建。
- **验收**：完整新默认构建/相关回归、删除路径的依赖扫描；第 11 节四种维护操作可追踪；不存在必须保留旧 VM 才能通过的测试。
- **依赖**：C30；本 PR 必做，不留“以后清理”的生产双栈。

### C32 · `docs(perf): record final vm architecture and issue results`

- **改动**：以最终引擎源码/普通 ELF 完成最终固定矩阵、原始 V8、profile、native 栈/内存报告；提交双重验收表、复现命令、哈希和覆盖限制，重写 PR 描述为最终结果。
- **证据**：计时、perf 与构建/测试串行；正式 5 轮交错、敏感/回归项追加 10 轮与独立计数；默认栈有效成绩不能由预算诊断补算。最终源码因清理改变时，不能把前一 ELF 当最终 ELF。
- **验收**：六项架构能力和五项问题逐项有证据；相关完整语义验证对应最终生产代码；所有未解决项明确列出，未达门槛则 PR 保持未完成。报告提交只改文档时明确引擎 source commit 与 PR head 的区别。
- **依赖**：C31 和所需最终修复。复用仍有效且配置一致的语义证据，只有受变更影响或存在疑点才重复扩测。

## 8. 有条件的类型块提交

C28 后先用完整新核心 profile 作决定；类型块是可选层，显式帧/根/恢复能力不依赖它。不选择类型块时，在 C29 记录原因即可，不视为遗漏必做架构。

若数据支持在本 PR 加入，则在 C29 前插入：

- **X01 · `feat(vm): execute bounded numeric plans with baseline state maps`**：版本预算、输入 guards、计划 owner、明确 side exits；操作算法复用共同 Number 规则。强制每个退出点与 baseline 对照；GC/call/debug/suspend 前物化，不做 JIT/OSR。
- **X02 · `perf(vm): retain only measured numeric block specializations`**：热点计数与有限版本选择，冷热/类型抖动/真实负载 A/B，量化 plan 构建/内存；只保留有可信净收益的实现。若失败，撤掉实验实现，结论写入报告，最终不保留无效层。

引入 X01/X02 后，C29–C32 的语义与性能门槛覆盖 baseline 和专化出口，最终普通配置也必须验收。不能让可选优化拖成另一个尚未关闭的生产执行架构。

## 9. 问题、架构与 commit 的对应

| 验收对象 | 主要实现提交 | 最终证明 |
| --- | --- | --- |
| #16 第 1 项：native 栈 | C09、C11、C25、C27 | C29/C32 默认 Earley、原始组合、递归、小栈与重入 |
| #16 第 5 项：数值流程 | C03、C06、C10；有条件 X01/X02 | C29/C32 数值/真实负载耗时与指令、完整转换语义 |
| #16 第 7 项：调用成本 | C07–C09、C12、C27 | C29/C32 吞吐/分配/保留内存及调用语义 |
| #16 第 9 项：局部更新 | C05、C06、C12、C13 | 发布码/诊断及 C29/C32 循环结果 |
| #16 第 10 项：PC/帧管理 | C02、C06、C11、C28 | 独立归因、观察契约及最终回归 |
| 直接槽内核 | C03–C06、C14 | C30 默认入口，C31 旧 local 协议退出 |
| 显式帧 | C09、C15、C25、C27 | 调用链/状态测试，C31 普通递归路径退出 |
| 值与 roots | C03、C07、C08、C20、C25、C27 | 强制 GC、weak、挂起/回复和宿主根 |
| 统一异常/恢复 | C10、C11、C17、C21–C24 | 转换/内置/generator/async/模块共同驱动 |
| 数据流与发布 | C04、C05、C12–C14、C26 | verifier、source/live maps、外部格式契约 |
| 可维护结构 | 全程，C31 收敛 | 唯一 owner、依赖边界、四种功能修改路径 |

## 10. PR 合并前检查

1. C01 能力清单全部有新核心 owner；default entry 和实际测量入口一致。
2. 架构能力全部完成；具体算法选择可以变化，但不能通过删掉能力换取完成。
3. 本轮五项问题有明确结果。PC 假设的否定结论需有独立诊断；其他问题未解决则继续修复。
4. 正确性、性能、资源和平台证据对应最终代码；报告没有混合普通/诊断 ELF 或不同源码。
5. 旧重复执行器与桥接已退出；独立 binary 格式适配不会偷偷执行旧 VM。
6. 最终 PR 说明问题和行为变化、架构决策、验证、实际收益及限制。保持 issue #16 其他未覆盖条目开放，不用 `Closes #16` 误关闭整份优化跟踪。

单 PR 是交付约束，逐 commit 是实现和审查组织方式。中间切片不是单独合并目标；最终同时交付架构和问题解决结果。

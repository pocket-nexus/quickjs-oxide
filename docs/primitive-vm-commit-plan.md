# 栈 VM：一个 PR 内的 10 个 commit

状态：2026-09-12，S01 实施中，尚无完整计划提交通过验收。一个 PR 按 **S01–S10 共 10 个提交**交付架构、代码结构、完整语义迁移和 #16 的五项验收；以下编号表示计划中的提交，不表示已有实现。

目标见[架构计划](primitive-vm-plan.md)，目录与算法见[实施设计](primitive-vm-implementation-plan.md)，能力和结构验收见[迁移清单](primitive-vm-migration.md)。

## 当前实施记录

- S01 进行中：共享 scope、binding 与线性操作模型已迁入 `compiler/model/{scope,bindings,ir}.rs`，保持原有字段、声明顺序、栈效果与绑定判定；resolution/lowering 等已有显式导入的消费者改从模型所有者导入。类型可见性限于 compiler，未增加公有 API。
- 本次是结构移动，复用现有编译器行为测试，不新增文件形状镜像测试。跨模块现状同步到 architecture.md。
- S01 未完成项：FunctionIr 与 parser context/builder 生命周期分离、其余生产通配导入清理、flow/relocation 与可选分析预算核对、诊断入口和完整 oracle 验收。S02–S10 均未开始；不能把模型拆分视为新执行核心覆盖。

## 1. 提交顺序与关口

| 提交 | 完整交付单元 |
| --- | --- |
| S01 | 编译器结构、线性栈 IR 流程与基础诊断 |
| S02 | 指令契约、验证流程、发布与 executable 布局 |
| S03 | 帧/槽所有权、纯 Number 算法与栈执行核心 |
| S04 | 显式调用、转换恢复、异常与完整绑定 |
| S05 | 属性及同步内置 JS 回调迁移 |
| S06 | generator、async/Promise 与 async generator |
| S07 | 模块、宿主、API 与二进制入口集成 |
| S08 | 局部指令融合与 PC 观察边界 |
| S09 | 调用存储优化与同架构布局实验 |
| S10 | 完整验收、默认切换、旧路径清理与结果文档 |

- **A：S01–S03。** 编译/发布边界、帧/槽所有权和原语栈核心可运行、可测量。
- **B：S04–S07。** 普通调用、内部回调、挂起与全部既有入口完成新核心覆盖。
- **C：S08–S10。** 在完整语义上优化，完成 #16 验收并切换、删除旧路径。

每个提交中的实现、对应测试、源码契约及相关架构说明和结构检查一起完成。语义检查随提交执行，不推迟到 S10；诊断样本不充当正式性能成绩。临时桥明确记录覆盖；经过旧 VM 的样本不能宣称新核心已覆盖，禁止按测试或 benchmark 名选择路径。

## 2. 编译与执行基础

### S01 — `refactor(compiler): organize stack compilation and diagnostics`

把既定栈架构、基础诊断与编译器结构整理合在一个提交：

- 明确本 PR 的五项问题、状态所有者和模块边界，采用线性栈 IR、有限局部优化与现有 RC/循环回收。
- 将 compiler 的共享模型按 ir/bindings/scope 归属；parser 的 context/builder 区分临时解析状态和后续产物。复用已有 resolution/lowering，保持完整语法、hoist/capture/eval 与声明顺序。
- 整理 resolved IR → 栈码草稿、块边界、正常/异常/恢复栈状态及片段重定位。消费已有存储；可选分析超预算时不使用未收敛事实。
- 建立最终码、编译阶段和执行成本的诊断入口；先接现有基线，新 frame/slot 路径的计数在 S03/S04 接入同一口径。复用 benchmark harness，正式热路径关闭诊断。

**验收：**编译器/语法/绑定 oracle、TDZ/private Reference、try/finally、恢复与源码位置用例；错误顺序保持。显式导入、源码契约和架构说明同步，不为文件移动增加镜像测试。不恢复 Tachyon 工具或历史结果文件。

### S02 — `refactor(code): unify stack contracts verification and publication`

形成一套可由 compiler、VM 和外部格式共同消费的代码契约：

- 统一指令栈效果、潜在效果和操作数描述。保留不同验证层分别证明的事实，不能删掉动态恢复检查。
- 把发布验证大函数按参数/绑定、控制流、私有状态、模块与函数树拆分；长 tuple 改成命名工作项，保持检查顺序，共享必要上下文，避免重复扫描整树。
- 分开纯验证与 Atom/heap 发布事务，保留已有 VerifiedFunction。发布输入归 code，编译请求编排归 api，生产验证不再依赖 compiler::EvalCompileContext。
- 定义 FrameLayout、code/sites/handlers 与 Runtime bindings 的边界；每帧通过一个 executable owner 取得 metadata，复用已有共享数组。

**验收：**真实发布、外部格式、畸形 code、mutation 反例与失败回滚；合法参数/局部/跳转范围不缩小。测试按语义组织，物理归属及源码检查同步迁移，不以只换哈希代替验证。

### S03 — `perf(vm): build the owned stack execution core`

把新存储和首个实际消费者一起交付：

- 建立 RunningExecution、FrameStore、SlotStore、typed binding、冷状态、运行登记 guard 和资源限额。按 max_stack 预留窗口、复用容量，定义 move/copy/clear；区分原始实参与可写形参。
- 从 host_bridge 提取已有绑定共享读写/捕获/关闭规则，建立运行 owning 值与长期挂起 raw 边的交接契约。测试构造、调用和恢复保留各自验证。
- 复用现有 pow/ToInt32，按 operations/integer/format/float16 整理纯 Number 算法；对象转换与栈操作各有所有者。
- 一个主 match 执行字面量、栈操作、普通局部、确切 Number 和静态分支。冷 payload 由 pending 拥有，RunExit 保持小型；经证明无分配/回收/回调的槽引用事务可留在循环内。

**验收：**区间互斥、扩容后无旧引用、逻辑 pop 的引用处理、返回最后引用、Runtime owning cycle 防护；zero queue 容量预检和事务不重复提交。验证 NaN、±0、溢出、位移、乘除余幂及 BigInt/String 慢路，记录 #5 路径和初步成本。调用/回调的未迁移范围明确归 S04–S07。

## 3. 调用与完整语义迁移

### S04 — `refactor(vm): drive calls conversions and unwinding with explicit frames`

在同一 driver 中完成普通调用与一条完整回调/恢复流程：

- 调用请求拥有 callee、receiver、实参、realm 和正常/异常恢复点；ordinary/bound/constructor 调用通过显式帧 push/pop，完整处理 this、new.target 与 derived return。
- ToPrimitive 状态和 step 归 value/conversion；建立 parent operation、单次回复和恢复阶段。一条 getter/valueOf 回调完整通过新 driver，其余领域调用点归 S05。
- unwind 统一安排 catch/finally、break/continue、IteratorClose 和 return/throw，保留各自错误优先级；关闭 capture 后才清帧，pending 和结果始终有明确 owner。
- 接通 lexical/const/TDZ、closure、每迭代 cell、默认参数、mapped/unmapped arguments、direct eval/with、private 与 readonly view。建立真实 host 重入的 delimiter 骨架，回调前结束内部借用。

**验收：**有限递归、小栈、可捕获无限递归、缺少/额外实参、跨 realm、嵌套 finally 和清理抛错；getter 次数与转换顺序。验证 `x+(x=2)`、回调改绑定后抛错，以及 `function f(a){'use strict';a=2;return arguments[0]}` 保留原实参。统计调用分配、初始化和引用记账；保留真实 native 栈保护。

### S05 — `refactor(runtime): resume synchronous JavaScript callbacks through the driver`

按领域完成同步回调迁移，保留逐调用点清单：

- **object/Proxy：**Get/Set、getter/setter、Object/Reflect、trap 与 receiver；复用现有属性存储内核。
- **Array/iterator：**callback、sort、species、迭代与 close；状态保存阶段、下标和必要值，回调后按语义重读可变化内容。
- **String/RegExp/buffer：**replacement 和 TypedArray/buffer 参数转换；回调后重新取得 view/buffer 凭证。
- **其余同步内置：**逐项核对 Function、scalar、Math、collections、Date、JSON、Error 和 globals。无回调 helper 保持普通函数。

**验收：**trap invariant、holes/原型访问器、修改 length、排序/迭代副作用、IteratorClose、Unicode/零长度匹配、resize/detach、共享内存、BigInt 与 GC；保留 PR19 的 Array/TypedArray 回退。全部内部同步回调由 driver 推进，不递归等待 JS，也不假装成外部 host。默认预算原始 Earley-Boyer 进行新核心覆盖筛查；异步/模块/API 余项明确归 S06/S07。

### S06 — `refactor(vm): unify suspension across generators and async execution`

共用 frame/stack/control 与 freeze/thaw，分清各语言状态机：

- **generator：**初始状态、next/throw/return、yield/yield*、finally 与 reentry。
- **async/Promise：**同步前缀、await、thenable assimilation、job 恢复；保持已有微任务顺序与 draining 政策。
- **async generator/iteration：**请求队列、各 Promise capability、异步迭代/close，以及 finally 内 await。

挂起时源 owner 保活到目标 raw 边全部发布；恢复时 heap owner 保活到运行 roots 和状态验证全部完成。关闭、失败和被放弃状态有唯一释放责任，不永久注册 owning wrapper 或全局 root。

**验收：**强制 GC、最后引用、半转换失败、单次 completion、重复恢复拒绝、交错请求、yield*、私有/捕获状态与关闭回收。内部 poll 不新增调度时机；不引入 #20 的 Fiber 调度或新的可挂起 host ABI。

### S07 — `refactor(api): integrate modules host and binary entries with the stack driver`

完成全部既有生产入口的可测新核心路径：

- 模块 link/evaluate、live imports、循环模块、TLA、dynamic import 与 loader 失败/重入；保留模块自身状态机和特殊 linking 验证。
- Context、native/web adapters、CLI 和已支持的二进制入口接入统一 driver；外来 code 翻译后进入统一验证。
- 真实同步 host 边界保留 ABI，以 delimiter 和 native 栈预算处理 JS→native→JS；正常、异常及 unwinding 退出均解除运行登记。
- 审核 S05/S06 留出的全部调用点，收口编译请求与 code 发布的责任；测试和文件归属检查跟随真实入口。

**验收：**模块声明/初始化顺序、TLA 错误和 realm、binary fixtures/round trip/畸形输入、wrong-runtime 拒绝、平台与宿主重入。测试配置能让全部入口使用新核心；默认切换和旧桥删除在 S10。

## 4. 优化与最终交付

### S08 — `perf(vm): fuse stack operations and publish precise observation state`

在完整绑定、异常和恢复流程之上合并执行开销优化：

- 有限 UpdateLocal、CompareBranch 和保持读取顺序的局部融合；discard/prefix/postfix 共用算法，复用 S01/S02 的效果、栈、重定位和源码契约。
- 热循环维护局部 pc/sp，在准确观察点直接向已知 FrameId 发布 fault/resume 和语义阶段 site；覆盖 debug/hook、GC/release、interrupt/fuel、host 与挂起。
- 融合和 PC 发布分别诊断、分别 A/B，再进入同一提交。原逻辑操作的 fuel 权重与调试契约保持，不能盲目每 N 条才更新 PC。

**验收：**`x += g()` 不推迟旧 x 读取；postfix 返回转换后的 Numeric；const 更新在转换后 PutValue 报错；NaN 下不混淆否定 `<` 与 `>=`。最终码、动态分派、fault/resume/backtrace 和恢复状态正确，取得 #9/#10 独立证据；PC 成本不显著时如实记录。

### S09 — `perf(vm): reduce call storage costs and tune stack layout`

优化完整语义下的调用和存储成本：

- 改进 frame 热冷布局、容量复用、metadata owner 与独占 outgoing 参数区；区分必要复制、冗余保活和初始化，保持 arguments/eval/capture 的正确关系。
- 在同一栈 VM 内逐项评估规范内存栈与栈顶缓存、typed enum 与紧凑码，以及必要的热冷拆分。一次隔离一个变量，记录去留依据。
- 缓存值 owns/moves、分支入口以及异常/观察/挂起物化均有明确契约；无可信收益或维护成本过高的实验删除，不留下闲置选项。

**验收：**#7 的分配、retain/release、初始化、峰值/活跃槽与吞吐；暂停区段不可复用，不靠保留死值制造低成本。比较 code bytes、.text、实际 native frame、编译时间和 native/WASM 成本；实验不重新打开 ISA/SSA/GC 选型。

### S10 — `refactor(vm): finish validation and retire the previous execution path`

按顺序完成同一提交的最终交付：

1. 新核心配置运行相关 QuickJS oracle、完整回归/Test262、native/Web/WASM、原始 V8 与固定 50+8，核对编译/内存/调用/暂停成本及全部能力清单。
2. 验收通过后切换全部默认入口，删除旧 VmHost、重复 activation/帧投影、旧驱动与迁移桥，清理通配导出和无用实验。
3. 在切换和删除后的最终源码上重新完成相应检查及正式发布验收，记录源码/构建身份。固定工作量交错至少 5 轮，敏感项 10 轮；诊断与正式计时分开。
4. 更新架构说明、源码契约、实际 commit 导览和 #1/#5/#7/#9/#10 的真实结果。完成新增 Number、转换顺序、异常/恢复、binding/eval 四种维护演练。

**验收：**默认预算原始 Earley-Boyer 独立及组合、小栈、有限/无限递归和重入通过；新核心完全覆盖且旧路径退出。不增加 skip，不按测试名 fallback，不改冻结预期掩盖回归，不用旧 receipt 替代本轮结果。#16 的其余优化与 #20 的 Fiber 调度不随此 PR 自动完成。

## 5. 审查规则

正式 PR 以这 **10 个完整提交单元**组织。文档、诊断、测试和局部结构调整并入其所属单元；开发过程中的临时提交在整理 PR 时归并，避免把每个 helper 或修正重新扩成独立的计划 commit。阶段验收可以发生在一个提交内部，验收顺序与证据仍需记录。

代码结构任务按[实施设计第 15 节](primitive-vm-implementation-plan.md#15-代码结构的独立改进清单)验收；接口、类型、算法和反例在同一提交可审查。较大提交按本文件的领域条目组织审查说明，保留逐调用点账本。文件移动与语义改变在 diff/说明中清楚标识，不引入只有占位抽象、没有消费者的提交。

每次增加恢复路径同时完成 roots、异常和释放责任；构建、测试、正式计时与诊断分阶段串行。架构、结构和 #16 问题验收同时满足后才完成本 PR。

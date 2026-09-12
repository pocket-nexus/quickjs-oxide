# 栈 VM：架构迁移与验收账本

状态：2026-09-12。用户已确定使用栈 VM，**全部实现项待做**。本表与[架构计划](primitive-vm-plan.md)、[实施设计](primitive-vm-implementation-plan.md)、[S01–S10 逐 commit 计划](primitive-vm-commit-plan.md)共同定义一个 PR 的交付。提交合并后，能力与结构条目仍逐项验收。

## 1. 起点与范围

- PR19 基线：`c52d4dc7747641756dff8cb7159b9885e9cc8b17`，生产 Rust 源码与历史调查的 `1cc51bb5fcc5c36912d3197d877219ae513dc4b5` 一致。
- 当前分支 `vm/primitive-execution-core`；PR #21 基于 `perf/ordinary-property-kernel`，不称为默认分支。
- 目标是栈 ISA、线性栈 IR、有限局部改写、显式普通调用与内部 callback、集中执行槽所有权和准确观察边界。
- 本轮维持 RC/循环回收，明确运行 owning 值与长期挂起 raw heap 边交接。GC 重写、SSA/寄存器后端和 #20 的 Fiber 调度不属于此账本。
- 暂存/部分新入口通过不等于迁移完成。每个条目最后填写真实入口、提交、验证身份和剩余路径；现有历史 receipt 不改成新核心成绩。

## 2. 外部契约

| 表面 | 保持的契约 | 主要验证 |
| --- | --- | --- |
| Runtime/Context | 身份、realm、同步 compile/eval/execute/call/construct、byte-source | api/context/realm、Rust API 用例 |
| 值与生命周期 | owning handle 保活、clone/drop、wrong-runtime 拒绝；raw Clone 不负责对象边 retain | root/heap/value、强制 GC、最后引用与挂起交接 |
| 参数与 binding | actual arguments 与可写形参、mapped/unmapped、TDZ/const/cell/private/eval | 参数、闭包、eval、scope/private 语义测试 |
| host | 同步 callback ABI、JS→host→JS、回调前无内部借用 | native/web adapter、delimiter、异常退出清理 |
| 资源限制 | native 栈守卫、VM 帧/槽限额、中断/fuel、内存超限 | 默认/小栈递归、可捕获错误、融合前后预算契约 |
| debug/error | fault/resume、语义阶段 source site、backtrace、realm | code/debug、runtime trace、调试模式融合规则 |
| jobs | async 同步前缀、Promise assimilation、宿主 draining 与任务顺序 | Promise/async/modules traces |
| binary | 已支持外部 QuickJS 格式、round trip、畸形输入拒绝 | binary fixtures/翻译/发布、C oracle |
| 平台与 API | safe Rust/MSRV、原公有 API 边界、CLI/native/web/WASM | workspace/平台检查、[架构说明](architecture.md) |
| 一致性 | 完整既有语言支持和诊断，不加 skip 或重写冻结预期 | 完整回归/Test262 与相关 oracle |

内部结构允许重做，外部行为变化不因整理架构自动获得授权。

## 3. 状态所有权验收

下表同时包含结构改进和必须保持的不变量；其中的错误情形是迁移防护要求，不表示现有实现已经存在这些错误。

| 状态 | 唯一责任 | 目标不变量 | 状态 |
| --- | --- | --- | --- |
| 编译中的 scope/binding/IR | compiler 模型与解析阶段 | lowering 消费已解析身份，复用 binding 事实 | 待做 |
| code/layout/sites/handlers | code 发布对象 | 继续统一验证后发布，保留已有不可变共享 | 待做 |
| 当前 frames/pc/sp/slots | RunningExecution 与 frame/stack 模块 | 一份动态状态；host/观察表引用其身份和已发布视图 | 待做 |
| 参数与局部 owning 值 | SlotStore/binding 布局 | 复用调用容量；原始实参与可写形参保持正确关系 | 待做 |
| 语义 callback 进度 | value/object/builtins 中的领域状态 | 恢复点显式，VM 不复制领域算法或递归等待内部 JS | 待做 |
| operation 调用与回复 | VM operation 登记及 driver | 正确 owner、单次回复、getter 不因恢复重复执行 | 待做 |
| abrupt completion/cleanup | unwind 与有效控制区域 | 统一展开协议，保留各项语言清理优先级 | 待做 |
| 长期挂起状态 | heap 原始记录；suspend 负责事务交接 | 无永久 Runtime-owning 环、半恢复或最后 root 丢失 | 待做 |
| host 重入观察 | 运行登记 guard/delimiter、已发布状态 | 回调前结束借用、视图准确、退出后解除登记 | 待做 |
| Number 语义 | value/number | 运行与折叠共用纯算法，通用转换保持独立责任 | 待做 |

代码结构独立验收如下，证据与拆分规则见[实施设计第 15 节](primitive-vm-implementation-plan.md#15-代码结构的独立改进清单)。

| 结构任务 | 提交与验收 | 状态 |
| --- | --- | --- |
| compiler 入口、共享模型、解析临时状态 | S01；复用现有阶段，消费式交接，不生成新的巨型 model | 待做 |
| host_bridge 的绑定/构帧/挂起/领域操作分归属 | S03–S07；已有读写 helper 复用，测试/恢复特殊验证保留 | 待做 |
| 验证大函数、长 tuple 与发布流程 | S02；命名工作项、明确检查顺序，已有 VerifiedFunction 和反例保持 | 待做 |
| code/compiler 反向依赖 | S01–S02/S07；发布输入归 code，编译请求编排归 api，生产验证不导入 compiler | 待做 |
| 显式导入与 Number 文件职责 | S01–S03/S10；imports 可追踪，运算/转换/格式化分责，公有边界不扩大 | 待做 |
| 测试、物理归属检查与架构/源码契约 | 随迁移更新，S10 收口；旧反例和 mutation 有效，无失联检查 | 待做 |

## 4. 能力迁移账本

| 能力 | 目标责任/提交 | 关键验证 | 状态 |
| --- | --- | --- | --- |
| 完整语法与名字解析 | parser/model/resolution，S01 | hoist、eval、class/private、错误顺序；无数字子集前端 | 待做 |
| 栈 lowering/控制流/发布 | lowering/flow/code，S01–S02 | 合流栈形状、异常/恢复、TDZ、源码重定位与畸形 code | 待做 |
| Frame/Slot 容器 | execution/frame/stack，S03 | 区间独立、容量复用、clear/move、原始实参、运行登记 | 待做 |
| 栈原语与 Number | number/run，S03 | Int/Float、NaN/-0、BigInt/String 慢路、目标旧引用释放 | 待做 |
| 普通调用/constructor | call/driver，S04 | 限额、参数/this/new.target/realm、bound、derived return | 待做 |
| 转换/异常/finally | conversion/operation/unwind，S04 | getter 次数、不同抛错点、清理优先级、单次释放 | 待做 |
| binding/eval/arguments | resolution/bindings，S04 | captured、每迭代 cell、mapped/unmapped、private/readonly | 待做 |
| 局部 update/条件融合 | optimize/run/code，S08 | 快照、prefix/postfix/discard、NaN、效果/site/预算 | 待做 |
| properties/Proxy | object + 对应 builtin，S05 | receiver、trap invariant、递归 getter、PR19 回退用例 | 待做 |
| Array/iterator | 对应 builtin，S05 | holes、species、sort、动态 length、IteratorClose | 待做 |
| String/RegExp/buffer | 对应 builtin，S05 | replacement/Unicode、resize/detach、共享内存与 BigInt | 待做 |
| 其余同步内置 | 各领域 owner，S05 | intrinsic 逐项审核、toJSON/replacer、修改中迭代、realm | 待做 |
| generator | suspend + generator 驱动，S06 | next/throw/return、yield*、reentry、关闭/失败/GC | 待做 |
| async/Promise | suspend/async/jobs，S06 | 同步前缀、assimilation、微任务次序、pending roots | 待做 |
| async generator/iteration | 专用队列与恢复，S06 | 交错请求、finally await、异步 close | 待做 |
| modules | modules + driver，S07 | cycles/live import、TLA、dynamic import、loader 重入 | 待做 |
| API/host/binary/platform | 入口适配与统一验证，S07 | 所有入口、delimiter、round trip、畸形输入和平台 | 待做 |
| PC/observer/interrupt | observe + run，S08 | fault/resume、GC/release、host/debug、融合 fuel 权重 | 待做 |
| 调用与布局优化 | call/frame/stack，S09 | 原生帧、峰值槽、复制/retain、缓存物化、编码成本 | 待做 |

S05 收口同步内置调用点并列明异步/模块/入口的后续责任，S07/S10 审核全部内部 JS 调用。真实 host 同步边界必须能指出实际 embedder callback，不能用来藏未迁移内部递归。

## 5. 最终完成标准

| 项目 | 必需证据 | 状态 |
| --- | --- | --- |
| #1 | 默认预算原始 Earley-Boyer 独立/组合；有限/无限递归、小栈与 JS→native→JS | 待做 |
| #5 | Number 实际直接路径、完整语义、算术/Crypto/Navier-Stokes A/B | 待做 |
| #7 | 分配、初始化、retain/release、峰值/活跃槽及调用吞吐；参数/捕获语义正确 | 待做 |
| #9 | 最终码、动态分派；更新/条件融合的快照、错误和 source site | 待做 |
| #10 | 独立成本归因与完整观察点；成本不显著也如实记录 | 待做 |
| 架构 | 小型协议、一个 driver、一个热分派、领域状态就地归属、清楚拥有/恢复 | 待做 |
| 可维护性 | 新 Number、转换顺序、异常/恢复、binding/eval 四种修改演练 | 待做 |
| 完整验证 | 50+8 固定、原始 V8、相关 oracle、全回归/Test262、native/Web/WASM | 待做 |
| 旧路径退出 | S10 完成后所有默认入口只进新栈核心；VmHost/重复动态帧和迁移桥删除 | 待做 |

S10 内先完成新核心全量验收，再默认切换和删除旧路径，最后在最终源码上重新核验并更新真实实现文档；这些顺序合并到同一提交，验收记录仍分开。构建/测试/正式计时/诊断分开，不拿旧 receipt 或无优化代码的历史成绩填本表。本轮不关闭 #16 其他问题，也不声称完成 #20 的 Fiber 调度与取消。

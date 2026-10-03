# 现行执行路径

本页描述当前源码中的执行机制。对应的主要文件是 `src/engine/compiler/flow.rs`、`src/engine/code/exec.rs`、`src/engine/vm/execute.rs`、`src/engine/vm/stack/transfer.rs`、`src/engine/object/ordinary_storage/ic.rs` 和 `src/engine/vm/property_driver.rs`。

## 编译与发布

编译器先生成栈指令，并在降低阶段使用临时数据流和效果信息选择有界数值操作。发布阶段验证指令宽度、边界、控制流目标、栈容量与操作描述符，再生成单一 `ExecCode` 字流。数值区域保留普通指令作为准入后的通用执行入口。运行时从已发布的执行字和元数据读取操作数、目的地与 continuation 位置。

当前数值操作包括局部累加、乘积直接写入、数组元素复制、前增索引后的加法、数组与局部联合写入、现有数组元素更新和比较分支。选择器识别对应的有限指令形态；执行时验证实际 Number、数组元素及绑定状态。成功路径在准入的帧或数组访问中完成计算和写入。比较操作直接选择分支位置。

发布器还为部分普通指令生成同宽或有界片段的快速入口：局部与参数 Number 自增、密集 Array 元素读取、直接绑定的数组读取及索引更新、Number 比较后直接跳转，以及属性或数组读取后更新数值局部变量。这些入口在运行时检查值、绑定、数组存储与输出容量；成功后跳至已验证的片段末尾，未满足准入条件时从原有栈指令位置继续。`PutArrayEl` 对符合条件的标量元素在执行循环内写入，其他状态交给属性写入语义处理。

## 普通绑定写入

`Put`、`Set` 和初始化使用 `FrameSlots` 的所有权转移入口。`Put` 与初始化消费操作数 owner；`Set` 保留操作数并为目的地取得所需 owner。目标槽和操作数在同一帧窗口借用中验证。目标原值分为未初始化、无 owner 的标量和有 owner 的值；释放需求决定局部完成或观察边界。参数与局部直接绑定复用同一转移实现。词法状态、捕获绑定和特殊绑定由相应语义操作处理。

## 命名属性读取

发布的属性站点持有静态键和读取缓存。`ordinary_storage/ic.rs` 从当前对象与原型链选择数据值、访问器或完整缺失，并在适用时更新位置缓存。缓存记录单形态或双形态的位置事实，也记录访问器位置；多形态站点经过有界冷却后再次尝试记录位置。命中时读取对象当前槽值。冷站点在遍历时选出的数据值直接交给消费者，不再从新建的位置缓存重复读取。完整缺失按本次查找结果消费。

普通 `GetField`、`GetField2` 和直接局部或参数绑定的 `BorrowedField` 入口复用上述选择流程。数据值在堆借用期间复制或取得 owner；借用绑定保留原绑定中的接收者。数组下标读取使用独立的密集元素准入路径。

`execute_frame` 在当前认证的帧窗口中完成符合条件的命名数据读取。选中的访问器以拥有式状态交给属性驱动器；一般属性语义、回调和可观察清理由驱动器继续。驱动器返回后使用对应的 fault、fallthrough 和 resume 位置继续执行。

## 帧、所有权与生命周期

`SlotStore` 为参数、局部和操作数提供窗口；`FrameTransaction` 与 `FrameSlots` 约束短时访问。调用输入、帧函数与捕获存储持有原始堆 ID，使用当前状态显式退休；放弃执行由 Weak 注册兜底。片段取得一次 `RuntimeState`，通过 `FrameExecution` 借用实际当前帧与窗口。普通调用、返回、tail 退休与 Base 构造在同一 opcode 循环继续，使用携带的 fallthrough 与窗口事实，保留容量、占用与动态目的地检查。

pending 操作持有跨边界所需 owner。未迁移的对象、native、eval、发布与挂起 helper 结束片段借用后继续，其边界在 profiling 构建中计数。ready driver 对剩余 `VmAction::observes_activation` 动作执行物化；已物化帧在这一步发布当前 PC。公共 API 取得状态与认证外部 root，panic 或破坏性清理失败会隔离 runtime，后续入口拒绝继续使用它。

捕获变量与形状分别存于类型化 arena。每个 ID 包含完整代际身份；引用释放、零计数队列和显式回收沿用同一堆生命周期规则。详细存储契约见[类型化存储](typed-arenas.md)。

## 自动循环回收

循环节点发布消费统一的净增长预算，普通引用计数清理按批次返还预算。普通帧安装不分配可收集节点，无需重复轮询；Base 构造先完整发布 receiver、参数与返回 owner，再用已持有状态服务分配压力。剩余 legacy 执行边界与最外层 execution turn 保留服务位置。VM 没有独立的 GC 指令或分支轮询；显式 GC 与自动 GC 使用同一个完整回收器。预算、weak roots、重入和借用规则见 [GC 策略](../cycle-collection.md)。

## 验证入口

- 数值选择和回落：`src/engine/compiler/tests/numeric_region.rs` 与 VM 数值测试。
- 普通绑定转移：`src/engine/vm/stack/transfer.rs` 的测试。
- 属性选择与完成：`src/engine/object/ordinary_storage/ic.rs`、`src/engine/vm/stack.rs`、`src/engine/vm/property_driver.rs` 的测试。
- 全局语义：`cargo test --locked --workspace --all-targets` 与 [Test262 指南](../test262.md)。
- 成本测量：[测量方法](measurement.md)。

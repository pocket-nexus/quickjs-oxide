# 现行执行路径

本页描述当前源码中的执行机制。对应的主要文件是 `src/engine/compiler/flow.rs`、`src/engine/code/exec.rs`、`src/engine/vm/execute.rs`、`src/engine/vm/stack/transfer.rs`、`src/engine/object/ordinary_storage/ic.rs` 和 `src/engine/vm/property_driver.rs`。

## 编译与发布

编译器先生成栈指令，并在降低阶段使用临时数据流和效果信息选择有界数值操作。发布阶段验证指令宽度、边界、控制流目标、栈容量与操作描述符，再生成单一 `ExecCode` 字流。数值区域保留普通指令作为准入后的通用执行入口。运行时从已发布的执行字和元数据读取操作数、目的地与 continuation 位置。

当前数值操作包括局部累加、乘积直接写入、数组元素复制、前增索引后的加法、数组与局部联合写入、现有数组元素更新和比较分支。选择器识别对应的有限指令形态；执行时验证实际 Number、数组元素及绑定状态。成功路径在准入的帧或数组访问中完成计算和写入。比较操作直接选择分支位置。

## 普通绑定写入

`Put`、`Set` 和初始化使用 `FrameSlots` 的所有权转移入口。`Put` 与初始化消费操作数 owner；`Set` 保留操作数并为目的地取得所需 owner。目标槽和操作数在同一帧窗口借用中验证。目标原值分为未初始化、无 owner 的标量和有 owner 的值；释放需求决定局部完成或观察边界。参数与局部直接绑定复用同一转移实现。词法状态、捕获绑定和特殊绑定由相应语义操作处理。

## 命名属性读取

发布的属性站点持有静态键和读取缓存。`ordinary_storage/ic.rs` 从当前对象与原型链选择数据值、访问器或完整缺失，并在适用时更新位置缓存。缓存保存位置事实，读取时取得对象当前槽值。选择结果由栈读取或直接绑定读取消费；数据值在堆借用期间复制或取得 owner。

`execute_frame` 在当前认证的帧窗口中完成符合条件的命名数据读取。选中的访问器以拥有式状态交给属性驱动器；一般属性语义、回调和可观察清理由驱动器继续。驱动器返回后使用对应的 fault、fallthrough 和 resume 位置继续执行。

## 帧、所有权与生命周期

`SlotStore` 为参数、局部和操作数提供窗口；`FrameTransaction` 与 `FrameSlots` 约束短时访问。帧保持值的 root，堆记录保持其拥有的边，pending 操作持有跨边界所需的状态。提交前完成准入；提交后的剩余工作沿已选的 continuation 前进。帧物化和 PC 发布由实际观察需求触发。

捕获变量与形状分别存于类型化 arena。每个 ID 包含完整代际身份；引用释放、零计数队列和显式回收沿用同一堆生命周期规则。详细存储契约见[类型化存储](typed-arenas.md)。

## 验证入口

- 数值选择和回落：`src/engine/compiler/tests/numeric_region.rs` 与 VM 数值测试。
- 普通绑定转移：`src/engine/vm/stack/transfer.rs` 的测试。
- 属性选择与完成：`src/engine/object/ordinary_storage/ic.rs`、`src/engine/vm/stack.rs`、`src/engine/vm/property_driver.rs` 的测试。
- 全局语义：`cargo test --locked --workspace --all-targets` 与 [Test262 指南](../test262.md)。
- 成本测量：[测量方法](measurement.md)。

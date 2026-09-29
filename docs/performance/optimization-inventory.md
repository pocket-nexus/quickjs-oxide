# 当前优化机制清单

本页按生产代码中的专用表示、缓存、准入和局部完成机制列出主要优化。每项描述实际进入执行或准备路径的工作。各机制的性能效果须针对负载测量；现有样本见[当前测量](current-measurement.md)。执行顺序与所有权契约见[现行执行路径](roadmap.md)。

## 编译与发布

| 机制 | 当前路径 | 实现 |
| --- | --- | --- |
| 编译期名称复用 | 解析器将名称归入 `NameTable`；解析和降低阶段按 `NameId` 复用已构造的 `JsString`。 | [`compiler/names.rs`](../../src/engine/compiler/names.rs) |
| 紧凑执行字 | `ExecCode` 用 32 位字保存 opcode 与短操作数，宽操作数留在后续字中，由执行入口按需读取。 | [`code/exec.rs`](../../src/engine/code/exec.rs) |
| 有界数值区域 | 数据流与效果选择器发布七类数组与 Number 区域；执行时验证值和存储状态，成功后完成对应片段。 | [`compiler/flow.rs`](../../src/engine/compiler/flow.rs)、[`vm/execute.rs`](../../src/engine/vm/execute.rs) |
| 已发布的快速入口 | 发布器选择 Number 自增、密集数组读取及索引更新、直接绑定读取、比较分支和数值累加片段的专用 opcode。 | [`code/exec.rs`](../../src/engine/code/exec.rs)、[`code/exec_opcode.rs`](../../src/engine/code/exec_opcode.rs) |

## VM 执行

| 机制 | 当前路径 | 实现 |
| --- | --- | --- |
| 即时标量计算 | Number 双操作数运算、增减、立即值真值判断及部分比较分支在执行循环或认证槽访问中完成。 | [`vm/execute.rs`](../../src/engine/vm/execute.rs)、[`vm/stack/number.rs`](../../src/engine/vm/stack/number.rs) |
| 直接绑定转移 | `Put`、`Set`、初始化和重置按目标原值的释放需求处理；Number 到 Number 的直接写入使用专用槽操作。 | [`vm/stack/transfer.rs`](../../src/engine/vm/stack/transfer.rs)、[`vm/stack/number.rs`](../../src/engine/vm/stack/number.rs) |
| 密集数组元素访问 | 合格的数组下标读取、标量元素写入与数值片段在执行循环中完成。 | [`vm/execute.rs`](../../src/engine/vm/execute.rs)、[`vm/stack/window.rs`](../../src/engine/vm/stack/window.rs) |
| 命名属性站点缓存 | 位置缓存验证当前形状、revision 与原型布局；冷查找选出的数据值在同一次堆借用中交给消费者，选中的访问器进入拥有式读操作。 | [`object/property_ic.rs`](../../src/engine/object/property_ic.rs)、[`object/ordinary_storage/ic.rs`](../../src/engine/object/ordinary_storage/ic.rs) |
| 借用绑定读取 | `BorrowedFieldLocal` 和 `BorrowedFieldArg` 从直接绑定借用接收者，与普通命名读取共用选择器。 | [`vm/execute.rs`](../../src/engine/vm/execute.rs) |
| 普通调用准备 | Bytecode 函数对象复用发布代际匹配的认证事实；帧入口可批量初始化已发布为普通状态的局部槽。 | [`vm/call/ordinary.rs`](../../src/engine/vm/call/ordinary.rs)、[`vm/stack/call.rs`](../../src/engine/vm/stack/call.rs) |
| 帧内完成与按动作物化 | `FrameTransaction` 和 `FrameSlots` 支持局部完成；新普通调用帧以未物化状态进入，ready driver 按当前 `VmAction` 分类进行物化和 PC 发布。 | [`vm/stack/window.rs`](../../src/engine/vm/stack/window.rs)、[`vm/call/ordinary.rs`](../../src/engine/vm/call/ordinary.rs)、[`vm/driver/ready.rs`](../../src/engine/vm/driver/ready.rs) |

## 对象与集合存储

| 机制 | 当前路径 | 实现 |
| --- | --- | --- |
| Shape 复用 | 弱 canonical 缓存与属性追加 transition 复用布局；共享 Shape 使用 copy-on-write，唯一 owner 的形状可以原位追加。 | [`heap/runtime/mod.rs`](../../src/engine/heap/runtime/mod.rs)、[`object/shape.rs`](../../src/engine/object/shape.rs) |
| 密集 Array 表示 | Array 用密集元素载荷支持直接访问；符合条件的稀疏布局在定义边界恢复密集表示，长度截断可批量替换布局。 | [`object/array_storage.rs`](../../src/engine/object/array_storage.rs)、[`object/allocation.rs`](../../src/engine/object/allocation.rs) |
| Proxy trap 位置缓存 | 每种 Proxy 内部方法有对应的 handler trap 读取站点；命中时验证位置并取得当下数据槽值。 | [`object/property_ic.rs`](../../src/engine/object/property_ic.rs)、[`object/internal_methods/method.rs`](../../src/engine/object/internal_methods/method.rs) |
| Array/Arguments 参数快取 | 合格的密集 Array 或快速 Arguments 直接按下标顺序构造参数快照，供调用和 `Reflect` 操作使用。 | [`builtins/reflect.rs`](../../src/engine/builtins/reflect.rs)、[`builtins/function/arguments.rs`](../../src/engine/builtins/function/arguments.rs) |

## 值、名称与堆生命周期

| 机制 | 当前路径 | 实现 |
| --- | --- | --- |
| 紧凑标量值 | VM 内部值将 Number、布尔值和 short BigInt 等标量直接存入 `JsValue`；短 BigInt 算术在范围内直接计算。 | [`value/js_value.rs`](../../src/engine/value/js_value.rs)、[`value/bigint.rs`](../../src/engine/value/bigint.rs) |
| String 多种表示 | 字符串使用 Latin-1、UTF-16 平面存储与有界 rope；共享 rope 缓存展平结果，内容 hash 按需缓存，独占平面串可原位连接。 | [`value/primitive.rs`](../../src/engine/value/primitive.rs) |
| Atom 直接编码与复用 | 小的规范整数属性名直接编码；其余键使用 runtime 内的 intern table，常用键可固定，释放的槽可复用。 | [`atom/mod.rs`](../../src/engine/atom/mod.rs) |
| 类型化 arena 与局部释放 | 捕获变量、Shape、leaf 和共享对象使用各自对应的 arena；可证明的非观察释放在局部完成，显式 GC 控制空零计数队列容量。 | [`heap/mod.rs`](../../src/engine/heap/mod.rs)、[`heap/slot_ownership.rs`](../../src/engine/heap/slot_ownership.rs)、[`heap/gc.rs`](../../src/engine/heap/gc.rs) |

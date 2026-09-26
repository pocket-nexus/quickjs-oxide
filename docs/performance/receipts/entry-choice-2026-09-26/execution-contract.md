# 融合入口与普通写入的执行契约

本页复核当前源码中的入口选择及写回边界。它描述已实现行为，不把 `u8` 编码、函数拆分或内联注解定为未来架构。测量和准入结果另见同目录 README；本文没有重新运行引擎、测试或 benchmark。

## 事实在哪个阶段成立

`FusionPlan::build` 在 JS 函数发布时扫描 canonical 指令、控制流入口及候选形态，把每个首 PC 至多一个 flag 放进随已发布函数保存的 `Option<Rc<[u8]>>`。Dense matcher 还用每条指令的 `stack_contract` 验证内部峰值和净变化；旧 S1–S4 候选按各自模式和内部控制流入口检查，不应笼统称作都执行了相同的栈认证。`FusionEntry::local_choice` 只在当前 GetLocal/GetLocalCheck 的当前 `pc.fault` 上分类该 flag；它不保存调用中的值类型，也不缓存到下一次派发。[源码：融合计划](../../../../src/engine/code/fusion.rs#L20-L75)、[Dense 发布认证](../../../../src/engine/code/fusion/dense.rs#L326-L381)。

以下表中的 `p` 是进入当前 GetLocal/GetLocalCheck 时的 `pc.fault`。`ProgramCounter` 在每轮开始令 `fault = resume`；只在命中后设置新 `resume` 并 `continue`，或在整条 canonical 指令处理完后设置 `resume = next_pc`。`Result` 错误及 `RunExit` 离开时，PC guard 发布当下的 fault/resume；可观察释放边界会先发布 fault。因而“未命中”只表示本次候选放弃，不能把后续 canonical 指令或 driver 的副作用也称为零副作用。[源码：PC guard](../../../../src/engine/vm/run/program_counter.rs)、[执行循环](../../../../src/engine/vm/run.rs#L421-L433)、[循环尾](../../../../src/engine/vm/run.rs#L2275-L2335)。

## 五类 LocalFusionChoice

| 候选 | 发布时已定的事实 | 本次执行仍需检查；可能出口 | 放弃候选时与 PC |
| --- | --- | --- | --- |
| `LocalAdd` | 首个可写 Normal local；直接 local 或常量与 `Add`、同槽写回、可选 `Drop` 构成 4/5 条形态。数值常量形态在发布时确认常量为 Number；另有用于字符串桥的形态。 | 数值 helper 重新读当前 local、右操作数及其 Number 表示；成功只写 Number local，`resume=p+4/5`。若数值 guard 失败，`local_add_supported` / `local_add_constant_supported` 还可能选择 `RunExit::AddLocal`，由 driver 完成含字符串等原语加法，并在需要时把错误位置移到 canonical `Add` 或 store PC；其 `Result` 错误直接传播。 | 数值 helper 的 `None` 发生在写回之前。未选桥时从 `p` 执行 canonical GetLocal；选桥时离开 `run` 的 fault/resume 仍在 `p`，随后由既有 driver 负责后续 PC 和异常。不能把桥称为“回落后只执行 GetLocal”。 |
| `Update` | 可写 Normal local，同槽 `Inc/Dec/PostInc/PostDec` 与 `Put/Set`、可选 `Drop`；flag 携带方向、前后缀、是否丢弃和跨度长度。 | `update_number_local` 读取当前 Direct Number；可选结果槽的容量和空位在修改 local 前预检。`Ok(true)` 完成 local 和可选结果提交，`resume=p+3/4`；`Ok(false)` 放弃；`Err` 经 `?` 从当前 fault PC 直接退出，不伪装为 miss。 | `Ok(false)` 无槽或深度改变，仍从 `p` 执行 canonical GetLocal。预检的内部错误也在首次写入前返回；其诊断位置保持当前 `p`，不重复执行跨度。 |
| `CompareBranch` | 两个允许的直接 producer、比较 opcode、`IfTrue/IfFalse`，可选尾 `Goto` 的 4/5 条形态；内部入口检查保留尾 `Goto` 的特殊豁免。 | 当前两个绑定仍须是可直接读取的 Number；handler 只读取并计算比较。命中时 `resume` 是 `If` 目标，或未取分支的尾 `Goto` 目标／`p+4`；无新 Result 或 RunExit。 | `None` 不消费栈、不写 local/heap，`fault/resume` 仍为 `p`，从 canonical GetLocal 开始。 |
| `FieldAdd` | 可写 Normal accumulator、Normal base local、`GetField; Add;` 同槽写回、可选 `Drop` 的 5/6 条形态；Number 是执行期要求。 | 当前 accumulator 必须是 Number、base 必须是 Direct Object；IC 的非拥有读取须命中可直接读取的 Number，不能执行 getter/Proxy/转换；成功只写 Number local，`resume=p+5/6`。无新 Result 或 RunExit。 | 任一 guard 的 `None` 在写回前发生；当前 frame owner 仍持有 base，`fault/resume` 保持 `p`，canonical GetLocal 接管，包括原本需要的 getter/Proxy/异常与 IC warm-up。 |
| `Dense` | 13 种精确短跨度之一；发布器验证 opcode/slot/常量/内部入口、stack peak 与 delta。flag 只传递结构事实，不传递动态 Array 状态。 | 当前 Direct 绑定、Number、非负 Int 索引、真实 dense Array 与其元素、借用权限、canonical peak 容量仍逐次检查。写形态还在提交前预检 `property_generation`。`Some(end)` 才推进 `resume=end`；handler 无 `Result`/`RunExit` 出口。 | `None` 前无槽、深度、owner、heap 或 generation 提交，`fault/resume` 保持 `p`，从 canonical GetLocal 执行。短 `&JsValue`/heap 借用不越过提交、释放、重入或挂起；成功写只覆盖已有自有 dense Number，并在最后递增 generation。 |

flag 为零或不属于 GetLocal 的候选时，`LocalFusionChoice::Canonical` 不执行上述 helper，直接处理当前 canonical GetLocal。`GetArg` 的 Dense 入口仍在自己的臂中，本文五类选择表不覆盖该独立派发。[源码：入口映射](../../../../src/engine/code/fusion.rs#L37-L75)、[GetLocal 分支](../../../../src/engine/vm/run.rs#L1435-L1606)、[GetArg 分支](../../../../src/engine/vm/run.rs#L1872-L1901)。

这些候选的失效边界不同：发布期 opcode/入口事实随不可变函数与 PC 保持，帧槽与值类型只在当前借用中有效；IC、Array 布局、元素和 release readiness 每次使用前重新观察。入口选择没有跨重入的 site cache。对 Dense，历史候选规格把 `None`、`Some(end)`、owner 与错误顺序写为明确事务；当前 handler 仍会读回部分 canonical opcode，不能由本页宣称所有静态重验已从 release 机器码消失。[规格：Dense 不变量](../../numeric-array-spans.md#6-错误生命周期及回退的不变量)、[执行器](../../../../src/engine/vm/run/fusion/dense.rs#L178-L207)。

## 普通 local / argument 写入分类

`direct_write_class` 只读取**当前旧绑定**：旧值为 Direct Int/Float 返回 `Number`；其余 Direct 值调用 `slot_value_release_readiness_jsvalue`，得到 `Ready` 或 `NeedsBoundary`；Captured、Uninitialized 等非 Direct 返回 `Other`。这个证明仅供**同一条 Put/Set 指令**消费。readiness 检查本身不改引用计数；跨 JS 调用、释放、重入、挂起或任何可能改变延迟队列/对象状态的操作后必须重查。[源码：分类](../../../../src/engine/vm/run.rs#L315-L343)、[readiness](../../../../src/engine/heap/slot_ownership.rs#L169-L214)。

| 旧值类别 | 当前写法、owner 和错误顺序 | PC / 生存期 |
| --- | --- | --- |
| `Number` | 若栈顶也是 Direct Number，`store_proven_number_operand` 在检查 operand 后直接写入；`Put` 消费栈顶，`Set` 保留赋值结果。失败则走同一指令的普通 copy/pop 与 replace；被替换的旧 Number 只 drop，不触发 heap owner release。 | 快提交不调用用户代码或释放 owner。当前指令正常完成后才推进 `resume`；缺栈顶等错误由已有路径在当前 fault PC 报告。 |
| `Ready` | copy/pop 新值、replace 旧绑定，再调用 `release_displaced`。该函数只在同一指令的 preflight 后释放，期间只有 move 和可能一次 retain；`Ready` 的定义排除了 drain、延迟工作和 JS 回调。不能把“旧对象释放可安全在槽借用内完成”推广到任何其他时刻。 | 没有跨指令缓存 readiness。错误沿当前指令的 `Result` 路径传播，PC 为当前 Put/Set；完成后推进到下一 PC。 |
| `NeedsBoundary` | 经 `release_outside_slots!`：若帧尚未 materialize，先返回 `RunExit::Materialize`；否则结束槽借用、发布 fault/active PC，再执行原来的 copy/pop、replace 和旧 owner release。这保留了最后 owner 释放、deferred cleanup 或错误可能观察到的边界。 | 在当前 Put/Set 的 fault PC 建立观察点；不能在重新进入帧后复用旧 readiness 结论。 |
| `Other` | 不做上述直接替换，返回 `false`，交给原有 bridge/binding 路径。`PutLocalCheck/SetLocalCheck` 对 Uninitialized 的 TDZ 出口位于分类之前，保持原异常顺序。 | 从当前 Put/Set PC 交给原处理，不先推进 resume，也不把非 Direct 值当成 Number。 |

local 和 argument 两臂共用分类及这四类分支，写入位置分别调用 `replace_local` / `replace_parameter`。上述描述限于当前实现的路径与既有错误传播顺序，**不声称所有内部资源错误都可回滚已执行的普通写入**。[源码：local](../../../../src/engine/vm/run.rs#L1823-L1871)、[argument](../../../../src/engine/vm/run.rs#L1903-L1938)、[Number 提交](../../../../src/engine/vm/stack/number.rs#L203-L250)、[release](../../../../src/engine/vm/run.rs#L290-L305)。既有测试覆盖赋值结果、对象 owner 回落与 `-0`：[VM 测试](../../../../src/engine/vm/tests.rs#L362-L384)；PC guard 的错误/退出行为有[单元测试](../../../../src/engine/vm/run/program_counter.rs#L39-L78)。

## Crypto materialize：已证明的规则与缺少的因果记录

固定上游 crypto.js（commit `2034d98fc8c5f8044e186267593f5d5ea5232caf`）中，`BigInteger` 以 `new Array()` 建立数字数组（58–67 行）。本引擎的空 Array 为 `dense: Some(empty)`；`bnpSquareTo` 先设 `i = 2*x.t`，再以 `while(--i >= 0) r_array[i] = 0` 从高位开始写（430–438 行）。若 `x.t>0` 且目标仍是 fresh dense Array，第一次索引 `2*x.t-1>0`。允许普通自有属性定义时，`index < dense_len` 覆盖、`==` 追加、`>` 触发 `materialize_dense_array`；heap 的 `dense.take()` 令后续 Dense 叶函数看到 `dense: None`，诊断为 `array_materialized`。[Array 初值](../../../../src/engine/heap/object_records.rs#L970-L984)、[写入规则](../../../../src/engine/object/properties.rs#L1449-L1470)、[转换](../../../../src/engine/heap/object_storage.rs#L1245-L1305)、[诊断](../../../../src/engine/object/ordinary_storage.rs#L1312-L1357)。

[历史发布 manifest](../all-dense-6db6bfb0/README.md) 中 `am3` 的 R4 PC 20–24、R2 PC 26–30、R0 PC 51–53，与固定源码的 `this_array[i]&0x3fff`、`this_array[i++]`、`w_array[j]` 顺序相符。修正后的 [profile v2](profile-v2-summary.json) 记录 PC 20/26 各 744,364 次尝试、各 463,618 次命中、各 280,746 次 `array_materialized`；PC 51 有 744,364 次尝试、零命中，均标 `array_materialized`。`bnpSquareTo` PC 24 的 `dense_store` 有 39,672 次尝试、零命中、39,667 次该标签，另有 5 次 `beyond_array_length`。

历史发布后 dump 只收 `am3` 等四函数；profile v2 保留 `bnpSquareTo` 的函数/PC/kind，却没有该 PC 到源码行的 opcode 对照。因此 PC 24 对应高位清零的**精确行号**仍是源码顺序推断。更重要的是，诊断标签只观察失败时的 Array 状态，不记录转换者、目标 ObjectId 或前后 PC。上述源码与规则证明 fresh 目标高位首写的转换机制，尚不能把 Crypto 全部 1,490,963 次 `array_materialized` 逐对象归到 `bnpSquareTo`；`bnpMultiplyTo` 也有倒序写，目标还可能复用。要建立逐对象因果需另有有界的数组身份、旧 dense 长度、写索引、转换 PC 与后续读取 PC 记录，现有 profile 没有。

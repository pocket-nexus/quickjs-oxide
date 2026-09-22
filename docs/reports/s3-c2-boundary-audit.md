# S3-C2 边界审计与接线清单

> 历史快照说明：本文的“未启动/未实现”描述仅对应当时快照。
> 当前 C2–C4 与 B1c/B1d 接线已完成，最新状态见 [B/C 集成实施记录](s3-bc-implementation.md)。

> 日期：2026-09-22。静态核对快照为 `3392e4e95a8c65c700e862e743123558ee290149`。
> 依据：[C 计划 §2.3–§2.4、C2](s3-c-plan.md) 和
> [C/B 开头实施记录](s3-c-b-opening.md)。本文只准备 C2，不实现或启用 TOS。
> **C0 的 E/RSS、有界残余归属、A4 表示裁决仍未关闭，C1 尚未性能接受。**
> 本次未运行 Cargo、测试或 benchmark；下列测试是待实施要求，不是新增通过记录。

## 1. 审计结论与分类

当前没有 TOS；所有 operand 都在 `SlotStore.slots` 中。`FrameTransaction`
只持有 store/window 的独占借用，结束短 `RunSlots` 借用并不结束事务。
因此，未来不能把 `drop(slots)`、`let _ = slots` 或重新调用 `transaction.slots()`
当作已经 flush 的证明。正常观察边界须显式恢复 backing，事务 Drop 只兜底
Result 错误和 Rust unwind。

本审计使用以下分类，**均指 C2 要做的改造，不表示当前已有缓存能力**：

| 分类 | C2 执行规则 |
| --- | --- |
| cache-aware | 读取逻辑 depth/top；缓存非空时不调用只认识 backing 的 operand helper；纯标量成功可以继续驻留 |
| canonical-only | 调用前 spill；helper 期间不重新留下 hole；完成后可在明确位置恢复缓存准入 |
| binding-only | 只访问 args/locals，索引不受 TOS 影响；若随后 dup、release、分配或调用通用 helper，仍按该边界规则处理 |
| exit/observation | 交给外部观察者前 Empty；正常路径显式 spill，错误和 unwind 由事务 Drop 恢复当前已提交栈形 |

建议首版保留独立 `run_window()` 的 canonical 模式，只让事务持有的 cache-aware
facade 参与驻留。`RunSlots` 必须能区分“借用事务缓存”与“canonical 独立窗口”；
不得在 `run_window()` 中创建临时事务再返回借用，也不得给每个短 facade
复制一个独立缓存。具体 Rust 类型待 C2 实施时确定。

状态不变量沿 C 计划：`depth` 是逻辑深度；Cached(d) 的第 d−1 个物理槽为
已认证的 `None`，值只由事务持有；Empty 时无 hole。缓存不给原 capacity
增加一个名额；`live_slots` 统计包括缓存 owner。C2 只准入
Undefined/Null/Bool/Int/Float/ShortBigInt。

## 2. 全部构造入口

### 2.1 实际构造点

搜索 `FrameTransaction {`、`RunSlots {` 得到以下全部生产构造点；不能只改
`run()` 的那一次调用。

| 文件 + 符号 | 当前行为 | C2 接线要求 |
| --- | --- | --- |
| [stack/window.rs](../../src/engine/vm/stack/window.rs) · `SlotStore::frame_transaction` | 唯一 `FrameTransaction` 构造器；先 `check_current` | 初始化 Empty，事务独占窗口；新增 Drop 只纯 move，不 release/分配/借 Runtime |
| 同文件 · `FrameTransaction::slots` | 第一处 `RunSlots` 构造 | 借用同一事务的 TOS；短借用结束不 spill；重复获取必须看到同一缓存 |
| [stack.rs](../../src/engine/vm/stack.rs) · `SlotStore::run_window` | 第二处 `RunSlots` 构造；可独立使用 | 保留 canonical 模式，push 立即写 backing；不制造临时缓存 owner |
| `stack/window.rs` · `SlotStore::with_linked_own_read_selected` | 第三处 `RunSlots` 构造；查询结束后临时借输出窗口 | 保持 canonical；输入查询、输出提交、释放仍分界；测试包装 `with_linked_own_read` 同样处理 |

`with_linked_own_read_selected` 的生产调用点是
[property_driver.rs](../../src/engine/vm/property_driver.rs) · `read_progress_selected`。
它不是 `frame_transaction()` 调用者，但同样能创建 facade，必须列入检查。

### 2.2 六个生产事务调用点

| 文件 + 符号 | 事务内实际工作 | C2 分类及必须处理的位置 |
| --- | --- | --- |
| [run.rs](../../src/engine/vm/run.rs) · `run` | 整个 resident match，反复获取 slots；持有 `ProgramCounter` | cache-aware 主入口；§5 的所有出口和内部观察边界都要闭合 |
| [driver/ordinary.rs](../../src/engine/vm/driver/ordinary.rs) · `enter_selected` | peek callee/argv，域认证，普通/native 分类及参数转移 | 初版 canonical-only；已有 `drop(transaction)` 在安装 child、物化、native 调用前，保留该结构 |
| [conversion_driver.rs](../../src/engine/vm/conversion_driver.rs) · `complete_primitives` | primitive 输入、AddStore、分配结果、替换 local 或 push output | 初版 canonical-only；输入取走后、分配/错误构造/旧 binding release/PC publication 前 Empty；输出恢复也不可在随后的 release 前留下 hole |
| [conversion_driver/local_add.rs](../../src/engine/vm/conversion_driver/local_add.rs) · `complete_local_add` | 直接借 local，可能原地 String append、分配、错误重建、store | 初版 canonical-only；`with_local_add_*` callback 前 spill；保持实际 Add 子 PC、独立 RHS 和常量左次序 |
| [frame_operations/numeric.rs](../../src/engine/vm/frame_operations/numeric.rs) · `try_complete_primitive` | primitive 预检、转移、分配、previous/value 输出或交给 query driver | 初版 canonical-only；新增事务 Drop 后，在 `proxy_get_driver::start_numeric(runtime, execution, ...)` 前显式 `drop(transaction)`；所有继续访问整个 execution 的分支重新检查借用结束点 |
| `property_driver.rs` · `complete_read` | 消费输入、发布 receiver/key/result、释放 discarded/receiver | 初版 canonical-only；两处 `publish_read_result` 后仍有 receiver release；不能把短借用结束当缓存已恢复 |

### 2.3 四个独立 `run_window` 调用点

| 文件 + 符号 | 分类与原因 |
| --- | --- |
| [frame_operations/direct.rs](../../src/engine/vm/frame_operations/direct.rs) · `complete` | canonical-only。profiling/test 的 `ReleaseOperand` 事务，转移后显式 release；不作为普通构建的缓存驻留路径 |
| `frame_operations/numeric.rs` · `commit_output` | canonical-only。pending previous/value 按原顺序提交，后续失败保留已提交 previous；未提交 owner 在窗口外释放 |
| [property_write_driver.rs](../../src/engine/vm/property_write_driver.rs) · `write_progress` | canonical-only。key 转换后一次性取 value/key/base，再 release 与 dispatch |
| [iterator_driver.rs](../../src/engine/vm/iterator_driver.rs) · `apply_next` | canonical-only。iterator region 更新后 push value/done；后续 driver 直接读规范栈 |

### 2.4 测试构造点的迁移清单

测试的重复调用按函数归并，不增加生产准入。既有 canonical 测试仍须保留，
新增 cache-on 变体另走真实 `FrameTransaction`，不能把所有测试自动换成缓存模式。

| 文件 | 现有构造入口/测试组 |
| --- | --- |
| `stack.rs` tests | `owned_property_ic_capacity_preflight_and_receiver_forms_preserve_owners`；`ordinary_field_leaf_declines_without_consuming_stack_inputs`；`typed_number_leaf_declines_without_consuming_or_writing_inputs`；`typed_number_leaf_preserves_deferred_and_borrowed_inputs_until_fallback`；`recovery_string_index_leaf_preserves_spelling_and_final_key_owner`；`dense_read_leaf_preserves_declined_inputs_and_neighboring_operands`；`typed_read_leaf_preserves_declined_inputs_and_release_guards` |
| [stack/number.rs](../../src/engine/vm/stack/number.rs) tests | `fused_pair_declines_before_mutation_and_clears_both_number_owners`；`fused_local_result_capacity_failure_leaves_binding_and_stack_unchanged` |
| `stack/window.rs::primitive_transaction_tests` | `local_add_declines_bad_rhs_before_the_canonical_left_copy`；`frame_transaction_keeps_owners_across_gc_and_partial_output_failure`；`primitive_transaction_pending_owners_survive_failed_commits`；`with_linked_own_read` 测试包装覆盖的 lookup/output 路径 |
| [stack/store/tests.rs](../../src/engine/vm/stack/store/tests.rs) | 六个 `direct_store_*` 低层测试；其中 consume/keep 使用真实事务，其余仍有独立 `run_window`；C2 必须增加缓存 top 与 backing top 的配对覆盖 |
| `frame_operations/numeric.rs` tests | `resident_numeric_preflight_preserves_canonical_missing_left_consumption`；`resident_numeric_postfix_output_failure_keeps_previous_and_pc` |

## 3. facade 与直接 backing 操作

### 3.1 `RunSlots` 的完整分类

以下覆盖 `stack/window.rs::RunSlots` 当前全部方法。

| 方法 | C2 分类 | 接线要求 |
| --- | --- | --- |
| `has_operand_capacity`、`depth` | cache-aware | 继续使用逻辑 depth；缓存不扩大窗口；profile 活槽包括 TOS |
| `peek`、`pop` | cache-aware | peek(0) 识别缓存，其余按逻辑位置；cached pop 取唯一 owner 并 depth−1，首版不自动提升下一 backing 值 |
| `push`、`push_pending` | cache-aware | 先检查新槽/容量，再 spill 旧缓存并装新标量；失败保留原栈形，pending 不被 take；owning 输入走 canonical backing |
| `store_local_from_top`、`store_parameter_from_top` | cache-aware | 复用 C1 direct 准入；consume 从 TOS move，keep 仍 dup；先完成目标/源/dup 预检，再提交；返回 displaced owner，绝不自行 release |
| `binary_number` | cache-aware | 读两逻辑输入、类型证明、计算、一次提交；回调返回 `JsValue`，Number 输入并不保证标量输出；owning 结果必须落 backing，不能 discard 或 decline 后重跑回调 |
| `consume_number_pair`、`update_number_local` | cache-aware | 迁移已有融合事务；optional result 容量先验证；不得先清缓存再遇错误；保持 compare/update 的逻辑权重 |
| `local`、`parameter` | binding-only | local/arg 不与 operand hole 重叠；借出的引用不能跨替换、分配、release 或 callback |
| `replace_local`、`replace_local_pending` | binding-only | 自身可替换 binding；pending 失败不消费；任何 displaced release 前按 §5 canonicalize |
| `local_add_supported`、`local_add_constant_supported` | binding-only + 逻辑容量 | guards 不能漏算 cached depth；真正进入 primitive callback 时 spill，不因纯 guard 把所有路径提前刷回 |
| `rotate_operands`、`insert_copy`、`duplicate_operands` | canonical-only | 初版复杂 shuffle/dup 统一先 spill；保留原容量、复制失败及部分提交协议 |
| `release_operand` | canonical-only | 先恢复 operand backing 再走原 readiness/no-drain 路径；不在缓存析构代办释放 |
| `property_ic_read`、`property_ic_write_scalar` | canonical-only | 直接扫描/替换 operand backing；不能跨 IC helper 保存 hole |
| `typed_array_number_write`、`array_immediate_read`、`ordinary_field_immediate_read` | canonical-only | 原地 property 快路仍有 backing 读写和 readiness/释放；初版不移植这些事务 |
| `array_kept_immediate_read` | canonical-only | 即使通过 facade peek/pop/push 实现，也有 String key 的 runtime 查询与 release；不能因“没有直接数组索引”而遗漏它 |

### 3.2 事务直接方法

| `FrameTransaction` 方法 | C2 要求 |
| --- | --- |
| `peek` | 必须 cache-aware；只改 `RunSlots::peek` 会漏掉此入口 |
| `validate_call_value_domains` | 当前经 store helper 读取 canonical operands；首版调用前 Empty。若改为 cache-aware，只重用逐 operand 原认证顺序，不能整帧重排错误 |
| `take_native_call_operands` | reserve native buffer 及 operand 批转移前 Empty；保留 callee、receiver 的释放顺序 |
| `with_local_add_inputs`、`with_local_add_constant`、`with_local_add_constant_left` | 进入 primitive callback 前 Empty；其回调可能分配/修改唯一 String，不是纯局部读取 |
| `slots` | 只借缓存，不复制或隐式 flush |
| 待新增 `spill_to_backing` / Drop | 前者正常路径显式调用；后者只移回已预留 hole，不持 Runtime、不 drop owner、不添加 unwind 中会再次 panic 的 assert/expect |

### 3.3 backing 权限全集

实际 backing 字段是私有的；以下生产符号拥有直接索引或直接改变 depth 的能力。
测试中的 synthetic slot 修改不进入生产，却需要用来注入 hole/容量/源槽错误。

| 文件 + 符号 | 迁移策略 |
| --- | --- |
| `stack.rs` · `peek_current`、`pop_current`、`operand_push_index`、`install_operand` | 保留 canonical 实现；cache-aware facade 不可在 Cached 状态直接下沉到这些方法 |
| 同文件 · `binary_number_current` | C2 明确接通逻辑双输入与统一结果准入，或先 spill 使用现实现；不能只把 push 改缓存而保留此方法直接读两槽 |
| 同文件 · `property_ic_read_current`、`typed_array_number_write_current`、`array_immediate_read_current`、`ordinary_field_immediate_read_current`、`property_ic_write_scalar_current` | canonical-only；这些方法会成对/成组三槽读取、替换、清槽和改变 depth |
| 同文件 · `rotate_operands_current`、`replace_operand`、`release_operand_current` | canonical-only；`insert_copy_current`、`duplicate_operands_current` 虽复用 peek/push，也沿 canonical 路径处理 |
| 同文件 · `local_current`、`parameter_current`、`local_mut`、`parameter_mut`、`replace_local_current`、`replace_local_pending_current` | binding-only；不能让外部 binding 引用延长缓存提交或释放的临界区；`replace_parameter_current` 目前仅测试使用 |
| 同文件 · `snapshot_argument_tail` | 参数区读取；其上层 `snapshot_actual_arguments` 可能 dup owner，外部调用边界应已 Canonical |
| 同文件 · `take_native_call_operands_current` | canonical-only，批量 move argv、减 depth、随后 pop/release callee/receiver |
| 同文件 · `push_frame_storage`、`clear_unpublished` | canonical-only；包含 caller argv move、locals 初始化、错误回滚；调用前旧事务已结束 |
| 同文件 · `take_frame_span`、`complete_take_frame`、`clear_frame` | canonical-only；上层 `take_frame` / `take_frame_owners` 要求完整规范 operands，不能留下 TOS owner |
| [stack/call.rs](../../src/engine/vm/stack/call.rs) · `push_ordinary_frame` | canonical-only；读取 parent callee/receiver/argv 并转移到 child；保留原 actual argv 与 writable parameter 区分 |
| `stack/number.rs` · `consume_number_pair_current`、`update_number_local_current` | 由 cache-aware facade 适配；不改变已有 Number/容量预检语义 |
| [stack/store.rs](../../src/engine/vm/stack/store.rs) · `store_binding_from_top_current` | C1 新增 backing 直接提交点；必须适配 cached top 或明确 canonical fallback，不得读取 hole 后把合法 store 当源槽错误 |
| `stack/window.rs` · 三个 `with_local_add_*` | 直接 local 借用，但 callback 可触及 runtime；按 canonical-only 边界处理 |

`check_current`、`binding_counts`、`actual_argument_count` 与 `depth` 只认证身份/
区间或读逻辑数值，不负责恢复 hole。不能把它们的成功等同于 backing 已规范化。

## 4. 原有融合和 fast read 的交界

| 位置 | C2 不可破坏的契约 |
| --- | --- |
| [run/fusion.rs](../../src/engine/vm/run/fusion.rs) · `update_local`、`compare_branch` | 消费 cache-aware Number helper；guard decline 零效果；仍按 canonical span 跳转和计数 |
| `run/fusion.rs::record_span` | 是逻辑指令权重；当前没有 instruction budget，不新增虚构的每指令中断检查 |
| `run.rs::borrowed_base_field_read` 及 this/local/captured/global 入口 | 基值保持 binding 的借用，不重建临时 owner；进入未迁移的属性 helper 前可 spill 已缓存的其他操作数，不能为“统一缓存”额外 dup base |
| `run.rs` · `GetField2` method span | GetField2、纯参数 push、CallMethod 逐步提交；参数 PC 或容量失败不得回到跨度起点重读 getter；内部热 push 可用 TOS，但交给 Call 前必须 Empty |
| `conversion_driver::{complete_primitives, complete_local_add}` | AddStore/LocalAdd 首版 canonical-only；错误仍定位实际 Add/store 子 PC，不因缓存恢复回放前缀 |
| `code/fusion.rs`、publisher | 本轮只审计，不改认证、跨度或 QuickOp；C2 不依赖 B1b，更不同时接 B1c/B1d |

## 5. `run` 的退出、错误与内部观察点

### 5.1 全部 `RunExit` 必须 Empty

`RunExit::observes_activation()` 仅排除 `Call` 和 `Complete` 的 activation
物化，并非缓存豁免。下表分组覆盖当前 enum 的全部 variants；正常出口统一
显式 spill，不能只在返回 `true` 的 observers 上做恢复。

| 类别 | 当前 variants | 后续消费者与检查 |
| --- | --- | --- |
| call/返回 | `Call`、`Complete`、`Apply`、`ApplyEval`、`Eval`、`Construct`、`ReturnDerived`、`InitDerivedConstructor` | `driver/ready`、ordinary entry/finish、call/construct；尤其验证不物化的普通 Call/Complete |
| 绑定/作用域 | `Binding`、`BindingError`、`LexicalUninitialized`、`Environment`、`InitializeDerived`、`PrivateInitialize`、`PrivateAccess`、`CloseCaptured`、`ResetCaptured` | 捕获、TDZ/const、private、eval/with 都只消费 canonical stack |
| 属性/类 | `SetProperty`、`GetField`、`GetElement`、`DefineProperty`、`DefineClass`、`ClassInitializer`、`Predicate`、`GetSuper`、`HomeObject`、`SuperProperty`、`CopyData`、`SetName` | getter/Proxy/构造/枚举可能分配或调用 JS；保持已有已提交前缀 |
| conversion/迭代/创建 | `Pure`、`ConvertAdd`、`AddLocal`、`ConvertPlus`、`ConvertPropertyKey`、`StrictEquality`、`Numeric`、`ForIn`、`NormalizeThis`、`Arguments`、`Rest`、`InstantiateClosure`、`Import` | helper 不应读到 hole；owning output 仍受 pending 纪律约束 |
| 异常/控制/挂起 | `Catch`、`DropCatch`、`NipCatch`、`Throw`、`PrimitiveThrow`、`Suspend` | catch/finally 与 iterator unwind、detach/freeze 都使用规范 operands/PC |
| 框架边界 | `Materialize`、`Bridge`、测试/profiling 的 `ReleaseOperand` | materialize 重入同一 PC 前 Empty；Bridge 不允许缓存 owner 脱离事务 |

### 5.2 resident `run` 内部不退出的边界

| 文件 + 符号/分支 | 当前顺序 | C2 插入点 |
| --- | --- | --- |
| `run::release_outside_slots!` | 必要时 Materialize；结束 slots；publish fault/runtime PC；重新借 slots 提交；再次结束 slots；release | 在第一次 publication 前恢复；**提交可能再次产生缓存结果，所以 release 前再恢复一次**；不改变 release 顺序或把释放推迟到 loop 末尾 |
| `run::resident_property!` | 物化 gate；结束 slots；publication；`property::complete`；重借 slots | 进入 property helper 前 Empty；helper 自己也须 canonical-only，不能只清入口 |
| `run` · `StrictEq/StrictNeq` 非 Object/Symbol 路径 | pop 两值、push Bool、`drop(slots)`、`release_dropped(left,right)` | Bool push 可能重新缓存；release String/BigInt 前必须再次 spill；“primitive”并不等于无 owner |
| `run` · `Not` 非 Object/Symbol 路径 | pop 输入、push Bool、结束 slots、release 输入 | 同上；scalar no-op release 的豁免必须有明确证明，首版可统一恢复 |
| `run` · local/arg store、`InitializeLocal`、`SetLocalUninitialized` ready 分支 | readiness → 提交 → `release_displaced` | readiness 不变；C1 consume 后可能 Empty，keep 仍可能 Cached；释放前采用统一 spill。`Ready` 证明只允许原 no-drain release，不准据此改变 deferred 契约 |
| `run` · Drop/Nip、非数比较/转换的 outside-release 路径 | 原处理可能只移走部分输入/保留结果 | 以**提交后的**逻辑栈为恢复对象，不能恢复入口栈形 |
| `run` · numeric resident fallback | materialize gate；结束 slots；`pc.publish_fault`；`numeric::complete` | publication/helper 前 Empty；短 slots 结束不算 spill；返回 false 的 PrimitiveThrow 同样 Empty |
| `run` · `copy_value`/`dup_jsvalue`、captured/global cell read/write、borrowed-base/property fast helpers | 可能查询 runtime、保留堆边或进入未迁移 helper | 纯 scalar 可维持 cache；首版其余路径显式 canonicalize。预检与提交间只允许原 move/retain，不新增能改变 readiness 的观察操作 |

### 5.3 `?`、显式 Err 与 Rust unwind

1. `run` 的 instruction lookup、PC checked_add、slot 访问/容量、dup、runtime
   helper/publication 都有 `?`；`Ret` / `DropGosub` 还有已 pop 后的显式 Err。
   不逐个假设“错误时没有缓存”；事务 Drop 必须覆盖所有提前离开。
2. [program_counter.rs](../../src/engine/vm/run/program_counter.rs) 的 Drop
   写回 fault/resume；新增 transaction Drop 仅恢复当前物理栈，不改 PC、不
   回滚已经提交的 pop/push/store。两个 Drop 组合测试须检查**两种状态**。
3. `binary_number` 的调用者回调可在低层测试 panic；缓存预检、取 owner、提交
   之间不能把 owning 值暂存在会经过可失败/可 panic 操作的无保护局部变量中。
   C2 的 scalar 输入不能作为未来 owning 缓存的安全证明。
4. 正常的 `unreachable!` / `expect` 仍由既有认证约束支撑；不得把 push 失败
   改成 panic。Drop 恢复中不加会在 unwind 再次 panic 的不变量断言；将检测
   放在进入/提交前，恢复只使用已认证 hole。
5. `ProgramCounter` 比 transaction 晚创建；不能依赖析构次序来满足**正常观察**
   的先后顺序。runtime publication、release、helper entry 仍显式 spill。

## 6. helper、host、GC 与 suspension 的闭合

| 边界/文件 + 符号 | C2 必须成立的条件 |
| --- | --- |
| [run/numeric.rs](../../src/engine/vm/run/numeric.rs) · `complete` / `materialize_thrown` | `primitive_output` 分配/释放/错误物化前 Empty；output 产生后若暂不迁移该 helper，就用 canonical output 借用；保留 postfix previous 已提交、next push 失败的前缀 |
| [run/property.rs](../../src/engine/vm/run/property.rs) · `complete` | canonical-only 必须覆盖 helper 全生命周期。其 ElementRead 分支可能 push base 后在同一短借用里 release key，不能在 base push 后留下缓存；decline 恢复输入不能重跑已发生 getter/coercion |
| `frame_operations/numeric.rs` · `try_complete_primitive` / `commit_output` | 输入与输出之间的分配/释放 Empty；失败 cleanup 显式释放未提交 pending；新增 Drop 后结束 transaction 才把 `&mut execution` 交给 query driver |
| `property_driver.rs::complete_read` / `publish_read_result` | publication 后可能 release receiver；即使 output 是 scalar，helper 返回前仍 canonical；`let _ = slots` 不执行恢复 |
| `conversion_driver::complete_primitives` 与 local_add | primitive helper、error construction、binding release 前 Empty；所有 pending 与 actual fault PC 按原协议；local/string borrowed callback 不留缓存或可逃逸引用 |
| [driver/ready.rs](../../src/engine/vm/driver/ready.rs) · `run` | 调用 resident run 返回即 canonical；随后 materialize、ordinary entry/finish、property/numeric 重入都不持旧 TOS |
| [driver.rs](../../src/engine/vm/driver.rs) · `run_frames_with_state` / `push_frame` / `push_direct_call_frame` | cold dispatch、iterator unwind、frame push、detach 前旧事务结束；同一个 VM 核心不新增第二套缓存ABI |
| [call/ordinary.rs](../../src/engine/vm/call/ordinary.rs) · `OrdinaryCall::install`；`stack/call.rs::push_ordinary_frame` | caller 原 argv、callee、receiver 全部 backing 可见；普通、method、tail call 与 frame reuse 均覆盖；parent depth 按原 consumed 更新 |
| `driver/ordinary.rs::{enter_selected,finish}`；[frame_exit.rs](../../src/engine/vm/frame_exit.rs) · `finish` | native 调用前已有显式 transaction drop；返回结果先移入 pending/parent，再 clear child；任何最后一个 owner release 前不留 hole |
| [execution.rs](../../src/engine/vm/execution.rs) · `HostBoundaryGuard`、`RunningExecution::drop` | 宿主重入与 callback panic 前 parent canonical；teardown 清理每个 frame 时没有事务孤立 owner；不把 release 移到 TOS Drop |
| [frame.rs](../../src/engine/vm/frame.rs) · `FrameStore::materialize`；[frames.rs](../../src/engine/vm/frames.rs) · runtime PC publication | 注册/发布 observer 前 Empty；C2 不修改 canonical fault/resume/source-map 单位 |
| [heap/slot_ownership.rs](../../src/engine/heap/slot_ownership.rs) · readiness / `try_release_slot_value_jsvalue` | 保持 Ready、QueueCapacity、Drain、Deferred、Borrowed、PrimitiveStorage 分支；共享/独占借用、zero_queue/deferred 已有状态不得被缓存 helper 意外 drain |
| [heap/ownership.rs](../../src/engine/heap/ownership.rs) · `Runtime::operation` / `drain_deferred_references`；[runtime_gc.rs](../../src/engine/heap/runtime_gc.rs) · `run_gc` | 任何可执行 deferred/GC 的边界之前先 canonicalize；“没有 live RunSlots”不够，事务可能仍持 TOS |
| [suspend/owned.rs](../../src/engine/vm/suspend/owned.rs) · `OwnedSuspension::{detach,freeze}` / `prepare` / `PreparedResume` | detach 的 `take_frame` 必须看到完整 operands；freeze、错误与放弃只处理既有 FrameStorage；不序列化 TOS；恢复后的新 transaction Empty |
| [suspend.rs](../../src/engine/vm/suspend.rs) · `freeze_entry` / `thaw` / rooted activation Drop | Yield/Await、恢复失败/外 runtime 拒绝、未恢复即放弃均不遗失缓存 owner；无新 suspension ABI |

已有 `run/numeric.rs::complete` 用 `push_pending(...)?` 直接退出，不能把它视为
完整 owning-output cleanup 的范例。C 计划 C3 已明确要求另做失败清理修复并
验证 heap BigInt postfix 第二输出失败；本审计不修复该项，也不把它算作 C2
收益。C2 接线时必须保留该问题的归属，不让 scalar 缓存掩盖 owning 错误路径。

## 7. 最小实施顺序与必须测试点

以下按依赖排序，每项独立可编译、可回退；先满足正式前置再启动性能验收。
不在 C2 同时接 owning TOS、扩 fusion 或切 QuickOp 派发。

| 工作项 | 具体实施 | 必须新增或扩展的测试 |
| --- | --- | --- |
| C2-P0 前置与接口冻结 | 关闭 C0/C1 决策；保留本表作接线清单；明确 canonical facade 与借用TOS facade 的表示、spill原因枚举 | 本轮现有 C1/Number/property 测试继续作为 canonical 对照；不能以本文代替 receipt |
| C2-P1 状态机 | `stack/tos.rs` 的 Empty/Cached、scalar 准入、预留hole、显式spill与事务Drop；独立 run_window保持canonical | Empty→Cached→Cached、push失败不变、pop不自动提升、spill幂等、容量0/1/满栈、hole/邻槽/逻辑depth/live_slots、重复短borrow驻留 |
| C2-P2 关闭所有非驻留边界 | §2全部构造点和事务直接方法；§5出口、release/publication、§6helper全部canonical-only；补显式drop消除NLL依赖 | 每类 RunExit 有缓存top；特别 Call/Complete/Materialize；Err与catch_unwind同时检查canonical backing、fault/resume；GC时事务仍活但已显式spill |
| C2-P3 基础标量操作 | cache-aware peek/push/pending/pop；标量一元与Bool branch；复杂shuffle仍spill | Int/Float/±0/NaN/ShortBigInt、underflow、pending失败保owner、原溢出仍失败、canonical-only shuffle前后顺序 |
| C2-P4 接 C1 + Number | direct local/parameter consume/keep；Number binary、compare-branch、local update使用同一logical栈 | C1六项低层与JS语义双路径；TDZ/const/captured/mapped不扩准入；Number回调返回Object/String/BigInt/Symbol时落backing且只执行一次；guard decline零效果；optional result容量失败 |
| C2-P5 观察协议回归 | 不扩大helper驻留；验证property/primitive/call/host/suspend出口实际闭合 | String/BigInt key释放；StrictEq/Not的Bool输出后release；最後owner、非空zero_queue/deferred、共享/独占state借用；getter/Proxy/valueOf一次执行；host重入/panic；tail call；yield/await/freeze/thaw/放弃 |
| C2-P6 诊断与裁决 | profiling添加hit/miss/spill原因/backing读写，普通构建零计数；隔离cache-on/canonical构建，保持相同源码/flags/输入 | 先定向/完整正确性及MSRV，再串行A/A与配对性能；反汇编确认真正减少流量；不把逻辑owner move当物理write；按C计划最多两轮调整或回退 |

回归扩展的现成落点包括：`ProgramCounter` 的
`local_pc_preserves_both_values_on_result_error_and_unwind`；numeric 的
`resident_numeric_preflight_preserves_canonical_missing_left_consumption` 和
`resident_numeric_postfix_output_failure_keeps_previous_and_pc`；transaction 的
`frame_transaction_keeps_owners_across_gc_and_partial_output_failure`；execution
的 `host_boundary_restores_registry_after_callback_panic` 与
`module_host_reentry_preserves_owned_parent_bindings_and_unwinds`；suspend 的
`failed_thaw_releases_partial_roots_without_detaching_heap_state`、
`abandoned_thaw_does_not_register_or_keep_the_runtime_alive` 和
`resume_rejects_foreign_runtime_before_registering_a_frame`。

每个新增 cached 用例须有同输入 canonical 对照，核对值、PC、深度、活槽、RC
和副作用顺序；仅断言“最终JS结果相同”不足以关闭内部恢复协议。

## 8. 本轮证据复查：找到什么、仍缺什么

本次只查 `docs/reports` 和当前 worktree 的 `target`；未重扫历史 worktree。
对 target 优先检查 E/LTO/C/B/RSS/残余/A4 命名目录的 report、metadata、receipt
及 build receipt，未进行二进制重跑或新的哈希全量认证。

| 已有文件/记录 | 本次核对 | 能支持的结论 |
| --- | --- | --- |
| `docs/reports/s3-a-plan.md` §8.13；`target/lto-compare/{scaling-lto,property-lto,v8-lto,fixedwork}/` 与 `stages.json` | LTO 与旧无LTO对照、阶段运行状态仍在 | 历史回退线索和现行归因入口；不是新C0关闭receipt |
| `target/bench-e-*`、`target/bench-e2-*` metadata/reports | 有旧E实验原样本/引擎哈希；例如scaling metadata保留旧源码与dirty状态 | 可追溯历史实验；未找到与最新协议完整绑定的E/RSS关闭决定 |
| `target/{bench-e-baseline,bench-e-lto,final-lto}/release/` | 三处 `qjs.build.json` 均不存在 | 不能仅凭目录名或现存二进制认证正式saved/previous/pre-A分母 |
| `target/s3-c-opening/inputs/`、`baseline-source.json`、`candidate-source.json` | 已有输入恢复及冻结源码清单；opening文档记录58 fixed、67 compile、9 original、3 bigint | 原计划“历史输入不可得”这一准备缺口已补；不等于已按新协议跑完整矩阵 |
| `target/s3-c-opening/validation-receipt.json` | 明确 `performance_acceptance: false`，列本轮功能验证和日志指纹 | C1/B1a正确性与构建工具验证；不支持C1性能接受 |
| `target/s3-c-opening/candidate-build/x86_64-unknown-linux-gnu/release/qjs.build.json` | `source_identity.kind=frozen-export`；1242文件构建前后验证；manifest身份与opening记录一致 | 本轮候选有实际发布配置构建；单个candidate build不补齐历史E/残余/A4决策 |

**本次没有发现新增的 E/RSS、有界残余或 A4 关闭结论。** 仍需补齐：

1. E后 saved、previous/C0、同协议 pre-A 的明确源码/构建/输入 identities，
   A/A噪声与完整性能矩阵；普通比较双方 fat LTO/CGU=1/无PGO/无profiling。
2. 峰值与稳态RSS、反复创建/释放后的内存证据；旧目录名含E不证明已补RSS。
3. 有界残余逐项关闭或分配到A4/C/B/D的决定，特别bigint256、typed-index、
   prop-delete、navier-stokes；不得继承旧无LTO关闭状态。
4. A4采用8B或保留16B的有数据裁决；当前16B静态断言只说明现状。
5. C1相对previous/saved的普通release配对结果、pre-A累计台账及明确接受/
   回退决定。此项通过后才把C1 facade当成C2的已接受性能分母。

后续接线复查可从以下只读命令开始；新入口必须同步补回本文。

```sh
rg -n 'FrameTransaction|RunSlots\s*\{|run_window\(|frame_transaction\(' src/engine/vm
rg -n 'store\.slots|self\.slots\[|split_at_mut|window\.depth|parent\.depth' src/engine/vm/stack.rs src/engine/vm/stack
rg -n 'return (Ok|Err)|drop\(slots\)|release_|update_active_bytecode_pc|numeric::complete' src/engine/vm/run.rs
rg -n 'take_frame|clear_frame|materialize|detach|freeze|thaw|HostBoundaryGuard' src/engine/vm
```

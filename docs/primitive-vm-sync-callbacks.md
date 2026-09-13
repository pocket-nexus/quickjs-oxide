# S05 同步回调调用点账本

状态：S05 实施中，尚未验收。源码基准为 `22dc64d` 加当前 S05 native 调用所有权工作区；
本文件跟踪迁移责任，不把旧同步入口仍存在等同于 owned 路径已覆盖。

## 当前覆盖与待办

- non-Proxy Get 的描述符/原型准备已统一到 object 的共享内核，准备不执行 getter。
  Array 孔位、Arguments、String 包装对象、TypedArray、namespace live cell 与
  autoinit 保持原存储算法；已选 getter 由 `vm/property_driver` 调度。
- GetField/GetField2、GetArrayEl/2/3、对象键 ToPrimitive、原语 base 和 bound
  字节码 getter 已接入。Proxy Get 的 handler 读取、trap 调用、缺少/null trap 的
  转发共用 object 阶段状态；普通字节码回调由显式子帧回复，嵌套 handler Proxy
  由继续状态栈推进。普通 target 的 Get 不变量检查不调用 JS。
- Proxy target 的 GetOwnProperty、descriptor Has/Get、嵌套 Has/IsExtensible
  由 object/value 的共享阶段推进，owned 查询内部不再同步等待这些步骤。
  ToPrimitive 的 Proxy Get 也使用同一查询 continuation。
- Proxy [[Call]] 的 apply 读取、逐层可调用标记检查、转发及 trap 调用共享
  object/call 阶段；普通调用、bound Proxy、getter、转换和查询 trap 已接入。
  非 callable Proxy trap 保留先 Get apply 再拒绝的顺序，原深度 guard 跨最终回复保活。
- PutField/PutArrayEl 的普通 Set、setter、Proxy Set、receiver GetOwnProperty/
  DefineProperty 由共享 object 阶段和 owned scheduler 推进，包含对象键的
  string-hint 转换。计算写入先完成 RHS，再转换键，最后检查 base；读取保持
  自己的原顺序。strict/sloppy 的完成与诊断由新旧入口共用。
- Array length 的两次 ToNumber 与后置 length/writable 检查、TypedArray 整数
  索引 Set/Define 的 Number/BigInt 转换已接入共享阶段；转换后重新获取 buffer
  凭证，保留无效索引、不同 receiver、Define 前置检查及后置写失败的原规则。
  这些赋值路径不再同步等待转换中的 JS；旧/native 入口仍消费同一阶段。
- Drop/Nip 释放最后临时引用时，由 driver 冷路径消费不改变所有权的热预检结果，
  防止 transfer() 返回的新 buffer 被丢弃后把转换回调的余下指令交给旧 VM。
- super 的 HomeObject、冻结 base、读取/调用/写入已接入。普通读取的 getter
  使用实际 receiver，调用点读取按 pinned 规则使用 base 作为 getter receiver；
  方法调用继续使用原 receiver。读取先转换键，写入先检查 base；冻结 base
  独立保活到 getter/Proxy 回复，对象键继续复用 value 的转换阶段。
- `in`、`delete` 已接入，分别保留 RHS 对象先检查和键先转换的顺序；Proxy
  deleteProperty/preventExtensions 共用 boolean 阶段并保留各自 target 不变量。
  Symbol 键贯穿转发、descriptor、IsExtensible 等待并在放弃后释放。
  preventExtensions 的 owned 查询可独立运行，Object/Reflect native 入口尚待接入。
- Proxy getPrototypeOf/setPrototypeOf 共用原型阶段，owned 查询推进 handler/trap、
  target IsExtensible 和原型身份比较。Get 先检查返回值类型，Set false 立即完成；
  请求/返回原型及 Proxy 边贯穿等待，在放弃后释放。独立查询验证三个旧路径
  计数均为零；Object/Reflect 生产 native 入口仍未接入，不能据此视为全部覆盖。
- `vm/call/native` 抽出 native activation：准备阶段重验 payload/realm/min argv，
  可失败预留完整实参并保留额外 argv；完成时先在 native 帧及其 realm 中
  创建 JS engine error，再解除帧登记。放弃/异常展开释放 argv 和帧根。
  目前原同步调用器消费此所有者，尚未将 native 内置的回调改为 owned continuation。
- Reference/with 普通存在性重查改用 HasProperty，避免把 TypedArray 无效
  数字键的 Get(undefined) 当作存在；相关原 oracle 保持并通过。动态环境的
  剩余 Proxy/同步 Set 路径、其他 traps、Object/Reflect 原 native 入口、
  native/非普通字节码 callable 及其他同步内置仍待完成领域恢复。
- Array/iterator 的 callback/sort/species/close，String/RegExp replacement，
  TypedArray/buffer 参数转换，以及 Function/scalar/Math/collections/Date/
  JSON/Error/globals 的全部同步调用点仍按原 S05 要求逐项迁移。
- generator、async/Promise、async generator 属 S06；模块/host/API/二进制入口
  收口属 S07。S05 结束时仍须完成原始 Earley-Boyer 默认预算的覆盖筛查。

## 诊断与证据口径

`owned_bridge_exits` 是整帧旧指令交接；`owned_sync_call_bridges` 是从 owned
调用点进入待迁移同步 Runtime 调用边界的次数，包含无回调 callee，并非所有
内部嵌套调用的总数。已准备的 PendingCall 先返回常驻分派器再执行；转换和
属性查询中尚未迁移的 native/非普通字节码调用仍有同步等待路径。它们不是 host delimiter，不满足
S05 的全部回调由 driver 推进要求。临时请求及其继续状态的 Box 分配尚未纳入
S04 call_preparation 的统计范围。

准备完成后放弃请求的测试确认 callee 未执行且 Runtime 可释放；真实 loader
重入与 native 小栈回归继续覆盖登记和原有预算。新读取用例同时检查旧分派、
整帧交接和同步调用桥三个计数；Array.map 反例明确保留同步桥计数 1。

## 直接调用边界清单

下表逐调用表达式记录 `object`、`builtins`、`value`、`vm` 中当前的
`call_internal`、`call_value_internal`、`construct_internal`。生成时排除了
独立测试文件和内联 test-only 模块/函数，并使用现有 Rust 词法扫描器去除
注释/字符串；本轮共有 128 个直接表达式，行号指向本次
源码；后续改动以函数和调用位置复核。它是第一轮直接边界清单，**尚不是完整
回调调用图**：经 Get/Set、ToPrimitive/ToString/ToNumber、species 和 iterator
等 helper 间接触发的调用必须在各领域迁移时补入，不能仅凭该表清空验收 S05。

| 调用位置 | 边界 | 责任/状态 |
| --- | --- | --- |
| [src/engine/object/class_fields.rs:222](../src/engine/object/class_fields.rs#L222) `call_class_instance_initializer` | `call_internal` | 旧入口保留；S04 owned class 已迁移，S07/S10 审核入口 |
| [src/engine/object/class_fields.rs:286](../src/engine/object/class_fields.rs#L286) `run_class_static_initializer` | `call_internal` | 旧入口保留；S04 owned class 已迁移，S07/S10 审核入口 |
| [src/engine/object/class_fields.rs:339](../src/engine/object/class_fields.rs#L339) `call_class_static_block` | `call_internal` | 旧入口保留；S04 owned class 已迁移，S07/S10 审核入口 |
| [src/engine/object/internal_methods/boolean.rs:394](../src/engine/object/internal_methods/boolean.rs#L394) `finish` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods/prototype.rs:336](../src/engine/object/internal_methods/prototype.rs#L336) `finish` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:164](../src/engine/object/internal_methods.rs#L164) `call_value_internal` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:482](../src/engine/object/internal_methods.rs#L482) `call_proxy_trap` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:844](../src/engine/object/internal_methods.rs#L844) `proxy_get` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:891](../src/engine/object/internal_methods.rs#L891) `internal_set` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:992](../src/engine/object/internal_methods.rs#L992) `proxy_set` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:1104](../src/engine/object/internal_methods.rs#L1104) `proxy_get_own_property` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:1177](../src/engine/object/internal_methods.rs#L1177) `proxy_define_own_property` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/internal_methods.rs:1396](../src/engine/object/internal_methods.rs#L1396) `call_proxy` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/object/ordinary.rs:107](../src/engine/object/ordinary.rs#L107) `finish_prepared_read` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1043](../src/engine/builtins/array.rs#L1043) `call_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1080](../src/engine/builtins/array.rs#L1080) `call_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1106](../src/engine/builtins/array.rs#L1106) `call_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1161](../src/engine/builtins/array.rs#L1161) `call_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1259](../src/engine/builtins/array.rs#L1259) `close_iterator_preserving_throw` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1762](../src/engine/builtins/array.rs#L1762) `call_array_prototype_iteration` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:1998](../src/engine/builtins/array.rs#L1998) `call_array_prototype_reduce` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:2068](../src/engine/builtins/array.rs#L2068) `call_array_prototype_find` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:2319](../src/engine/builtins/array.rs#L2319) `flatten_into_array_with_limits` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:2528](../src/engine/builtins/array.rs#L2528) `native_element_locale_value` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:2667](../src/engine/builtins/array.rs#L2667) `call_array_prototype_to_string` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array.rs:3129](../src/engine/builtins/array.rs#L3129) `compare_array_sort_slots` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/element.rs:63](../src/engine/builtins/array_buffer/typed_array/element.rs#L63) `finish_sync` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/find.rs:70](../src/engine/builtins/array_buffer/typed_array/find.rs#L70) `call_typed_array_find` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/iteration.rs:90](../src/engine/builtins/array_buffer/typed_array/iteration.rs#L90) `call_typed_array_iteration` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/reduce.rs:77](../src/engine/builtins/array_buffer/typed_array/reduce.rs#L77) `call_typed_array_reduce` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/sort.rs:313](../src/engine/builtins/array_buffer/typed_array/sort.rs#L313) `compare_typed_array_sort_indices` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array/species.rs:319](../src/engine/builtins/array_buffer/typed_array/species.rs#L319) `typed_array_filter_result` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array.rs:1044](../src/engine/builtins/array_buffer/typed_array.rs#L1044) `call_typed_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array.rs:1087](../src/engine/builtins/array_buffer/typed_array.rs#L1087) `call_typed_array_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array.rs:1483](../src/engine/builtins/array_buffer/typed_array.rs#L1483) `collect_typed_array_iterator` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/array_buffer/typed_array.rs:1524](../src/engine/builtins/array_buffer/typed_array.rs#L1524) `collect_typed_array_iterator` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/date/prototype.rs:467](../src/engine/builtins/date/prototype.rs#L467) `call_date_to_json` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/dispatch.rs:244](../src/engine/builtins/dispatch.rs#L244) `call_function_prototype_call` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/error/mod.rs:286](../src/engine/builtins/error/mod.rs#L286) `aggregate_error_iterator_to_array` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/eval.rs:107](../src/engine/builtins/eval.rs#L107) `call_global_eval` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/eval.rs:450](../src/engine/builtins/eval.rs#L450) `execute_string_eval` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/function.rs:276](../src/engine/builtins/function.rs#L276) `call_function_prototype_apply` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/function.rs:282](../src/engine/builtins/function.rs#L282) `call_function_prototype_apply` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/function.rs:551](../src/engine/builtins/function.rs#L551) `is_instance_of` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/function.rs:809](../src/engine/builtins/function.rs#L809) `ordinary_is_instance_of` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/concat.rs:256](../src/engine/builtins/iterator/concat.rs#L256) `resume_iterator_concat_next_raw` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/concat.rs:358](../src/engine/builtins/iterator/concat.rs#L358) `call_iterator_concat_return` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:513](../src/engine/builtins/iterator/mod.rs#L513) `call_iterator_from` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:669](../src/engine/builtins/iterator/mod.rs#L669) `call_iterator_wrap_resume` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:694](../src/engine/builtins/iterator/mod.rs#L694) `iterator_wrap_primitive_next` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:894](../src/engine/builtins/iterator/mod.rs#L894) `call_iterator_consumer` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:990](../src/engine/builtins/iterator/mod.rs#L990) `call_iterator_reduce` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:1056](../src/engine/builtins/iterator/mod.rs#L1056) `iterator_close_normal` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:1310](../src/engine/builtins/iterator/mod.rs#L1310) `helper_callback` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/iterator/mod.rs:1534](../src/engine/builtins/iterator/mod.rs#L1534) `resume_iterator_flat_map` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/json/reviver.rs:211](../src/engine/builtins/json/reviver.rs#L211) `internalize_json_property` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/json/stringify.rs:391](../src/engine/builtins/json/stringify.rs#L391) `check_value` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/json/stringify.rs:404](../src/engine/builtins/json/stringify.rs#L404) `check_value` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:387](../src/engine/builtins/map.rs#L387) `call_map_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:433](../src/engine/builtins/map.rs#L433) `call_map_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:726](../src/engine/builtins/map.rs#L726) `call_map_get_or_insert` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:802](../src/engine/builtins/map.rs#L802) `call_map_for_each` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:1017](../src/engine/builtins/map.rs#L1017) `call_map_group_by` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/map.rs:1050](../src/engine/builtins/map.rs#L1050) `call_map_group_by` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/math.rs:855](../src/engine/builtins/math.rs#L855) `call_math_sum_precise` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/object.rs:126](../src/engine/builtins/object.rs#L126) `call_object_group_by_with_element_limit` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/object.rs:169](../src/engine/builtins/object.rs#L169) `call_object_group_by_with_element_limit` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/object.rs:314](../src/engine/builtins/object.rs#L314) `call_object_from_entries` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/object.rs:476](../src/engine/builtins/object.rs#L476) `object_iterator_next` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/object.rs:976](../src/engine/builtins/object.rs#L976) `call_object_prototype_to_locale_string` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/promise/all.rs:112](../src/engine/builtins/promise/all.rs#L112) `call_promise_aggregate` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/all.rs:126](../src/engine/builtins/promise/all.rs#L126) `call_promise_aggregate` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/all.rs:491](../src/engine/builtins/promise/all.rs#L491) `finish_promise_aggregate_element` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:109](../src/engine/builtins/promise/convenience.rs#L109) `call_promise_try` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:132](../src/engine/builtins/promise/convenience.rs#L132) `call_promise_try` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:205](../src/engine/builtins/promise/convenience.rs#L205) `call_promise_race` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:277](../src/engine/builtins/promise/convenience.rs#L277) `promise_iterator_record` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:311](../src/engine/builtins/promise/convenience.rs#L311) `invoke_promise_then` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/convenience.rs:320](../src/engine/builtins/promise/convenience.rs#L320) `reject_promise_capability` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise/finally.rs:130](../src/engine/builtins/promise/finally.rs#L130) `call_promise_finally_handler` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:632](../src/engine/builtins/promise.rs#L632) `call_promise_constructor` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:642](../src/engine/builtins/promise.rs#L642) `call_promise_constructor` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:894](../src/engine/builtins/promise.rs#L894) `execute_promise_resolve_thenable_job` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:906](../src/engine/builtins/promise.rs#L906) `execute_promise_resolve_thenable_job` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:923](../src/engine/builtins/promise.rs#L923) `execute_promise_reaction_job` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:944](../src/engine/builtins/promise.rs#L944) `execute_promise_reaction_job` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:1121](../src/engine/builtins/promise.rs#L1121) `call_promise_catch` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/promise.rs:1196](../src/engine/builtins/promise.rs#L1196) `promise_static_resolve_core` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/builtins/reflect.rs:373](../src/engine/builtins/reflect.rs#L373) `call_reflect_apply` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/regexp/constructor.rs:396](../src/engine/builtins/regexp/constructor.rs#L396) `construct_intrinsic_regexp_arguments` | `construct_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/regexp/exec.rs:93](../src/engine/builtins/regexp/exec.rs#L93) `regexp_exec_abstract` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/regexp/replace.rs:321](../src/engine/builtins/regexp/replace.rs#L321) `finish_regexp_symbol_replace` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:420](../src/engine/builtins/set.rs#L420) `call_set_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:444](../src/engine/builtins/set.rs#L444) `call_set_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:722](../src/engine/builtins/set.rs#L722) `call_set_for_each` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:1028](../src/engine/builtins/set.rs#L1028) `start_set_like_keys_iterator` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:1077](../src/engine/builtins/set.rs#L1077) `set_like_iterator_next` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:1127](../src/engine/builtins/set.rs#L1127) `close_set_iterator_for_set_method` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/set.rs:1137](../src/engine/builtins/set.rs#L1137) `call_set_like_has` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/string/regexp.rs:219](../src/engine/builtins/string/regexp.rs#L219) `call_string_regexp_method` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/string/replace.rs:157](../src/engine/builtins/string/replace.rs#L157) `call_string_prototype_replace` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/string.rs:981](../src/engine/builtins/string.rs#L981) `call_string_prototype_split` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/weak_collection.rs:358](../src/engine/builtins/weak_collection.rs#L358) `call_weak_collection_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/weak_collection.rs:424](../src/engine/builtins/weak_collection.rs#L424) `call_weak_collection_constructor` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/builtins/weak_collection.rs:663](../src/engine/builtins/weak_collection.rs#L663) `call_weak_map_native` | `call_internal` | S05：owned 领域恢复待逐项迁移 |
| [src/engine/value/conversion/primitive.rs:199](../src/engine/value/conversion/primitive.rs#L199) `finish_primitive_steps` | `call_internal` | 旧同步消费器；部分 owned 转换已迁移，其余 S05 待做 |
| [src/engine/value/conversion.rs:161](../src/engine/value/conversion.rs#L161) `native_to_number` | `call_internal` | 旧同步消费器；部分 owned 转换已迁移，其余 S05 待做 |
| [src/engine/vm/async_from_sync_iterator.rs:48](../src/engine/vm/async_from_sync_iterator.rs#L48) `get_async_iterator_record` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_from_sync_iterator.rs:74](../src/engine/vm/async_from_sync_iterator.rs#L74) `get_async_iterator_record` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_from_sync_iterator.rs:278](../src/engine/vm/async_from_sync_iterator.rs#L278) `call_async_from_sync_iterator_resume` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_from_sync_iterator.rs:377](../src/engine/vm/async_from_sync_iterator.rs#L377) `close_async_from_sync_iterator_normally` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_from_sync_iterator.rs:419](../src/engine/vm/async_from_sync_iterator.rs#L419) `settle_async_from_sync_capability` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_function.rs:313](../src/engine/vm/async_function.rs#L313) `settle_async_function` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_generator.rs:344](../src/engine/vm/async_generator.rs#L344) `call_async_generator_prototype_resume` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/async_generator.rs:1031](../src/engine/vm/async_generator.rs#L1031) `settle_front_async_generator_request` | `call_internal` | S06：挂起/Promise 状态机待迁移 |
| [src/engine/vm/call_bridge.rs:135](../src/engine/vm/call_bridge.rs#L135) `invoke` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/construct_driver.rs:284](../src/engine/vm/construct_driver.rs#L284) `enter_request` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/conversion_driver.rs:542](../src/engine/vm/conversion_driver.rs#L542) `invoke` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge/private_elements.rs:246](../src/engine/vm/host_bridge/private_elements.rs#L246) `get_private_field_value` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge/private_elements.rs:280](../src/engine/vm/host_bridge/private_elements.rs#L280) `put_private_field_value` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge.rs:1196](../src/engine/vm/host_bridge.rs#L1196) `call_iterator_method` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge.rs:2958](../src/engine/vm/host_bridge.rs#L2958) `dynamic_import` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge.rs:3136](../src/engine/vm/host_bridge.rs#L3136) `call_with_borrowed_arguments` | `call_value_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge.rs:3158](../src/engine/vm/host_bridge.rs#L3158) `apply` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/host_bridge.rs:3168](../src/engine/vm/host_bridge.rs#L3168) `apply` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/iterator_driver.rs:776](../src/engine/vm/iterator_driver.rs#L776) `invoke` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/proxy_get_driver.rs:1228](../src/engine/vm/proxy_get_driver.rs#L1228) `advance` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |
| [src/engine/vm/with_driver.rs:192](../src/engine/vm/with_driver.rs#L192) `read` | `call_internal` | 迁移边界或旧 VM 消费器；按 S05/S06 接入，S10 删除旧路 |

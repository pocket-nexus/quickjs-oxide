# 属性写入与 shape transition：合同、证据及采用边界

本文件只汇总已实现代码、已完成单测、已有逻辑 Profile，以及主 agent 通知的独立时间点估计。编写时没有启动构建、测试、Profile 或 benchmark。正式时间区间、RSS、代码体积和完整 Test262 结果应以主线最后保存的回执为准。

## 1. 普通对象已有 scalar 字段写入

独立提交：`3cb5fd54`，基线 `7723e825`。
独立树：`/tmp/oxide-named-scalar-write`。
普通计时二进制：`/home/eric/.cache/oxide-six-property/release-target/release/qjs`；相邻目录保存 build.py 构建回执。Rust 1.88.0，release/fat LTO/CGU=1，无 profiling。

### 为什么选择

当前 PutField 一律交给 VM property driver。已有 ordinary writable own scalar 字段虽然能在 Set probe 中同步完成，仍要经过驱动器交接、PropertyKey owner 及通用 Set 入口。现有帧借用、共享 slot 替换和 fallback 协议已能表达直接完成；没有增加新 trait、写 IC、结果类型或逐函数元数据。

基线逻辑 Profile，固定完整进程计数：

| 负载 | set_property action | storage Set probe | Set state created | missing committed |
|---|---:|---:|---:|---:|
| Richards | 50,751 | 50,772 | 9 | 141 |
| DeltaBlue | 22,662 | 23,970 | 37 | 4,363 |
| RayTrace | 235,680 | 235,717 | 23 | 194,985 |

来源：`/tmp/oxide-six-baseline-profile/raw/{richards,deltablue,raytrace}.cost.jsonl`。这些是逻辑事件，不是时间比例，也不能直接当成 PutField scalar 可优化次数。尤其 Set state 本来就很少，因此主要机制应写作“同步字段写入减少 VM 交接与通用入口工作”，不能归因于删除大量 continuation。

原始 benchmark 提供实际消费者：Richards 的 state/count/currentId 等数值字段反复更新；DeltaBlue 的 mark/value/stay 等字段；RayTrace 的 Color/Vector 创建后有已有数值字段更新。但是 RayTrace 大量写入仍是首次创建字段，不能声称这个候选覆盖全部属性成本。

### 精确准入与生命周期合同

- 只在静态键 `Opcode::PutField` 尝试；动态键 PutArrayEl 沿已有路径。
- `frame.property_generation.checked_add(1)` 必须成功，溢出则原驱动器处理；成功写入后递增，维持帧内属性事实失效规则。
- 发布的 executable 必须属于本 runtime，linked key 必须存在且非空；atom 的 owner 由该 executable 保持，不新建 PropertyKey owner。
- base 必须是 Object；ObjectKind 和 payload 必须同时是 Ordinary，排除 Proxy、模块 namespace、数组、函数及其他 exotic 对象。
- 新值与旧 data slot 都必须是无 heap/atom owner 的立即值：Undefined、Null、Bool、Int、Float、ShortBigInt。不做数值转换，因此 NaN、负零等不经过另一种转换协议。
- 必须找到当前对象自身的槽，且 descriptor writable。原型链、缺失、accessor、VarRef、AutoInit、只读或引用型旧值均不提前提交。
- receiver 的 release readiness 必须是 Ready：验证活跃代际身份；没有待处理 deferred reference 或 zero cleanup，且退休当前帧 owner 不会触发末次引用回收。
- 帧借用维持两个 operand 的 root。own slot 选择、flags/存储种类核对与替换在同一 mutable runtime state 借用中完成，中间无 JS 回调或所有权释放。
- 替换复用 `RuntimeState::replace_property_slot`，不直接越过共享存储内核。旧新值均立即值，所以没有边、atom retain/release 或清理队列工作，也不改变对象 layout。
- 只有替换成功后才消费两个 operand：丢弃无 owner 的 scalar 并释放原 base owner。赋值表达式保留结果由原 bytecode 栈组织负责，未修改此约定。
- 每一个正常 decline 保留 operand 与对象内容，原 driver 继续处理严格模式抛错、继承 setter、Proxy、Array length、引用值替换与可观察清理。该能力不跨回调保存 slot 位置。

### 已有验证

Rust 1.88.0、profiling 构建下，`ordinary_storage::ic::tests` 14 项通过，其中新增 4 项覆盖：

- 直接内核更新成功，receiver 引用计数不增加；
- own/missing/readonly/accessor/Proxy/Array/reference old value 回退，末次 receiver、借用中和 deferred work 准入拒绝；
- 赋值返回值、前后缀更新、own 属性遮蔽原型 setter、descriptor 修改、setter 回调中的再次写入、严格只读错误、Proxy receiver、Array length 及引用旧值替换；
- 16 次已存在 scalar 字段写入准确记录 16 次 `ordinary_scalar_field_write_in_execute`。

fmt、source layout 和 diff check 通过。全量测试及 Test262 采用主线集成回执。

### 代价与当前时间结果

这个实现仍要做每次 own slot 查找，没有安装写位置缓存。Shape.find 对不超过 8 个字段线性扫描，对更大 shape 使用既有 HashMap。新增准入涉及 slot 认证、release readiness、runtime 借用与类型检查；失败路径可能在 driver 中重复查找。特别是 RayTrace 大量 missing writes，是必须检查的新增成本，不能把不利结果只解释成“覆盖不够”。Reference 型新值会较早拒绝，但立即值写入到 reference 型旧槽会走过更多检查再回退。

主 agent 通知的独立 8 对时间点估计：Richards candidate/baseline 约 0.9616（耗时下降 3.84%）；Earley-Boyer 约 1.0106（耗时上升 1.06%）。这里不自行推断显著性或噪声区间，最终报告应保留正负结果及正式区间。不能将 Richards 收益推广到全部六项。

可直接复用历史 `ordinary_property_write.js` 作为本轮 participant；保留其文件内容和 hash，不为改变说明而修改历史输入。

## 2. shape transition 反向边去重

独立提交：`bf5d9b88`，基线 `7723e825`。
独立树：`/tmp/oxide-transition-edges`。
普通计时二进制：`/home/eric/.cache/oxide-six-build-transition/release/qjs`，构建身份同上。

### 目标与确定问题

本方向主要目标是限制重复使用同一 shape transition 引起的辅助索引增长，保持现有形状查找、所有权及弱引用回收合同；时间是需要测量的相关成本。

旧实现的 `append_transition` 即使 canonical successor 命中，也调用 `record_transition`：forward HashMap 的 `(parent, entry)` 被覆盖成相同 target；reverse `shape_transition_parents[target]` 却每次 Vec.push 同一 `(parent, entry)`。因此两个形状一直存活、同一边重复使用 N 次，反向记录可增长到 N 条；该增长由源码与定向测试直接证明，不需要先假设它占据多少 CPU 时间。

### 新合同和精确代价

- 保留原 forward HashMap insert，用返回的旧 target 判断关系是否已存在。旧 target 与新 target 完整 ShapeId（含 generation）相同，立即返回，既不搜索 reverse Vec，也不追加。
- 如果旧 target 不同，从旧 target 的 reverse Vec 中删除这个 `(parent, entry)`，为空时移除该 map 项，再追加到新 target。这样旧代际 shape 延迟 cleanup 不会通过残存反向边删掉刚绑定的新 forward 边。
- 新关系仍用现有 reverse Vec.push；没有引入 HashSet、全局扫描或新的固定元数据。
- 边仍是弱引用；append_transition 为调用者建立的 shape owner 及释放责任不变。没有通过保留 shape 来换取缓存命中。
- 重复命中路径：既有 HashMap 查找/insert 后多一次完整 ID 比较，避免了 reverse HashMap 查找、Vec 追加及其潜在扩容。是期望常数工作，不应写成无条件 O(1)：HashMap 冲突/扩容仍有最坏成本。
- 重绑路径：对旧 target 的 reverse Vec 执行 retain，成本 O(d_old)，其中 d_old 是该 target 的不同入边数。新边 push 摊销常数，但发生扩容时最坏 O(d_new)。不存在“所有操作都消除了线性搜索”的结论。
- 既有 unlink 仍会扫描对应 reverse Vec，canonical_successor 的 fallback 仍会比较 shape entries；本改动没有消除这些成本。

逻辑关系数现在随不同 forward 边增长，同边重复使用不再增长。**这不是全引擎内存上限**：不同 shape/不同边仍可增加；Vec/HashMap capacity 可能保留历史高水位，删除部分边不自动缩容。可证明的是相同活跃边重复使用不会扩大对应 reverse Vec 的 length 或 capacity，而不能声称所有容量始终等于当前边数。

### 已有验证与结果范围

Rust 1.88.0、profiling 下整个 shapes 模块 10 项通过，包括新测：

1. 相同 parent→target 重复 append/release 10,000 次，forward 边 1、reverse 边 1、reverse capacity 等于首次建立时、target strong count 为 1，机制事件准确为 10,000；随后父边失效可完整移除两个索引。
2. 旧 target 释放但延迟 apply_cleanup，同 arena index 的新 generation 接替后，旧 reverse 项已移除；应用旧 cleanup 后新 forward/reverse 边仍匹配，后续 target 回收可完整移除。

旧有测试还覆盖共享布局、唯一追加、形状回收、atom 生命周期及 key 顺序。fmt/source layout/diff check 通过。事件 `shape_transition_duplicate_avoided` 与 `shape_transition_rebound_edge` 仅 profiling 构建存在。

主 agent 已告知六项独立时间尚无明显收益。本方向应按“重复边存储增长得到限制”验收；不要宣布六项整体提速，也不要因为没有时间收益就忽略已证明的资源改进。真实负载是否大量重复同边、RSS 变化及总时间代价仍应使用主线机制与资源回执。

focused 输入：`/home/eric/.cache/oxide-six-transition-evidence/repeated-shape-edge.js`，预期 stdout `4999950000\n`。保持空 shape 与 x shape 的两个对象根，再重复 100,000 次 Object.create(null)+x 字段初始化，避免弱 target 在每轮消失。创建与释放对象的其他成本仍包含在完整进程时间中，因此该时间不能单独归因于 reverse Vec。

## 3. 下一步调研优先级

1. **先完成本轮采用决策。** 核对 scalar 字段的 Richards 收益与 Earley 等非参与路径代价；transition 按重复边 count/capacity、实际事件和 RSS 判断资源范围。结果在分辨率以内时不要扩充框架。
2. **区分属性读 IC 失效来源。** 基线 cache hit/miss 分别为 Richards 104,019/97,905，DeltaBlue 187,874/146,074，RayTrace 470,894/502,269。约半数 selection 进入 miss，但这并不等于重复全语义查找；当前 miss selection 已直接返回结果。先在诊断构建区分 2-shape PIC 超容量、短命 shape 代际、实际原型修改、尚未变热等，再决定是否改缓存。特别说明：heap 的 layout epoch 早已只对 used_as_prototype 对象的 layout 修改递增，普通新对象初始化不会一律触发全局原型失效。不能以“每个新对象都清空原型缓存”作为方案前提。
3. **RayTrace 构造时首次字段写入。** 194,985 次 missing committed 支持做最小调查：已有 canonical transition 命中后，storage.rs 仍 `slots.clone()` 再整体替换布局。先区分 slots 复制/retain、shape 操作、原型检查和构造调用开销；探索复用已选 missing 事实或局部追加事务。先验证可观察原型 setter、非 extensible、回退、原子所有权与失败清理，不把无条件字段预建当成语义等价。
4. **有测量支撑后再扩大 write 消费范围。** 如果 Richards 的剩余时间主要来自 reference 字段，可研究转移写入 owner 并安全释放旧目的地；当前 scalar 测试不足以证明这一步。若主要损失来自 missing failure，则考虑共享一次选择进度或缩小准入，先用区分实验竞争，不默认安装写 IC 或增加一种通用结果类型。

本文未声称缩短编译、启动、尾延迟或降低完整应用峰值；这些都需要对应目标与独立测量。

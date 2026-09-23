# S3-A 计划：8B 值表示——融合实施

> 状态：W1–W5 的 16B 实现与两轮修复已落地，但阶段 A 的性能门禁尚未全部通过。
> 最新补丁及发布配置对照见 §8.13（`d4f78697`）；完整正确性记录见 §8.12，
> §8.13 另列补丁后的验证范围。双方 fat LTO + CGU=1、无 PGO 下，scaling
> 几何均值约持平，V8 八项仍落后于 pre-A；不得沿用旧无 LTO 结果宣告关闭。
> 后续按 `performance-architecture.md` §11 的 E → 有界残余批 → A4 → C → B → D
> 推进，保留每项未关闭回退的台账，不代表阶段 A 已验收完成。
> 本文负责方案 A 的表示与所有权设计；性能比较以 §5 和
> [benchmark README](../../scripts/benchmark/README.md) 的现行协议为准，
> 阶段顺序以架构文档 §11 为准。历史实测保留各自构建口径，不混算。

---

## 0. 设计决定（动工前钉死）

### D1：内部值类型与转换层先行——`Value` 只是公共 API

**事实**：`Value` 由 `src/engine/api/mod.rs:28` 公开导出（`pub use
crate::engine::value::{JsString, JsStringError, Value}`），内部数百个文件
直接使用它。方案 A 要求内部 8B，必须把「公共值」与「内部值」拆开，否则
改动面失控。

**决定**：

1. 公共 `Value`（`src/engine/value/mod.rs:13-24`，含 `ObjectRef` 等带
   `Rc<Runtime>` 的 root 类型）**保持不动**，只在 `engine::api` 与宿主回调
   适配层出现；公共签名一个不改。
2. 新增 crate 内部类型 **`JsValue`**（`src/engine/value/js_value.rs`）：
   - W1–W5 为 **16B 句柄 enum**：标量内联（`Int(i32)`/`Float(f64)` 等），
     堆类型为 `{index: u32, generation: u32}` 句柄；
   - A4 阶段再编码为 **u64 索引 NaN-box**（`performance-architecture.md`
     §4.1），若实测不划算则停在 16B（§4.3 退路，已比现状 32B 小一半）。
   - 不实现 `Copy`/`Drop`；显式 `dup`/`release` 纪律见 §1.2。
3. 转换层仅两个方向、只挂在 `Runtime` 上：
   - `unroot`（进引擎）：`&Value → JsValue`（dup 堆边）与
     `into_jsvalue(Value) → JsValue`（消费 root，省一次 retain/release 对）；
   - `root`（出引擎）：`JsValue → Value`（retain + 包装 Rc root）。
4. **边界规则**：`Value` 不得出现在 `engine::api` 与转换层之外——评审
   规则，若便宜则加进 `scripts/checks/check-source-layout.py`。
5. **穿越点清单**（公共 `Value` 进出引擎的全部位置，转换层只挂这些点）：
   - `Context::eval` / `eval_bytes` → `Value`（`api/context/script.rs`）；
   - `Context::execute` → `Value`（`api/context/calls.rs`）；
   - `Context::take_exception` → `Option<Value>`（`api/context/mod.rs`）；
   - `Context::new_array_from_values(Vec<Value>)`（`api/context/objects.rs`）；
   - native 调用参数缓冲：`&[Value]` / `Vec<Value>`（`builtins/dispatch.rs`
     等），内部以 `RawValue` 持有、边界再 root；
   - promise jobs / module loader / test262 agent 均经 `engine::api` 或内部
     `RawValue`，没有额外的公共 `Value` 签名；
   - `adapters/native` 仅转导出 `engine::api`，`adapters/web` 只用 wasm 侧
     `wasm_bindgen::JsValue`，不直接持有引擎 `Value`。

### D2：String/BigInt 存储层句柄化，公共 `JsString` 不动

**事实**：`RawValue`（`src/engine/heap/identity.rs:200-224`）对
Object/Symbol 已是句柄（`ObjectId`/`Atom`），但 `String(JsString)` /
`BigInt(JsBigInt)` 仍是 `Rc` 负载（`value/primitive.rs:20`、
`value/bigint.rs:104-115`）。`performance-architecture.md` §6 的 typed
arena 清单只列了 Object/VarRef/Shape/Context/FunctionBytecode——不堆化
String/BigInt，8B 无从谈起。

**公共 `JsString` 保持 runtime-free 纯计算类型**，约束证据：
`from_static` 全仓 1377 处、`try_from_utf*` 853 处均为无 runtime 的纯
构造；`impl JsString` 约 183 个方法全部 runtime-free；
`value/collection_key.rs` 是显式「no heap access」纯模块；任何带 runtime
的公共字符串表示都会破坏尺寸目标与公共表面。句柄化只发生在**值存储层**。

**决定**：

1. `HeapNodeKind`（`src/engine/heap/identity.rs:127-133`）新增 **`String`
   与 `BigInt`** 两个独立 kind（不合并），与 §6 typed arena 对齐；新增
   `StringId`/`BigIntId` 句柄（`heap/identity.rs`，
   `{index: u32, generation: u32}`）。**arena 节点持有现成的
   `JsString`/`JsBigInt`**（Rc 负载原样）；对象槽、常量池等对
   String/BigInt 的边纳入既有事务化 retain/release 与 `Edges` 遍历。
2. **cycle 处理：cascade-only，永不做 anchor**——string 节点零堆出边
   （rope 子节点留在 `Rc` 树内，不进 arena），BigInt 无出边，平凡成立。
3. `StringRepr`/rope 算法与 `impl JsString` 整体不动；arena 节点只是
   `Rc` 的一个持有者。同一节点多次读出克隆同一个内部 `Rc`，`ptr_eq`
   身份快路天然保留，`same_representation` 语义不变。
4. **公共 `JsString`/`Value` API 零改动**（`Rc<StringRepr>`，runtime-free
   构造全保留），`adapters`/oracle 不受影响；
   `RawValue::String(StringId)`/`BigInt(BigIntId)`，`JsValue` 同；堆→值
   边界经 arena 解引用后克隆 `Rc`。
5. `AtomTable` 基本不动：`strings` 仍按 `JsString` 键、`released_strings` /
   `WeakJsString` 保持；§6.4 的 hash 缓存（`StringRepr` 头部缓存 hash、
   atom 表换 FxHash）作为独立项照做。
6. **内容相等适配**：`RawValue` 的 `PartialEq` derive
   （`identity.rs:200`）必须移除——句柄 id 相等 ≠ 内容相等。
   `collection_key.rs`（`same_value_zero`/`hash`）、StrictEq、switch
   字符串匹配改为「id 相等快路 + arena 解引用内容兜底」；动工先盘点
   全部 `RawValue` 相等性使用点。
7. BigInt：原先 Short 也全 arena 的决定已由 §8.7 修订：当前 16B
   `JsValue`/`RawValue` 使用完整 `ShortBigInt(i64)` 立即值，真正 Heap BigInt
   保留 `BigIntId`；公共 API 与算术内核保持不变。性能关闭仍须对应测量。
   不把恢复短值优化推迟到 A4；将来 8B NaN-box 的短整数编码范围另行决策。
8. 内存语义注意：字符串/BigInt 从「`Rc` 独立分配」变为「arena 节点 +
   free-list 复用 + generation」——teardown 的 `live == 0` 断言与
   `GcStats`/`HeapCounts` 公共诊断（`api/mod.rs:15` 导出）口径需同步
   更新。
9. **测量点**：瞬态字符串（concat/slice/`number_to_string`）的 arena
   churn 会推高 zero_queue 水位，而 zero_queue 非空使 IC 快路 decline
   （`ordinary_storage/ic.rs:27-35`）；测量必须含 string-heavy 负载，
   确认不放倒 IC 快路。

### D3：`Atom` 内部 `u32`，品牌只留边界

**事实**：`Atom { raw: u32, generation: u32, table_id: u64 }`
（`src/engine/atom/mod.rs:52-57`）16B，相等比较逐 16B；shape 线性扫描
（≤8 项）与迁移表键全在吃这个体积。

**决定**：

1. 新增内部类型 **`AtomIdx(u32)`** newtype；保留 immediate-int 高位 tag
   （`ATOM_TAG_INT`，QuickJS parity 不动）。16B branded `Atom` 只留公共
   API（`PropertyKey` 等）与跨 runtime 进入点。
2. **存活不变量**（与 `live_node_fast` 同一论证）：内部 `AtomIdx` 只能由
   「已 retain 该 atom 的 owner」持有——shape entry、字节码
   `property_key_atoms`、pinned 集。可信路径免品牌校验（debug 构建全量
   校验），边界全量。
3. `AtomTable::Entry.ref_count` 改 `Cell<u32>`，retain/release 在共享借用
   下完成——`Symbol` 从所有快路 decline 名单移除。
4. 级联：`ShapeEntry`（`object/shape.rs:73-77`）24B→~8B（u32 atom +
   flags）；shape 迁移表键、shape fingerprint 同减；
   `RawValue::Symbol/Private` 负载 16B→4B，为 `RawValue` 8B 化扫清最后
   一个超标变体。
5. `Atom` 的 `Hash` 现为 `generation<<32|raw`（`atom/mod.rs:59-65`）；
   内部 `AtomIdx` 直接以 raw 作 hash（Fx），不再移位拼装。

---

## 1. 终态设计

### 1.1 类型格局

- **`JsValue`**（crate 内部，16B enum）：标量内联 +
  `String(StringId)`/`BigInt(BigIntId)`/`Symbol(AtomIdx)`/`Object(ObjectId)`；
  无 `Copy`/`Drop`。
- **`RawValue`**（堆存储形态）：同一套句柄 + `Private`/哨兵；与 `JsValue`
  互转是无分配的同 id 拷贝。
- **`Value`**（公共）：只在 `engine::api` 边界与宿主回调适配层出现（D1）。

### 1.2 所有权纪律（一条规则）

每个存储位置（堆槽、帧、操作数栈、记录、常量池、pending_exception）持有
其句柄的一条边：

- **store**：拷贝入库 → retain；**move 入库 → 交接生产者边，不产生计数对**；
- **读出**：dup（retain）交出 owned `JsValue`；纯读取可原地借用；
- **overwrite / pop / finalize**：release；
- **String/BigInt 节点只在真创建点分配**：字符串/大整数产生运算、字面量
  publish、api/host 输入转换；**store 永不分配**；
- dup/release 走既有快路纪律（可信 `Cell` retain；release 经
  `release_or_defer`）；
- **无 RAII 包装**：`JsValue`/`RawValue` 均无 `Drop`，所有权全靠上述显式
  纪律——任何「自动释放」包装都会把堆访问需求带进值类型的 drop 路径，
  与「值的 drop 不需要堆」的既有纪律冲突。

### 1.3 owner 记录的 Drop 例外（suspension/边界层）

挂起/恢复/放弃类记录（构造器 resume state、proxy 请求、`ReturnOwner`
等）持 `runtime: Runtime` 字段并实现 `Drop`，对仍被持有的 `JsValue` 边
调 `release_jsvalue`——与 `ObjectRef` 先例同构（`object/mod.rs:107-111`：
owner 容器带运行时释放，不是值类型带 Drop）。约束：

1. **适用范围**：只给 suspension/边界层 owner 记录；堆节点内的值存储
   （对象槽、常量池）仍由 finalize/overwrite 纪律负责，不走此路。
2. **Drop 必须 nothrow**：只调 `release_jsvalue`（内部走
   `release_or_defer`，借用被持有时进 deferred 队列）；禁止在 Drop 里
   直接 `borrow_mut().unwrap()`；不跑 JS、不产生 JS 可观察行为，纯边
   释放。
3. **不双释放由构造保证**：消费一律经 `Option::take`（Drop 看到 `None`
   即跳过）；`Vec<JsValue>` 字段在 Drop 里 drain 逐个 release。
4. **成本口径**：此 `Rc` 是按控制流事件（每次挂起/放弃一次）付的边界
   所有权，且替代等量的存量成本（原 `Value` 记录内含 `Rc` 根，持 N 个
   值则 N 个 `Rc`，现为记录级 1 个）——不属于本阶段消灭的「按值流动
   按次征收的 `Rc` 税」。若未来 profile 点名挂起记录变热，可对特定记录
   类型降级为显式 release 穿线，属测量门禁后的微优化，默认不取。

## 2. 借用与分配放置规则

1. **转换提出借用区**：值→`RawValue` 的转换必须发生在任何 `state` 借用
   之外。值刚从持有借用的结构读出的路径，先结束借用、转换、再重新借用
   ——单线程引擎、两次借用之间无 JS/native 回调，拆分借用语义不可见。
2. **物化沉进事务**：批量/事务性存储路径（`retain_edges_transactionally`、
   publish、dense 写）把 String/BigInt 节点分配放在事务内部（本来就持
   `&mut`、本来就走边），同 shape 分配先例（`get_or_create_shape`/
   `append_transition` 在持有 `&mut RuntimeState` 的 store 事务内分配堆
   节点）。
3. **借用拓扑审计先于编码**：`RefCell` 借用次序是运行期行为，编译器抓
   不到。每个工作流动笔前，先列出它触到的热路径调用点当前的借用持有
   情况（谁持 `state` 借用、转换/分配放在哪一层），按规则 1/2 放置后
   再写代码。重点审计：`property_ic_write_scalar` 及 IC 写路径、dense
   写、bytecode publish、挂起/恢复、`raw_property_value` 全部调用点。
4. **升级条款**：某条路径疑似无法提出借用区时，举证标准 = 两次借用之间
   存在 JS 可观察行为；成立则对该点用规则 2。两条都走不通才允许复审
   「独立 `RefCell` 侧 arena」方案（拆锁式治标，与 §6 typed arena「物理
   拆分、纪律统一」方向冲突），**不允许静默采用**。

## 3. 执行序列（compiler-driven）

回退单位 = 整个大阶段（分支级）；中间 commit 不要求可编译；WIP commit
只留本地，推送以绿为准。

| 工作流 | 内容 |
| --- | --- |
| **W1** | 句柄与转换层地基：`HeapNodeKind::String/BigInt` + typed arena（allocate/retain/release/finalize/counts + trusted 访问器）；`StringId`/`BigIntId`/`AtomIdx` 句柄；`AtomTable::Entry.ref_count` `Cell` 化；`JsValue` 与四转换函数（unroot/dup/release/root）完整实现；deferred release 通路 |
| **W2** | 堆存储层：`RawValue` 句柄化（String/BigInt/Symbol/Private 全句柄，移除 `PartialEq` derive）+ `raw_value_edges` + 事务 retain-release + collection_key/index heap 化；`raw_property_value` 退役为纯 strip，仅供边界 |
| **W3** | VM 核心：`FrameBinding`/`SlotStore`/`run.rs` → `JsValue`；帧建立/拆除、挂起编解码、调用约定的显式 dup/release/move |
| **W4** | builtins 与 drivers 签名 `Value`→`JsValue` |
| **W5** | api 边界：`eval`/call/host 回调/promise jobs/module loader 的唯一 `Value`↔`JsValue` 转换层 |
| **W6** | 测试适配 + 全门禁 |

A4（NaN-box 编码）为独立的测量门禁后续阶段，不在本序列内。

## 4. 验收规则

**设计一致性（评审第一顺位）**：

1. 每个引入的类型/函数/构造必须属于 §1 终态设计；仅用于让中间态编译
   通过的临时构造一律不接受——编译器报错要求的改动，要么按终态设计
   改到底，要么不改。
2. 借用与分配放置符合 §2；任何新增分配点必须能指出它属于「真创建点」
   或「事务内部」。
3. 公共表面不变：`tests/checked_string_construction.rs` 零改动通过；
   `engine::api` 签名、`adapters/*` 零改动；任何需要改公共测试的迹象
   即警报。
4. `runtime`+`Drop` 只出现在 §1.3 范围内的 owner 记录上；逐迭代结构或
   逐值容器中出现即评审驳回。

**尺寸断言（编译期钉死）**：`JsValue` = 16B；`AtomIdx` = 4B；
`ShapeEntry` = 8B；`RawValue` ≤ 16B。

**语义门禁**：`RawValue`/`JsValue` 均不 derive `PartialEq`，相等性使用点
全部改为「id 快路 + 内容兜底」，SameValueZero 语义逐点核对
（`collection_key`、StrictEq、switch 字符串匹配）；teardown
`live == 0` 断言与 `GcStats`/`HeapCounts` 口径适配。

**全门禁（大阶段末一次）**：`cargo fmt --check` → clippy 1.88
`-D warnings` → `cargo test --locked --workspace --all-targets` →
Test262 `--check`/`--focused`/`--full` 零回归（清理 `GIT_*`，只产
current-source receipt，不改 `current.conf`）→
`check-source-layout.py` + rust-only 门禁 → benchmark receipts
（`property_read_probe.py` + `scaling.py` + `run.py`，串行、独立输出
目录，协议见 §5）→ profiling 计数器符合实际执行路径（消除 root/dup 后的
下降属于预期收益，需逐项解释；禁止仅为保留旧数字而增加计数）→ 实测结果记入本文档。

**委托执行交底**：任务拆分委托时，提示词必须包含 §1 终态设计、§1.2
所有权纪律、§2 放置规则与「临时构造不接受」条款；评审先看设计一致性，
再看编译。

## 5. 阶段性能比较协议

现行协议（2026-09-22 修订）与
[benchmark README](../../scripts/benchmark/README.md) 一致：

1. **固定基线**：从 pre-A `85afd564`（代码与 `deac0a39` 相同）按发布配置
   重建：`CARGO_PROFILE_RELEASE_LTO=fat`、`CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`，
   **无 PGO、无 profiling**。候选使用相同 toolchain、target 与 flags；记录
   源码、构建环境、二进制及 workload 哈希，不能直接复用旧无 LTO 二进制。
2. **阶段与累计对照**：本阶段 vs 上一阶段、本阶段 vs 保存基线，双方均使用
   上述协议。后续 E 若另存 post-A 的 `saved` 基线，明确其源码身份，并继续
   单列相对同协议 pre-A 的结果；相对 E 后基线变快不等于阶段 A 回退已关闭。
3. **PGO 复核**：不要求每阶段重训；每个大阶段收尾建议（非强制）双方按
   相同训练负载分别重训并单列 LTO+PGO 结果，不能冲抵无 PGO 发布配置的回退。
   E 的构建配置实验单独标注双方协议，不代替上述阶段门禁。
4. **历史与跨协议报告**：§8.12 及之前的无 LTO/CGU16 数据继续只与各自旧基线
   比较；§8.13 同时保留两套结果，现行回退裁决使用 LTO 列。跨协议对比只用于
   标明双方构建协议的累计/用户口径报告，不用于阶段门禁或性能归因。

## 6. 风险

- **手工 RC 纪律扩大 panic 面**（VM 接线起）：debug 构建维持全量
  generation 校验 + 冻结向量兜底；trusted 访问器遇 stale 即 panic 的
  政策不变。泄漏类回归的可观测性补漏手段见 §7。
- **触及面全仓最大**：值类型是所有模块的公共依赖；融合大扫除期间允许
  长时间不绿，回退单位是整个大阶段（分支级）。
- **8B 索引 NaN-box 无生产先例**（附录 A.8）：每次 deref 多一次 base
  load + bounds check；A4 必须以测量定去留，退路（16B enum）不是失败
  而是默认值。
- **行为敏感点**：集合键 SameValueZero 与内容 hash（`collection_key.rs`）、
  StrictEq/switch 的字符串路径、teardown `live == 0` 断言、
  `GcStats`/`HeapCounts` 口径；`same_representation` 与
  `released_strings` 在 D2 下**不变**（公共 `JsString` 不动）。

---

## 7. 工程化补漏手段：边账本 / 边守卫 / move 优先签名

> 动机：句柄化之后，泄漏类错误从「类型层面不可能」变成「每个交接点都可能」，
> 而现有唯一检测器（teardown `live == 0` 计数断言）只报总数、不报来源，
> 修复流程退化成手工排查。本节钉死三条补漏手段；它们**只在边界与 debug
> 构建出现，发布热路径零成本**，且不构成对 §1.2「无 RAII 包装」纪律的
> 任何豁免（值类型永远无 `Drop`）。执行顺序：7.1 → 7.2 → 7.3；7.2/7.3 与
> W3 尾部合并执行最省。

### 7.1 边账本（debug-only edge ledger）——把计数断言升级为溯源断言

**决定**：

1. `RuntimeState` 新增 `#[cfg(debug_assertions)]` 账本：节点句柄 →
   `LedgerEntry { outstanding_edges: u32, alloc_site: Backtrace }`；
   retain/release 只核销计数（不落回溯，控制成本）；teardown `live != 0`
   时打印每个幸存节点的**内容摘要 + 创建点回溯 + 残余边数**，随后照常
   assert。
2. 挂点收敛在 `Heap` 的 `allocate_string`/`allocate_bigint`（创建）、
   `retain_raw`/`release_raw_no_drain`（计数增减）与 `AtomTable` 的
   index 级 retain/release；**调用点零改动**。
3. 发布构建（非 debug_assertions）无字段、无调用、零成本；不改变任何
   语义，因此可以独立于工作流落地、先上。

**验收**：人为制造一个已知泄漏（如撤销某处释放）→ teardown 报告直接给出
创建调用栈；全绿时账本为空；`cargo test --lib` 在 debug 下时长不回退超过
实测阈值的 2×（超了就把回溯采样改为仅创建点）。

### 7.1 实现记录（2026-09-21）

- 账本落在 `Heap`（`alloc_sites: Vec<Option<AllocSite>>`）与 `AtomTable`
  （`alloc_sites: Vec<Option<AtomAllocSite>>`），而不是 `RuntimeState`：
  挂点 `reserve`/`allocate` 与 retain/release 都在 arena/atom 表内部，放进
  `RuntimeState` 需要把借用穿过每个挂点，反而扩大改动面。
- 残余边数直接读节点自带的 `strong` 计数，retain/release 零改动；因此
  「计数核销」不落账本，只在创建点写一条记录（`O(1)` 槽位索引，槽位回收
  时清除）。
- 创建点回溯仅在显式诊断环境变量（`QJS_EDGE_LEDGER` / `QJS_TEARDOWN_PROBE`
  / `QJS_TRACE_ROOTS`）下采样，保证 debug 测试时长不回退：实测全量 lib
  套件 54.7s（基线 54.5s）。
- teardown 报告：`live != 0` 时打印 `[ledger]`（外部根 + 残余边数 + 创建
  调用栈）与 `[atom-ledger]`（非 pinned 且 refs > 0 的 atom，最多 8 条），
  随后照常 assert；`QJS_TEARDOWN_PROBE` 保留「只打印不 panic」的诊断语义。
- 创建栈只能定位幸存节点的创建位置，不能直接证明遗漏释放发生在该位置。
  realm/context 创建的大量节点可以被一个遗漏的外部边连带保活；须结合
  `strong − 内部入边` 与实际所有权交接路径定位根因。

### 7.2 边界边守卫（scoped edge guard）——用编译器消灭「忘释放生产者边」

**事实**：§2 的真创建点（边界转换）产生带一条生产者边的值，目前每个调用点
手工保证「存储成功后释放」——这是历次泄漏的最大重复模式（数十个调用点、
每个都靠人肉覆盖每个错误出口/早退）。

**决定**：

1. 新增栈局部守卫类型（示意）：

   ```rust
   pub(crate) struct ConvertedValue<'a> {
       runtime: &'a Runtime,        // 借用，无 Rc 计数
       value: Option<RawValue>,     // 携带一条生产者节点边
   }
   impl Drop for ConvertedValue<'_> {
       fn drop(&mut self) {
           if let Some(value) = self.value.take() {
               self.runtime.release_converted_value_edge(&value);
           }
       }
   }
   impl<'a> ConvertedValue<'a> {
       /// 存储采纳：移交值与边，守卫排空，Drop 不再释放。
       pub(crate) fn take(&mut self) -> RawValue { /* value.take() */ }
   }
   ```

2. **采纳是唯一显式动作**：`converted.take()` 移交边给事务存储；未被采纳
   的边在守卫作用域退出时由 Drop 经 `release_or_defer` 释放（借用中压队，
   既有通路）。
3. **逃逸不可能**：生命周期参数让守卫类型上逃不出当前函数；它不进堆槽位、
   不进操作数栈、不被 memcpy——§1.2 的 memcpy/析构时刻约束不适用，Drop
   合法。
4. 应用面：`raw_property_value`（及其 `*_jsvalue` 变体）的全部调用点改为
   守卫式；`ConvertedValue::take()` 后的存储路径沿用现有事务（不变）。
5. 槽位纪律不变：pop/overwrite/teardown 的手工 release 不受守卫覆盖
   （那里不允许 Drop），由 7.1 账本检测兜底。

**性能论证**：发布热路径零变化——Drop 的释放就是原手写释放的同一调用，
代码生成相同，多一次栈上 `Option` 检查；守卫不碰值布局、不产生新计数对。

**验收**：`raw_property_value` 调用点盘点无手写 `release_converted_*` 残留
（`check-source-layout.py` 加对应源级规则）；全量测试全绿；评审先看「守卫
未逃逸、take 只取一次」。

### 7.2 实现记录（2026-09-21）

- 守卫类型 `ConvertedValue<'a>` 落在 `heap/ownership.rs`：持有 `&Runtime`
  与 `Option<RawValue>`；`raw()` 克隆句柄、`take()` 移交值与边、`disarm()`
  在 by-value 拥有者接管后停止跟踪；`Drop` 经
  `release_converted_value_edge` 释放生产者边。
- `Runtime::raw_property_value` 返回 `Result<ConvertedValue<'_>, ...>`；
  22 个文件、41 处调用点全部改为守卫式，删除手写 release 与
  `release_constant_edges` 辅助（净删 217 行）。
- 顺带修复旧泄漏：Promise capability capture、iterator helper 与
  `allocate_string` 失败路径原先未释放生产者边。
- 源级规则：`check-source-layout.py` 新增「调用 `raw_property_value` 的
  文件不得出现手写 `release_converted_{value,node}_edge`」。
- 验证：默认 lib 2270 通过、`test262-host` 2326 通过，clippy `-D warnings`
  / `cargo fmt --check` / source-layout / rust-only 全绿。

### 7.3 move 优先签名——把「交接生产者边」从评审规则变成签名规则

**事实**：§1.2 已钉死「move 入库 → 交接生产者边，不产生计数对」，但目前
采纳路径多为「借入 + 内部 dup + 调用方事后 release」，靠人肉配平。

**决定**：

1. **采纳即 by value**：吞掉值所有权的存储/记录/缓冲函数参数一律取
   `JsValue`（或 `RawValue`）by value，并明确成功与失败的边责任：成功交接，
   失败释放或连同错误返还。值类型没有 Drop，按值传参本身不会释放边，
   也不能阻止调用方丢弃值或 `.clone()` 复制非拥有句柄。
2. **产出即 owned**：返回 owned 值的函数保持 by-value 返回，值类型标注
   `#[must_use]`——仅提示未使用结果，`drop(JsValue)`/`drop(RawValue)`
   不会释放边，不能用它们消除告警来代替 `release_jsvalue`。
3. **dup 显式化**：调用方需要「存完再用」时显式 `dup_jsvalue`；dup 从被调
   函数内部挪到调用方可见处。
4. 执行与 W3 尾部/W4 签名切换同波完成（同一批调用点），逐文件推进，
   编译器驱动。

**性能验收**：仅在实现实际转移原有生产者边时，采纳点才消除一对
dup/release。按值传参不保证寄存器传递或计数下降；必须检查函数体、错误
出口和 profiling。纯借读接口保留借用，不机械改写。

**已知覆盖缺口（诚实清单）**：`let v = pop();` 后遗忘（赋值未用）Rust 不告警；
槽位 pop/丢弃的手工 release 不受签名规则覆盖——二者由 7.1 账本检测。

**验收**：clippy `-D warnings` 全绿；全量测试全绿（teardown 断言照常）；
评审规则写入「采纳签名一律 by value」。

### 7.3 实现记录（2026-09-21）

- 值类型标注 `#[must_use]`：`Value`、`JsValue`、`RawValue` 三个类型定义
  直接标注；删除 5 处冗余的函数级 `#[must_use]`（clippy
  `double_must_use`）；库内 28 处、非库目标（CLI oracle 测试与 conformance
  runner）99 处「丢弃返回值」改为显式 `drop(...)`，全仓无
  `#[allow(unused_must_use)]`。
- 采纳签名改 by value（本轮清单）：`retain_raw_root`、`root_raw_value`、
  `decode_raw_jsvalue`（suspend 与 async-from-sync 两处实现）、
  `CollectionIndex::insert/remove` 由 `&RawValue` 改为 `RawValue`；调用方在
  仍需句柄处显式 `.clone()`。这些历史签名变更本身不是所有权迁移证明：
  `RawValue::clone()` 只复制句柄、不 retain；`root_raw_value` 仍新增公共根，
  不自动释放输入边。应按函数契约分别审计借读、dup、消费与失败出口。
- 诚实缺口：行式 rg 清单低估了规模——全仓实际有约 106 个 `&Value` 与 28 个
  `&RawValue` 参数签名；属性写、dense array、typed array 等 `&Value` 采纳
  路径仍是借用签名。按 §7.3「逐文件推进，编译器驱动」在下一波继续。
- 验证：默认 lib 2270 通过、`test262-host` 2326 通过，clippy
  `--workspace --all-targets -- -D warnings` 零警告，`cargo fmt --check` /
  source-layout / rust-only 全绿。

### 7.4 早期实现状态与 A 阶段验收口径（历史快照）

下表为早期状态，当前实现与证据以 §7.8 和收口报告为准。

| 手段 | 状态 | 覆盖 | 缺口 |
| --- | --- | --- | --- |
| 7.1 边账本 | 完成并实战验证 | 全部 heap 节点 + 非 pinned atom | 创建栈采样需 `QJS_EDGE_LEDGER` 等环境变量；atom 报告截断 8 条 |
| 7.2 边守卫 | 完成 | `raw_property_value` 全部 41 个调用点 + 源级规则 | 两个通用辅助保留为 `drop(ConvertedValue::new(...))`；`roots.rs` 一处非守卫释放（不调用转换函数，规则允许） |
| 7.3 move 优先 | 部分完成 | `#[must_use]` 三类型 + 全仓零 allow；6 个采纳函数改 by-value | 约 106 个 `&Value`、28 个 `&RawValue` 签名仍为借用，属性写/dense array/typed array 待逐文件推进 |

**A 阶段尚未验收**：上述历史实现与局部门禁不替代 §1–§5 的完整要求。
内部 `Value` 往返、String/BigInt store 再分配属于 W4/W5 终态缺口，不能仅
以 §7 检查通过为由延期到 B/D。纯借读签名本身不是缺口。

**失败清单纠正**：oracle 测试语义断言失败后，`Runtime::drop` 的泄漏断言
可能再次 panic，导致测试二进制 SIGABRT；此前同一进程中的“剩余失败数”
和“约 19 个 realm 泄漏”均不可靠，也不能把未执行测试视为通过。

### 7.5 本轮验收基础设施与证据边界

- `scripts/checks/run-oracle-isolated.py` 对编译后的精确 Rust 测试二进制先
  `--list`，再逐测试 `--exact` 独立进程运行；有界并行、进程组超时终止，
  每测试保存日志及 JSON receipt，区分失败、abort、signal、timeout、
  ignored、缺汇总及缺测试。汇总必须与枚举集合逐一配平。
- runner 强制移除 `QJS_TEARDOWN_PROBE`，保留正常 teardown/release 断言；
  用该变量获得的诊断输出不能作为通过证据。过滤运行明确记录 scope，
  `passed` 只表示所选范围，完整 oracle 验收还要求 `full_scope=true`。
- debug teardown 独立检查非 pinned atom 存活数，覆盖 heap 已空而 atom
  仍泄漏的情况。pinned intrinsic atoms 不纳入泄漏；global symbol 注册表
  是非拥有索引，不能豁免非 pinned symbol 的剩余引用。
- Symbol 的内部值释放保留共享借用的 Cell 快路；可变借用期间以 `AtomIdx`
  进入现有 deferred 队列，避免 owner Drop 为 branding 强行借用而 panic。
- 若 Runtime 自身被 Rc 环保活、析构完全没发生，teardown 无法检测；这仍需
  审计宿主回调/挂起 owner 的持有关系及专门生命周期回归，不能声称账本完备。
- 本节描述实现能力，不宣称本轮 oracle、Test262 或基准已经通过；最终结果
  必须附当前源码 receipt。源码修复完成后统一执行门禁，不重复跑全套定位。

隔离运行示例（测试二进制路径取 `cargo --message-format=json` 的当前构建产物，
不得用 glob 随意挑旧二进制）：

```sh
cargo test --locked -p quickjs-oxide-cli --test oracle --no-run --message-format=json
python3 scripts/checks/run-oracle-isolated.py \
  --binary /absolute/current/debug/deps/oracle-HASH \
  --output target/s3-a-oracle-isolated --jobs 4 --timeout 120
```

**基线证据**：pre-A 实现回退点为 `deac0a39`；随后仅文档修订的
`85afd564` 可作为同代码基线重新构建。现有 `target/bench-e-baseline` 与
`target/bench-e-*` 为较早阶段资产，路径元数据不足以认证它就是 A 的固定
分母；`target/bench-f-*` 含 PGO 配置，也不能直接用作 §5 的同协议比较。
若没有开工前不可变 receipt，须如实标注“从 pre-A commit 重建”，固定
commit、构建 flags、二进制 hash 与工作负载 hash，不能倒填为原始实测。

### 7.6 前一收敛快照的源码证据（历史，未替代 W6）

本节记录早期迁移状态；下列残余适配已由 §7.8 后续实现继续收口，不代表当前待办。

尺寸由现有编译期断言约束：`JsValue = 16B`、`AtomIdx = 4B`、
`ShapeEntry = 8B`、`RawValue ≤ 16B`。两种值均无 Copy/Drop；生产环境无
PartialEq（`JsValue` 的测试专用表示比较不用于语言相等性）。A4 的 u64 编码
仍独立，不把 16B 工作流的完成误写为已经得到 8B。

本轮直接保留句柄的路径包括：普通属性读取、VM Set 的 assigned operand、
普通属性新增/覆写、dense 数组新增/覆写、稀疏 Array index 定义、mapped
Arguments 写入、setter 调用参数、TypedArray Set 原始值与元素数值转换、
Array length 两次 ToNumber 原始值。Array index 的内部定义复用
`PropertyDescriptor<RawValue>` 与原有 SameValue/事务规则；StringId 不变
回归覆盖普通属性、dense 和 sparse 路径，尚需当前构建执行证据。

`SetResume`、`ArrayLengthResume`、`TypedWriteResume`、setter/转换请求等
控制流 owner 持有运行时（或现有 ObjectRef 的运行时）并清理未交接的边；
没有给 `JsValue`/`RawValue` 增加 Drop。ArrayLength 保留 QuickJS 非数字
输入两次 ToNumber 的顺序，并复用同一 resume 分配。

后续同轮实现已将 Promise settlement、Promise.finally settlement capture、
WeakMap（包括 computed callback 的结果）、FinalizationRegistry heldValue、
挂起 normalized_this、private field、dense push/pop、unmapped Arguments、
Function.bind 捕获与 async generator request 输入改为传递原句柄。
Promise.finally 的具体挂起记录在取消/异常时释放 settlement，thunk 创建时
复制同一 raw handle；WeakMap 与 FinalizationRegistry 在事务中保留输入边。
普通 computed literal 定义与 iterator append 也直接使用 raw descriptor 核心。
这些是源码事实，不等于当前源码已取得全套 oracle/基准通过证据。

**剩余适配必须逐点分类**：公开 API、真实宿主输入、常量 publish 和真正的
字符串/BigInt 创建点可产生 arena 节点；内部 `Value` 往返后又进存储不因此
变成合法创建点。最终源码盘点应使用具名调用链，而不是不稳定的行数/总数：

- `code/runtime.rs` 的常量 publish、`qjs_value_printer` 的公开 Value 输入属于
  边界；Arguments length 是新建数字，不能用转换调用数量推断 arena 分配。
- 一般 ordinary 与 dense/sparse Array Define 已用 raw descriptor 核心保留
  当前值，attribute-only 修改不再重建 String/BigInt；公开输入只转换显式
  value 字段一次。`try_define_ordinary_value` 先确认可处理再转换，decline
  不分配。普通 Set 已覆盖 canonical existing/missing own property。
  但 Proxy/general Define producer 仍有公开 Value 描述符，global/VarRef
  特殊存储仍有旧适配，不能因此声称所有内部 descriptor 流均已无往返。
- iterator wrap/from/helper/concat 捕获、module evaluation failure 缓存、
  pending exception 的内部入口已改为 JsValue 原句柄；公共 Value 入口保留
  在适配层。helper 的部分 callback item/close reason 仍有 Value 算法状态，
  不能把捕获存储闭合等同于 D1 全局无 Value。
- RegExp result 的编号捕获和命名组共享原 StringId，indices 对象身份保留；
  capture substring 与 input 首次发布属于真实创建。JSON module 默认导出
  和 legacy async-from-sync Value 适配仍应按具体内部调用者分类。

不能把 `raw_property_value` 有守卫、或测试未泄漏，当成 store 无分配的证明。
上述尚未闭合的内部调用链及 §5 门禁未有完整证据前，A 仍不得标记验收完成。


### 7.7 历史验证记录（仅对应 d184986f）

`d184986f0da982931f35a3f2d923f10f5857756a` 曾完整通过 2576 个库测试和
912 个 oracle 测试（包含 ignored），正常 teardown 断言保留。完整 Test262
80032 eligible 中 79982 pass，43 runtime / 7 parse 失败与固定基线一致。
源码指纹为 `f3fde6dacadbfdfca58b1eb7c3381e5e9f17af827eced765440d88f8320409d0`。

后续 builtins、异常传输、包装对象和失败交接迁移改变了源码。本节通过记录
**不能用于认证当前版本**；本轮按用户要求不运行 unit/oracle/Test262。

### 7.8 当前实现收口（2026-09-21）

W1–W5 的 16B 路线已完成本轮实现收口：内部普通值与异常值使用 JsValue；
String/BigInt 属性、容器、包装对象保留 arena 句柄；API/真实 host 边界才物化
Value。global/VarRef、Proxy/general descriptors、Iterator callback/close reason、
JSON module 和 legacy async-from-sync 内部链已完成迁移与具名边界盘点。

Step/Resume/实际挂起 owner 统一回收；交接遵守先验证和预留、再 take 的顺序。
构造器 newTarget/argv、Promise、String concat、Proxy trap、VM discard、
数值结果部分入栈、暂停编码/恢复失败均有显式释放。JsValue/RawValue 无 Drop，
没有增加通用逐值 RAII，oracle 期望与 teardown 断言保持原要求。

源码、静态检查及性能 receipt 详见 本地记录（不纳入 Git）。
W6 当前源码语义门禁待执行，不能声称当前版本已证明零泄漏或零 oracle 偏差；
A4 的 8B NaN-box 仍是独立测量阶段，不计入本轮 16B 工作流。

最终性能快照为 `161bdbb2`：属性读取下降约 16%/19%/29%，但 scaling 耗时比
几何均值 +4.9%，BigInt 算术耗时 2–2.5×，已完成的 V8 两项约降分 9%。
不能据表示收口宣称全部性能目标完成；回退、部分采样与后续顺序见收口报告。


## 8. 回退修复计划与实测记录（2026-09-21 起）

> 历史规则说明：以下原始修复计划及 §8.1–§8.12 保留当时的执行顺序、
> 构建协议和裁决，不再作为当前后续阶段的暂停指令。现行路线见
> `performance-architecture.md` §11；现行比较按本文 §5，发布配置结果见 §8.13。
> 允许按路线推进不等于历史回退已关闭，仍须在同协议台账中逐项追踪。

**当时的顺序是先解决所有回退，再推进。** 以下为 2026-09-21 的修复安排：
只进入 A 内部的回退修复 R0–R5；A4、B 及其他阶段的新优化全部暂停。
修复可以并行实施在不重叠的模块，但性能采样必须串行。已有属性读取收益必须
保留，不能用其平均收益抵消 BigInt、容器、编译或任一规模的回退；也不能
通过恢复内部公共 Value 双管道、弱化 oracle 期望或删除 teardown 断言换速度。

本节是静态调查后的修复计划，**不是修复完成声明**。本轮未运行测试、构建、
新 benchmark 或 CPU profile。测量取自 本地记录（不纳入 Git），
对应实现 `161bdbb2348f4320ee288fd66b8ae57ac1166ae6`，pre-A 对照为
`85afd56493393065080680d562d4116ff9bd7974`。调查 HEAD `f664a361` 相对该实现
只有报告文档变化，因此源码机制仍对应现实现；后续代码改动必须换新的 receipt。

### 8.1 完整回退台账，不能只处理三个大项

下表是全部 22 类、86 个 case/size 的现有 scaling 记录，单位为耗时变化；
正数表示变慢，负数表示变快。即使变化很小，也暂记“待判定”，不得未经
同协议复核就称为噪声并关闭。原始重复样本、构建信息见 evidence 的 artifacts。

| case | 32 | 128 | 512 | 2048 | 修复/归因任务 |
| --- | ---: | ---: | ---: | ---: | --- |
| map-int | +11.82% | +7.50% | +9.85% | +10.36% | R2 |
| map-string | +16.89% | +18.38% | +20.70% | +18.39% | R2 |
| set | +9.18% | +6.65% | +5.60% | +6.46% | R2 |
| map-churn | +0.05% | -0.29% | +5.72% | +10.04% | R2 |
| set-churn | +8.49% | -0.01% | +2.94% | +2.96% | R2 |
| set-intersection | +9.51% | +9.43% | +5.79% | +8.88% | R2 |
| prop-write | +0.14% | -3.36% | -3.08% | +0.98% | R3 |
| prop-delete | +21.99% | +28.29% | +20.87% | +22.05% | R3 |
| array-truncate | +7.73% | +3.70% | +8.98% | +5.77% | R3 |
| scope | -6.77% | +7.61% | +4.29% | -0.05% | R4 |
| constants | -10.82% | +15.55% | +12.87% | +12.38% | R4 |
| module | -24.14% | -5.44% | +9.24% | +7.94% | R4 |
| module-imports | +8.82% | -5.92% | +10.91% | +1.34% | R4 |
| long-key | +7.23% | +9.46% | +4.95% | +13.74% | R2 |
| map-iterate-churn | -1.31% | -0.45% | +0.54% | +0.07% | R2 |
| set-iterate-churn | +0.22% | -1.30% | +0.09% | -0.56% | R2 |
| array-index | -1.33% | +2.98% | -4.69% | +3.45% | R4 |
| array-holey | +6.68% | +8.70% | +4.94% | +4.52% | R3 |
| typed-index | +1.75% | +7.97% | +0.65% | -0.64% | R4 |
| arguments | +9.48% | +1.52% | -0.60% | -7.64% | R2 / R3 |
| mapped-arguments | +3.72% | +4.35% | -3.43% | -1.97% | R2 / R3 |
| regexp-groups | +11.36% | +5.53% | 不适用 | 不适用 | R4 |

另列以下独立门禁，不能被 scaling 几何均值替代：

- **BigInt**：32/64 位工作负载 400→1000 ns，256 位 500→1000 ns，即
  2.5×/2.5×/2×。上游报告一个迭代含 `sum += a*a; a += incr` 的三次数值
  运算，并非单次加法；Date.now 粒度较粗，不据此分摊各个算子的成本。
- **V8 Richards / DeltaBlue**：完整三次记录的分数分别 41.6→37.8、53.1→48.3，
  均约下降 9%。Crypto 仅一对样本，其他五项尚无完成记录；14/48 样本完成，
  缺失 34 项。未完成不是通过，也不是已证明回退，仍属覆盖缺口。
- **已有收益保护**：property int/object/string.length 耗时约下降 16%/19%/29%；
  默认 microbench 的其余四项中位数持平，empty_loop 的粗时钟结果不能当成
  整体加速证明。后续修复同时保留这些对照。
- **内存目标未证明**：JsValue 16B、RawValue ≤16B、AtomIdx 4B、ShapeEntry 8B
  是布局证据；没有全引擎 RSS 减半证据。叶节点新增 arena 槽可能抵消部分
  节省。原架构的 8B 流量预期不能直接归给当前 16B 实现，FrameBinding
  等实际容器也不能按 JsValue 大小直接推算。

### 8.2 R1：恢复短 BigInt 的免 arena 算术与传输

**已确认机制**：`value/bigint.rs` 相对 pre-A 的算术内核没有实质改写，仍为
`Short(i64)` / `Heap(Rc<BigInt>)`。损失的是 Short 原来的免节点存储与运算。
现 `vm/numeric.rs::{bigint_payload, allocate_bigint_jsvalue}` 对 Short 同样
查询 arena、借用 runtime、克隆 payload，并为每个结果 reserve/publish；
`vm/numeric/operation.rs` 还显式释放输入边。final 已有叶节点最后引用直接
回收快路，仍不能消除结果分配、输入查找和非最后引用的管理成本。
这是确定存在的新增机制；其占 2–2.5× 回退的具体份额尚无最终 profile 证明。

**方案**：按修订后的 D2.7 增加 `JsValue::ShortBigInt(i64)` 与
`RawValue::ShortBigInt(i64)`，Heap BigInt 继续 `BigIntId`。保留 16B 编译期
断言、不增加 Copy/Drop，不为短值伪造句柄。统一 BigInt payload 发布入口：
仅真实 `JsBigInt::as_i64()` 的 Short 直接返回标量，其余保留原 Rc payload。
先复用原 JsBigInt 算术内核，不同时重写 checked arithmetic 算法。

一次闭合以下链条，禁止只优化 VM 算术：numeric、常量 publish、API root/
unroot、BigInt 构造器、TypedArray decode、对象/容器槽、挂起 argv、包装对象
valueOf，以及 GC edges/producer_edge/slot ownership。Short dup 复制 i64，
release 无边；HeapCounts 如实反映不再分配 Short 节点，不能伪造诊断数字。

语言层仍只有一种 BigInt。SameValue/StrictEq/SameValueZero、混合表示比较、
hash 与 `same_representation` 必须闭合：Short 按值、Heap 保留 Rc 表示身份，
不同句柄仍需内容兜底；不要直接使用 i64.hash 破坏现有 BigInt hash 契约。
保留溢出晋升、i64::MIN、除零、负指数、混合 Number、移位及 TypedArray 截断。
完整 i64 内联可在 16B 阶段完成，不等 NaN-box 的 48-bit payload 决策。

若只做合并 runtime borrow、叶节点发布内联等小改，只能记为降低开销，
不能预先宣称恢复短整数性能。唯一引用节点就地覆盖和全局小整数缓存涉及
alias、错误路径、身份与生命周期，不作为首选补丁。

### 8.3 R2：容器与共同调用成本

**先排除已修问题**：native argv 缓冲池、Map/Set borrowed receiver/key、
不变 ABI 借用 NativeInvocation、primitive predicate 避免 Box、叶最后释放
快路均已在测量快照中。不能把“恢复池”或“借用 ABI”重复列成待实现根因。

**已确认新增机制**：`value/collection_key.rs` 的 String hash 与内容相等
通过 `heap.string` 解引用 generational handle；不同 StringId 即使 payload
相同，仍经过 arena 查询后才内容比较。pre-A 直接持有 JsString。long-key
即使命中已有 hash 缓存也不能免除内容比较所需的解引用。BigInt 键类似，
R1 会恢复短整数键的免节点路径。**map-int 不走字符串路径**，其回退必须
独立处理，不能用字符串 hash 解释全部容器回退。

修复顺序：在同一 heap 借用和已验证 live key 的作用域内复用 payload/record
查找结果，避免重复类型、generation 与索引查询；借用不能跨执行 JS、修改
arena 或释放 owner 的边界。容器存储提交仍必须 retain 它实际拥有的边。
再检查 `vm/stack.rs` 的 take/prepare、`vm/call/native.rs` 的 activation/finish、
`builtins/dispatch.rs` 和 `vm/proxy_get_driver/native.rs` 的同步调用固定成本，
在成功路径复用已验证 frame/metadata，避免重复准备与即时释放的中转边。
异常、changed ABI、constructor、迭代器与宿主调用仍走完整 owner 协议。

具体落点是 `builtins/map.rs` 的 find_record 后再次 get record，以及 has 复用
取值 helper 的无用值复制；改为一次验证、一次查找，直接返回存在性或借用
record，get 在退出事务前保留返回边。`heap/value_storage.rs` 已有可信 live-edge
读取入口，可在明确满足其前提的作用域复用，不引入通用 unchecked indexing。
`heap/collection_index.rs` 的 8 项缓存从 WeakJsString 表示身份改为 StringId；
这是代码变化，但 long-key 重用稳定 key、map-string 原先也创建不同表示，
没有证据把这两项回退归因于缓存失效。保留带随机 seed 的内容 hash，不能用
32-bit 指纹替代。动态 map-string 临时键的新节点成本同时纳入 R3。

容器已有的重复 hash/lookup、churn 压缩与迭代策略不能冒称 A 新引入。
如果最终 profile 证明它们是有效补偿点，可采用单次 lookup/hash token、
同事务返回 record 等局部改进；必须保持 SameValueZero、NaN/±0、插入顺序、
迭代期间删除重插与 Set intersection 分支的可观察顺序。不能仅按普通 Set
重写 intersection，忽略用户可见 size/has/keys 获取与调用。

map/set churn 与 iterate-churn 的小正值同样留在台账。最终 profile 要同时含
map-int、map-string、long-key、set-intersection；一次字符串热点不能代表四类。
旧 `/tmp/s3-a-map-perf.data` 属于 `2f5f7c08`，只能提示采样方向，不能给 final
的 native/VM/ownership 成本分配百分比。

### 8.4 R3：动态键、数组和 Arguments

**prop-delete** 工作负载每项先 `o['p'+i]=i` 再 `delete o['p'+i]`，包含两次
动态字符串创建、转键以及最后 Object.keys，不能全部归因删除内核。
`vm/numeric.rs` 创建 StringId → `value/conversion.rs` intern → release 的
临时节点生命周期是相对旧实现确定增加的成本。当前 dictionary 阈值为 1，
不能把备用 clone 分支当成每次删除都 O(n) 的证据。

先优化新短字符串 reserve/publish/release 和 ToPropertyKey 的 payload 借用，
合并可信 atom/index 的重复查询；保留 StringId 生命周期。必要时以预生成键、
仅动态转键、原端到端三类采样区分成本，端到端仍是放行依据。Add→PropertyKey
融合仅在证实必要后考虑，必须保留 ToPrimitive 顺序、副作用、长度上限和异常。

**array-truncate** 包含逐项 native push、delete a[0] 的 dense→sparse，再
length=0；物化与截断算法 pre-A 已存在，整数索引的 atom brand 也不查 arena。
先消除共同调用开销，再定位真实多出的布局/借用成本，不把已有线性算法当根因。
**array-holey / prop-write** 优先检查已选择 raw data 写入中重复的 shape/index/
length 验证，复用无可重入区间里的选择结果；不能漏 Proxy、accessor、原型
setter、非 extensible/frozen 与 length blocker。带洞 dense 是额外存储设计，
不是未经定位就实施的默认补丁。

**Arguments** 小 arity 回退而较大 arity 改善，先定位每次调用固定成本。
`vm/stack.rs::snapshot` → `object/arguments.rs` raw layout → release snapshot
仍有堆值 retain/store/release 中转；需要时改为专用 owned layout 事务，成功
采用原边，半提交或失败完整释放。mapped VarRef 共享、删除/重定义解联、重复
形参、callee poison、getter/Proxy 的读取顺序必须保留，不能改成普通 data slot。

### 8.5 R4：编译、模块、RegExp、索引与 V8 覆盖缺口

scope/constants/module/module-imports 是生成程序执行一次的进程计时，包含
启动、parse/compile/link/evaluate/teardown。constants 是大量 unresolved
`typeof global_i`，不是纯常量读取；module 同时含 namespace 枚举与读取。
不能将这些回退直接套到 VM 算术，也不能仅因数值较小就判为计时噪声。
**已确认的可修成本**：`code/runtime.rs` 的 FlatConstant::AtomString 先经
raw_property_value 创建草稿 StringId，再 intern 并为非立即整数 atom 的
canonical 字符串 allocate_string，替换 constant；两个 producer 都需清理。
pre-A 草稿直接是 JsString，没有这两次节点发布。修复为在编译发布事务内
保留草稿 JsString，完成 intern/canonical 决策后只发布最终 StringId 一次；
保留立即整数 atom fallback、auxiliary_atoms 根与失败回滚。这属于初次发布
新产物，不是恢复运行时 Value 存储。具体 constants opcode 是否使用该分支
仍需核对，不据该机制直接宣称解释了整个 constants/module 百分比。

`modules/namespace.rs` 的 own keys 对 AtomIdx brand 后又分类/读取，新增
代际与 entry 校验；可在同一状态事务合并读取，保留域认证、UTF-16 词典序
和 VarRef 身份。`vm/pure_operations.rs` 的普通 TypeOf 也有字符串节点发布，
但 TypeOfIsUndefined 专门 opcode 不应被误算进这条成本；先核对 codegen，
若需要复用 typeof 句柄，必须显式设置 runtime/realm 根及其清理生命周期。

按各阶段拆解，检查常量首次发布、AtomIdx 重新 brand/查表、binding/module
namespace 查找及 cleanup；对确定的重复操作复用已验证索引或一次事务内的
借用结果，保留 TDZ、live binding、循环模块与错误缓存语义。首次创建节点
是合法创建，不能为省分配退回公共 Value 存储。

regexp-groups 同时创建 pattern、执行正则、生成捕获/indices/groups 并枚举键。
现编号捕获与命名组已共享原 StringId，不能再把该优化记为未做。重点区分真实
substring 节点创建、对象结果组装/shape/atom 成本和匹配内核，只有前两者存在
表示迁移相关路径；尚无证据证明 matcher 算法退化。保留 indices 对象共享、
unmatched captures、属性描述符与命名组顺序，不能通过减少结果语义提高分数。

array-index/typed-index 的正负混合结果先核对稳定性，再定位索引转换、选择器、
slot 与 typed 读写；立即 Number 仍内联，没有证据表明丢掉普通 Number 算术
优化。不得把 TypedArray 边界、detach、转换顺序等检查移出必要路径。

Richards/DeltaBlue 的约 9% 降分目前只有端到端证据，共同候选为 VM 分派、
frame/binding 运输、临时 owner、属性写入与生命周期；**尚未完成定量归因**。
用这两项的最终源码短采样指导局部修复，不以 map 的中间版 profile 代替。
Crypto 及其余 V8 缺失项按有界分批方式补齐；未齐之前不声称全 V8 或所有
场景无回退。布局缩小可能改变代码布局/缓存行为，这需要采样佐证，不能凭猜测
把统一 inline 或切换 LTO/PGO 当成已找出的根因。

### 8.6 R0–R5 排程及关闭标准

| 顺序 | 交付物 | 关闭条件 |
| --- | --- | --- |
| R0 证据冻结 | 本台账、固定 pre-A/build flags/输入、代码与二进制 hash | 不混用中间 profile；所有正值、未完成项都有归属 |
| R1 BigInt | 16B 完整 i64 内联及所有生产/消费/存储闭合 | Short 闭环、跨 i64 晋升、真正 Heap、容器键和 typed 边界均无回退与语义偏移 |
| R2 容器/调用 | 可信借用与调用固定开销修复 | 所有 map/set/long-key 规模关闭，原 owner/错误清理保持完整 |
| R3 属性/数组 | 短字符串生命周期、raw 写入和 Arguments 运输修复 | 对应每个 case/size 关闭，并保留读取收益 |
| R4 剩余归因 | 编译/模块/RegExp/索引及 V8 定位和修复 | 不遗留“共同开销”占位解释；未完成覆盖补齐 |
| R5 汇总验收 | 同一最终源码的性能、内存和 §4/§5 语义证据 | 所有回退关闭才允许后续阶段；当前仍未满足 |

R1、R2、R3/R4 可分模块并行调查/实现；涉及共同表示、GC 和 collection hash
时先落单一协议再集成，不能各自创造不同的所有权规则。采样串行，复用同一
最终构建；只在相关修复后做有针对性的比较，不反复运行全套去猜根因。

每条关闭记录至少包含：新增成本的源码证据、补丁、同源 before/after receipt、
该 case 全部规模的结果、错误/清理协议审查及语义验收状态。中间几何均值 +4.9%
只能描述该组样本，既不是全引擎总分，也不能作关闭条件。小幅正值需足够重复、
交替基线/候选次序并报告分布；若仍无法排除回退，状态保持待定，不临时放宽
门槛。没有当前源码 oracle/teardown 证据就不能写“零偏差、零泄漏”。

本轮遵守“不跑测试”，因此 R5 的语义证据明确待验收；该限制不会自动放行
阶段 A。历史 d184986f 的全套通过不适用于后续源码。先把回退修复完整，再按
统一最终快照完成已定义验收，之后才讨论 A4 或其他阶段。


### 8.7 第一批确定成本的实现（待性能关闭）

本批只修改已确认存在的成本，未改变 BigInt 算术内核、公共 API、oracle 期望或
teardown 断言：

- R1：JsValue/RawValue 的 ShortBigInt(i64) 在 16B 内恢复。numeric、API、
  常量边界和 typed 创建绕过 arena；存储、GC、异常根、包装对象、比较/hash、
  typeof/ToNumber/ToString 与闭包 scalar 读写均识别 Short。大整数保持 BigIntId。
  旧内部表示断言相应调整，语言 oracle 期望不改。
- R4：AtomString 编译草稿保留 JsString，完成 intern/canonical 决策后仅发布
  最终 StringId；立即整数 atom fallback、atom 根、发布失败回滚保持原协议。
- R2：Map 查找复用比较时已访问的 record，has/delete 不再取出无用 value；
  Map/Set 删除及 Set 插入合并查询与修改的状态借用。未改 hash、安全检查或
  容器数据结构；其中删除旧有重复工作属于补偿优化，不是声称它由 A 新引入。
- R3：String ToPropertyKey 在一次状态借用内直接读取 payload 并 intern，
  不再为了运输克隆 JsString；输入边仍由原统一出口释放，失效句柄错误分类保持。

按用户要求不执行测试。现阶段只确认上述源码成本已移除；后续以该批不可变
提交的同协议 benchmark/profile 判断各回退是否关闭。未完成的语义门禁和
§8.1 逐项性能门禁仍有效，不能据本批补丁宣称阶段 A 完成。


第一轮 profile 还定位到 ShortBigInt 在 `vm/stack.rs::copy_value` 漏掉 scalar
分支，落入 `inline(never)` 的 copy_reference。固定工作量 bigint32 cycles
仍高于 pre-A 20.2%，揭示上游 400 ns 粗粒度结果不能作为关闭证明。已追补该
分支，并补齐 run immediate release、ordinary/IC、dense pop、array iterator、
inline scalar push 与原始参数 scalar 判定；这些均是 Short 无边值准入，不
放宽 Math/ToNumber 的 Number 类型检查。追补效果另用新提交测量。


### 8.8 修复后的 benchmark/profile 与剩余方案

可复核数据已写入 本地记录（不纳入 Git）。
第一批实现提交 `f774eaa3` 的普通/计数 release 构建都使用 rustc 1.94.1、no PGO、
LTO off、CGU16；与 pre-A `85afd564`、修复前 `161bdbb2` 三方串行比较。
全部 86 个 scaling 组合、每引擎每组 3 次均输出正确；这不是 oracle 测试。
perf 使用同一普通二进制，fixed-work stat 与 997Hz flat record 分别采集，
无 callgraph。源码、二进制、工作负载与 artifact hash 在 evidence 中，不能
用该 profile 冒充后续 Short 快路追补提交的采样。

**第一批测量，尚未包含 Short 快路追补**：

| 项目 | pre-A → 修复前 → f774eaa3 | 判定 |
| --- | --- | --- |
| bigint32 上游 ns | 400 → 1000 → 400 | 粗时钟显示恢复；固定工作量仍 +20.2% cycles，不能关闭 |
| bigint64 上游 ns | 400 → 1000 → 500 | 大幅挽回，但仍回退；样本并非纯 Short |
| bigint256 上游 ns | 500 → 1000 → 1000 | 长值回退未修复 |
| constants | f774 相对修复前耗时降低约 2–10% | 双发布成本确已删除；相对 pre-A 各规模约 -4.5% 至 +2.2%，仍需稳定性证据 |
| prop-delete | 相对修复前降低约 1–6% | 相对 pre-A 仍 +15–21%，继续阻断 |
| map-int / map-string | 相对 pre-A 约 +7–10% / +16–20% | 去重未消除整体回退 |
| long-key | 相对修复前降低约 2–6% | 相对 pre-A 仍有约 +3–8% 的规模，继续阻断 |
| Richards 分数 | 41.4 → 37.8 → 38.0 | 未关闭 |
| DeltaBlue 分数 | 52.1 → 48.1 → 48.2 | 未关闭 |
| property int/object/string.length | 耗时相对 pre-A -17.1% / -17.2% / -30.7% | 已有读取收益保留 |

86 组中仍有 66 组中位数为正回退，几何均值 +4.59%；不是全引擎总分，
也不是“其余 20 组已正式验收”。小幅波动和此次变化方向翻转的 case 均留待
稳定性判定。完整逐规模数值在新 evidence，§8.1 旧快照不覆盖或倒填。
本轮 V8 只复测已知回退的两项，其余六项未测，原覆盖缺口仍然阻断。

**P1：Short 标量快路遗漏，已定位并追补。** fixed-work bigint32 的
copy_reference/dup_jsvalue 热点发生在几乎全 Short 的循环中，不能解释成
堆节点 retain。源码确认 copy_value 和 run immediate 判定漏 Short，已由
`f29c620b` 补齐及静态扫查同类入口；最终复测结果另附，不沿用 f774 的结果。

**P2：长 BigInt 节点和 ownership 成本，下一批高优先级。** fixed-work
bigint256 为 1000×1000 次上游内核，cycles 2.475G→4.599G（+85.84%），
instructions +62.06%、branches +67.49%；branch misses 仅约增加 1.5 万，
不能用“预测失败爆炸”解释。上游 bigint64 每千次内循环中仅后 489 个 sum
结果进 Heap，而 256 每千次约有 3000 个 Heap 结果，因此 Short 方案不可能
单独解决长值。原算术内核未变，profile 中节点发布、dup、释放和 payload
访问均可见。下一步先减少真正新增的节点/owner 往返：纯 Rust 算术在一个
短 heap 借用内读取两 payload，结果离开借用后发布，消除中间 Rc 克隆及
重复借用；同时审查叶发布和非末引用释放。不得变成跨 JS 回调的借用，也
不得未经唯一所有权证明就覆盖原节点。payload 借用只是局部方案，不承诺
单独消除 2× 回退。

**P3：native 成功收尾的大 Result 搬运，优先于仅删 Rc clone。** map-int
固定工作量 cycles 19.560G→21.388G（+9.35%）、instructions +3.21%。
finish_completion_reusing 的 flat self 从约 0.31% 增至 2.15%；当前汇编
可见正常返回分支搬运四组 16-byte 载荷及尾字段，样本集中于结果搬移。
这支持将正常 Completion 交接与冷 RuntimeError 物化拆开，避免正常值在
多个大 Result 临时量间搬运；必须保留 frame finish 失败时已生成值的释放、
诊断 frame 可见性、argv 回收与 unwind owner。NativeActivation 收尾的
额外 Runtime clone 可通过最后移交 owner 消除，但样本不支持把它当唯一主因。
pre-A ObjectRef clone 本就执行 heap retain/state borrow，不能再当成新增成本。
函数采样边界会随内联变化，不能把百分比差当成精确因果份额；最终需窄补丁
机器码和端到端对照验证。set/intersection/array push 继承此共同调用方案。

**P4：动态键定长 hash 与叶引用成本。** prop-delete fixed-work cycles
+23.82%、instructions +10.58%，而 branch misses 下降。after FxHasher::write
占 self 7.61%，array-truncate 中为 6.44%；当前 `hash.rs` 只特化 write_u64，
JsString::Hash 的 write_u32(content_hash) 落到通用 chunks/零填充/复制循环。
优先给定长整数写入提供等价内联实现，保留原字节序/填充与 hash 一致性，
不改 Map 随机 seed 的内容哈希。删除 dictionary 内核本身未显示新增主热点。
`heap/gc.rs::release_reference` 对空队列的多引用 String/BigInt 先校验一次，
随后 release_raw_no_drain 再校验一次；可在首次认证的 slot 内完成非末引用
递减。strong==1 的就地释放已存在，不能重复报为新优化；非空队列、trace、
zombie/溢出与错误路径仍保留原处理。专门叶发布接口是否能省宽 NodeData
搬运仍须汇编确认，不直接升级成独立 arena 重构。

**P5：RegExp 新字符串图边的收集与事务。** regexp-groups cycles +8.03%，
instructions 基本持平；after Edges::extend/push 约占 self 5.22%/3.57%。
真实 capture StringId 加入数组/groups 后，相对旧内嵌 JsString 新增图边，
当前 Edges inline4 溢出后转 Vec，逐边 push。可按已知数量预留或直接遍历
执行事务 retain，减少临时集合增长/重复遍历；必须保留回滚、完整 edges 和
编号/命名捕获共享身份。采样仅数百，百分比是定位线索，不能精确分摊回退。

**P6：仍需拆解的编译/模块、索引及 V8。** constants 单次节点发布已经修复，
但固定工作量重复进程的 cycles 仍约 +5.15%；module 约 +3.97%，instructions
仅 +0.23%，旧 JsString 内容比较仍是双方相近的大热点。不能将旧模块查找
算法认定为 A 新根因。typed-index cycles +4.50%，instructions 反而约 -2.78%；
VM/slot/typed selector 热点分布变化支持检查代码布局、借用及重复选择，但
尚不足以锁定一个新增算法。scope、arguments、array-index/holey 的小幅或
混合回退仍需要同源更稳定采样和具体调用链，不能从相邻 case 倒推已解决。
V8 已进一步定位到一项明确新增的 owner 往返：
`ordinary_storage.rs::prepare_linked_own_read_selected` 将已有存活边的
ObjectId 临时提升为 ObjectRef，仅为调用 ordinary_read_probe_atom，退出又
Drop；pre-A 直接借原 ObjectRef。对应 helper 在 Richards self 1.87%→3.43%、
DeltaBlue 1.68%→2.94%。下一步让私有 probe 接收 ObjectId，由原 base/slot
owner 保活，移除额外 retain/release 和 Runtime clone；getter receiver 及返回
值的真实拥有边仍保留。这是比把所有 V8 回退归因 native finish 更直接的方案，
尚待窄补丁证明收益，不将自适应样本百分比差当成准确的回退占比。

这些项下一步应以窄改动 A/B 和 cache/分支计数区分执行工作、布局与测量波动，
不能换 PGO/LTO 掩盖同协议回退。Richards/DeltaBlue 的 profile 使用上游自适应
工作量，只作热点定位，不拿它的原始总 cycles 作固定工作量比值。

本轮没有执行 unit/oracle/Test262。MSRV 1.88 全 workspace/all-targets/
all-features Clippy -D warnings 已通过；格式、source layout、rust-only 检查
通过。静态检查、benchmark 正确输出与性能归因各有作用，不能互相替代；
阶段 A 和后续推进门禁保持关闭，直到逐项回退与语义证据满足原要求。


### 8.9 最终追补快照 f29c620b：实测结论与下一步

`f29c620b692b555bb4b61f1d1514ad4b9435c3f4` 已重新构建普通 release 并通过
MSRV 全目标/全特性 Clippy；编译配置不变。最终快照重新测量了全部 86 个
scaling 组合（每引擎每组 3 次）、上游 BigInt（3 次）和 fixed-work BigInt
perf stat（交替顺序 3 次，并另录 after flat profile）。输出全部正确，细节
见 evidence 的 `final_snapshot`，不与 f774 样本混合计算。

| BigInt 内核 | 上游 pre-A → 最终 ns | fixed-work cycles 中位数变化 | instructions 变化 |
| --- | --- | ---: | ---: |
| 32 | 400 → 400 | +8.90% | +1.66% |
| 64 | 400 → 400 | +27.16% | +15.99% |
| 256 | 500 → 1000 | +87.11% | +62.35% |

Short 误走 copy_reference/dup_jsvalue 的热点已从最终 bigint32 flat report
中消失，固定工作量 cycles 残余由第一批的约 +20.2% 收窄至 +8.9%。这验证
追补确实消除了该多余调用路径；**仍不能写短 BigInt 整项已无回退**，上游
400 ns 的粗粒度相等掩盖了剩余成本。最终热点转向普通 numeric completion、
slot replace 和 release 分派；下一步审查标量是否仍反复进入通用 fallible
结果/清理交接，结合机器码区分调用/搬运与布局成本。64 含 Heap 晋升，256
仍以 P2 的节点/owner 成本为主，均继续阻断。

最终 scaling 仍有 69/86 组中位数变慢，耗时比几何均值 +5.06%。Map-int
约 +8.6–9.8%、map-string +16.6–23.4%、prop-delete +16.6–19.0%、RegExp
+5.8–6.5%，均未关闭。constants 中较大规模又出现约 +15–18%，与第一批
接近持平的结果不一致；不能选择有利快照宣布关闭，也不能未经区分就把这
归因于 Short 补丁。保留两次完整记录，后续用稳定的编译/执行分段和配对
采样判别。常量单次节点发布的源码事实成立，但不等于该 benchmark 已修复。

属性读取与 V8 本轮最近的测量仍对应 f774；没有把其结果标成 f29 的新证据。
本批已提交的是表示/运输中确定的冗余成本修复，性能门禁仍未通过。后续顺序：
先 P3 native 正常结果交接、P4 定宽 hash/非末叶释放与 V8 借用 ObjectId 的
局部补丁，再 P2 长 BigInt 和 P5 图边事务；P6 小幅/混合回退继续定向归因。
各模块可并行实施，构建和性能采样统一串行；仍不推进 A4 或其他阶段。


### 8.10 必要正确性验证补齐（2026-09-21）

**纠正要求解释**：“不要浪费时间跑测试”指避免反复、无目的地运行全套，
不是禁止必要测试。此前文档将其写成“不运行测试约束”是助手误读；本节
更正该解释和当前验证状态。历史性能记录里的 tests_executed=false 仍如实
保留，不把后补验证倒填到旧执行记录。

本次冻结生产实现，完整执行一次全 workspace/all-features 测试构建；库与
oracle 各用独立进程完整枚举（包含 ignored），保留正常 debug teardown，
显式移除 QJS_TEARDOWN_PROBE，避免 destructor abort 掩盖未执行用例。
证据见 本地记录（不纳入 Git）。

| 验证范围 | 结果与处理 |
| --- | --- |
| oracle | 912/912 通过；无 abort、timeout、skip 或缺失汇总 |
| 引擎库 | 2576 项完整执行，首次 2573 passed、1 failed、2 abort；均已定位为 3 处旧 Short-as-heap 测试夹具 |
| 夹具修正后的定向验证 | 3 个失败用例及 2 个补充 Short 成功表的用例，共 5/5 通过；未重复运行整套库或 oracle |
| 其余工作区 | 10 个 test artifacts，合计 184 个测试通过；其中空测试目标明确记为 0，不算额外覆盖 |
| 完整 Test262 | 102037 total / 80032 eligible / 79982 pass，原有 43 runtime + 7 parse 失败逐项与固定基线一致，无新增差异 |
| focused Test262 覆盖 | 从已认证 full 结果派生 6844/6844 pass，并验证冻结 manifest/结果；不是另跑一次 focused |

本次只改 `#[cfg(test)]` 中的夹具和断言前清理：`1n`、`42n` 已是无边 Short，
不能再用来测试“堆值退出 scalar 快路”；拒绝表改为 i64MAX+1 的真实 Heap
BigInt，成功表加入 Short。两个原 abort 的测试先保存检查结果、释放测试
自己持有的 raw 边，再断言，避免首个断言失败引起 teardown 二次 panic。
原游标、引用数、队列、读写拒绝及 teardown 断言全部保留。生产实现、公开
API、oracle 输出期望、Test262 admissions/negative contracts/固定基线均未改。
因此没有重复性能基准；§8.9 的生产实现性能结论仍适用。

完整 Test262 的当前源码指纹：
`0caf82311ee4d16cebf610b48d252726c0ed1118052ded6708231a130f39196b`。
runner binary SHA256：
`3ac12e83082534ecc4de6fbb358c60ec0ff27fc8be5699f58897d9e7468157ab`。
`--full` 单次执行含构建/输入认证约 299 秒，workers=8；验证完整结果向量，
不只是对比总通过数。原始第一次库失败及其日志 hash 保留，不重写为首次全绿。

当前已有必要的语义与 debug teardown 验证证据；这只覆盖实际执行的用例，
不是对所有程序零泄漏的无限保证。性能回退仍按 §8.9 阻断阶段 A，不能因为
本轮正确性验证通过就推进 A4 或后续阶段。


### 8.11 继续回退修复：最终快照 d0deb0fe（2026-09-21）

**结论：不是只剩 BigInt，也没有宣告阶段 A 完成。** 本轮把已确认的实现成本落地，验证了收益；短 BigInt 固定工作量恢复到 pre-A 附近，长 BigInt 的主要回退显著缩小，但仍未消除。Map/Set、动态删除、部分数组/索引/编译组以及 V8 仍阻塞 §8 门禁，不能推进 A4。完整分组、三次原始样本、二进制/工作负载指纹、profile 和验证范围见 本地记录（不纳入 Git）。§8.9–8.10 是此前 f29/dc787 快照，不代表本节的新结果。

#### 已定位、实现并验证的机制

代码、测试及汇编确认了下述机制；除 R5 两处内联的窄对照外，多数性能数据是组合补丁结果，不能把组合收益全部归给某一个补丁。

| 新增成本或缺口 | 本轮实现与证据 | 关闭边界 |
| --- | --- | --- |
| 纯 BigInt 两个操作数分别借用 heap、clone/drop payload Rc | 同一次共享借用直接调用原 `JsBigInt` 内核；Short/Short 不借 heap | 保留混合类型与回调转换顺序、错误文案；算术算法没换 |
| 每个长结果都新建 arena 节点 | consuming 算术可转移唯一输入节点；独立计算新 payload 后替换，公共 Rc 不变 | borrowed add、额外持有边、相同 id 的两份输入均不能非法复用；Short 结果仍内联 |
| 16B BigInt 载荷在 publish 中做 392B、399B 两次复制 | String/BigInt 专用叶节点直接发布；最终 BigInt 汇编只有 16B payload store，栈帧从 824B 降至 16B | 保留 free-list、generation、weak-link、ledger 检查；不是更换 arena 布局 |
| 叶节点 retain/release 经过不必要的可变借用、通用分派与宽 cleanup | validated shared retain；直接叶节点释放、单次身份校验；公共 Heap API 仍返回准确 cleanup 计数 | trace、借用冲突、旧 zero queue、错误与 deferred drain 原顺序保留；没有关闭断言 |
| native finish 正常路径搬运宽 Result；后续又发现两次 136B activation 搬运 | 冷错误构造与正常值分开；原地取出 callable 后释放参数；最终汇编确认两次整段搬运消失 | native frame 在错误构造时仍有效；callable 先于参数释放，finish 失败释放产出值 |
| 固定宽度 hash 落入通用字节分块；单个图边反复构造 Edges | 专用 u8/u16/u32/usize 写入，保持原端序映射；直接扩展 0/1 个 raw edge，批量 spill 使用迭代器剩余长度 | GC 边不删、顺序不变，事务 retain/rollback 不变 |
| ordinary own-read 和普通原型链 fallback 各临时创建 receiver ObjectRef | 两条内部入口都借用已有 base 的 ObjectId；只有返回 Special 或其他确需独立 owner 的位置保留 promotion | 公共域校验、getter/receiver/prototype 独立所有权保留；新增测试准备 getter 后释放 base 仍能调用 |
| TypedArray 数值叶读取往返公共 Value | 数值 decoder 返回 Number，再投影内部 JsValue；BigInt typed 路径保持原样 | Uint32 边界、负零、Infinity、NaN payload 测试通过；不能据此宣布 typed benchmark 全关闭 |
| R4 新增 VM 包装调用使纯 Short 又多执行指令 | 反汇编发现 numeric completion 的两个操作数各多一次 RunSlots::pop 包装调用，run 另调用 supported；只对这两个已有小函数增加内联指示 | R5/R6 汇编确认直接 pop_current、无 supported callsite；不是盲目改所有函数的内联 |

实现提交：`39911ad1`（第一批）、`dc038e36`（叶引用/native 原地清理）、`93febb08`（两处 VM 内联）、`d0deb0fe`（剩余 ordinary fallback receiver 借用）。`JsValue=16B`、`RawValue≤16B`、`AtomIdx=4B`、`ShapeEntry=8B` 约束保持；没有恢复公共 Value 内部传输，没有放宽 oracle，也没有把 16B 宣称为已实现 8B 或整机内存减半。

#### BigInt 三方交错固定工作量

同机、同 Rust 1.94.1、无 PGO、LTO off/CGU16；每格 3 次交错执行，比较中位 cycles。baseline=`85afd564`，before=`f29c620b`，after=`d0deb0fe`。每次 1000×1000 的原算术内核，独立 Python 大整数计算期望输出，全部校验成功。正值表示更慢。

| 工作量 | f29 对 pre-A | 最终对 pre-A | 最终指令数对 pre-A |
| --- | ---: | ---: | ---: |
| bigint32 | +8.33% | -0.29% | -1.12% |
| bigint64 | +26.26% | +11.37% | +9.69% |
| bigint256 | +90.84% | +38.45% | +40.69% |

不能混用跨轮数据：R3 为 +2.67/+17.98/+58.35%，R4 为 +7.42/+21.41/+44.16%；短值的反复促成了上述 codegen 排查。只增加两个内联指示的 R5 窄对照为 −0.47/+9.56/+37.66%，最终 R6 再次维持短值接近基线。此前“短 +9%、长 +87%”是旧快照；最终三方数据以本表为准。短值约零差距不能包装成显著加速，也不代表所有 BigInt 宽度已经关闭。

原上游 Date.now/min-of-many bigint 基准仍单独保存，32/64 的粗粒度相等不能代替固定工作量。profile 中旧通用释放分派不再是长 BigInt 的主要热符号；实际还存在 live-edge dup/retain、arena 分配/回收以及 VM 绑定和结果交接。不能把全部剩余 cycles 都归给某个 self 百分比，也不能声称乘法算法本身变慢。

#### 完整 scaling 台账（最终对 pre-A，wall-time 中位数 %）

下面每组均实际执行 3 次并核对输出；列顺序为 size 32/128/512/2048，RegExp 仅 32/128。小进程组含启动、编译及销毁成本，正回退仍保留；样本区间重叠不是自动关闭许可。

| case | 各 size 的最终变化 |
| --- | --- |
| map-int | +4.25% / +4.78% / +4.65% / +11.01% |
| map-string | +11.54% / +19.11% / +12.09% / +16.70% |
| set | +9.19% / +8.52% / +8.57% / +8.56% |
| map-churn | -5.21% / -2.54% / +8.95% / +5.85% |
| set-churn | +7.07% / +12.25% / +2.97% / +4.14% |
| set-intersection | +9.24% / +8.56% / +5.09% / +9.23% |
| prop-write | -4.58% / -4.37% / +1.68% / -0.34% |
| prop-delete | +9.22% / +10.06% / +9.06% / +10.16% |
| array-truncate | +8.66% / +0.66% / +2.69% / +1.85% |
| scope | -11.26% / -0.67% / +1.98% / -9.73% |
| constants | -15.54% / +8.57% / +2.10% / +0.43% |
| module | +8.44% / -13.10% / -3.86% / +3.28% |
| module-imports | -20.90% / +0.68% / -1.78% / -0.71% |
| long-key | +3.14% / +0.36% / -0.94% / +0.24% |
| map-iterate-churn | -0.86% / -0.61% / -2.49% / +1.21% |
| set-iterate-churn | -0.21% / -0.23% / -1.46% / -2.56% |
| array-index | -3.58% / +1.37% / +0.53% / -2.22% |
| array-holey | -0.57% / -4.06% / -1.86% / +0.42% |
| typed-index | +6.55% / +2.33% / +5.21% / +5.22% |
| arguments | -9.64% / -4.38% / -6.51% / -21.07% |
| mapped-arguments | +4.75% / +0.93% / +1.27% / +4.76% |
| regexp-groups | +0.91% / +0.25% |

86 组中 66 组优于本轮同跑 f29，但仍有 55 组中位数慢于 pre-A；不能用平均改善抵消任一未关闭项。每格原始样本位于证据 JSON，可据此核对 min/max，不能只保留有利规模。

V8 仍仅 Richards/DeltaBlue 两项（三次/引擎），没有伪造完整 V8 suite 分数。property read probe 使用 2000 万次、3 repeats；它只是数据属性读取诊断，不代表整个引擎。


| V8 case | pre-A score | 最终 score | 分数变化（越高越好） |
| --- | ---: | ---: | ---: |
| richards | 41.4 | 38.4 | -7.25% |
| deltablue | 52.9 | 49.4 | -6.62% |

| property probe | pre-A ns/op | 最终 ns/op | 耗时变化 |
| --- | ---: | ---: | ---: |
| prop_read_int | 226.18 | 201.72 | -10.81% |
| prop_read_obj | 284.31 | 244.59 | -13.97% |
| prop_read_string | 388.90 | 270.67 | -30.40% |

#### 必要正确性验证与尚未关闭的工作

最终源码重新构建后运行完整隔离 core/oracle，正常 teardown，无 probe 模式、无忽略失败列表、无 abort 掩盖。其他 workspace 目标和 MSRV 1.88 clippy 也检查；完整 Test262 与 frozen vector 逐项比较，focused 从本次完整结果派生，不重复执行。具体计数与源码指纹如下。

- 最终 core **2585/2585**，oracle **912/912**，其余 workspace **184/184**；include-ignored，无失败、abort 或遗漏。
- 完整 Test262：102037 variants，80032 eligible，79982 pass；既有 43 fail-runtime/7 fail-parse 完全不变，unsupported/skipped 也与冻结向量逐项一致。focused **6844/6844** 是本次 full 的派生核验。
- 最终源码 `d0deb0fe`，engine semantics fingerprint `e9de7bd3af59542ab80bb732fc0ce399a4eb72532bd5fc1756af6347d8ce32d9`。MSRV 1.88 all-targets/all-features clippy、fmt、source-layout 696 files、diff check 通过。
- R3/R4 有各自独立 core/oracle 完整记录，R4 另有完整 Test262 记录；R5 只改两处内联，执行 443 项 VM 单测和 24 项数值 oracle（467/467），没有为此重复 Test262。R6 新的 fallback owner 改动先通过 148 项 object 测试，再做上述最终集成验证。


剩余工作按以下证据边界推进，不进入 A4：

1. **长 BigInt**：已证明本轮 arena/ownership/codegen 修复有效，尚有非零回退。下一步对 VM 已有 live edge 的复制/归还做窄 A/B，分离 checked identity、计数、宽 Error 返回与 caller 搬运；保留外部入口完整检查和引用计数溢出规则。当前没有证据允许直接删 retain、把共享节点当唯一或跳过 generation。此项是待验证方案，未标为关闭。
2. **Map/Set、V8**：两条新增临时 receiver root 和 native 搬运已消除，仍慢说明它们不是全部根因。继续对共享的 property/call 准备、对象引用处理及 frame handoff 分摊固定工作量成本；旧有 ObjectRef clone、Map hash/链索引算法不能直接冒充本轮新增原因。V8 adaptive profile 的总 cycles 不作固定工作量比较。
3. **动态键/数组/RegExp**：hash/leaf/edge 修复已落地，但剩余组须按字符串生成、atom intern、属性变更和回收分别测量。不得为了减少 graph edge 成本漏掉 captures/groups 的必要引用。RegExp 接近基线也不等于所有正值自动验收。
4. **TypedArray、compile/module 等小组**：本轮仍有正回退或跨轮变动，保持待查。需要增加单次有效工作量、用同一最终二进制做配对计数/编译阶段 profile；不能把已有 JsString::eq 热点或一次中位数差直接当新增算法回退。TypedArray 桥消除的局部机制不等于全部 typed workload 收益已经拿到。

本节列出的已确认成本对应修复均已合入本轮；剩余假设需要下一轮独立因果实验。**本节不等于“所有回退已修复”，阶段 A 门禁仍未通过。**

### 8.12 第二轮回退修复：并行根因确认与实测关闭（2026-09-22）

**结论：scaling 86 组几何均值由 +2.13% 转为 −2.52%（优于 pre-A），正回退组 66→35；prop-delete、typed-index、regexp-groups、构造、Richards、DeltaBlue、raytrace、navier-stokes、arguments、array-index、iterate-churn 等已关闭或反超；map-string、长 BigInt、set-churn、crypto、V8 regexp、earley-boyer 仍有残余，阶段 A 门禁未全部通过。** 本轮起点为 ea7aeb54（源码与 §8.11 的 d0deb0fe 一致）；上一会话的实验大提交（对象 6b6f92eb）经三方实测无净收益已整体丢弃，本轮只把其中经独立 A/B 证实的点修复重新独立实现。所有改动已提交（3a3042ca）；最终二进制 sha256 `a942272259485fc1b0d8b183f85a70ed3296b4925bcd4a881fe49214d5c4220b`（rustc 1.94.1、LTO off、CGU16、无 PGO），对照 pre-A `85afd564` 与未修改 ea7aeb54（`target/s3-a-owned-plain`）。证据目录：`target/final-run/`（不纳入 Git；scaling/property/microbench/v8/fixedwork/stages）。

#### 已确认根因与对应修复（每项均有独立 A/B 或反汇编证据）

1. **叶复制走通用 dup_jsvalue**（vm/stack.rs `copy_reference`）：String/BigInt 复制经历二次 kind 分派与宽 RuntimeError 搬运。修复为直接 `retain_string_handle`/`retain_bigint_handle`。单项 bigint256 +42%→+30.6%。
2. **字符串拼接前的临时 Rc clone 破坏原地追加**（vm/numeric.rs `add_primitives`/`add_primitives_ref`）：拼接前克隆左 payload 使 buffer 非唯一。修复为唯一边下原地追加并转移左节点所有权；新增 `vm/numeric/string_tests.rs`。
3. **BigInt 热路径重复身份校验**（heap/value_storage.rs、vm/numeric.rs）：调用方持活边期间 `heap.bigint()` 仍走全套 generation/kind 校验；`a*a` 同节点做两次唯一检查。修复：`bigint_fast`（debug 保留断言）与同 id 跳过第二次检查。
4. **codegen 不稳定（§本轮核心确认）**：三处 `matches!(slots.peek(i), Ok(...))` 的含 Drop `Result` 临时值，只有 LLVM 恰好完全内联 peek 并 SROA 时清理才被折叠，导致同源码在不同构建间「展开 vs 循环 + 成功路径无用 Result 清理」翻转（退化构建 `run` 内 4 处 `drop_in_place<Result<(),Error>>`、29 处就地错误串物化；ea7aeb54 二进制即处于退化态）。修复：三处改为直线双检查+显式解构（run.rs、run/numeric.rs）；`fusion::compare_branch`、`consume_number_pair_current` 加 `#[inline]`（数字比较从两层调用恢复完全内联，ucomisd 8→12）；peek 族错误串物化外提为 `#[cold]` 辅助（stack.rs、stack/number.rs）。反汇编逐项确认。
5. **动态键与字符串生命周期**（prop-delete +10%→−6%）：VM `ToPropertyKey` String 分支多一次 payload clone（property_keys.rs，改用既有 `intern_property_key_string_id`）；`Utf16Units` 的 61 槽 `from_fn(|_| None)` 初始化 lower 成 61 次未内联闭包调用（primitive.rs，改 `[const { None }; 61]`）；新增平坦串借用迭代器 `BorrowedUtf16Units`（rope 变体装箱满足 MSRV clippy），`utf16_units` 加 `#[inline]`。
6. **字符串索引释放条件缺陷（上一轮四点修复之一，正确性向）**：stack.rs 字符串索引叶用「底层字符共享」绕过释放就绪证明——字符共享只能证明拼写存活，不能证明槽位退休免分配。修复：slot_ownership.rs 新增叶专用 `slot_leaf_release_readiness`（strong>1 或 free-list 有容量即 Ready，与 `try_release_leaf_reference` 准入一致），删除 stack.rs 的绕过；两处旧断言按新语义改为确定性夹具（预留 free-list 容量、先存结果后断言），生产语义未弱化。
7. **公开 callable 检查缺失 runtime 域校验（正确性缺陷，上一轮四点之二）**：object/allocation.rs `as_callable` 直接转 `as_callable_object` 丢失 `belongs_to` 域校验，异域同值句柄会被当本域 callable。修复恢复 operation+域校验，容器 callsite 改用句柄入口消除 `from_borrowed_handle` 往返；保留新增测试 `public_callable_promotion_rejects_foreign_matching_handles`。
8. **对象转换多余 retain/release（上一轮四点之三）**：object/mod.rs `into_handle`/`into_atom` 在无 pending 清理时直接转移边并解除 Drop；有 pending 时保留原清理边界；新增两测试锁定语义。
9. **属性读 descriptor kernel 绕行**（Map 构造与集合方法读）：函数/集合/迭代器 payload 的自身属性全在 shape/slot，原先仍走 owned-descriptor kernel（每次构造 11 次 retain_raw）。修复：`reads_are_slot_faithful` 扩展借用 probe；`owned_descriptor::into_data_value` 转移已拥有值边。
10. **Map/Set 调用与存储所有权**：set/add 返回接收者与记录值改受信 retain（`retain_live_object_handle`，debug 全量断言、trace/借用冲突回退）；get 直接按 payload 臂构造 Completion；set/operations.rs probe/intersection 借用 `object_id` 消除每记录 3 次 ObjectRef clone。
11. **release 内核每边成本**（heap/gc.rs、ownership.rs）：非终释放也走全套槽位校验+宽 `Result<Option<HeapCleanup>>` 往返。修复：`try_release_nonfinal`（strong>1 且 zero-queue 空时原位递减，其余全回退）；`release_replaced_raw_value` 标量快返回；`drain_zero_queue` 拆冷热。
12. **构造路径固定成本**（vm/call/prototype.rs、builtins/iterator/collection.rs）：每次 `new Map()` 两个 Box 分配 + new_target 双 dup。修复：`constructor_prototype_source_now` 借用构造帧拥有的 new_target 同步读（getter/Proxy 回退可恢复协议）；CollectionStep 空 iterable 同步 Complete。构造 −18.3%（反超）。
13. **「读→临时 owner→立即释放」纯往返（V8 主因）**：Richards 每轮 1164 万次 `copy_reference` 对象 retain 随即被 GetField 消费释放；DeltaBlue 另有 134 万次全局 cell 读。修复：borrowed-base GetField 融合（PushThis/GetVar/GetVarRef/GetLocal 后紧跟 GetField 时借用绑定自身的边完成链接读，一次消费两指令；`roots.rs::borrow_cell_object_fast` 沿用全部 decline 纪律）。ObjectRetain 1164 万→708 万。
14. **数组/typed 叶释放去通用分派**（stack.rs，取自 6b6f92eb）：已认证 Direct 槽直接 `release_jsvalue`，免逐槽 binding 分派。
15. **typed 数值读桥**、**AtomString 双发布**等 §8.11 已收录项保持不变。

#### 证伪与否决记录（避免重复尝试）

- BigInt lease（value_storage 事务租借）与唯一节点原地覆盖：三方实测无收益（6b6f92eb 全批 bigint256 +41.7% vs 修复前 +41.9%），不再作为方向。
- 三个 Map 诊断补丁组合使新键插入 −6%→+8.7%，否决。
- stack.rs 热槽操作的冷错误外提初版与热槽全链 `#[inline(always)]`：bigint 明显回退，均已撤销；最终采用的 peek 族冷外提是在 codegen 稳定化之后单独 A/B 通过的版本。
- 61 条默认工具链 clippy 报告为 1.94 新增 lint（collapsible_if/is_multiple_of），非门禁；MSRV 1.88 clippy 才是门禁且已全绿。

#### 最终测量（同一提交快照 3a3042ca、同一二进制；均含 baseline/head/final 三方、3 次轮换取中位）

固定工作量 perf stat（cycles:u vs pre-A；括号为指令数变化）：

| workload | ea7aeb54 | 最终 |
| --- | ---: | ---: |
| bigint32 | −2.7% | **−6.8%**（−4.4%） |
| bigint64 | +14.6% | **+5.2%**（+5.9%） |
| bigint256 | +40.8% | **+19.2%**（+34.2%） |
| map-int | +7.3% | +4.5%（−2.0%） |
| map-string | +14.9% | +11.3%（+4.8%） |
| prop-delete | +9.9% | **−6.1%**（−10.4%） |
| typed-index | +0.7% | **−5.1%** |
| Map 构造 | +12.7% | **−18.3%** |
| Map 覆盖更新 | +11.1% | +5.3%（−0.9%） |
| Map 新键插入 | −6.8% | **−9.0%** |

scaling 全 86 组（wall 中位，`target/final-run/scaling*`）：几何均值 head +2.13% → final **−2.52%**；仍为正的 35 组集中在 map-string（+6.7~+11.7%）、map-int 大规模（+3.9~+7.9%）、set-churn（+8~+9%）、map-churn 512/2048（+8.8/+8.9%）、set（+1~+5.6%）、long-key（−1.6~+4.1%）等；prop-write、prop-delete、array-index、typed-index、arguments、iterate-churn、regexp-groups（−15%）、scope/constants/module 多数规模为负或关闭。

V8 全套八项（score，越高越好；head 的 earley-boyer 三次超时如实记录）：

| case | pre-A | ea7aeb54 | 最终 | 最终 vs pre-A |
| --- | ---: | ---: | ---: | ---: |
| richards | 41.2 | 37.7 | 43.3 | **+5.1%** |
| deltablue | 52.6 | 48.2 | 52.5 | −0.2% |
| crypto | 54.5 | 51.4 | 52.8 | −3.1% |
| raytrace | 78.5 | 71.6 | 78.4 | −0.1% |
| earley-boyer | 101 | 超时×3 | 96.4 | −4.6% |
| regexp | 74.7 | 62.2 | 67.2 | −10.0% |
| splay | 271 | 257 | 266 | −1.8% |
| navier-stokes | 203 | 180 | 219 | **+7.9%** |

property probe（20M 次、3 repeats）：prop_read_int 227.69→159.44 ns（−30.0%）、prop_read_obj 288.96→201.13（−30.4%）、prop_read_string 396.18→224.12（−43.4%）——原有读取收益保留并扩大。

#### 正确性验证（最终快照）

- `cargo test --lib` plain 2304/2304；`--features profiling` 全绿；oracle 全绿；workspace 全绿（`target/final-run/stages.json` 与各 log）。
- 完整 Test262：102037 total / 80032 eligible / **79982 pass**，与冻结向量逐项一致（`r3fj complete Test262 vector matches`）。
- `cargo fmt --check` 干净；**MSRV 1.88** workspace/all-targets/all-features clippy `-D warnings` 全绿。
- 两处旧测试按第 6 项新语义更新为确定性夹具（非弱化）；`JsValue=16B` 编译期断言、oracle 期望、teardown 断言全部保留。

#### 未关闭残余与归因（下一轮入口）

1. **map-string +11%**：键生成/回收侧已优于 pre-A（诊断 workload −33.6%）；残余在容器持 arena 键的哈希/比较、leaf retain/release 与批末 teardown（collection_index、heap/gc）。
2. **bigint256 +19%（指令 +34%）**（2026-09-23 修正归因）：主因是句柄化后每次拷贝/释放的 **ownership 事务**（借用 + 世代/kind 校验 + readiness 预检 + 延迟释放管道），不是原先记录的槽边界 16B/32B store-forward 失速与 `run/numeric::complete` 宽结果搬运。反证：同工具链同 flags（LTO off、CGU16）下 pre-A `85afd564` → A 终版 `d4f78697`，bigint256 instructions +31.9%、cycles +16.8%，与本节 nolto 列逐位吻合；IPC 反升（指令数驱动而非 stall）；pre-A `push_current` 同样是 32B `movups` 对，宽度搬运不是 delta；`replace_local_current` 不在热路径；`numeric::complete` 反而变便宜（final 1.32% vs pre-A 9.11%）。微负载每操作指令差（pre-A→final）：`a=a` +477、`a=a+1n` +886、`s=s+a` +271、`b=a*a` +1084；`a=a` final 约 15% 指令为 pre-A 不存在的所有权事务，官方 bigint256 每内层迭代 +2310 ≈ mul+add(reuse)+add 三项之和。窄修方案见 [阶段 A 收口计划](s3-a-closure-plan.md) T1；lease/唯一复用仍属已证伪方向。bigint64 +5.2% 同源（仅 ~49% 迭代产生 Heap 结果）。
3. **Map 覆盖更新 +5.3%、set-churn +8~9%**：native 调用编组（`take_native_call_operands`、`prepare_native_arguments` 的重叠 8B 拷贝失速）与剩余 ownership 往返。
4. **V8 crypto −3.1%、earley-boyer −4.6%、regexp −10%、splay −1.8%**：本轮首次补齐全套覆盖；regexp 与 scaling 的 regexp-groups（−15%）方向相反，说明残余在正则执行/子串路径而非 groups 组装，未逐项归因。
5. **布局敏感性**：LTO off + CGU16 下任意源改动可使无关 case 摆动 ±5–10%（本轮多次复现，指令数为稳定副指标）；insert 曾因 std BTree drop 失去内联出现 +13% teardown，重建后自然消失。评估小幅残余时必须配对同构建采样。

> 2026-09-23：下一轮入口已具体化为 [阶段 A 收口计划](s3-a-closure-plan.md)
> （T1 所有权事务窄修 / T2 帧槽 24B），并纳入 E 同协议重测新增的 fixed
> 字符串簇（`string_build1/3/large1`、`int_to_string`）与 `map_delete`
> 归因；bigint256 以 [全量重测结果](s3-full-rerun-results.md) §4.6 为准
> （m0/pre-A 墙钟 1.21×、指令 1.43×）。

### 8.13 窄补丁收尾与 LTO 双协议对照（2026-09-22）

#### 两个 bigint256 窄补丁（已实施，提交 d4f78697）

1. **`vm/run/numeric.rs::complete` 冷错误物化外提**：错误物化（NativeErrorKind 跳转表、0x50B Error 搬运、PC 发布、Vec drop、unwind pad）原全部内联在热函数体内（栈帧 504B）。外提为 `#[cold] #[inline(never)] materialize_thrown` 后帧 504B→280B，drop glue 与 unwind pad 移出。独立 A/B 为布局中性（错误路径不执行，insn ±0.00%），但叠加在补丁 2 之后四 workload 全部不劣化且小幅改善（bigint64 −1.7~−2.4%），按叠加序保留。
2. **`heap/gc.rs::try_release_leaf_reference` 释放链融合**：原路径 out-of-line `validate_slot_identity`（sret Result 搬运）→ 冗余 bounds 复查 → retire 再调 `reclaim_vacant_slot`（内部 3 次槽查找）。重写为一次槽查找完成全部判定，非终递减零函数调用；新增 `retire_validated_leaf` 保留 weak-link/generation/free-list/ledger 语义。bigint256 **insn −1.71% 稳定复现**；cycles 在 ±2% 布局噪声内。
3. **证伪记录**：`allocate_bigint`（self ~4%）反汇编确认 reserve→publish 已完全融合、无重复工作可删，剩余为 440B 槽跨步固有访存（归 D）；`finish_bigint_operands` 单借用融合实测使 short BigInt 明显回退（+4.65% insn），已回退不保留。
4. 合并态（安静配对，5 次轮换）：bigint64 +4.2%→**+2.1%**，bigint256 +18.3%→+17.8% cyc、insn +34.2%→**+31.9%**；bigint32/map-int 无交叉回退。补丁后全库 2304 测试、MSRV 1.88 clippy、fmt 全绿。

#### LTO 双协议对照（同日、同源码、双方同 flags；证据 target/lto-compare/）

配置：nolto = LTO off + CGU16（旧协议）；lto = fat LTO + CGU=1（新协议，即发布配置）。基线 pre-A `85afd564` 按各自配置重建。

**固定工作量（final vs 各自配置的 pre-A，cycles / insn 中位）**：

| workload | nolto | lto |
| --- | --- | --- |
| construct | **−20.1% / −17.6%** | **−18.1% / −16.5%** |
| insert | **−7.7% / −1.7%** | **−10.5% / −1.8%** |
| prop-delete | **−6.6% / −10.1%** | +9.7% / −2.8% |
| typed-index | **−3.7% / +0.5%** | +9.1% / −0.1% |
| bigint32 | −4.6% / −4.4% | −0.2% / +5.1% |
| map-int | +3.4% / −2.0% | +6.0% / −0.3% |
| update | +3.2% / −0.2% | +8.0% / +0.7% |
| map-string | +8.0% / +5.3% | +13.3% / +6.9% |
| bigint64 | +5.2% / +5.4% | +13.6% / +14.4% |
| bigint256 | +21.7% / +31.9% | +32.1% / +38.8% |

**scaling 86 组几何均值**：nolto **−2.19%**（34/86 为正）；lto **−0.29%**（42/86 为正）。
**property probe**：nolto −30/−30/−43%；lto 仍全胜但收窄为 **−18.5/−13.0/−17.4%**。
**V8 全套（score，final vs pre-A）**：

| case | nolto | lto |
| --- | ---: | ---: |
| richards | +1.7% | −4.8% |
| deltablue | −2.1% | −8.7% |
| crypto | −3.1% | −4.3% |
| raytrace | −0.7% | −3.1% |
| earley-boyer | −4.1% | −8.7% |
| regexp | −10.2% | −11.2% |
| splay | −1.8% | −6.9% |
| navier-stokes | **+7.9%** | **−19.2%** |

#### 结论（诚实口径）

1. **fat LTO 给 pre-A 的提升显著大于给当前实现**（pre-A 各 workload −22~−31%，当前实现 −15~−22%）。机制：pre-A 的 Rc/Result 管道是大量本地小操作，LTO 几乎能全部内联消解；而句柄化的结构性成本（集中校验/所有权事务、440B 槽跨步）不是内联能消掉的；同时本轮手工 `#[inline]` 已提前收割了我们这侧的部分 LTO 红利。
2. **发布配置（LTO）下阶段 A 仍是净回退**：V8 八项全负（−3~−19%），scaling 约持平；LTO off 下的净收益画面有相当成分来自「pre-A 没机会做跨模块内联」。navier-stokes/typed-index 的翻转（insn 持平、cycles 大幅变差）是典型样本。
3. 属性读、Map 构造、insert、prop-delete（insn 口径）在两个协议下都是真实结构性收益。
4. **门禁含义**：按 §5 的发布协议（双方 fat LTO + CGU=1、无 PGO、无 profiling），回退台账须以 lto 列为准重新裁决。E 已完成同协议重测并冻结基线（[全量重测结果](s3-full-rerun-results.md)，2026-09-23）；PGO 双边复核另列，不是普通阶段比较的前提。保留同协议 pre-A 对照，不能因冻结 E 后基线而抹去阶段 A 的回退。这组数据用于 T1/A4 的候选排序，但 LTO 后仍有差距并不证明全部来自值宽度或栈流量；应在发布配置下重新归因，再分配给 T1/A4/D（C 已关闭、B 不在活动路线）。

# S3-A 计划：8B 值表示——融合实施

> 状态：待实施。起点为 pre-A 代码基线，无前置实现资产。本文档是
> `performance-architecture.md` §4（方案 A）的实施计划与验收规则；若两处
> 表述冲突，**以本文档为准**。约束与证据附录继承
> `performance-architecture.md` §0/§11/附录。

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
7. BigInt：`RawValue::BigInt(BigIntId)` 全 arena（`Short` 也进 arena，其
   分配成本列为测量点）；**A4 开放决定**——NaN-box 下 short 收缩为
   **±2⁴⁷ 内联**（48-bit payload + kind tag，超出晋升堆句柄，语义透明），
   默认取前者，A4 开工时按测量复核。
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
目录，协议见 §5）→ profiling 计数器无漂移 → 实测结果记入本文档。

**委托执行交底**：任务拆分委托时，提示词必须包含 §1 终态设计、§1.2
所有权纪律、§2 放置规则与「临时构造不接受」条款；评审先看设计一致性，
再看编译。

## 5. 阶段性能比较协议

1. **固定基线**：S3-A 开工前保存一份基线——无 PGO、无 LTO 的 release
   构建（`CARGO_PROFILE_RELEASE_LTO=off
   CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`），全量基准数字记入报告，
   作为所有阶段的固定分母之一；
2. **每阶段只比两次**：本阶段 vs 上一阶段、本阶段 vs 保存的基线；两侧
   都是无 PGO、无 LTO 的同 flags 构建。**不做每阶段 PGO 重训**；
3. **例外与复核**：E 阶段测的就是构建配置本身，按
   `performance-architecture.md` §3 已有口径；每个大阶段（A/B/D）收尾时
   **建议**（非强制）做一次 LTO+PGO 双方复核——LTO 会改变内联与代码
   布局，无-LTO 下的阶段胜率偶尔会在最终构建配置下翻转，复核只为确认
   符号不变；
4. 跨协议对比允许用于累计/用户口径的报告，须标注双方构建协议。

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

### 7.3 move 优先签名——把「交接生产者边」从评审规则变成签名规则

**事实**：§1.2 已钉死「move 入库 → 交接生产者边，不产生计数对」，但目前
采纳路径多为「借入 + 内部 dup + 调用方事后 release」，靠人肉配平。

**决定**：

1. **采纳即 by value**：吞掉值所有权的存储/记录/缓冲函数参数一律取
   `JsValue`（或 `RawValue`）by value；调用方在值消失后再无处可 release，
   「存了忘放」在类型上不成立。
2. **产出即 owned**：返回 owned 值的函数保持 by-value 返回，值类型标注
   `#[must_use]`——「创建后既没存也没放」作为语句出现时编译器告警。
3. **dup 显式化**：调用方需要「存完再用」时显式 `dup_jsvalue`；dup 从被调
   函数内部挪到调用方可见处。
4. 执行与 W3 尾部/W4 签名切换同波完成（同一批调用点），逐文件推进，
   编译器驱动。

**性能论证**：by-value 16B 枚举传参 = 寄存器移动，codegen 不变；每个采纳点
**删掉一对 dup/release**（借入 dup + 事后 release），严格更少计数动作。

**已知覆盖缺口（诚实清单）**：`let v = pop();` 后遗忘（赋值未用）Rust 不告警；
槽位 pop/丢弃的手工 release 不受签名规则覆盖——二者由 7.1 账本检测。

**验收**：clippy `-D warnings` 全绿；全量测试全绿（teardown 断言照常）；
评审规则写入「采纳签名一律 by value」。

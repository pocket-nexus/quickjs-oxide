# S3-A 计划：8B 值表示实施拆分（A0–A4）

> 状态：设计点已钉死，待开工。本文档是 `performance-architecture.md` §4
> （方案 A）的实施计划；若两处表述冲突，**以本文档为准**。只含计划，不含代码。
> 约束、验证门禁、证据附录均继承 `performance-architecture.md` §0/§11/附录。

---

## 0. 代码复核钉死的三个设计点

动工前的代码复核暴露三个架构文档未钉死、决定范围的点。本节给出决定，
每个决定含论证与边界。

### D1：`Value` 是公共 API 类型——内部值类型与转换层先行

**事实**：`Value` 由 `src/engine/api/mod.rs:28` 公开导出（`pub use
crate::engine::value::{JsString, JsStringError, Value}`），内部数百个文件直接
使用它。方案 A 要求内部 8B，就必须把「公共值」与「内部值」拆开，否则改动面
失控。

**决定**：

1. 公共 `Value`（`src/engine/value/mod.rs:13-24`，含 `ObjectRef` 等带
   `Rc<Runtime>` 的 root 类型）**保持不动**，只在 `engine::api` 与宿主回调
   适配层出现；公共签名一个不改。
2. 新增 crate 内部类型 **`JsValue`**（`src/engine/value/` 新模块）：
   - A0–A3 阶段为 **16B 句柄 enum**：标量内联（`Int(i32)`/`Float(f64)` 等），
     堆类型为 `{index: u32, generation: u32}` 句柄；
   - A4 阶段再编码为 **u64 索引 NaN-box**（`performance-architecture.md`
     §4.1），若实测不划算则停在 16B（§4.3 退路，已比现状 32B 小一半）。
   - 不实现 `Copy`/`Drop`；显式 `dup`/`release` 纪律见 §4.2。
3. 转换层仅两个方向、只挂在 `Runtime` 上：
   - `unroot`（进引擎）：`&Value → JsValue`（dup 堆边）与
     `into_jsvalue(Value) → JsValue`（消费 root，省一次 retain/release 对）；
   - `root`（出引擎）：`JsValue → Value`（retain + 包装 Rc root）。
4. **边界规则**：A2 完成后，`Value` 不得出现在 `engine::api` 与转换层之外
   ——评审规则，若便宜则加进 `scripts/checks/check-source-layout.py`。
5. A0 必须先做**穿越点盘点**（eval 结果、call 参数/返回、host 回调、
   module loader、promise jobs、test262 agent、`adapters/*`），盘点结果记入
   A0 的测量文档。

### D2：String/BigInt 仍是 `Rc`——新增两个堆 kind，typed arena 覆盖

**事实**：`RawValue`（`src/engine/heap/identity.rs:200-224`）对
Object/Symbol 已是句柄（`ObjectId`/`Atom`），但 `String(JsString)` /
`BigInt(JsBigInt)` 仍是 `Rc` 负载（`value/primitive.rs:20`、
`value/bigint.rs:104-115`）。`performance-architecture.md` §6 的 typed arena
清单只列了 Object/VarRef/Shape/Context/FunctionBytecode——**这是文档缺口**。
不堆化 String/BigInt，8B 无从谈起。

**决定**：

1. `HeapNodeKind`（`src/engine/heap/identity.rs:127-133`）新增 **`String` 与
   `BigInt`** 两个独立 kind（不合并）——与 §6 typed arena 的 per-kind 拆分
   对齐；句柄 `StringId`/`BigIntId` 与其余 id 同型（`{index, generation}`）。
2. **cycle 处理：cascade-only，永不做 anchor**。rope 只引用 string、BigInt
   无出边，两者按构造无环，与 VarRef/Shape 同级（zero-queue 级联回收，
   不进 trial-deletion）。
3. String 节点负载 = 现 `StringRepr`（Latin1/Utf16/Rope），但 rope 子节点从
   `JsString` 改为 `StringId`；`RefCell<RopeState>` 线性化缓存留在节点内
   （节点级内部可变性是既有先例）。
4. 公共 `JsString` 改为 **root 句柄类型**（`{runtime, id: StringId}`），公共
   表面不变：checked 构造与 UTF-16 门禁不变
   （`tests/checked_string_construction.rs` 是硬门禁）；`same_representation`
   从 `Rc::ptr_eq` 改为 id 相等（更便宜）。
5. `AtomTable` 适配：`strings` 映射改以 `StringId` 为键；`released_strings`
   的 `WeakJsString`（std `Weak`）改为 **generational 弱句柄**（`StringId` +
   存活校验）；顺带落 §6.4 的 hash 缓存（字符串节点头部缓存 hash，atom 表
   换 FxHash）——intern 路径从「每次全串重算 SipHash」变为 O(1) 查表。
6. BigInt：`Short(i64)` 在 16B 阶段保持内联；**A4 的开放决定**——NaN-box
   下 short 范围收缩为 **±2⁴⁷ 内联**（48-bit payload + kind tag，超出晋升堆
   句柄，语义透明，由算术 canonicalization 保证），若编码预算紧张则全堆化。
   默认取前者，A4 开工时按测量复核。
7. 内存语义注意：字符串从「`Rc` 独立分配」变为「arena 节点 + free-list 复用
   + generation」——teardown 的 `live == 0` 断言与 `GcStats`/`HeapCounts`
   公共诊断（`api/mod.rs:15` 导出）口径需同步更新。

### D3：`Atom` 16B 品牌——内部 `u32` + 品牌只留边界

**事实**：`Atom { raw: u32, generation: u32, table_id: u64 }`
（`src/engine/atom/mod.rs:52-57`）16B，相等比较逐 16B；shape 线性扫描
（≤8 项）与迁移表键全在吃这个体积。

**决定**：

1. 内部类型 **`AtomIdx(u32)`** newtype；保留 immediate-int 高位 tag
   （`ATOM_TAG_INT`，QuickJS parity 不动）。16B branded `Atom` 只留：
   公共 API（`PropertyKey` 等）与跨 runtime 进入点。
2. **存活不变量**（钉死，与 `live_node_fast` 同一论证）：内部 `AtomIdx` 只能
   由「已 retain 该 atom 的 owner」持有——shape entry、字节码
   `property_key_atoms`、pinned 集。可信路径免品牌校验（debug 构建全量
   校验），边界全量。
3. **收尾 S1b**：`AtomTable::Entry.ref_count` 改 `Cell<u32>`，retain/release
   在共享借用下完成——`Symbol` 从所有快路 decline 名单移除（S1 §4.5 的
   遗留）。
4. 级联：`ShapeEntry`（`object/shape.rs:73-77`）24B→~8B（u32 atom + flags）；
   shape 迁移表键、shape fingerprint 同减；`RawValue::Symbol/Private` 负载
   16B→4B，为 A3 的 `RawValue` 8B 化扫清最后一个超标变体。
5. `Atom` 的 `Hash` 现为 `generation<<32|raw`（`atom/mod.rs:59-65`）；内部
   `AtomIdx` 直接以 raw 作 hash（Fx），不再移位拼装。

---

## 1. 阶段拆分（每步独立可编译、可测、可回退）

| 阶段 | 内容 | 决定依据 |
| --- | --- | --- |
| **A0** | 地基，无语义变更。两lane可并行、独立合入：**A0-v** 引入内部 `JsValue`（16B 句柄 enum）+ `root`/`unroot` 转换层 + size 断言，先不接线；A0-v 内先做穿越点盘点（D1.5）。**A0-a** atom 内部瘦身 `AtomIdx` + `Cell` refcount（D3） | D1/D3 |
| **A1** | String/BigInt 堆化：`HeapNodeKind::String`/`BigInt` + typed arena + `RawValue` 改句柄 + atom 表/弱引用适配 + 公共 `JsString` 句柄化 | D2 |
| **A2** | VM 接线：`SlotStore`/`FrameBinding`/`run.rs` 切 `JsValue`，显式 dup/release；API 边界走转换层 | D1 |
| **A3** | `RawValue`/`PropertySlot` 句柄化收尾（此时全为 16B enum 形态） | D1/D2/D3 |
| **A4** | NaN-box u64 编码（或按 §4.3 退路停在 16B，由测量决定）；BigInt short ±2⁴⁷ 决定在此复核（D2.6） | D1 |

每阶段末尾一次 `docs(perf): record S3-A<n> measurements`。

范围确认：**先做 A0+A1**（地基 + 堆化，风险可控、独立可评审），A2–A4 按
门禁逐个推进；本文档覆盖 A 全程，不意味着一次合入。

## 2. PGO 基线随阶段更新的约定（钉死）

E 之后的分母是 PGO 二进制，但 A 每改代码 profile 即失效。约定：

1. **日常迭代**：改前/改后都用**非 PGO 普通 release**（同 flags）看比率，
   快、且不受 profile  staleness 影响；
2. **正式 receipt**（每阶段末的测量记录）：对改前/改后**各自重训一次**
   `scripts/benchmark/pgo.py`，训练负载冻结（cases/sizes/operations/repeat
   写进报告）；receipt 已含 `profdata_sha256`，可审计；
3. **跨协议对比**（如「改后 PGO vs 改前非 PGO」）允许用于累计/用户口径的
   报告，但必须标注双方构建协议；**单阶段设计收益归因仍只认同协议配对**——
   否则会把编译器布局噪声算成设计收益（`performance-architecture.md` §3
   E 实测的教训）。

## 3. 验证门禁

沿用 `performance-architecture.md` §11 八条：`cargo fmt --check` → clippy
1.88 `-D warnings` → `cargo test --locked --workspace --all-targets` →
Test262 `--check`/`--focused`/`--full` 零回归（清理 `GIT_*`，只产
current-source receipt，不改 `current.conf`）→ `check-source-layout.py` +
rust-only 门禁 → benchmark receipts（`property_read_probe.py` +
`scaling.py` + `run.py`，串行、独立输出目录）→ profiling 计数器无漂移 →
更新本文档实测章节。

A1 额外门禁：`tests/checked_string_construction.rs`（公共 `JsString` 构造
语义）与 `GcStats`/`HeapCounts` 相关诊断测试必须逐条核对后适配。

## 4. 风险（继承 §4.6，按阶段具体化）

- **手工 RC 纪律扩大 panic 面**（A2 起）：debug 构建维持全量 generation
  校验 + 冻结向量兜底；trusted 访问器遇 stale 即 panic 的政策不变。
- **触及面全仓最大**：值类型是所有模块的公共依赖；A0/A1 与 A2–A4 之间
  必须有完整的门禁绿色窗口，严禁跨阶段混合提交。
- **8B 索引 NaN-box 无生产先例**（附录 A.8）：每次 deref 多一次 base load +
  bounds check；A4 必须以测量定去留，退路（16B enum）不是失败而是默认值。
- **A1 的行为敏感点**：字符串身份（`same_representation`）、atom 身份恢复
  （`released_strings`）、teardown `live == 0` 断言——三处都有测试/诊断
  覆盖，改动时逐条核对。

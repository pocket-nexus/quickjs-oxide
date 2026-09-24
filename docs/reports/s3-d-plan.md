# 阶段 D 实施计划：数据导向堆与形状/键存储（2026-09-24）

> 状态：已收口（2026-09-24）。D1a/D1b 已提交（`84654cc8`/`98bd54c3`）；
> D2/D3/D4 已落地并提交（D4 为 `17a77e6e`/`2bb3595c`，见 §4 实施记录）；
> D4 指令与 slice RSS 门禁按累计口径接受（slice 缺口与 native 编组链
> 一并归 B 重新立项），D5 按启动门禁未满足降级为 backlog。基线为
> `feat/pr27-a4`
> （= PR27 tip，T1/T2 已收口），裁决依据见
> [A4/D/B 决策报告](s3-a4-d-b-decision.md)。本计划取代
> `performance-architecture.md` §6 的旧 D 草图（旧草图含过时事实，
> 见 §2）。测量二进制为 post-T2 发布构建 `c4f1c433…`，对照为 pinned
> QuickJS 2026-06-04（`d0f8966b…`）。

## 1. 目标

D 的总目标是**在不改变值表示（保留 16B `RawValue`/`JsValue`）与信任模型的
前提下，消除堆布局、形状缓存与键哈希的常数级浪费**，把对象/字符串/Map 的
RSS 从对 QuickJS 的 4–12× 压到 ~2–3×，并降低属性/索引/Map 路径的指令数。

本计划同时回答上一轮遗留问题：旧 §6 只有 5 条方向性草图，且部分前提已
被代码演进推翻；本轮基于最新代码与实测重新定义切片、实现点、不变量与
验收。

**D 明确不做**（防止边界蔓延）：

- 不改 8B 值表示（A4 已数据裁决保留 16B，见决策报告 §4.1）。
- 不重写解释器派发（归 B，须重新立项；D4 也不纳入 native 调用编组链）。
- 不改 `unsafe_code = forbid`（`Cargo.toml` workspace lints）；内联存储
  不得引入 `unsafe`。
- 不新增依赖为默认路线（当前依赖仅 `num-bigint`/`num-traits`，
  `Cargo.toml:13-15`）；确需 smallvec 类容器时单独决策。

## 2. 相对旧 §6 的修订

旧 §6（`performance-architecture.md:311-330`）与 §1.6 的以下前提已过时，
本计划按当前树修正：

| 旧表述 | 当前事实 |
| --- | --- |
| `JsString` 不缓存 hash，每次 probe 全串重算 | 已缓存未加盐 `content_hash`（`value/primitive.rs:142/935`，`Hash for JsString` 在 `primitive.rs:2125-2129`） |
| atom 字符串表用 SipHash | 已用 `FxBuildHasher`（`atom/mod.rs:367`） |
| shape 迁移键是 24B `ShapeEntry` | `ShapeEntry` 已 8B，有尺寸断言（`object/shape.rs:82-87`） |
| D 的第 5 项是 S2.1 快速释放单独立项 | 本轮实测未支持其为独立高收益项，移除；D5 改为条件项 |

**本轮新增的关键事实**（改变了切片优先级，详见 §3.2/§3.3）：

1. 多属性对象字面量/连续 define 的**每对象私有 shape** 缺陷：10k 个
   `{x:1,y:2}` 产生 10167 个 shape（1 属性为 168），释放对象后回落到
   167。根因在 in-place append 与 `shape_cache` canonical 条目的一致性。
2. `get_or_create_shape` 的缓存命中路径**每次分配 `Box<[ShapeEntry]>` 并
   用 std `RandomState`（SipHash）哈希**，在多个固定负载里占 3–13% 自时间。
3. 值/槽尺寸实测（临时探针，`cargo test --lib`，已移除）：
   `ArenaSlot` 440、`NodeData` 392、`SlotState` 408、`ObjectData` 224、
   `ObjectPayload` 160、`PropertySlot` 32、`AutoInitProperty` 32、
   `Shape` 88、`ShapeFingerprint` 32、`CollectionRecords` 152、
   `MapRecord` 32、`CollectionIndex` 72、`JsString` 8、`JsBigInt` 16。
4. D5 的反证：`proto_mutate` 与 `unrelated_mutate` 微负载指令数完全相同
   （1,178.74M vs 1,178.74M），当前测量不到「全堆 epoch 失效」的成本；
   D5 降级为条件项。

## 3. 证据基线

### 3.1 差距与内存（决策报告，2026-09-24）

- 指令差：prop_write 24.59×、array_write 19.55×、array_read 13.90×、
  typed_array_read 12.30×、arguments_read 11.60×、map-int 10.38×、
  empty_loop 6.71×、locals 8.27×；`prop_write` 670 vs 27 insn/写。
- RSS（1M 项 `ru_maxrss`）：objects 623.4 vs 126.6 MiB（4.93×）、
  arrays 608.1 vs 149.9（4.06×）、strings 581.5 vs 49.4（11.77×）、
  maps 796.9 vs 107.7（7.40×）。
- 归因：`run::run` 自时间 50–88%；prop_write = ownership 29% +
  property 24% + slots 24% + interp 19%；map-int 的 native 编组链
  ≈25%、属性读机制 ≈10%、哈希 ≈7%。

### 3.2 新增实测：shape 私有化与哈希（本轮）

**shape 私有化**（dev profiling 构建 `-d` 计数 + 发布二进制 `perf stat`）：

| 用例 | shapes（10k 对象） | 备注 |
| --- | ---: | --- |
| `a.push({x:1})` | 168 | 共享 |
| `a.push({x:1,y:2})` | 10167 | 每对象一个私有 shape |
| `a.push({x:1,y:2,z:3})` | 10167 | 同上（只多 1 个/对象） |
| `a=null` 后 | 167 | 私有 shape 随对象释放 |

读写微负载（各 10M 次 `a[j].x`，oxide `c4f1c433` vs QuickJS）：

| 微负载 | oxide insn | QuickJS insn |
| --- | ---: | ---: |
| `ic_share_1prop`（对象 1 属性） | 43.642B | 3.499B |
| `ic_share_2prop`（对象 2 属性） | 50.338B（**+15.4%**） | 3.499B（**0%**） |

即：QuickJS 的 IC 不受属性个数影响；我们的 2 属性对象因每对象私有 shape
使 IC 无法特化。证据文件：`target/a4db-decision/micro/ic_share_{1,2}prop.js`。

**SipHash / shape 缓存**（`target/a4db-decision/profiles/oxide/*.data`
符号自时间）：

| 负载 | SipHash write | `hash_one` | `get_or_create_shape` |
| --- | ---: | ---: | ---: |
| arguments_read | 13.4% | 2.3% | 1.4% |
| prop_clone | 8.9% | 2.7% | – |
| prop_create | 4.1% | 1.0% | – |
| prop_delete | 3.5% | 2.2% | – |
| map-string | 3.4% | 3.0% | – |
| map-int | 1.3% | 3.2% | – |

**cache-references**（同批 `stats.json`，oxide vs QuickJS）：map-int
1.394B vs 1.07M（1300×）、map-string 624.8M vs 1.13M（553×）、
prop-delete 376.4M vs 3.90M（96×）、arguments_read 86.0M vs 0.32M（265×）、
prop_create 58.6M vs 0.27M（218×）。说明 Map/属性路径的内存流量结构不同，
不只是指令数。

### 3.3 新增实测：profiling 记账（dev `--profile-json`）

10k 个 `{x:1,y:"s"+i}` 的记账快照：

- `arena_slots`：30545 槽 × 440B = 13.44MB used / 14.42MB capacity
  （其中 ~10k 是私有 shape 槽）；
- `property_slots`：20890 槽 × 32B = 668KB used / 1.33MB capacity
  （**2× 容量浪费**，来自每对象独立 `Vec` 翻倍）；
- `objects` 10170、`shapes` 10167、`string_nodes` 10141。

该记账（`heap/profiling.rs:86-221`、`api/profiling.rs` `MemorySnapshot`）
可直接作为 D 的逐片验收工具。

## 4. 切片与实现计划

顺序：**D1 → D2 → D3 → D4 →（D5 条件启动）**。每片独立提交、独立
A/B、独立回退；D1 与 D2 文件面基本不重叠，可并行改代码但测量必须串行。

### D1 shape 缓存一致性 + 查找免分配（最高优先）

**目标**：修复多属性对象私有 shape；把 shape 查找路径的分配与 SipHash
清零；shape 相关 cache-references 明显下降。

**证据**：§3.2。影响面 = prop_read/prop_write/prop_create/prop_clone/
arguments_read/map-* 的公共前置路径。

**实施点**：

1. **D1a 一致性修复**（独立最小提交）：
   - **根因定位**：`append_unique_layout_inner`（`object/properties.rs:67-120`）
     对命中 `MIN_UNIQUE_SHAPE_APPEND_ENTRIES`（`properties.rs:36-38`）的
     shape **无条件** unlink `shape_fingerprints`/`shape_cache`
     （`properties.rs:92-95`），随后原地改 entries 并把 fingerprint 重新
     指向同一个已被改写的 shape（`properties.rs:108-118`）。若该 shape
     原本是旧 fingerprint（如 `{x}`）的 canonical 条目，旧条目的
     canonical 映射即被摧毁 → 下一次 `{x}` 查找 miss → 新 shape（实测
     10k 对象 10167 shapes）。
   - **修复**：新增 `RuntimeState::shape_is_canonical(shape) -> bool`
     （`heap/runtime/mod.rs`，读 `shape_fingerprints` + `shape_cache`），
     在 `store_selected_property_slot` 的 in-place 分支
     （`object/storage.rs:588-601`）前置：canonical 时返回
     「不可原地」信号，改走 `append_transition`
     （`heap/runtime/mod.rs:328-360`）；非 canonical 保持现有
     unlink→改写→restore 路径（`properties.rs:108-118` 语义不变）。
   - **debug 不变量**：`shape_cache` 中 `fingerprint → shape` 成立时
     `heap.shape(shape).entries() == fingerprint.entries`；在
     `validate_object_layout` 或 `unlink_finalized_shapes` 路径加
     `debug_assert`。
   - **测试**：在 `object/storage.rs:641-691` 现有 fingerprint 测试上新增
     「canonical 中间 shape 不被原地追加」用例；微负载见验收。
2. **D1b 查找路径免分配 + 哈希**（独立提交）：
   - **Shape 侧**：`Shape` 增加 `fingerprint_hash: u64`，在 shape 构造点
     （`get_or_create_shape` 建形分支，`heap/runtime/mod.rs:288-325`）
     对 `prototype + entries` 用 `FxHasher` 一次算出。
   - **缓存结构**：`shape_cache: FxHashMap<u64, Vec<ShapeId>>`、
     `shape_fingerprints: FxHashMap<ShapeId, ShapeFingerprint>`
     （`heap/runtime/mod.rs:119-120`）。查找时对**借用**的
     `&[ShapeEntry]` 流式 FxHash，直接命中 bucket；碰撞逐项比较
     entries + prototype（保证与 hash 无关的正确性）。删除
     `get_or_create_shape` 里为查找构造的
     `entries.to_vec().into_boxed_slice()` 分配
     （`runtime/mod.rs:288-325`）；`ShapeFingerprint`
     （`object/operations.rs:17-20`）仅保留作校验/调试载荷。
   - **消费点迁移**（全部改用存储的 `fingerprint_hash`）：
     `append_unique_layout_inner` 的 unlink/restore
     （`object/properties.rs:92-118`，新增
     `RuntimeState::shape_cache_remove(shape)` /
     `shape_cache_restore(shape)`）、`unlink_finalized_shapes`
     （`runtime/mod.rs:555-563`）、断言
     （`runtime/layout.rs:73-74`）、fingerprint 测试
     （`object/storage.rs:668-689`）。
   - **过渡表**：`shape_transitions` 由
     `HashMap<ShapeId, HashMap<ShapeEntry, ShapeId>>` 改为
     `FxHashMap<ShapeId, Vec<(ShapeEntry, ShapeId)>>`（实测典型 1–4 项，
     线性扫描）；`shape_transition_parents` 换 `FxBuildHasher`
     （`runtime/mod.rs:119-123`）。
3. **D1c（可选，D1a 后评估）**：对象字面量一次性建形（lowering 传完整
   entries，等价 QuickJS `JS_NewObjectFromShape`），彻底消除中间 shape；
   仅在 D1a/D1b 后仍有可测成本时启动。

**不变量**：shape 缓存条目只在 shape 的 entries/prototype 未被原地修改
时有效；transition 命中仍须 `heap.shape(target).is_ok()` 校验（保持现有
弱引用语义）；错误路径的 fingerprint/cache 回滚语义不变
（`object/properties.rs:108-118` 与 `object/storage.rs:641-691` 测试）。

**验收**：
- 回归微负载（新增到 `target/a4db-decision/micro/`）：
  `{x:1,y:2}`×10k 存活时 shapes ≤ 200；`a=null` 后 ≤ 200；
  `ic_share_2prop` insn / `ic_share_1prop` insn ≤ 1.02（当前 1.154）。
- 固定行不回退：arguments_read、prop_clone、prop_create、prop_delete
  指令下降；SipHash/`hash_one` 符号占比在对应 profile 中消失或 <0.5%。
- `cargo test --locked --workspace --all-targets` 全绿；Test262 冻结向量
  零回归（口径见 §6）。

**回退**：D1a 单独可回退（恢复 in-place 条件）；D1b 与 D1c 独立提交。

### D2 typed arenas（per-kind 槽存储）

**目标**：把 440B 统一 `ArenaSlot` 拆为 per-kind arena，叶节点槽从
440B 降到 ~24B；消除 `Vec` 翻倍峰值。

**证据**：§3.3（10k 对象仅 arena 就 13.4MB）；`ArenaSlot` 440B 断言
（`heap/edges.rs:151`）；strings RSS 11.77×。

**实施点**：

1. **类型泛型化**：`SlotState`（`heap/mod.rs:224-231`）与 `Node`
   改为 `SlotState<T>` / `Node<T> { strong: Cell<u32>, data: T }`，
   非 `Live`/`ZeroQueued` 变体不携带 payload；`ArenaSlot<T>
   { generation: u32, state: SlotState<T> }`。`Heap.slots: Vec<ArenaSlot>`
   （`heap/mod.rs:262-265`）拆为
   `objects/shapes/var_refs/contexts/functions/strings/bigints` 各自
   `Arena<T>` + 各自 free list。
2. **`Arena<T>` 分段增长**：`chunks: Vec<Vec<ArenaSlot<T>>>`，块 64Ki
   槽，索引 = `chunk * CHUNK + offset`；`Vacant` 可不经 `T` 构造，
   避免整表 `Vec` 翻倍峰值（当前 1M 槽 ru_maxrss 可到 used 的 ~1.5×）；
   现有 `reserve_vacant/publish/abort_initializing`（`heap/arena.rs:78-205`）
   泛型化。
3. **weak 链仅对象**：`gc.rs:889-1003` 的 `weak_prev/weak_next` 移到
   `ObjectArena` 的并行向量（随槽增长，仅 weak 路径触碰），
   不给其它 kind 的槽加宽；`weak_collections` 语义不变。
4. **访问层**：`RawId` 布局 12B 不变，`index` 改为 per-kind 索引（kind
   消歧）；`validate_slot_identity`
   （`heap/object_storage.rs:2202-2228`）、`live_node(_fast/_mut)` 按
   kind 分发。`heap/*.rs` 中 37 处 `self.slots[...]` 直接索引
   （`gc.rs` 43 处 `.slots`、`module_storage.rs:802/875/959/1094`）改为
   `slot(id)/object_slot(id)/slot_mut(id)` 等窄访问器。
5. **调试账本**：`alloc_sites`（`gc.rs:2576-2597` 记录/清除，
   2612/2642 读取）改为 per-arena，索引空间随 per-kind 索引。
6. **叶节点**：`allocate_string_leaf`/`allocate_bigint_leaf`
   （`heap/arena.rs:126-152`）槽只存 `generation + strong + JsString/
   JsBigInt`（≈24–32B）；`StringRepr` 仍为 `Rc` 外部载荷（值/共享语义
   不变）。
7. **配套**：`counts()`（`heap/arena.rs:49-76`）、`memory_categories`
   （`heap/profiling.rs:86-221`）按 kind 出数；`ArenaStorage` 每 arena
   独立 trace（`heap/profiling.rs:9-77`）。
8. **尺寸算术**（探针实测：ArenaSlot 440 / NodeData 392 / SlotState 408 /
   ObjectData 224 / Shape 88）：object 槽 ≈ 4(generation) + tag +
   Node<ObjectData 224> ≈ 240B；shape 槽 ≈ 104B；叶槽 ≈ 24–32B。

**不变量**：generational identity 语义不变（槽复用必换 generation）；
zero_queue 仍存 `RawId` 且跨 kind 保序；weak 链仅对象；teardown 时所有
arena 归零；profiling 记账不重复计数。

**验收**：
- 新增尺寸断言：object 槽 ≤ 256B、shape 槽 ≤ 128B、leaf 槽 ≤ 32B
  （在 `heap/tests/` 或 `edges.rs` 同址）。
- RSS：objects/arrays/strings/maps 1M 行相对 E 基线下降 ≥ 35%（目标
  objects ≤ 380 MiB、strings ≤ 250 MiB）；`arena_slots`
  capacity/used ≤ 1.1。
- 固定行不回退 >2%；GC/弱引用/teardown 测试全绿
  （`heap/tests/weak_collections.rs`、`release_cleanup_tests.rs`、
  `heap/tests/storage.rs`）。

**回退**：per-kind 拆分是机械改造，按 kind 分批提交（先 leaf + shape，
再 object/其余），每批可独立回退。

**D2 实施记录（leaf 批次，2026-09-24）**：

- 与计划的差异：本批不立即把统一 `Vec<ArenaSlot>` 拆成 chunked
  per-kind arena，而是先把叶节点（String/BigInt）移出统一槽表，落到
  独立的紧凑 `Vec<LeafSlot>` + free list，并从 `NodeData` 删除
  `String`/`BigInt` 变体；object/shape/var_ref/context/function 仍共用
  原 440B `ArenaSlot` 表，待 shape 批次与 object/其余批次继续拆分。
- `LeafSlot { generation: u32, strong: Cell<u32>, value: LeafValue }`
  = 24B 实测（`LeafValue { Vacant, Retired, String(JsString), BigInt(JsBigInt) }`；
  折叠状态：`strong == 0` 即 zero-queued，generation 饱和 → `Retired`）。
  `RawId` 12B 布局与跨 kind 序号语义不变。
- 分发：`RawId::is_leaf()` 在 retain/release/validate/counts/is_live/
  profiling 等入口分流；typed 包装器（`retain_object`/`retain_string`…）
  直连单态体，通用入口仅服务边表等宽路径；热小函数补
  `#[inline(always)]`（retain/release/preflight/publish/reserve 等），
  消除分发引入的 out-of-line 回归。
- GC：叶边不参与 trial/可达性标记；叶不入 weak 链、不做 anchor/zombie；
  `try_release_leaf_reference`/`retire_validated_leaf`/
  `release_leaf_raw_no_drain`/`reclaim_leaf_vacant`/`finish_leaf` 覆盖叶的
  zero_queue 全路径；`slot_leaf_release_readiness` 以 `leaf_free` 容量为准。
- profiling：`ArenaStorage<T>` 泛型化，`AllocationTrace::record(
  allocation_id, old, new)`；shared arena = id 1、leaf arena = id 2；
  `memory_categories` 增 `leaf_arena_slots`/`leaf_arena_free_indices`。
- 实测（vs D1a `84654cc8`，同机串行 `taskset -c 2`，5 样本中位）：
  - 固定行指令：prop_read −0.24%、prop_write −1.36%、prop_create −0.74%、
    prop_clone −0.16%、prop_delete −0.62%、arguments_read +0.92%
    （门禁 ≤2% ✓）。
  - RSS（1M 行）：strings.js 581.3 → 177.3 MiB（−69.5%，门禁 ≤250 ✓）、
    maps.js 797.0 → 401.7 MiB（−49.6%）；objects/arrays/smallstrings 不变
    （其槽仍在统一 arena，待后续批次）。strings.js wall 1.287 → 0.926s。
- 验证：lib 2317 通过；workspace `--all-targets` 全绿；release 零警告；
  `--features profiling` 仅剩基线既有失败
  `engine::vm::driver::tests::private_field_initialization_uses_fresh_identity_in_published_frames`。
- 新增测试：`heap/tests.rs` 的
  `leaf_arena_keeps_leaf_slots_compact_and_out_of_the_shared_arena`
  （LeafSlot ≤32B、统一槽表不因叶增长、typed lookup 报错、释放回 free）。

**D2 实施记录（cold payload 批次，2026-09-24）**：

- 批次顺序调整：profiling 实测 objects.js 仅 88 个 shape（canonical
  形状复用极高），shape arena 对 1M 内存负载几乎无收益；真正撑大统一槽的
  是 `NodeData` 的最大变体 `FunctionBytecodeData`（392B）。因此本批先
  **box 冷载荷**：`NodeData::FunctionBytecode(Box<FunctionBytecodeData>)`，
  shape arena 顺延。
- 尺寸：`NodeData` 392 → 224（= `ObjectData`）、`SlotState` 408 → 240、
  `ArenaSlot` 440 → 272（`edges.rs` 断言同步；plan 的 ≤256B 目标留待
  weak 链外移或 D3 的 `ObjectData` 瘦身）。
- 实测（vs D1a `84654cc8`）：objects.js 623.2 → 455.8 MiB（−26.9%）、
  arrays.js 608.0 → 440.6 MiB（−27.5%）；maps/strings 不变（叶批已收益）。
  固定行指令全部 ±0.83% 内（prop_read −0.01%、prop_write −0.61%、
  prop_create −0.36%、prop_clone +0.20%、prop_delete +0.19%、
  arguments_read −0.83%）。
- 验证：lib 2317 通过（尺寸断言更新）；release 零警告。

### D3 对象/属性存储：槽瘦身 + 内联槽 + 写快路径

**目标**：`PropertySlot` 32B→24B（拉伸目标 16B）；消灭每对象一次
`Vec` 分配与 2× 容量浪费；降低 prop_write 的 ownership 事务占比。

**证据**：`PropertySlot` 32B、`AutoInitProperty` 32B、`property_slots`
容量 2×（§3.3）；prop_write 670 vs 27 insn/写，ownership 29%。

**实施点**：

1. **D3a 槽瘦身**（两个独立提交，先 accessor 后 autoinit）：
   - **尺寸事实**：`PropertySlot` 现 32B。`AutoInitProperty` 的 32B
     借枚举 niche 压进了槽；但
     `Accessor { get: Option<ObjectId>, set: Option<ObjectId> }`
     （`heap/object_records.rs:10-13`）的 `Option<ObjectId>` 无 niche
     时 12B，两项 24B payload + tag 无法再压。**只 box `AutoInit`
     无法把槽降到 24B**，必须同时压缩 accessor。
   - **accessor 打包**：`ObjectId` 保留 8B，`Accessor { get: ObjectId,
     set: ObjectId }`，以 `{ index: 0, generation: 0 }` 为 null 哨兵。
     安全性依据：generation 从 1 起、`checked_add(1)` 溢出即 retire
     （`gc.rs:1360-1365`、`1717-1722`），0 永不指向活槽。迁移
     `PropertySlot::Accessor` 共 30 处 / 12 文件（`object/storage.rs`、
     `ordinary_storage.rs`、`properties.rs`、`arguments.rs`、
     `private_elements.rs`、`access.rs`、`realm/bindings.rs`、
     `heap/mod.rs`、`heap/gc.rs`、`vm/generator.rs`、
     `builtins/qjs_value_printer.rs`、`builtins/regexp/replace.rs`）。
     备选 `NonZeroU32` generation newtype 因构造点/测试面过大否决。
   - **autoinit box**：`NativeBuiltin { name, length, min_readable_args }`
     （`heap/object_records.rs:27-…`，4 处生产构造）无法从
     `NativeFunctionId` 派生（`descriptor()` 仅含 cproto，
     `builtins/native.rs:1528`，无 name/length 表），故
     `AutoInit(Box<AutoInitProperty>)`；一次性分配只发生在 realm/
     builtin 初始化，不在热路径。
   - **结果**：最大 payload = `Data(RawValue)` 16B → 目标
     `size_of::<PropertySlot>() == 24`，新增断言。
   - **16B 拉伸 spike**（独立，不阻塞主线）：`RawValue` niche 或按
     shape flags 分离 data 载荷表示；有实测收益才上默认路径。
2. **D3b 内联槽**：`ObjectData.slots: Vec<PropertySlot>`
   （`heap/object_records.rs:466-484`）改为无 unsafe 的
   `enum Slots { Inline { len: u8, slots: [PropertySlot; 2] },
   Spilled(Vec<PropertySlot>) }`。
   - API：`len/is_empty/get/get_mut/push/replace/iter/clone_from`；
     不提供 `as_slice()`（内联+溢出无法安全返回连续切片），需要连续
     访问的调用点改 `iter()`。内联第 3 次 push 转 `Spilled`
     （`Vec::with_capacity(3)`，拷贝 2 槽）。
   - 迁移对象载荷 `.slots` 直接访问：`object/*.rs` 约 53 处
     （properties 11、ordinary_storage 11、private_elements 4、
     function_initialization 4、storage 3、array_storage 3、
     property_ic 2、dictionary 2、arguments 2、其余 11）+ 
     `heap/object_storage.rs` 13 处（`gc.rs` 43 处是 Heap 槽索引，
     属 D2）。
   - 尺寸算术：`ObjectData` 224B − Vec 24B + 枚举 56B ≈ 256B；
     2 属性以内对象零堆分配；断言 `size_of::<ObjectData>() ≤ 272`。
   - `validate_object_layout` 的 `slots.len() == shape.entries().len()`
     语义不变；内联空位不进入任何语义路径。
3. **D3c 写事务快路径**：`replace_property_slot_with_status`
   （`heap/object_storage.rs:728-779`）对「旧值 immediate、新值
   immediate、receiver 非最后 owner」的组合跳过
   `retain_edges_transactionally`/`property_slot_edges`/readiness 预检
   （沿用 `try_property_ic_write_scalar`（`object/ordinary_storage/ic.rs:407-470`）
   已有证明模式），并给出「证明与提交之间无可回调」的论证。
4. 配套：`memory_categories` 的 `property_slots` 计入内联容量；
   `ObjectData` 克隆/替换路径（`replace_object_layout`）适配。

**不变量**：`ObjectData.slots` 与 shape entries 平行且等长；所有
edge/atom 释放路径仍覆盖内联与溢出两种形态；`PropertySlot` 尺寸变化
不影响 `slot_matches_storage` 语义；写快路径只在无回调、非最后 owner
时启用（否则回退旧事务）。

**验收**：prop_write insn/写下降 ≥ 25%（目标 ≤ 500，QuickJS 27 不作
本阶段可达目标）；prop_read/prop_create/prop_clone 不回退；
objects 1M RSS 再降 ≥ 20%；`property_slots` capacity/used ≤ 1.1。

**回退**：D3a/D3b/D3c 三个独立提交。

**D3a 实施记录（2026-09-24）**：

- accessor 打包：`PropertySlot::Accessor { get: AccessorRef, set: AccessorRef }`，
  `AccessorRef` 以 generation==0 为 null 哨兵（8B）；新增
  `PropertySlot::accessor(get, set)` 构造器，读取点统一 `AccessorRef::option()`。
  尺寸：accessor pair 24B → 16B。
- autoinit box：`AutoInit(Box<AutoInitProperty>)` + `PropertySlot::auto_init(...)`
  构造器；`AutoInitProperty::realm()` 取代 `property_slot_edges` 的宽
  or-pattern。测试模式断言改为 `slot.auto_init_payload()` 投影
  （`#[cfg(test)]` 辅助）。
- 尺寸实测：`PropertySlot` 32 → **24B**（断言在
  `heap/tests/storage.rs`）；`ObjectData`/`ArenaSlot` 不变（224/272）。
- 实测（vs D1a）：objects.js 455.8 → 425.2 MiB、arrays.js 440.6 → 425.1 MiB；
  固定行全部 ±1.6% 内（prop_clone +1.57%、arguments_read +1.58%）。
- 验证：lib 2318、workspace 全绿；`--features profiling` 仅剩基线既有失败；
  release 零警告。

**D3b 实施记录（inline slots，2026-09-24）**：

- `ObjectData.slots: Vec<PropertySlot>` 改为 `Slots` 枚举：
  `Inline { len: u8, slots: [PropertySlot; 2] }` 前两槽内联，第 3 次 push
  转 `Spilled(Vec)`（`with_capacity(3)`）。
- API 比计划更宽：除 `len/get/get_mut/push/replace/iter/clear` 外实现
  `as_slice/as_mut_slice`（内联活跃前缀本身就是连续切片，安全）、
  `Deref/DerefMut`、`&Slots/&mut Slots` 的 `IntoIterator`，并补齐
  `swap_remove/remove/shrink_to/try_reserve/extend/capacity`。计划以
  “无法安全返回连续切片”为由拒绝 `as_slice`，该论证不成立；提供后
  迁移面显著缩小，字典/稀疏数组路径语义不变。
- `shrink_to` 在溢出向量缩到 ≤2 槽时回到内联；`accounted_capacity`
  供 profiling 使用（内联按 len 计容量）。
- 尺寸：`Slots` 56B、`ObjectData` 224 → 256B、`ArenaSlot` 272 → 304B
  （断言更新；另加 `ObjectData ≤ 272` 断言）。对象槽变宽 32B，但每个
  ≤2 属性对象省掉一次 Vec 分配与 2× 容量浪费。
- 实测（vs D1a）：objects.js 425.2 → **349.1 MiB**（较 D3a 再降 17.9%，
  较 D1a −44.0%；D3 门禁“再降 ≥20%”以 D3a 为基为 23.4% ✓，≤380 目标 ✓）；
  arrays.js 425.5 MiB（数组走 dense 载荷，不受影响）；固定行全部
  ±0.50% 内。profiling `property_slots` capacity/used = 1.0004 ≤ 1.1 ✓。
- 顺带：`NativeCProto::SetterMagic` 与 `RawValue::Exception` 的
  `expect(dead_code)` 因可达性变化不再触发（lint 报 unfulfilled），改为
  `allow(dead_code, reason = ...)`，语义注释保留。

**D3c 实施记录（写事务快路径，2026-09-24）**：

- `replace_object_slot_with_status` 新增 immediate→immediate 快路径：
  旧槽与新载荷都是无 edge/无 atom 的标量时，直接写入并返回空
  `HeapCleanup`，跳过 `validate_replacement_slot`、`property_slot_edges`、
  `retain_edges_transactionally` 与释放/drain。跳过校验的依据是不变量
  「活槽与 shape storage 平行且匹配」：`validate_object_layout`
  （`allocate_object` 入口）与所有替换路径都强制该不变量，且新载荷是
  标量（不可能是 Private），故存储类别校验冗余。条件比计划的
  「receiver 非最后 owner」更强：旧值为标量时释放不触碰任何引用计数，
  receiver 是否最后 owner 无关。快路径只做一次 `object_mut` 查找
  （D3c 迭代中从两次查找合并）。
- `RuntimeState::replace_property_slot`：载荷无 atom（非 Symbol/Private）
  时跳过 `retain_slot_atoms` 的迭代/收集机械（原为 12.7% cycles 热点，
  空 `Vec` 仍需走一遍 filter_map+collect）。
- `apply_cleanup` 对 `HeapCleanup::default()` 提前返回。
- IC 写路径（`try_property_ic_write_scalar`）重排：先定位槽并读取旧值，
  只有旧值非标量时才做 `slot_value_release_readiness_jsvalue` 预检
  （需先 drop heap borrow 再 re-borrow；就绪探针不改堆，槽索引保持有效）。
  旧值为标量时释放无物可放，readiness 与回收无关，直接提交。缓存 miss
  现在先于 readiness 预检填充（纯缓存预热，语义无副作用）。
- 新增测试 `immediate_property_replacement_skips_the_transactional_edges`
  断言快路径返回空 cleanup、槽已替换、receiver strong 不变。
- 实测（vs D3b，7 样本中位，固定负载）：prop_write 3358.43M →
  **2457.86M（−26.82%）**；按计划口径 insn/写 670 → ≈488（≤500 ✓，
  ≥25% ✓）。prop_read −0.00%、prop_create −1.17%、prop_clone +0.02%、
  prop_delete +0.11%、arguments_read −0.04%，均 ≤2% 门禁。D3c 不改布局
  （尺寸断言不变），RSS 不变。
- 验证：lib 2319、workspace 全绿；`--features profiling` 仅剩基线既有
  失败；零警告。

### D4 Map/Set 记录与索引

**目标**：Map/Set 记录存储从「HashMap + BTreeSet 每记录两棵节点」改为
稠密记录数组；字符串键哈希不再每次全串 SipHash；`CollectionsState` 等
辅助表去 SipHash。

**证据**：`CollectionRecords` 152B/实例 + 每记录 HashMap 节点 +
BTreeSet 节点；map-int cache-references 1300×、map-string 553×；
map-string SipHash 6.4%（含 `hash_one`）；map_get 同键微负载 7.29×。

**实施点**：

1. `CollectionRecords`（`heap/collection_records.rs:15-26`，现为
   `entries: HashMap<usize,(MapRecord,u64)> + order: BTreeSet<usize>`）改为
   稠密槽 + 有序 live 索引：
   - `slots: Vec<Slot>`，`Slot { record: MapRecord, hash: u64 }` +
     `free: Vec<u32>`；`live: Vec<LiveEntry { id: u32, slot: u32 }>`
     按 id 升序（id 单调 ⇒ 天然插入序），删除置 `slot = DEAD` 墓碑；
     `live_len` 单独维护；`next_id: usize` 时钟不变。
   - 语义映射：`get(id)` = `live` 二分 → 槽；`next_at_or_after(cursor)`
     = 二分后跳过墓碑（O(log n)，无需 BTreeSet）；`ids()/iter()` 自定义
     `DoubleEndedIterator + ExactSizeIterator` 扫 `live` 跳墓碑；
     `insert_hashed` = 复用 `free` 或 push 槽 + `live.push`（id 递增）；
     `remove` = 二分 + 墓碑 + 槽入 `free`；`take_all` 排空并归零
     `slots/live/key_index` 容量（时钟保留）。
   - **容量回收契约**（`heap/collection_records.rs:201` 测试）：删除后
     几何收缩（沿用现值策略 `capacity > 4*live+64` → `shrink_to(2*live+32)`，
     收缩时重建 `free`），`take_all` 后容量为 0 且下一次 `insert` 仍返回
     原 `next_id`（id 不复用）。
   - **必须保住的 API/语义**（调用点与测试即契约）：
     `len/is_empty/next_id/get/get_mut/replace_value/ids/iter/
     next_at_or_after/find/find_entry/preflight_insert/
     precompute_insert_hash/insert/insert_hashed/remove/take_all/validate`；
     `next_at_or_after` 的 cursor 是**记录 id**（`builtins/map.rs:842`、
     `map/callback.rs:201`、`set.rs:529/778`），id 有效性以
     `id < next_id` 为界（`heap/object_storage.rs:2052/2097`、
     `qjs_value_printer.rs:974`），墓碑/过期 id 必须 miss；墓碑参考模型
     测试（`collection_records.rs:229`）逐条对齐；`validate` 仅发布期
     全量校验（live 有序唯一、槽引用有效、`key_index` 一致）。
   - `CollectionIndex`（`heap/collection_index.rs`）仍以记录 id 为
     桶内容，`retained_capacities` 语义不变（测试依赖）。
2. `CollectionIndex`（`heap/collection_index.rs:20-30`）哈希：
   - 字符串键：`StringRepr`（`value/primitive.rs:141`）增加
     `seeded_hash: Cell<Option<u64>>`（懒计算，不影响 `JsString` 8B 与
     共享语义）；进程级 `OnceLock<RandomState>` 一次性随机种子，
     对 `hash_code_units` 流式 FxHash 后缓存。`CollectionIndex::hash`
     （`collection_index.rs:77-105`）与 `collection_key::hash` 字符串分支
     （`value/collection_key.rs:88-96`）改读该缓存值，不再每次全文
     SipHash；`buckets` 已是 identity hasher。
     - 安全口径：随机种子不可预测，抗 hash-flooding 性质保留；不再
       逐 Map 独立种子（与 V8/QuickJS 的 per-runtime 种子同级），
       在计划验收中显式记录该信任模型收敛。
   - 现有 per-index memo（`STRING_HASH_CACHE_SIZE = 8`）保留为过渡，
     正式方案为上述 `StringRepr` 缓存。
   - BigInt 键维持现状（未测到热点），如后续测量需要按同法缓存。
3. `CollectionsState.maps/sets: HashMap<ObjectId, HashSet<usize>>`
   （`heap/collections.rs:20-21`）与 `WeakCollectionRecords` 改
   `FxBuildHasher`。
4. **明确不做**：native 调用编组链（map-int ≈25%）归 B 重新立项；
   D4 只改记录/索引/哈希。

**不变量**：Map/Set 插入顺序、`forEach` 期间删除可见性、迭代器失效
语义与 QuickJS 对齐（现有 `ActiveCollectionRecordGuard` 语义不变）；
字符串键哈希种子变更不得影响可观察行为。

**验收**：map-int/map-string/map_delete 指令下降 ≥ 10%；Map 1M RSS
下降 ≥ 25%；Map/Set/WeakMap Test262 全绿；字符串键 SipHash 符号占比
<0.5%。

**回退**：记录存储与哈希缓存两个独立提交。

### D4 实施记录（2026-09-24）

**提交**：`17a77e6e`（记录存储）、`2bb3595c`（字符串键哈希）；最终配置
（下称 d4d）为两提交之和。

**实施**：
- `CollectionRecords` 改为稠密槽数组 + 有序 live 索引：`slots:
  Vec<Slot { record, hash }>`、`live: Vec<LiveEntry { id: usize,
  slot: u32 }>`（按 id 升序）、`live_len`、`next_id`；删除置 `DEAD`
  墓碑，几何压缩回收（`slots.len() > 4*live_len + 64` → 重建，O(live)
  摊销）。API 与语义（id 不复用、cursor 失效边界、双向 ExactSize
  迭代器、`take_all` 时钟保留）全部保留；`validate` 改为校验 live
  有序唯一、槽引用与 `key_index` 一致。
- **偏离 1**：计划的 `free: Vec<u32>` 未实现——墓碑 + 压缩已把槽数
  限定在 `4*live+64` 内，少一份状态；容量契约测试仍覆盖收缩与
  `take_all` 归零。
- **偏离 2**：`LiveEntry.id` 保留 `usize`（非计划的 `u32`），以保住
  `usize::MAX` 耗尽的现有语义与测试。
- `CollectionIndex` 字符串键改走进程级随机种子的 FxHash
  （`hash.rs:collection_hash_seed` + `FxHasher::with_seed`、
  `collection_key::string_hash`），非字符串键保留每索引 SipHash。
- **偏离 3**：计划的 `StringRepr.seeded_hash` 每节点缓存最终不做：
  实测每百万字符串 +16 MiB RSS（strings.js 176.7→192.0、maps.js
  326.7→342.1），而指令数与无缓存版逐项相同（per-index memo 已覆盖
  同节点复用）；改为未命中即时 FxHash。信任模型收敛（进程级种子、
  与 V8/QuickJS per-runtime 同级）照计划记录。
- **偏离 4**：非字符串键扩展到 seeded FxHash 的尝试在 fat LTO 下
  map_set_int 回退 +9.1%（368.6M→402.2M，疑布局敏感；哈希分布已
  验证 max bucket ≤2），已回滚并保留每索引 SipHash；回滚后 d4c2 与
  d4b 逐项一致。
- 计划点 3 的 `CollectionsState` 已不存在，实际改
  `CollectionIteratorCurrentIndices` 与 `WeakCollectionRecords` 为
  `FxBuildHasher`。
- `ArenaSlot` 304 → 288B（非 272：272 出现在已回滚的
  no-`key_hasher` 变体）。

**实测**（vs D3c，5 样本中位）：
- map-int 39235.93M → 36579.17M（−6.77%；对 d1a 累计 −8.00%）
- map-string 12588.50M → 11461.80M（−8.95%；累计 −11.19%）
- map_delete 513.62M → 480.11M（−6.52%；累计 −8.20%）
- map_set_int −7.61%、map_set_string −6.06%、weak_map_set ±0.00%、
  map_get_rotate_keys −5.44%、map_get_same_key −0.75%（per-index
  memo 早已覆盖同节点）。
- RSS：maps.js 401.2→326.8 MiB（−18.55%）；objects.js 348.7→333.1
  （−4.46%）；arrays.js 424.8→409.5（−3.59%）；strings.js
  176.7→176.5（−0.08%，无回退）。
- 字符串键路径 profiling 无 SipHash/`hash_one` 符号（占比 <0.5% ✓）。

**门禁结果**：
- 指令下降 ≥10%：**未达**（slice 6.5–9.0%；对 d1a 累计 8.0–11.2%）。
  剩余成本集中在 native 调用编组链（~25%，计划明确归 B），D4 边界
  内无更多结构性空间；按「记录收窄 + 累计口径」接受或扩大范围待裁决。
- Map 1M RSS 下降 ≥25%：slice −18.6% 未达；对 d1a 累计 −59.0%
  （797.0→326.8 MiB）。
- Map/Set/WeakMap Test262：全量冻结向量复跑（`--full`，workers=2，
  `target/test262-full.tsv`）中 Map/Set/WeakMap/WeakSet 共 1620 行全部
  pass、零 fail；全量分类汇总与阶段 A 收尾状态逐项一致（pass=80010、
  unsupported-negative-provenance=2534，其余类别与冻结 receipt 相同），
  +28 行经核对恰为 `00bb387f fix(lexer)` 的 14 条契约 ×2 变体，D 未
  引入行级变化。字节比对门禁仍因冻结 receipt 停留 `022e7b48` 而不过
  （基线晋升为独立事项，见 `s3-a-closure-plan.md`）。
- 验证：lib 2319、workspace `--all-targets` 全绿、零警告；
  `--features profiling` 仅剩基线既有失败。
- **裁决（2026-09-24）**：D4 按「记录收窄 + 累计口径」接受——指令
  slice 缺口与 native 调用编组链一并归 B 重新立项处理，Map RSS 以对
  d1a 累计 −59.0% 计；不再扩大 D4 范围。D5 按启动门禁未满足降级为
  backlog（见下）。

### D 收尾记录（2026-09-24）

**范围**：D1a/D1b（shape 缓存一致性 + FxHash 指纹免分配）→ D2（叶节点
typed arena + 冷载荷装箱）→ D3a/D3b/D3c（accessor 打包、lazy-intrinsic
装箱、双内联槽、immediate 写快路径）→ D4（Map/Set 稠密记录 + 字符串键
seeded FxHash）。全部按片独立提交、可单独回退。

**最终证据**（对 D1a 累计，fat LTO、`taskset -c 2`、中位）：
- 尺寸：`ArenaSlot` 440 → 288B；`ObjectData` 256B（`≤272` 断言）。
- 1M RSS：objects.js 425.2 → 333.1 MiB（−21.7%）；maps.js 797.0 →
  326.8 MiB（−59.0%）；strings.js 无回退（D4 去掉每节点缓存后
  176.7→176.5 MiB）。
- 指令：map-int −8.00%、map-string −11.19%、map_delete −8.20%；
  prop_write −26.82%（D3c slice）；固定行其余项 ≤±0.5%。
- 未达项：D4 slice 指令 ≥10%（6.5–9.0%）、Map slice RSS ≥25%
  （−18.6%）；D3 各片门禁均达成。

**正确性/门禁**：`cargo test --locked --workspace --all-targets` 全绿
（lib 2319）、零警告、`--features profiling` 仅基线既有失败；
Test262 全量复跑与阶段 A 状态逐项一致（无 D 引入的行级变化，见上）；
`python3 scripts/checks/check-source-layout.py` 通过（698 个 reachable
Rust 文件）。

**Test262 基线晋升（2026-09-24 完成）**：原挂账已解决。先对齐覆盖契约
（`0e72387f`：门禁硬编码契约、`current.conf` 的 `engine_semantics_files/
trees` 与工作区指纹统一为 7 文件 + 6 棵树 `adapters,apps,conformance,
examples,src,tests`），再晋升基线（`f69e545c`）：
`engine_semantics_source=0e72387f`、`engine_semantics_sha256=1ce1d4f2…`；
全量 pass 79982→80010、eligible/runnable 80032→80060、
unsupported-negative-provenance 2562→2534（+28 行即 `00bb387f` 的 14 条
契约 ×2 变体，与阶段 A 逐行核对一致）；focused 保持 6844/6844，仅首行
身份刷新。`--check`、`--focused`（字节一致重放）、`--full`（80010 pass /
80060 eligible / 102037 total）全部认证通过；`README.md`/
`docs/status.md`/`docs/test262.md` 指标块由
`scripts/test262/current-test262-metrics.mjs --write-docs` 同步。

**下一步**：B 重新立项（自改写专用字节码，先写 spike/设计文档，见
[性能架构](performance-architecture.md) §11）。

### D5 per-prototype validity cell（条件项，需先补证据）

**降级理由**：本轮 `proto_mutate`（每 1024 次读写一次 `P.prototype`）
与 `unrelated_mutate`（写无关原型）指令数完全相同
（1,178.74M vs 1,178.74M），`property_ic.hit` 200196、`miss` 3；当前
微负载测量不到「任一 proto 写全堆杀 depth>0 IC」的成本。旧 §6 的收益
主张缺证据。

**启动门禁（两者同时满足）**：
1. 在真实原型链负载（deltablue/richards 类）中，depth>0 的 IC 命中占
   属性读 ≥ 30%，且 proto 写频率足以造成反复重特化；
2. 有同协议 A/B 证据表明重特化成本 ≥ 3% 指令或 ≥ 2% wall。

**若启动**：`Location.prototype_epoch`（`object/property_ic.rs:10-20`）
改为 per-prototype cell（槽索引 + generation + 值），
`invalidate_property_layout`（`heap/object_storage.rs:14-18`）只 bump
具体原型；IC 条目记录链上 cell 的校验值。实现前须单独写 spike 计划。

## 5. 排程

1. **D1a**（最小一致性修复）→ 独立提交 + 微负载/Test262 验证。
2. **D1b**（查找免分配/哈希）→ 独立提交。
3. **D2**（typed arenas）→ 按 kind 分批（leaf+shape 先行）提交。
4. **D3a-accessor → D3a-autoinit → D3b → D3c** 独立提交；16B 拉伸
   spike 可并行但不上默认路径。
5. **D4** 记录存储 → 字符串哈希缓存，独立提交。
6. **D5** 仅在门禁满足时另立 spike；否则记入 backlog。
7. D1 与 D2 可并行改码；所有测量串行（`taskset -c 2`）。

## 6. 验收门禁

- **构建/正确性**：`cargo fmt --check`、`cargo check --all-targets`
  零警告、`cargo test --locked --workspace --all-targets` 全绿；
  阶段收尾跑 Test262 冻结向量零回归（对照口径：
  pass=79982 / eligible=80032 / total=102037，或按当轮冻结 receipt）
  与 `python3 scripts/checks/check-source-layout.py`。
- **基准**：双协议（LTO off + CGU16 与 fat LTO + CGU=1）配对轮换，
  **instructions 为主信号**，3 次中位；固定行 = bigint32/64/256、
  scaling（map-int/map-string/typed-index/prop-delete）、V8、
  fixed 字符串簇、`a=a` 微负载，以及本轮新增
  `ic_share_1prop/2prop`。
- **内存**：`ru_maxrss`（3 次中位）+ profiling 记账
  （`cargo build -p quickjs-oxide-cli --features profiling`，`-d -T
  --profile-json`）；每片记录 `arena_slots`/`property_slots`/
  `string_nodes` 的 used/capacity。
- **尺寸断言**：每片更新/新增 `size_of` 断言（`edges.rs:151` 同址）。
- **不回退**：prop_read/prop_write、Map 构造/insert、A 既有收益、
  E 基线固定行不得劣化 >2%（超出需与 A/A 噪声对照后判定）。
- **每项独立提交、可单独回退；不得跨片混合提交。**

## 7. 风险

- **D1a 语义风险**：in-place append 条件收紧可能让部分高频 define 走
  过渡路径变慢。缓解：以 `prop_create`/`prop_clone` 指令数为门禁，
  若回退则保留 canonical 判断但允许非 canonical 独占 shape 原地追加。
- **D2 触及 GC 核心**：weak 链、zero_queue、retire、teardown 断言集中
  在 `heap/gc.rs`；缓解为分批（先 leaf/shape）、保留 `RawId` 布局、
  debug 全量校验、GC/弱引用/teardown 测试必跑。
- **D3 改变布局校验**：`validate_object_layout` 与所有
  `PropertySlot` 匹配点（含 release/atom 收集）需同步；内联占位值
  绝不能泄漏到任何语义路径。
- **D3a 哨兵风险**：accessor 的 generation 0 空哨兵依赖「generation
  从 1 起且溢出即 retire」不变量；以 `debug_assert` + 构造集中化
  保证，测试覆盖 remove 后槽复用与 accessor 读写。
- **D4 顺序语义**：插入序、`forEach` 删除可见性、迭代器失效必须与
  QuickJS 对齐；以 Test262 Map/Set 全绿 + 现有 `ActiveCollectionRecord`
  测试为门禁。
- **哈希信任模型收敛**（D4）：字符串键从 per-Map 随机种子改为
  per-runtime 随机种子，需在提交信息与文档显式记录；不得退化为
  可预测种子。

## 8. 证据路径

- 决策与实测：`docs/reports/s3-a4-d-b-decision.md`、
  `target/a4db-decision/{stats.json,mem-results.json,profiles/}`
- 本轮新增：`target/a4db-decision/micro/ic_share_{1,2}prop.js`、
  `target/a4db-decision/ic/{no,unrelated,proto}_mutate.js`；
  dev profiling 记账命令见 §6
- 旧 D 草图：`docs/reports/performance-architecture.md` §6（本计划取代）
- 尺寸探针口径：临时 `cargo test --lib` 探针（本轮已移除，结论见 §2/§3.3）

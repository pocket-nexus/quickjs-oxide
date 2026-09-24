# A4 / D / B 下一阶段选择：调研与实验结论（2026-09-24）

> 状态：调研完成，给出数据裁决建议。本文不实现任何方案，只回答
> 「A4、D、B 先做哪个」。基线为 `feat/pr27-a4`（= PR27 分支 tip，
> T1/T2 已收口）；测量二进制为 post-T2 发布构建 `c4f1c433…`
> （源码与当前 HEAD `5182979a` 仅差 docs 提交）。

## 1. 问题与既有路线

`performance-architecture.md` §11 的路线是 **E（已完成）→ 残余批
T1/T2（已完成）→ A4 决策点 → D**，B 因与 C 耦合回退、须重新设计。
A4 是「决策点」而非无条件迁移：在 E 基线上做 8B NaN-box spike 与验收
矩阵（bigint256/arguments/typed-index/RSS），用数据裁决「做」或按 §4.3
停在 16B。本轮把该裁决所需证据补齐，并与 D、B 的实测头寸做横向比较。

## 2. 方法与协议

- 引擎：`oxide` = post-T2 发布二进制（fat LTO + CGU=1、无 PGO/profiling，
  `target/release` 构建 receipt 对应 `c4f1c433…`）；对照 = pinned
  QuickJS 2026-06-04（`d0f8966b…`）。
- 负载：T2/T1 使用的固定与微负载、`s3-a-r1-perf` 的 bigint32/64/256 与
  scaling 生成负载（map-int/map-string/typed-index/prop-delete），以及
  新增微探针（empty_loop/int_local/prop_read/array_read/call0/
  bigint_loop/map_get_*）。
- 工具：`taskset -c 2` + `perf stat`（instructions/cycles 用户态，
  3 次中位）+ `perf record -F 1997` 符号归因；RSS 用 `wait4`
  `ru_maxrss`（3 次中位）。governor=powersave，单机串行。
- 证据目录：`target/a4db-decision/`（`profile.py`、`stats.json`、
  `profiles/{oxide,quickjs}/*.txt|.data`、`mem.py`、`mem-results.json`、
  `micro/`、`analyze.py`）。

限制：单机单轮；`perf` cycles 归因在长延迟指令附近有 skid；QuickJS 是
C 实现，符号级对比只用于定位结构性差异，不构成逐函数等价比较。

## 3. 实测结果

### 3.1 对 QuickJS 的指令/周期差距（固定负载）

| 用例 | oxide insn(M) | quickjs insn(M) | insn 比 | cycles 比 |
| --- | ---: | ---: | ---: | ---: |
| bigint256 | 7574.8 | 1646.0 | 4.60× | 6.86× |
| bigint64 | 4691.1 | 700.3 | 6.70× | 10.28× |
| bigint32 | 4175.1 | 514.8 | 8.11× | 10.09× |
| locals | 4141.5 | 500.8 | 8.27× | 10.01× |
| frames | 98.2 | 9.2 | 10.66× | 9.56× |
| empty_loop | 3773.1 | 562.1 | 6.71× | 10.67× |
| prop_read | 4388.8 | 445.1 | 9.86× | 12.67× |
| **prop_write** | 3374.9 | 137.3 | **24.59×** | 30.61× |
| prop_create | 2423.2 | 211.8 | 11.44× | 13.99× |
| prop_clone | 1595.5 | 274.1 | 5.82× | 7.87× |
| array_read | 4088.6 | 294.2 | 13.90× | 22.12× |
| array_write | 1312.3 | 67.1 | 19.55× | 20.67× |
| typed_array_read | 2498.4 | 203.0 | 12.30× | 17.02× |
| arguments_read | 2808.8 | 242.1 | 11.60× | 16.50× |
| int_to_string | 4306.9 | 809.2 | 5.32× | 8.20× |
| string_build1 | 2447.9 | 467.5 | 5.24× | 4.98× |
| string_build_large1 | 4114.1 | 1185.5 | 3.47× | 4.43× |
| map_delete | 529.7 | 65.2 | 8.12× | 11.34× |
| map-string | 12785.0 | 1869.1 | 6.84× | 12.15× |
| map-int | 39209.7 | 3778.2 | 10.38× | 16.51× |
| typed-index | 5481.7 | 618.9 | 8.86× | 12.32× |
| prop-delete | 9679.4 | 1592.4 | 6.08× | 9.50× |

差距集中在**属性/数组/索引/内建调用**（8–25×）与**解释器逐操作开销**
（empty_loop 6.7×、locals 8.3×），而不是值宽度。注意 QuickJS 64 位
**不是 NaN-box**：`quickjs.h` 中 `#ifndef JS_PTR64 #define JS_NAN_BOXING`，
64 位 `JSValue` 为 16B（union + int64 tag），`JS_SHORT_BIG_INT_BITS=64`。
即 QuickJS 用与我们相同的 16B 值仍快 5–17×。

### 3.2 逐操作成本（10M 次循环微探针，oxide vs QuickJS）

| 微探针 | insn 比 | cycles 比 | 备注 |
| --- | ---: | ---: | --- |
| empty_loop | 6.74× | 11.03× | 纯派发，6 op/iter：608 vs 90 insn/iter |
| int_local | 10.60× | 18.48× | `s=s+1` |
| prop_read | 8.59× | 12.01× | `s+=o.a` |
| array_read | 11.10× | 18.64× | `s+=a[j&3]` |
| call0 | 10.86× | 14.37× | `s+=g()` |
| bigint_loop（20 万次乘） | 1.16× | 0.97× | num_bigint 与 QuickJS BigInt 同量级 |
| map_get 同键（200 万） | 7.29× | – | QuickJS 有 string hash 缓存 |
| map_get 轮换键（200 万） | 5.87× | – | 我们键轮换 +39% insn，QuickJS +73% |
| prop_read 同键（200 万） | 8.37× | – | – |

按单操作换算的固定负载：`prop_write` 670 vs 27 insn/写（24.6×）、
`array_write` 209 vs 10.7、`array_read` 158 vs 11.4、
`typed_array_read` 183 vs 14.8。

### 3.3 热点归因（perf 符号自时间，oxide）

- **解释器循环**：empty_loop 88%、prop_read 68%、locals 57%、array_read
  50% 在 `run::run`。`perf annotate` 显示热指令是 **16B 操作数/值在栈
  临时区之间的搬运**（`movaps/movapd/movups` 到 `0x190/0x4a0(%rsp)` 等，
  locals 内约 37% 的 `run` 采样）与**带内存 Result 判定的 outlined 调用**
  （调用 `push_current` 后 `cmpl $0x2,0x30(%rsp)`）。QuickJS 对应
  84–100% 集中在单个内联 `JS_CallInternal`。
- **属性写**（24.6×）：ownership 29%（`replace_retained_object_slot`
  13.7%、`slot_release_readiness`、`dup/release_jsvalue`）+ property 24%
  （`retain_slot_atoms` 9.0%、`replace_object_slot_with_status` 3.9%）+
  slots 24% + interp 19%。每次写仍走「校验→事务性 retain→替换→释放旧边
  →drain zero queue」。
- **数组/索引读**：interp 45–50% + slots 24–34% + ownership 11–16%，
  `array_immediate_read` 5–18%；`arguments_read` 额外有 SipHash 13.4%、
  `property_slot_edges`/`retain_edges_transactionally` 10.8%。
- **Map/内建调用**：`proxy_get_driver::start_native_with_classification`
  + `take_native_call_operands_current` + `prepare_native_arguments` +
  `driver::ready::enter_call` 合计约 22–30%（map-int/map-string/map_delete）。
- **哈希**：SipHash/BuildHasher 在 prop_clone 24%（含 hashbrown）、
  arguments_read 13%、prop_create 4–7%、map-string 3–6%、prop-delete 6%。
  更正架构 §1.6 的过时表述：当前树 `JsString` 已缓存未加盐
  `content_hash`（`primitive.rs:142/935`），atom 表也已用 FxHash
  （`atom/mod.rs:367`）；剩余成本来自 `collection_key::hash` 对字符串键
  刻意用随机化 SipHash 重算全文（`collection_key.rs:95`，抗 hash-flooding
  语义）、仍用 std RandomState 的热表，以及 shape_transitions 嵌套
  HashMap（`heap/runtime/mod.rs:121`）。
- **字符串/大整数分配**：string_build_large1 中 `malloc/cfree` 7%、
  rope 构造 11%、`release_jsvalue` 7.8%。

### 3.4 内存形状（1M 项，ru_maxrss 中位）

| 负载 | oxide | QuickJS | 比 | 备注 |
| --- | ---: | ---: | ---: | --- |
| `{a:1}`×1M | 623.4 MiB | 126.6 MiB | 4.93× | ArenaSlot 440B + 每对象 Vec<PropertySlot> |
| `[i,i+1]`×1M | 608.1 MiB | 149.9 MiB | 4.06× | 同上 |
| `"s"+i`×1M | 581.5 MiB | 49.4 MiB | **11.77×** | 字符串节点约 610B/条 vs ~50B |
| Map("k"+i)×1M | 796.9 MiB | 107.7 MiB | 7.40× | 记录 + 键字符串 |
| 同值 "abc"×1M | 35.8 MiB | 18.7 MiB | 1.91× | 值数组 16B/项 |

静态事实：`ArenaSlot` 实测 **440B**（`src/engine/heap/edges.rs:151`
断言），单一混合 arena，最大变体 `ObjectData`/`ObjectPayload` 撑起；
`ObjectData.slots: Vec<PropertySlot>` 每对象一次独立分配；
`PropertySlot` 含 16B `RawValue`、16B accessor、以及带 `&'static str`
的 `AutoInit`；QuickJS `JSProperty` 为 16B。

### 3.5 A4 约束复核（既有证据 + 本轮确认）

- **身份**：`ObjectId/StringId/BigIntId` = u32 index + u32 generation（8B），
  52 位 payload 装不下完整身份。既有 spike（`git show 3e116f75:...`）
  用真实槽复用测试否决了「index-only 解码 + 回读当前 generation」：
  stale owner 会复活；截断 generation 同样碰撞。
- **ShortBigInt**：`i64` 装不进 52 位。实际 `bigint64` 负载内层
  `a*a`（`a0=1n<<27n`）约 2^54，`result` 约 2^64——A4 会把当前
  inline `ShortBigInt` 的热中间值推回堆节点，直接冲击 bigint64 固定行
  （当前 wall 0.90× pre-A）。
- **槽步长**：T2 spike 实测 A4 8B `Direct` + 现有句柄变体仍是 16B
  （`A4HandleBinding`），`FrameBinding`/`SlotStore` 不自动变 8B；要
  8B 槽必须 T2d 整体打包（`PackedBinding` 8B 但 `Option` 16B，空槽需
  保留 tag）。A4 单独落地不触及最热的帧槽缓冲。
- **迁移成本**：阶段 A（16B 句柄迁移）实证为「迁移 + 一整轮回退修复」；
  A4 触及所有模块且叠加上述三个设计点。

## 4. 三方案评估

### 4.1 A4（8B 值表示）——数据裁决：**保留 16B（暂缓）**

- 可及收益面窄：只削值搬运宽度与 `RawValue`/属性槽/常量池密度。
  本轮 profile 显示主导成本是**所有权事务、属性/索引查找、哈希、
  解释器模板代码**，不是宽度；`SlotStore` 与指令操作数不受益。
- 与 QuickJS 对照：QuickJS 64 位同为 16B 值仍快 5–17×，说明宽度不是
  结构性差距来源。
- 明确风险：身份重设计（信任模型变更或旁表）、ShortBigInt 覆盖缩小
  （bigint64 固定行回退风险）、T2d 依赖、迁移+修复成本。
- 结论：按 §4.3 退路**保留 16B**，把「A4+T2d 联合设计」记为 D 之后的
  可选复核项；如未来值宽度密集路径仍显著，再做受限 spike。

### 4.2 D（数据导向堆）——**建议优先**

> **2026-09-24 后续**：下面的切片清单是决策当时的排序草图，已被
> [阶段 D 实施计划](s3-d-plan.md) 取代。定稿计划新增了本轮实测发现的
> **多属性对象私有 shape 缺陷**（10k 个 `{x:1,y:2}` → 10167 个 shape，
> IC 读 +15.4% 指令），并据此重排：**D1 = shape 缓存一致性 + 查找免
> 分配**（最高优先），原 D1 哈希并入 D1b/D4，native 编组链移出 D
> （归 B），D5 IC validity cell 因微负载对照无成本证据降级为条件项。

- 证据最强且方向确定：RSS 4–12×、SipHash 3–24%、属性写 24.6×、
  数组写 19.6×、Map 内建调用链 22–30%、`ArenaSlot` 440B 与
  每对象 `Vec<PropertySlot>`。
- 可切片、可独立回退、每片可测，且不改变值语义/信任模型
  （以下为决策草图，定稿见 [s3-d-plan.md](s3-d-plan.md) §4）：
  1. **D1 哈希便宜化**：Map/Set 字符串键的随机化 SipHash 重算
     （`collection_key.rs:95`）与 std RandomState 热表、shape 迁移嵌套
     HashMap 的取舍；
  2. **D2 叶节点紧凑 arena**：String/BigInt 独立 typed arena（收
     610B/条），顺带缓解 `malloc/cfree`；
  3. **D3 对象/属性存储**：内联槽 + 更小的 `PropertySlot`/shape 更新
     事务（收 440B 混合槽与每写 retain/readiness 事务）；
  4. **D4 Map/Set 记录与内建调用快路径**（收 native 编组 22–30%）；
  5. **D5 IC validity cell**（per-prototype 失效，替代全堆 epoch）。
- 预期：架构估计对象/数组密集 +10–30%；本轮证据显示内存收益确定，
  且这些路径正是对 QuickJS 差距最大的区间（8–25×）。

### 4.3 B（quickening）——**重新设计、后置**

- 头寸最大：逐操作解释器开销 6.7–11× 指令、11–18× 周期，`run::run`
  自时间 50–88%，热指令是操作数搬运与 outlined 调用；这是与 QuickJS
  的结构性差距，理论上限最高。
- 但首批实现已按负结果回退：B1 的 QuickOp codec/projection 增加
  认证后解码与二次分类，八个定向输入 +14–54% 指令、guard decline
  25–72%、compile +1.3%，无净收益；C 文档明确「quickening 特化需要
  重新设计并独立立项」。
- 重新设计方向应是**自改写专用字节码**（CPython PEP 659 式：热指令
  原位改写为携带已解码操作数/类型的专用形式，命中路径不二次解码），
  而不是并行投影层；验收须直接对准 empty_loop/locals/prop_read 的
  每操作指令数。
- 建议在 D 的首批切片落地后启动（D 的紧凑存储与便宜键会让 B 的专用
  handler 更便宜），或与 D 并行做设计/可行性，但不要先于 D 动默认路径。

## 5. 结论

1. **下一步做 D**：定稿计划为 [阶段 D 实施计划](s3-d-plan.md)
   （D1 shape 一致性/免分配 → D2 typed arenas → D3 槽瘦身/内联/写快
   路径 → D4 Map/Set 记录与键哈希 → D5 条件项），每片独立 A/B 与回退。
2. **A4 记数据裁决：保留 16B**；不启动完整迁移，仅保留
   「A4+T2d 联合」为 D 后可选复核。
3. **B 重新立项**：以自改写专用字节码重新设计，排在 D 首批切片之后
   （或并行只做设计），不得复活已回退的 B1 投影层。
4. 已完成写回：`performance-architecture.md` §1.6/§6/§11 已更新并指向
   本报告与 [s3-d-plan.md](s3-d-plan.md)；`s3-a-closure-plan.md` §7
   已补后续指针。

## 6. 证据路径

- 归因与统计：`target/a4db-decision/{profile.py,stats.json,analyze.py}`、
  `profiles/oxide/*.txt`、`profiles/quickjs/*.txt`
- 内存：`target/a4db-decision/{mem.py,mem-results.json,mem/*.js}`
- 微探针：`target/a4db-decision/micro/*.js`（原始 perf stat 输出在运行日志）
- 既有证据：`s3-c-negative-result.md`、`s3-full-rerun-results.md`、
  `s3-a-closure-plan.md` §4.7–4.9、`s3-bc-implementation.md` §3
- A4 约束原测：`git show 3e116f75:src/engine/heap/tests/representation_spike.rs`

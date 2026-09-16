# S21 — Proxy 陷阱选择缓存与 [[Get]] 同步快路径

**执行指令（沿用 S14–S20 纪律）：** 分阶段提交全部实现；实现期间不运行 benchmark/cost profile/CPU profile；全部完成并逐项 review 后，仅对最终核心执行一轮测量（S0 与 QuickJS 复用 `target/performance-retained/` 既有三轮）。测量结果保留在本地 `target/performance-retained/`，不提交 Git。

2026-09-16 立项。前置：S14–S20 已全部落地（d63c34b0）。目标：消除调用探针族最后一组正差值——depth-proxy 0/32/128 相对 S0 +26.6%/+28.7%/+21.5%（latest-round 三轮中位，重复内波动 <1%）。探针族其余成员已达标（getter −3.0%~−12.6%、native +7.3%~−6.1%、mixed +2.5%~−0.1%），残留已定量确证为 proxy get 陷阱主干专属。

## 1. 根因回执（2026-09-16 补测证据）

证据来源：本轮对 `depth-proxy-0`/`depth-getter-0` 的 profiling 二进制 cost profile（`owned_execution_events` 逐迭代计数）与 release 二进制 perf 采样（20 kHz）；工作负载 `target/primitive-vm-s09-implementation/measurements/depth-proxy-*.js`。

**定量框架**：getter 读 1.20 µs/迭代、proxy 读 2.16 µs/迭代 → proxy 专属开销 ≈960 ns；S0 同差值 ≈470 ns（1.71−1.24 µs）。多出的 ≈490 ns × 100k 迭代 ≈ 49 ms ≈ 全部回退量（45.4 ms）。

**每迭代机械计数（proxy vs getter）**：query_dispatch 3(read/descriptor/complete) vs 0；dispatch_read.get.visit 5 vs 0；property_storage_read_probe 3 vs 1；heap root 拷贝 2 vs 0；slot 初始化/清理/搬运 9/8/27 vs 3/2/21；value_copies 6 vs 3。S18 已生效项（不再立项）：帧惰性 install、认证缓存、冷帧复用、query 存储池化、PooledBox 稳态零分配（全程各 1 次）、`property_return_direct` 快速回执、PC 发布全程 11 次。

**代码级根因（按耗时排序）**：

- **R-A 陷阱查找零缓存**：每读执行 `MethodStep::start` → `Search::read`（`object/internal_methods/method.rs:92-121`）：`root_proxy_snapshot`（2 对 heap root retain/release，`internal_methods.rs:367-386`）+ 对 `handler."get"` 发起**完整动态读**（`dispatch_read.rs:379-398` 的 `prepare_ordinary_read` + `ordinary_read_probe_atom` SipHash 探查，内部读无 IC 站点）+ 返回值每读重分类（`method.rs:172` `direct_call_target_from_value`）。handler shape 恒定时全部是重复劳动。perf：`internal_methods::get/method` 族 ≈9.8% + 探查族份额。
- **R-B invariant 检查每读走完整分派轮**：陷阱返回后 `request_descriptor`（`get.rs:187-198`）绕状态机一整圈再回 `descriptor()` 比对（`get.rs:205-245`），对恒空的 ordinary target 每次全量 `internal_get_own_property`。该函数对非 proxy target 本就是同步分支（`internal_methods.rs:452-464`），分派轮次是纯机器税。
- **R-C 状态机相位搬运税**：5 次 `dispatch_read::get` visit，每次对 168 B 的 `Step` 做 take/重写（`dispatch_read.rs:291-460`）；3 次 PooledBox 相位转换把 `ProxyGetResumeState`/`MethodResumeState`（含 8-Option `pending_effect`）memcpy 进出（`get.rs:116,129,146,190` 与 `get.rs:170` `into_inner`）。perf：`proxy_get_driver::*` ≈17.7% + memcpy ≈4%。
- **已排除项**：陷阱实参 Vec 已池化（`dispatch_read.rs:442`、`proxy_get_driver.rs:293,508` 的 `take_argument_buffer(3)`）；`property_key_value` 的 StringRc 拷贝为单次 Rc clone，量级不足立项；外层 megamorphic 站点 decline（`linked_read_declined` 1/迭代）为 proxy 接收者必要成本。

**深度不敏感性说明**：100k 循环在递归叶帧执行，上方休眠 JS 帧在惰性帧协议下零成本，耗时不随深度变化是预期行为。S0 在 512/2048 档因旧 VM 深递归 + proxy 方法栈上溢失败，维持无基线判定，以 128 档与内部一致性验收（§5）。

## 2. 修复设计

原则：不新增任何可观察语义；缓存零 owner（复用读 IC 的 Location 论证）；动态全量读永远保留为语义兜底路径。S21.1 为主项，S21.2 次之，S21.3/S21.4 以测量门控（YAGNI）。

### S21.1 — 陷阱选择缓存（主项，承接 R-A）

**机制**：复用 `PropertyReadCache`（`object/property_ic.rs:10-37`）——Location = `{domain, realm, shape, revision(layout), prototype_epoch, depth, slot}`，entry 零 owner，命中读**当日**槽值（值被覆写自动跟随，无需失效钩子）；2 路多态 + megamorphic 周期重试为既有行为。

**放置**：`RuntimeState` 新增 `proxy_trap_reads: [PropertyReadCache; 13]`，以 `PinnedAtom::proxy_method` 的 13 个陷阱名为索引（`atom/pinned.rs:179-196`）。不扩 `ProxyData`（保持 Copy 与 ArenaSlot 尺寸），不建 per-proxy 存储，无 GC 边。缓存键是 handler 的 shape 谱系，多个 proxy 共享同一 handler shape 时共享缓存。

**接线（`method.rs` Search::read，两处）**：
1. 保持现有次序：深度/limit 检查 → `proxy_snapshot_if_any` → `is_revoked` 检查（**revoked 判定必须先于缓存查询**）。
2. 借 state：`cache.read(&heap, domain_id, realm, data.handler)` 命中 → 在同一 borrow 内 copy+retain 槽值（协议照抄 `ordinary_storage/ic.rs:12-60` 的 hit-promotion；本路径不替换任何 slot、不触发释放，无需 S16 的 deferred/zero-cleanup 门控——需 ownership review 确认）→ drop borrow → `root_proxy_snapshot` → 构造 `MethodResume` 并**直接调用 `resume.resume(runtime, Completion::Return(value))`**。复用既有 resume 全部语义：Undefined/Null → 链式下探、`direct_call_target_from_value` 分类、非 callable TypeError、revoked 下探检查（`method.rs:131-185`），零逻辑复制。
3. 未命中 → `cache.miss(heap, atoms, domain, realm, Some(handler), atom)` 训练（miss 内部 locate 只认数据槽；accessor/字典化/proxy handler 自动 decline，`property_ic.rs:124-142`）→ 落入现有 `request_read` 全量路径，语义不变。
4. `MethodResume::resume` 的链式下探分支（`method.rs:160-168` 对 target-proxy 的 `request_read`）抽出与 Search::read 共用的 helper，同样先查缓存——多层 proxy 转发链逐层受益。

**失效论证（全部由既有 guard 承接，零新协议）**：handler 增删属性/defineProperty → layout revision 变 → miss 重训；`handler.get = 新函数`（同 shape 覆写）→ 缓存的是位置不是值，命中后读当日槽值自动取到新函数；原型链命中 → `prototype_epoch` guard；shape 被 GC 回收 → `heap.shape(id)` 查找失败 → miss（与读 IC 同一论证）；revoke → 次序在缓存之前；realm/domain 错配 → Location guard。

### S21.2 — ordinary-target 同步 invariant 检查（承接 R-B）

`get.rs` `ProxyGetResume::resume` 的 `Phase::Trap` 分支（`get.rs:187-198`）：陷阱返回后，若 `proxy_snapshot_if_any(target)?` 为 None（非 proxy target；`internal_get_own_property` 对该分支本就同步，`internal_methods.rs:452-464`），直接同步取 target 描述符并就地做 invariant 比对返回 `Complete`，不再 `request_descriptor` 绕分派轮。target 为 proxy 时保留现有 Descriptor 轮（其 [[GetOwnProperty]] 可重入 JS）。

**工序**：把 `descriptor()`（`get.rs:205-245`）中的比对逻辑（non-configurable non-writable data 的 same_value、no-getter accessor 的 Undefined 检查、TypeError 文案 "proxy: inconsistent get"）抽为自由函数，`descriptor()` 与新同步分支共用（DRY）。可观察行为逐位不变：检查时机仍在陷阱返回之后、检查结果与错误一致。

### S21.3 — 命中路径相位瘦身（门控项，承接 R-C 残留）

S21.1 的命中路径仍付一次 `MethodResumeState` 的 PooledBox 进出（构造后立即 resume）。若最终测量 depth-proxy 仍 >S0+2%：把 `resume()` 主体抽为接受裸字段的自由函数，命中路径不经 PooledBox 直接调用；`request_read` 路径包装不变。**先测后做，不预先实现。**

### S21.4 — 显式缓行清单（本阶段不做）

- trapless-forward 负缓存（无陷阱 proxy 的转发链）：`PropertyReadCache` 无缺失缓存，membrane 场景暂无基准诉求。
- `RootedProxy` 借用化（消 2 对 root retain/release）：陷阱可 revoke/递归改写两边对象，借用证明需要独立的边界分析，收益（~数十 ns）不抵风险。
- 外层 megamorphic decline 入口收敛、`property_key_value` 物化缓存：量级不足。

达标后以上一律不做（YAGNI）；未达标时按此顺序重开。

## 3. 语义红线（等价矩阵新增反例，全部先写测试）

1. `handler` 的 `get` 为 **accessor**（getter 每读可观察副作用）→ locate decline，每读必须触发 getter。计数锚点：该形态下 `query_dispatch.step.read` 恢复 1/读。
2. `handler` 本身为 **proxy**（陷阱查找本身触发嵌套 get 陷阱）→ decline，嵌套陷阱调用序逐位保持。
3. `handler.get` **同 shape 覆写**为另一函数 → 下一读立即调用新函数（缓存位置非值，测试锚定）。
4. `delete handler.get` / defineProperty 改布局 → revision 失效 → 回退 forward/新语义。
5. **revoke**：查缓存前判 revoked；陷阱内 revoke 当前 proxy 后，本次操作按既有语义完成（`RootedProxy` 注释契约，`internal_methods.rs:110-122`）。
6. 陷阱 Undefined/Null 的**链式下探**（proxy-of-proxy）：深度计数、`proxy_method_chain_limit` 上溢、逐层 revoked 检查不变（`method.rs:141-170`）。
7. invariant 结果与调用序：trap 后查 target 描述符的时机、TypeError 条件与文案不变（S21.2）。
8. GC：缓存不得延长任何对象生命周期；`run_gc` 后无 stale 命中（复用读 IC 论证 + 新增跨 GC 读测试）。
9. backtrace 陷阱帧序、`Reflect` 不变量阶段逐项保持；错误 PC/栈锚点断言照旧。

设计文档先行：动工前在 `docs/architecture/` 新增陷阱选择缓存条目（或扩展 `lazy-callback-frames.md`），写明"内部陷阱读的可缓存判据 = 数据槽 + 谱系 revision guard；accessor/proxy handler 永久走动态读"。

## 4. 逐工序步骤

| # | 工序 | 文件 | 验证 |
|---|---|---|---|
| 1 | 架构文档条目（§3 判据 + 失效论证） | `docs/architecture/` | review |
| 2 | 红线测试先行：§3 全部反例（accessor 陷阱、proxy handler、同 shape 覆写、delete、revoke、链式下探、跨 GC） | `method.rs`/`get.rs` 测试模块 + `call/protocol/callback_tests.rs` 先例 | 新测试对现行代码全绿（行为基线） |
| 3 | `PinnedAtom` 陷阱索引映射（13 项） | `atom/pinned.rs` | 单测 |
| 4 | `RuntimeState.proxy_trap_reads` + 查/训入口（含 ic.rs 式 copy+retain 协议） | `heap/runtime/mod.rs`、`object/property_ic.rs`（如需放宽可见性） | 单测 + ownership review |
| 5 | `Search::read` 接线 + `resume` 链式分支共用 helper | `object/internal_methods/method.rs:92-121,160-168` | 步骤 2 测试保持全绿；`proxy_method_completion_reuses_the_pending_owner` 保持 |
| 6 | 提交一：S21.1 | — | 全量工作区测试 + Test262 proxy 章节 |
| 7 | invariant 比对逻辑抽自由函数 + Phase::Trap 同步分支 | `object/internal_methods/get.rs:187-245` | 既有 get.rs 测试 + 红线 7 |
| 8 | 提交二：S21.2 | — | 同上 |
| 9 | 全量语义门禁：工作区测试、Test262 结果向量逐位对齐、边界矩阵、oracle 压力（陷阱缓存触及全部 proxy 操作，proxy 各 trap 章节必须全量） | `final-gates.py` 既有管线 | 向量相等 |
| 10 | 最终一轮测量（§5/§6） | `target/latest-round/measure.py` 模式 | 验收判定 |
| 11 | 若未达标 → S21.3（提交三），复测；达标 → 关闭 | — | — |

## 5. 验收标准（机械计数先行，不达标不进入耗时结论）

**机械计数（depth-proxy-0，每迭代，profiling 二进制）**：

| 计数 | 现值 | 目标 |
|---|---|---|
| `query_dispatch`（read+descriptor+complete） | 3 | ≤1（仅 complete） |
| `dispatch_read.get.visit` | 5 | ≤2 |
| `property_storage_read_probe` | 3 | ≤2（invariant 同步探查 1 + 裕量） |
| `slots_initialized` / `slot_clears` / `slot_moves` | 9 / 8 / 27 | ≤5 / ≤4 / ≤23 |
| `value_copies` | 6 | ≤4 |
| 稳态池分配（get/method_resume_allocation） | 各 1 | 各 1（不回归） |
| `query_bytecode_callback`、`property_return_direct`、`property_callback_lazy_install` | 各 1 | 各 1（S18 成果不回归） |
| 新增 `proxy_trap_read.hit` 事件 | — | ≥0.99 |

`copied_heap_roots` 2/迭代为 S21.4 缓行项，不设目标。红线形态反向锚点：accessor-陷阱 handler 下 `proxy_trap_read.hit` = 0 且 `query_dispatch.step.read` = 1/读。

**耗时（三轮中位对 S0 三轮中位，锁频/loadavg 约束照 §6）**：depth-proxy-0/32/128 相对 S0 ≤ +2% 或转负；512/2048 无 S0 基线，判定改为与 128 档差值 ≤ ±3%（固定成本一致性）且相对本轮 128 档绝对值不高于现值；探针族其余 9 项、fixed 58 项、original Score 3 项相对 latest-round 无 >2% 连带回退。

**语义门禁**：工作区测试全绿、Test262 结果向量逐位对齐（proxy 全章）、边界矩阵、oracle 压力、§3 红线测试逐条通过。

## 6. 测量与纪律

- 单变量归因：S21.1 与 S21.2 独立提交；实现期间零测量；最终仅一轮（探针族 33 项 ×3 + fixed 58 ×3 + original ×3 + depth-proxy/getter cost profile 各 1）。
- 环境约束沿用 §S14-S20/4：governor=performance 或记录锁定频率，采样前 loadavg < 核数 1/2，raw_samples 记录环境，超限作废重测。
- 对比口径：三轮中位对三轮中位；单轮不得作为立项或关闭依据。
- 结果留存 `target/performance-retained/`，不提交 Git。

## 7. 风险与回退

- **缓存误命中（soundness）**：全部 guard 复用读 IC 既有实现，无新失效协议；红线 3/4/8 测试直接锚定。回退开关：miss 训练与命中查询集中在单一入口，出现语义疑点可一行禁用回到全量动态读。
- **多 handler shape 抖动**：2 路多态 + megamorphic 周期重试为既有行为，最坏退化为现状（每读全量），无负优化风险。
- **S21.2 的 exotic target**：同步分支判据为"非 proxy"，`get_own_property` 对 exotic（TypedArray 等）已是同步既有实现；红线 7 覆盖。
- **提交粒度**：两个独立提交各自可单独 revert。

## 8. 状态

- [ ] 架构文档条目（工序 1）
- [ ] 红线测试先行（工序 2）
- [ ] S21.1 陷阱选择缓存（工序 3–6）
- [ ] S21.2 同步 invariant（工序 7–8）
- [ ] 语义门禁全量（工序 9）
- [ ] 最终测量与验收（工序 10）
- [ ] S21.3 门控判定（工序 11）

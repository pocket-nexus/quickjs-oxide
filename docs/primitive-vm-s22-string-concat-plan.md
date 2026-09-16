# S22 — 前插拼接融合随迁与驻留数值发布豁免

**执行指令（沿用 S14–S21 纪律）：** 分阶段提交全部实现；实现期间不运行 benchmark/cost profile/CPU profile；全部完成并逐项 review 后，仅对最终核心执行一轮测量（S0 与 QuickJS 复用 `target/performance-retained/` 既有三轮）。测量结果保留在本地，不提交 Git。与 [S21](primitive-vm-s21-proxy-trap-plan.md) 相互独立（S21 动 proxy 驱动，S22 动 run/fusion/转换驱动），可并行开发，最终合并为同一轮测量。

2026-09-16 立项。目标：消除 string_build3 +22.0%、string_build_large2 +28.5%（相对 S0，latest-round 三轮中位）——这是 S14–S20 期间**真实引入的恶化**：两用例的 S10–S12 三轮中位分别为 +3.8%、+10.1%（注意：立项总表引用的 +11.5%/+14.2% 是 R10 判定含噪声的单轮值，本文一律以三轮中位为准）。同根因家族的 int_to_string +11.7%、bigint64_arith +11.0% 随 S22.2 顺带受益。

## 1. 根因回执（2026-09-16 补测证据）

证据来源：`target/latest-round/` 的 cost profile（当前）对 `target/performance-retained/reports/latest/results.json`（S10–S12 终测）同用例 `owned_execution_events` 逐项 diff；代码锚点已逐条核对当前工作树。

**定量框架**：string_build3 305.7ms → 359.1ms（+53ms）、string_build_large2 340.9ms → 397.8ms（+57ms），两用例内层各 1.6M 次拼接 → 恶化量 ≈ **+33~35ns/次拼接**，与下表簿记增量精确吻合；字符串字节拷贝量两个时代相同（rope/flat 算法未变，slot_clears 逐字节相等）。

**string_build3 每次拼接机械计数（S10–S12 → 现在）**：

| 计数 | S10–S12（ConvertAdd 退出路径） | 现在（驻留路径） |
|---|---|---|
| `run_exit.ConvertAdd` | 1 | 0 |
| `numeric_completed_in_run` | 0 | 1 |
| `primitive_add_store_fused` | 1 | **0（融合丢失）** |
| `runtime_pc_publication` | 1 | **2** |
| `run_frame_fault_pc_write` | 1 | 2（本地写，廉价） |
| `slot_copy.StringRc` | 1 | **2** |
| `hot_value_releases` | ~0 | **1** |
| `slot_moves` / `value_copies` | 9 / 3 | 11 / 4 |
| `slot_authentication` | 2 | ~0 |

**机制**：`r = "x" + r`（前插）编译为 `PushConst; GetLocal; Add; PutLocal`。S15 把 String 形态 Add 从"ConvertAdd 退出 → conversion driver 融合直存"改为 run 驻留，但 S15.1 计划的"add_store 融合与 observable_release 语义随迁"**没有覆盖驻留路径**：

- **R-A 融合丢失**：`FusionPlan` 的 local_add 模式只识别以 `GetLocal(left)` 开头、存回同一 `left` 的形态（`code/fusion.rs:99-132`，flag 128/129），即追加形 `r = r + X`/`r += X`；前插以 `PushConst` 开头不匹配。追加形（string_build1/large1）走 `RunExit::AddLocal` → `complete_local_add` 借用读 + 直存 + `concat_owned` 原地追加，已达标；前插落入通用驻留分支（`run.rs:1771-1805`）：GetLocal 把 r 以 Rc clone 入栈（+1 `slot_copy.StringRc`）、结果 `push_pending` 入栈再由 SetLocal 单独存储（+2 `slot_moves`、+1 `value_copies`）、旧值覆写走热释放（+1 `hot_value_releases`）。
- **R-B 发布 #1**：驻留数值分支入口对已物化帧无条件 `update_active_bytecode_pc`（`run.rs:1787-1795`，RefCell `borrow_mut` + 注册表写）；脚本帧早期物化后每次拼接都付。而 String/Number/bool 形态的 `primitive_output` 不构造 JS 错误（`JsStringError::TooLong/OutOfMemory` 映射为引擎错误，不观察栈，`value/primitive.rs:71`）——该发布对这些形态是纯浪费。
- **R-C 发布 #2**：SetLocal 覆写引用值旧 r 走 `release_outside_slots!`（`run.rs:264-280`），无条件 `publish_fault` + `update_active_bytecode_pc`。该发布为"释放 Object 可能触发 zero-queue/GC 观察"服务；String/BigInt 是 Rc 值而非 arena 对象，释放不可重入 JS，发布同样是纯浪费。
- **同族证据**：int_to_string（`n + ""`）与 bigint64_arith 同签名——每操作 2 次/2.24 次发布（3.0M/7.2M）；string_build1 的 AddLocal 退出路径也付 2 次/op 发布 + 2 次槽认证，但被原地追加对 S0 的字符串收益掩盖。前插的字符串工作量与 S0 相同，簿记增量全额显形。

**已排除项**：rope/flat 拼接算法（`try_concat` 的 512/8192 阈值、分段合并、rebalance）两个时代一致，QuickJS 同款，不是恶化来源；string_build_large2 相对 S0 的既有 +10.1% 残差（rope 节点分配、段合并拷贝的常数项）为独立事项，见 S22.4 门控。

## 2. 修复设计

原则：不改变任何可观察语义（Add 求值序、错误 PC/栈、rope/flat 宿主可见区分）；每项独立提交、计数可独立验证。S22.1/S22.2 为主项，S22.3/S22.4 以测量门控（YAGNI）。

### S22.1 — 前插形态纳入 local_add 融合（承接 R-A）

- **FusionPlan 识别**（`code/fusion.rs:99-132` 旁增）：`[PushConst(c), GetLocal(right)|GetLocalCheck(right), Add, store]`，store 为 `PutLocal/PutLocalCheck(index==right)`（新 flag，span 4）或 `SetLocal/SetLocalCheck(index==right) + Drop`（新 flag，span 5）；局部准入条件照抄现模式（Normal kind、非 const）。常量的 String 判定留给运行期（构建期无常量表）。
- **运行期准入**（`run.rs` 新增 `Instruction::PushConst` 臂分支，对照 `run.rs:1201-1218`）：`fusion.const_add_span(pc)` 命中且 `executable.constant` 为 String 时，走 `local_add_constant_supported` 的镜像判定（`stack/window.rs:289` 旁；校验局部可读、非 Object、非 TDZ）→ `return Ok(RunExit::AddLocal)`，复用既有退出。
- **`complete_local_add` 支持常量左操作数**（`conversion_driver/local_add.rs`）：拼接序不可交换——`try_concat(常量, 局部)`；无原地追加准入（前插无法就地扩展，走既有 rope/flat 规则）；直存与旧值 drop 语义照抄现实现（借用读、`replace_local_pending`、融合 drop）。事件沿用 `primitive_add_store_fused`。
- 预期：build3/large2 回到 S10–S12 水平（+3.8%/+10.1%）——消 +1 StringRc copy、+2 slot_moves、+1 value_copy、+1 hot_value_release 与发布 #2（融合 span 内单点发布）。

### S22.2 — 发布豁免分级（承接 R-B/R-C，全数值族受益）

1. **驻留数值入口惰性发布**（`run.rs:1787-1795`）：删除入口 eager `update_active_bytecode_pc`，改在 `numeric::complete` 的错误物化分支（`run/numeric.rs:45-57`，构造 JS 错误前）发布——签名扩一个 `(active_frame, fault_pc)`。前置审计工序：枚举 `primitive_output` 全部错误面，确认"构造 JS 错误"是栈观察的唯一入口（BigInt RangeError、Symbol TypeError 均落在 thrown 分支；Symbol/BigInt 的 pre-materialize 门 `run.rs:1777-1783` 保持，保证发布时帧已物化）。
2. **释放边界按值形态豁免**（`run.rs:264-280`）：为被释放值确定为非 heap-object（String/BigInt/标量；Symbol 保守走原路径）的调用点提供豁免变体——跳过 materialize 门与 `update_active_bytecode_pc`（保留 `publish_fault` 本地写）。SetLocal/PutLocal 覆写路径先窥旧值形态再选宏。rope 递归 drop 深度有 `ROPE_MAX_DEPTH` 上界，无 JS 重入。
3. **`complete_local_add` 的发布同规则降级**（`conversion_driver/local_add.rs:58,153`）：String 常量形态错误面同 1，发布移入错误路径——AddLocal 退出族（build1/large1 与 S22.1 后的 build3/large2）每操作再省 1~2 次注册表写。
- 预期：build3/large2 降到 S0±2% 带；int_to_string 3.0M、bigint64_arith 7.2M 发布大幅收敛，两用例耗时可测改善（不设强目标，顺带项）。

### S22.3 — 驻留融合直存（门控项）

若 S22.1+S22.2 后 large2 仍 >S0+2%：把 const-add 融合形态从 AddLocal 退出改为 run 内驻留完成（消每操作 2 次槽认证与退出/重入），机制复用 `fusion::update_local` 的驻留局部直写先例（`run.rs:1220-1229`），旧值 drop 用 S22.2.2 的豁免释放。**先测后做。**

### S22.4 — 显式缓行清单（本阶段不做）

- large2 相对 S0 的既有 +10.1% 残差深挖（rope 节点 Rc 分配池化、段大小调优）：`ROPE_SHORT_LEN/ROPE_SHORT2_LEN` 参与 QuickJS 宿主可见的 rope/flat 区分（`try_concat` 注释契约），改阈值属语义敏感项；S22.1-3 落地后按新 profile 重新归因再议。
- 前插专用"反向缓冲"表示：复杂度不抵收益，QuickJS 同样无此优化。
- v8-richards/richards Score、v8-earley-boyer 残留：非字符串族，另行归因，不挂本阶段。

## 3. 语义红线（等价矩阵新增反例，全部先写测试）

1. **求值序**：前插融合只准入两操作数均为原语的形态；`({valueOf(){…}}) + r`、`"x" + {toString(){…}}` 等对象操作数保持通用路径，ToPrimitive 重入序逐位不变。
2. **拼接序不可交换**：`"x" + r` 与 `r + "x"` 结果区分（融合后 left/right 角色测试锚定）。
3. **TDZ/捕获/const 局部**：`GetLocalCheck` 的 TDZ 抛错点、captured 局部不融合、const 目标不融合——照抄 `local_add_span_rejects_intermediate_entries_and_other_targets`（`fusion.rs:264`）补前插形态。
4. **错误 PC/栈锚点**：字符串超长（构造 >2^30 单元）与 BigInt `1n/0n`、`Symbol()-1` 的 `e.stack` 行列锚点在惰性发布下逐位不变（照抄 `run/numeric.rs:157-163` 断言形式）；OOM 注入（`FAIL_NEXT_CONCAT_RESERVATION`）路径局部绑定不变。
5. **释放豁免安全性**：豁免仅限非 heap-object 值；Object 覆写（含带终结语义的值）保持 materialize+publish；跨 GC 读写测试确认注册表 PC 在下一个观察点前一致。
6. **rope/flat 宿主可见区分**：`try_concat`/`concat_owned` 的阈值行为与 host printing 兼容测试不变；`linearize` 状态迁移不变。
7. **backtrace**：融合 span 内抛错的帧序与 fault PC 对齐既有 `conversion_driver` 断言；`fusion.CompareBranch`/`UpdateLocalDiscard` 既有语义测试全绿。

设计文档先行：动工前修订 `docs/architecture/owned-fusion.md`——新增前插 const-add 形态条款与"发布豁免判据 = 释放值非 heap-object 且操作不构造 JS 错误"章节。

## 4. 逐工序步骤

| # | 工序 | 文件 | 验证 |
|---|---|---|---|
| 1 | 架构文档条款（§3 判据） | `docs/architecture/owned-fusion.md` | review |
| 2 | 红线测试先行：§3 全部反例对现行代码建立行为基线 | `fusion.rs`/`run.rs`/`local_add.rs` 测试模块 | 全绿 |
| 3 | FusionPlan 前插模式 + `const_add_span` | `code/fusion.rs` | 单测（模式/拒绝矩阵） |
| 4 | run 准入臂 + `local_add_constant_supported` 镜像 | `vm/run.rs`、`vm/stack/window.rs` | 步骤 2 基线保持 |
| 5 | `complete_local_add` 常量左操作数 | `vm/conversion_driver/local_add.rs` | 既有 local_add 测试 + 红线 1/2/4 |
| 6 | **提交一：S22.1** `perf(vm): fuse constant-left string concatenation into local add` | — | 全量工作区测试 + Test262 |
| 7 | `primitive_output` 错误面审计记录 + 驻留入口惰性发布 | `vm/run.rs:1787-1795`、`vm/run/numeric.rs` | 红线 4；`numeric_non_number_loops` 保持 |
| 8 | 释放边界豁免变体 + SetLocal 选路 | `vm/run.rs:264-280` 及调用点 | 红线 5；GC oracle |
| 9 | `complete_local_add` 发布降级 | `vm/conversion_driver/local_add.rs:58,153` | 红线 4 |
| 10 | **提交二：S22.2** `perf(vm): publish frame pc lazily for primitive completion and scalar release` | — | 同上 |
| 11 | 全量语义门禁（工作区、Test262 向量逐位、边界矩阵、oracle 压力） | 既有管线 | 向量相等 |
| 12 | 最终一轮测量（与 S21 合测；§5/§6） | `target/latest-round/measure.py` 模式 | 验收判定 |
| 13 | 未达标 → S22.3（提交三）复测；达标 → 关闭 | — | — |

## 5. 验收标准（机械计数先行，不达标不进入耗时结论）

**机械计数（string_build3，1.6M 次拼接，profiling 二进制）**：

| 计数 | 现值 | 目标 |
|---|---|---|
| `primitive_add_store_fused`（或驻留等价事件） | 0 | 1.6M（每拼接 1） |
| `runtime_pc_publication` | 3.19M | ≤ 启动量级 |
| `slot_copy.StringRc` | 3.2M | ≤ 1.6M |
| `hot_value_releases` | 1.6M | ≤ 启动量级（融合 drop） |
| `slot_moves` / `value_copies` | 17.6M / 6.4M | ≤ 14.5M / ≤ 4.9M（S10–S12 水平） |
| int_to_string `runtime_pc_publication` | 3.0M | ≤ 启动量级 |
| bigint64_arith `runtime_pc_publication` | 7.2M | 大幅收敛（保留 pre-materialize 门必要发布） |

**耗时（三轮中位对 S0 三轮中位，环境约束照 S14–S20 §4）**：string_build3、string_build_large2 相对 S0 ≤+2% 或转负；**底线**：不劣于 S10–S12 中位（+3.8%/+10.1%），劣于底线视为失败回退提交。string_build1/large1 及 fixed 全族无 >2% 连带回退；int_to_string/bigint64_arith 记录改善幅度（顺带项，不设关闭门槛）。

**语义门禁**：工作区测试全绿、Test262 结果向量逐位对齐、边界矩阵、oracle 压力、§3 红线逐条通过。

## 6. 测量与纪律

- 单变量归因：S22.1 与 S22.2 独立提交；实现期间零测量；最终与 S21 合并为唯一一轮（fixed 58×3 + 探针 33×3 + original 9×3 + 相关 cost profile）。
- 对比口径：三轮中位对三轮中位；本文一切"恶化/达标"判断均以三轮中位为准（立项总表的单轮值仅作历史记录）。
- 结果留存本地，不提交 Git。

## 7. 风险与回退

- **融合准入误报**：新模式的拒绝矩阵（对象操作数、TDZ、captured、const、跨局部 store）先行测试；运行期 String 判定失败自然落回通用路径，无负优化。
- **惰性发布漏观察点**：以工序 7 的错误面审计为准入证据；任何"构造 JS 错误/重入 JS"的新路径必须先发布——出现语义疑点可单点还原 eager 发布（一行）。
- **释放豁免误分类**：判据是值形态白名单（String/BigInt/标量），Symbol/Object 保守不豁免；GC oracle 与跨 GC 红线兜底。
- **提交粒度**：两个提交各自可单独 revert；S22.3 若实施为第三个独立提交。

## 8. 状态

- [x] 架构文档条款（工序 1）：`docs/architecture/owned-fusion.md` 新增常量左 LocalAdd 行与 "S22 constant-left LocalAdd and primitive publication exemption"。
- [x] 红线测试先行（工序 2）：`fusion.rs` 前插模式/拒绝矩阵；`local_add.rs` 拼接序、求值序/捕获/TDZ/BigInt/Symbol 回退、失败绑定与 PC、profiling `local_add_borrowed_span` 计数。
- [x] S22.1 前插融合（工序 3–5）：`FusionPlan` flag 130/131 + `const_add_span`；`run.rs` PushConst 准入臂（String 常量 + `local_add_constant_supported`）；`complete_local_add` 三形态 operand 分派 + `with_local_add_constant_left`，前插不原地追加。
- [x] S22.2 发布豁免（工序 7–9）：`numeric::complete` 惰性发布（错误物化前）；`primitive_release_owner` 豁免 SetLocal/PutLocal、SetArg/PutArg、Drop、Nip；`complete_local_add` 移除每操作发布、错误分支补发。
- [x] 语义门禁全量（工序 11）：工作区 `--all-targets`/`--doc`/`--features test262-host --lib --bins` 全绿；`--features profiling --lib` 2495/2495；Test262 full vector 逐位对齐 `79982 pass of 80032 eligible (102037 total)`。
- [x] 最终测量与验收（工序 12）：见 §9。
- [x] S22.3 门控判定（工序 13）：S22.1+S22.2 已使 large2 转负（−15.45%），**不触发 S22.3**，关闭。

## 9. 实测结论（2026-09-16，与 S21 合测单轮 ×3，core 6，governor=powersave，loadavg 0.25）

机械计数（`string_build3`，1.6M 次前插拼接，profiling 二进制）：

| 计数 | 现值 | 目标 | 判定 |
|---|---|---|---|
| `local_add_borrowed_span`（驻留融合等价事件） | 1,600,000 | 1.6M | 达标 |
| `runtime_pc_publication` | 3,394 | ≤ 启动量级 | 达标（原 3.19M） |
| `slot_copy.StringRc` | 4,800 | ≤ 1.6M | 达标（原 3.2M） |
| `hot_value_releases` | 0 | ≤ 启动量级 | 达标（原 1.6M） |
| `run_exit.ConvertAdd` / `numeric_completed_in_run` | 0 / 3 | — | 达标 |
| `slot_authentication` | 3.2M（2/拼接） | S10–S12 水平 | 达标 |

耗时（三轮中位对 S0 三轮中位）：string_build3 **+22.0% → −21.75%**、string_build_large2 **+28.5% → −15.45%**、string_build1 −49.1%、string_build_large1 −24.0%；顺带项 int_to_string +11.7% → +3.44%、bigint64_arith +11.0% → +1.93%、float_arith +10.8% → +1.25%。**主目标（build3/large2 ≤ S0+2% 或转负，且不劣于 S10–S12 的 +3.8%/+10.1%）全部满足并反超**；fixed 全族无 >2% 连带回退。S22.3「驻留融合直存」不再需要。

（同轮 S21 残余 depth-proxy 仍 ~+11%，属 S21 计划的未达标项，与 S22 无关。）


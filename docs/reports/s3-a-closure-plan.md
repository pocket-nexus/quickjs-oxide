# 阶段 A 收口计划：所有权事务窄修与帧槽瘦身（T1/T2）

> 状态：计划中（2026-09-23），尚未实施。对应路线：
> [架构 §11](performance-architecture.md) 修订后的
> **E（已完成）→ 残余批（T1 + T2 + 字符串簇归因）→ A4 决策点 → D**；
> B 重新立项、C 已关闭，均不在本计划内。

## 1. 背景与证据

### 1.1 E 基线与代码状态

- E 已完成并冻结同协议基线：[全量重测结果](s3-full-rerun-results.md)
  （fat LTO + CGU=1、无 PGO、无 profiling、`taskset -c 2`）。
- 阶段 C 按负结果关闭、B/C 实现整体回退：[阶段 C 负结果与撤回落](s3-c-negative-result.md)。
  当前代码树 = `bcfb4fe5`，代码内容与 A 终版 `d4f78697` 逐字节一致
  （`git diff d4f78697 38fbb5fc` 仅 docs 与 benchmark README）。
- E 的固定行（m0/pre-A）：bigint32 墙钟 0.79–0.84 / 指令 0.98（已快于
  pre-A）；bigint64 ~0.90 / 1.12；bigint256 **1.21 / 1.43**。
  fixed 字符串簇回退：`string_build1` 1.170、`string_build3` 1.178、
  `string_build_large1` 1.170、`int_to_string` 1.204、`map_delete` 1.128
  （尚未归因，见 T1.5）。

### 1.2 归因修正（已提交 `c1805e9d`）

bigint256 回退的主因是**句柄化后每次拷贝/释放的 ownership 事务**，不是
store-forward / 16B-32B 搬运：

- 同工具链同 flags（LTO off、CGU16）下 pre-A `85afd564` → A 终版
  `d4f78697`：bigint256 instructions +31.9%、cycles +16.8%，与
  `s3-a-plan.md` §8.13 nolto 列逐位吻合；
- IPC 反升（2.75→3.22 等）说明指令数驱动而非 stall；
- pre-A `push_current` 同样是 32B `movups` 对（`movups 0x10(%rbx)` 41%），
  宽度搬运不是 delta；`replace_local_current` 不在热路径；
  `numeric::complete` 反而变便宜（final 1.32% vs pre-A 9.11%）；
- `a=a` 微负载 final 约 **15% 指令**是 pre-A 不存在的所有权事务：
  `slot_leaf_release_readiness` 2.09%、`slot_value_release_readiness_jsvalue`
  1.27%、`try_release_slot_value_jsvalue` 1.88%、`release_displaced` 1.13%、
  `try_release_leaf_reference` 1.20%、`release_bigint_handle` 1.01%、
  `finish_reference_release` 1.13%、`release_jsvalue` 1.06%、
  `retain_bigint_shared` 0.83%、`copy_reference` 0.74%；
- 微负载每操作指令差（pre-A→final）：`a=a` +477、`a=a+1n` +886、
  `s=s+a` +271、`b=a*a` +1084；官方 bigint256 每内层迭代 +2310
  ≈ mul + add(reuse) + add 三项之和（≈2241）。

### 1.3 FrameBinding 32B 构成（已核实）

| 变体 | 载荷 | 尺寸 |
| --- | --- | --- |
| `Direct(JsValue)` | 16B 句柄 enum | 16B |
| `Private(PrivateNameRef)` | `AtomOwner{Option<Runtime> 8B + Atom 16B}` | **24B** |
| `PrivateCallable(CallableRef)` | `CallableRef(ObjectRef)` | 16B |
| `Captured(VarRefRoot)` | `VarRefRoot{Runtime, VarRefId}` | 16B |
| `Uninitialized` | — | 0 |

24B 载荷 + 8B tag = 32B，`Option<FrameBinding>` 同为 32B。**步长决定者是
`PrivateNameRef`，不是 `Direct`**——A 落地 16B 句柄后仍停在 32B 的原因在
这里。承载面：

- `SlotStore.slots: Vec<Option<FrameBinding>>`（`vm/stack.rs:22`）同时承载
  active frame 的 args/locals/**operands**（`install_operand`
  `vm/stack.rs:1203-1205`）；
- `FrameStorage.parameters/locals: Vec<FrameBinding>`（`vm/stack.rs:83-88`）；
- owned-storage 计数在 `vm/call/prepare.rs:88-90` 用
  `size_of::<FrameBinding>()` 乘容量，瘦身后自动反映。

### 1.4 共同根因

两件事是同一个根因的两面：**A 迁移后，内部存储与热路径仍残留
`Rc<Runtime>` 与可失败校验事务**。T1 修值拷贝/释放侧的事务，T2 修帧槽
存储侧的 root 变体。两者都必须在 A4 决策矩阵之前落地，否则 A4 会把
ownership 成本与 32B 帧槽算进宽度账。

## 2. 共同原则与不变量

- **内部存储与 trusted 热路径**：不携带 `Rc<Runtime>`、不跑可失败校验
  事务；不变量破坏走 panic（沿用 S1 政策）。
- **边界与挂起解码**：保留全量校验与可失败路径（返回 `Err`），跨 runtime
  防护只允许在边界/解码层做。
- **计数溢出**：fast 路径饱和为 immortal（T1.0 先钉死释放语义）；通用
  可失败路径保持 `checked_add`/`checked_sub`。
- **所有权恰好一次**：每次 retain 有且仅有一次配对 release；由 teardown
  断言、debug 边账本（`QJS_TRACE_ROOTS`）与挂起 round-trip 测试兜底。

## 3. T1：所有权事务窄修

### T1.0（前置）饱和计数的释放语义

- 现状：`retain_raw_fast`（`heap/gc.rs:1168-1176`）`saturating_add` 到
  `u32::MAX`，注释声明 "matching QuickJS's immortal value"；但
  `try_release_leaf_reference`（`heap/gc.rs:1247-1312`）在 `strong > 1`
  时无条件 `strong - 1`，`release_raw_no_drain`（`heap/gc.rs:1416+`）用
  `checked_sub(1)`，都不识别 `u32::MAX`。
- 决策：**释放入口把 `u32::MAX` 视为 immortal**——不减、不退休、直接
  成功。理由：饱和只可能由 fast retain 产生（checked 路径溢出即 `Err`），
  视 immortal 保持"不破坏所有权"且无需额外状态位。
- 实现点：`try_release_leaf_reference`（`strong == u32::MAX` →
  `Ok(Some(false))` 不递减）、`release_raw_no_drain` Live 分支
  （MAX → 跳过递减与 zero-queue 迁移），并覆盖经 `release_or_defer` 的
  对象路径；`retain_object_fast` 既有饱和同样受益。
- 验收：单元测试——构造 MAX 计数节点，release 不递减/不退休，再次
  retain 保持 MAX；普通计数路径行为不变。

### T1.1 叶 trusted retain（`copy_reference`）

- 现状：`copy_reference`（`vm/stack.rs:1752-1775`）叶分支调
  `retain_string_handle`/`retain_bigint_handle`（`heap/ownership.rs:393/434`）
  → `try_borrow` + `retain_raw_shared`（`heap/gc.rs:1202`）→
  `live_node(id)?` 全量校验 + `checked_add` + `Result`。
- 方案：新增 `Runtime::retain_live_string_handle`/`retain_live_bigint_handle`，
  镜像既有 `retain_live_object_handle`（`heap/ownership.rs:255-266`）：
  trace 环境回退全量；`try_borrow` 成功走
  `Heap::retain_string_fast`/`retain_bigint_fast`（或直接
  `retain_raw_fast(RawId::String/BigInt)`）；借用不可得回退 checked。
- 不变量：调用方持有另一个 owned 边（slot/常量池 owner 在调用期间存活）；
  `live_node_fast` 在 debug 构建断言身份；fast 计数饱和由 T1.0 闭合。
- 范围：只改 `copy_reference` 叶分支；Object/Symbol 与公共边界不动；
  短 BigInt 走 `copy_value` 内联标量臂，不受影响（bigint32 当前 0.98
  insn，不得回退）。
- 验收：bigint64/256 指令下降；`a=a`/`s=s+a` 微负载指令下降；bigint32
  不回归；全量测试绿。

### T1.2 `release_displaced` trusted commit

- 现状：PutLocal/SetLocal 守卫先证明
  `slot_value_release_readiness_jsvalue == Ready`（`vm/run.rs:1643/1696`），
  随后 `release_displaced`（`vm/run.rs:284-304`）再调
  `try_release_slot_value_jsvalue`（`heap/slot_ownership.rs:227`）重跑同一
  readiness（pending 检查 + `try_borrow_mut` + 按 kind 校验）。
- 方案：新增 trusted commit（如 `release_displaced_ready`）——在已证明
  Ready 后直接 `std::mem::replace(Undefined)` + `release_jsvalue`；注释
  保留"证明与提交之间只有 move/retain，不会 drain"的论证。
- `stack.rs:1392` 的同名调用点若没有同函数内前置证明，保持 checked。
- 不变量：trusted commit 只在"同一函数内已证明 Ready 且之间无可回调
  操作"处使用；未证明路径不变。
- 验收：微负载指令下降；无 double-release/leak；teardown 断言绿。

### T1.3 唯一性预过滤（bigint/string）

- 现状：`unique_bigint_mut`（`heap/value_storage.rs:59`）/
  `unique_string_mut`（`heap/value_storage.rs:22`）先 `strong_count()?`
  （全量校验 + `Result`）再 `live_node_mut()?`（全量校验 + `Result`）。
- 方案：先 `live_node_fast(id).strong.get() != 1 → Ok(None)` 快拒绝；
  唯一时再 `live_node_mut`。调用方持有该边，属 trusted 语义。
- 调用点：`vm/numeric.rs:128`（bigint 结果复用）、
  `vm/numeric.rs:418`（string concat 就地追加）。
- 验收：bigint256/64、`string_build*` 指令下降；短 BigInt 不回归。

### T1.4（可选）自赋值消解

- 触发条件：T1.1–T1.3 实测后 `a=a` 仍有显著残余。
- 方案：PutLocal/SetLocal 快路径比较 old/next 句柄相同 → 跳过
  replace + release（SetLocal 同时丢弃 TOS 副本，净效果为零）；不做
  opcode 层 fusion（C 机制已回退，不重建）。
- 前置：确认无可观察副作用；独立 A/B。

### T1.5 字符串簇与 map_delete 归因（先测后修）

- E 新增：`string_build1/3/large1`、`int_to_string`、`map_delete`。
- 先 profile 当前树（`perf record` + 指令计数），确认是否同一 ownership
  事务税；若是，评估纳入 T1.1/T1.3 覆盖面或另立窄修；若否，记录归因
  交 D/A4，不强行修。
- 归因（已完成）：`complete_local_add` 的 `constant_string`
  （`conversion_driver/local_add.rs:245`）对常量池 String 每次走 checked
  `dup_jsvalue`（string_build1 基准 1.6M 次），perf annotate 显示其周期
  占比 14.6%，热点在 Result 返回槽编组与校验事务——同一 ownership 事务税。
- 窄修（提交 `28a47f32`）：常量池节点在整帧执行期间持有该边，改 trusted
  `retain_live_string_handle`。隔离实测 LTO 指令：build1 −4.6%、
  build3 −3.3%、large1/large2 −3.0%，int_to_string/map_delete 中性；
  LTO 墙钟 −32%/−10%/−14%/−7%。累计对 A：string_build* 由回退转为领先
  （LTO 墙钟 0.76–0.93），`int_to_string`/`map_delete` 仍 +11~15%，
  交 D/A4 归因。

### T1.6 热路径内联钉住（实施中新增）

- 现象：T1.0–T1.3 落地后 LTO 协议下属性读探针回退：`prop_read_int`
  墙钟 +13%、指令数不变、前端停顿约 3 倍（`read_location` 由内联翻为
  outlined，perf 符号占比 0.43% → 6.25%）。
- 二分结论：非语义回退。移除 T1.0 MAX 检查（`lto-z-nomax`）或回退
  T1.1（`lto-no-t11`）均不恢复；仅加两个永不跳转比较的 T1.0 单独构建
  （`lto-t10`，指令数与基线逐位相同）已使 obj/string 墙钟 +5~6%。
- 处理：`PropertyReadCache::read_location` 加 `#[inline(always)]`
  （提交 `ca14ae98`），int 墙钟恢复基线 1.01×、指令 0.0% 变化、
  bigint 固定行无影响。
- 残余：obj/string LTO 墙钟 +5~9%（指令中性），nolto 中性/正向
  （obj −7.3%）；判为布局噪声带内，最终以官方 property 套件复测。
- 不做：其余内联钉住（无因果证据且增指令），避免 whack-a-mole。

## 4. T2：FrameBinding 32B → 24B

### 4.1 现状

- 变体为 root 形态（`vm/bindings.rs:17-23`）；
  `release_frame_binding`（`vm/bindings.rs:38-51`）对
  Private/PrivateCallable/Captured 是空操作（靠 `Drop`）。
- 释放调用点：`vm/stack.rs` 10 处 + `vm/run.rs:235`（ReleaseDropped）
  + `vm/conversion_driver.rs:286` + `vm/conversion_driver/local_add.rs:223`。
- 构造点：`vm/bindings.rs:205/220/235/246`（capture）、
  `vm/private_bindings.rs:56/118`、`vm/suspend.rs:156`（decode）。
- 挂起层已就绪：`GeneratorFrameBinding` 已是 `Private(Atom)`/
  `PrivateCallable(ObjectId)`/`Captured(VarRefId)`
  （`vm/suspend.rs:47-66/137-160`、`heap/gc.rs:1999-2005/2434-2440`），
  decode 已有 kind/liveness 校验。
- 当前树无 `FrameBinding` 尺寸断言（A4 spike 测试是 B/C 代码，已随回退
  删除）。

### 4.2 方案

1. 变体句柄化：`Private(AtomIdx)`、`PrivateCallable(ObjectId)`、
   `Captured(VarRefId)`。
2. `release_frame_binding` 显式释放：`release_atom_handle(brand(idx))`/
   `release_object_handle`/`release_var_ref_handle`；构造点显式 retain
   （capture 处已有 `root.clone()`，改为 retain + 存 id）。
3. 挂起 encode/decode 简化：encode 直接存 `AtomIdx`（现为 branded
   `Atom` 再转，`gc.rs:2434`）；decode 的 `belongs_to` 校验收敛到已有的
   kind/liveness 检查（跨域防护从"值品牌"移到"解码校验"，属信任模型
   变更，需在结果文档记录）。
4. 尺寸门禁重建（编译期）：
   `assert!(size_of::<FrameBinding>() == 24)`、
   `assert!(size_of::<Option<FrameBinding>>() == 24)`；重建 A4 反事实
   spike：8B `Direct` 下 16B binding。
5. owned-storage 计数与 RSS 自动反映槽步长 −25%。

### 4.3 不变量与风险

- 显式释放恰好一次；debug 边账本 + teardown 断言 + 挂起
  encode→decode→release round-trip 测试兜底。
- `PrivateCallable` 必须保持真 `[[Call]]`：decode 继续用
  `as_callable` 复核（现有行为）；构造点已由 runtime 校验。
- `AtomIdx` 为 per-runtime 表索引，单线程下帧不可能持有外来句柄；
  挂起 decode 的 kind 检查兜底。
- 风险面：显式释放纪律扩大 invariant-panic 面——按 §2 原则与账本兜底。

## 5. 排程

1. T1.0（饱和语义）→ T1.1 → T1.2 → T1.3，各自独立提交 + 独立 A/B；
   T1.5 归因并行进行，不混入同一提交；T1.4 按实测决定。
2. T2 可与 T1 并行改代码（文件面基本不重叠），测量必须串行。
3. T1/T2 全部完成后重跑 A4 决策矩阵；B 另立项。

## 6. 验收门禁

- 构建/正确性：`cargo fmt --check`、MSRV 1.88 clippy `-D warnings`、
  workspace 测试；阶段收尾跑 Test262 冻结向量零回归
  （pass=79982 / eligible=80032 / total=102037）与 source-layout 门禁。
- 基准：双协议（LTO off + CGU16 与 fat LTO + CGU=1）配对轮换，
  **instructions 为主信号**；固定行 = bigint32/64/256、scaling、
  V8、fixed 字符串簇、`a=a` 等微负载。
- 不回退：属性读/property、map 构造、insert 等 A 既有收益；LTO 墙钟
  残余差异须与同构建 A/A 噪声及 T1.6 布局证据对照后再判。
- T2 另加：尺寸断言、owned-storage 计数/RSS、帧密集微负载。
- 每项独立提交，可单独回退；不得跨项混合提交。

## 7. 文档与后续

- 已修正：`performance-architecture.md` §4.4/§11、
  `s3-a-plan.md` §8.13（提交 `c1805e9d`）。
- 实施结果记入 `s3-a-plan.md` 新增 §8.14 或
  `s3-full-rerun-results.md` 后续轮次；A4 决策矩阵引用本计划结论。
- B 的重新设计独立立项，不继承本计划。

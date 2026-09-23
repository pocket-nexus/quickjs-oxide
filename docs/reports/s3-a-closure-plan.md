# 阶段 A 收口计划：所有权事务窄修与帧槽瘦身（T1/T2）

> 状态：T1 已实施并实测（2026-09-23，结果见
> [s3-a-plan.md §8.14](s3-a-plan.md)）；T2 待实施。对应路线：
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
- E 的固定行（m0/pre-A）：bigint32 墙钟 0.79–0.84 / 指令 0.96（已快于
  pre-A）；bigint64 ~0.90 / 1.05；bigint256 **1.21 / 1.32**（指令列
  2026-09-23 按 final 构建直测更正，原 0.98/1.12/1.43 与 head 构建同源；
  见 [s3-full-rerun-results.md](s3-full-rerun-results.md) §4.6）。
  fixed 字符串簇回退：`string_build1` 1.170、`string_build3` 1.178、
  `string_build_large1` 1.170、`int_to_string` 1.204、`map_delete` 1.128
  （T1.5 已归因并窄修，见 §8.14）。

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

> T2 实测修正（2026-09-23）：句柄化后整个 enum 是 **16B**，不是预测的 24B。
> rustc 把枚举 tag 打进 `JsValue` 的判别值空位（niche-filling），
> `Option<FrameBinding>` 同为 16B。`PrivateNameRef` 的 24B 载荷在句柄化后
> 已不存在。见 §4.7。

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

- 状态（2026-09-23）：已评估、未实施。`assign_local.js` 局部残余确认
  （指令/操作 +13.8% nolto / +20.9% lto vs pre-A），但适用面仅字面自
  赋值，对 bigint256 等实际 workload 无效；暂缓，保留为可选项（见
  [s3-a-plan.md §8.14](s3-a-plan.md)）。
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
  `retain_live_string_handle`。隔离实测 LTO 指令：build1 −6.5%、
  build3 −3.1%、large1/large2 −4.0%/−2.8%，int_to_string/map_delete
  −3.6%/−0.3%；LTO 墙钟 −21%/−11%/−8%/−10%。相对 A 终版全面领先
  （LTO 墙钟 0.79–0.92），对 pre-A 差距收窄（`int_to_string`/`map_delete`
  仍 +12~15%），交 D/A4 归因。最终数据见
  [s3-a-plan.md §8.14](s3-a-plan.md)。

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

## 4. T2：FrameBinding 32B → 16B（实测）

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
- 行号基线漂移：T1.5/T1.6 已改动 `conversion_driver/local_add.rs`
  （`b3f3ad7c`）与 `vm/run.rs`（`56d5a51e`），T2 开工前按当前树重扫点位。

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
   `assert!(size_of::<FrameBinding>() == 16)`、
   `assert!(size_of::<Option<FrameBinding>>() == 16)`（实测：tag 打进
   `JsValue` 判别值空位，不是原估的 24B）；重建 A4 反事实
   spike：8B `Direct` 下 16B binding；并加 T2d 反事实（绑定 kind 复用
   A4 tag 空间、整体打包 u64 → 8B，空槽需保留 tag 而非 Rust niche）
   为 A4 联合设计喂数据。反事实断言
   全部用测试内镜像类型，不触碰产品类型；post-A4 的真实尺寸断言只能在
   A4 落地后写。
5. owned-storage 计数与 RSS 自动反映槽步长 −25%。

### 4.3 不变量与风险

- 显式释放恰好一次；debug 边账本 + teardown 断言 + 挂起
  encode→decode→release round-trip 测试兜底。
- `PrivateCallable` 必须保持真 `[[Call]]`：decode 继续用
  `as_callable` 复核（现有行为）；构造点已由 runtime 校验。
- `AtomIdx` 为 per-runtime 表索引，单线程下帧不可能持有外来句柄；
  挂起 decode 的 kind 检查兜底。
- 风险面：显式释放纪律扩大 invariant-panic 面——按 §2 原则与账本兜底。

### 4.4 决策记录（2026-09-23）

- **方案选定：句柄化**（§4.2 原案）。box 路线已评估否决：只 box `Private`
  的 A4 前同为 24B，但 A4 后仍 24B（`PrivateCallable`/`Captured` 载荷
  16B），需二次改造；box 全部三个 root 变体可在 A4 后达 16B 且保留 RAII，
  但每绑定一次分配。句柄化零分配、A4 后自动 16B，且挂起层已按 id 形态
  就绪（`GeneratorFrameBinding`，仅 `Private` 的 branded `Atom` 顺带瘦为
  `AtomIdx`），选它。
- **16B 是实测结果，24B 是错误预测**：原推理（A4 前最大载荷 16B，tag +
  对齐 → 24B）忽略了 rustc 的 niche-filling——tag 打进 `JsValue` 判别值
  空位，`FrameBinding`/`Option<FrameBinding>` 均 16B。A4 后 `Direct`=8B、
  句柄 ≤8B，载荷 8B + tag 仍需 16B（`A4HandleBinding` 镜像实测 16B）；
  8B 绑定只能由 T2d（整体打包 u64，空槽用保留 tag）得到，列为 A4 后联合
  设计决策项。
- **信任模型变更**：跨域防护从「值品牌」移到 decode 的 kind/liveness 校验
  （§4.2 第 3 点），须在 T2 结果文档记录。
- **全方案零 `unsafe`**：句柄化、显式 retain/release、尺寸断言均为安全
  Rust，`unsafe_code = "forbid"` 不变。

### 4.5 实施清单（2026-09-23 按当前树重扫，可直接实施）

**表示与所有权规则**

- `FrameBinding` 改为 `Private(AtomIdx)`/`PrivateCallable(ObjectId)`/`Captured(VarRefId)`；
  每个非 `Direct` 变体**恰好持有一条边**（atom / object / var-ref）。
- 非 `Direct` 变体不再有 `Drop`：任何移动、覆盖、丢弃都必须显式 release
  （`release_atom_index` / `release_object_handle` / `release_var_ref_handle`），
  `release_frame_binding` 是统一出口。
- 编译期门禁（`vm/bindings.rs` 模块级）：`size_of::<FrameBinding>() == 24` 且
  `size_of::<Option<FrameBinding>>() == 24`。

**构造与边转移（净 +1 条边归帧）**

- `private_bindings.rs:56`（`initialize_name`）：`retain_atom_handle(name.atom())`
  后存 `Private(AtomIdx::from_raw(name.atom().raw()))`；`name` 作用域释放，净 1 条边。
- `private_bindings.rs:118`（`initialize_callable`）：`retain_object_handle(id)` 后存
  `PrivateCallable(id)`；`callable` 释放，净 1 条边。
- `bindings.rs:279-288`（`close_frame_binding`）：新增 private-elements 按 id 变体
  （`private_name_index_from_raw_var_ref` / `private_callable_index_from_raw_var_ref`，
  校验 + retain + 返回 id）；先释放旧 `Captured` 边再写新变体。
- `bindings.rs:205/220/235/246`（`capture_frame_binding`）：`Direct`/`Uninitialized`
  臂只改存 id；`Private`/`PrivateCallable` 臂先建 cell（新边），再释放帧旧边；
  返回的 root 与绑定各持一条边（与现状一致）。
- `suspend.rs:137-157`（decode）：`Private` 先 `brand` 校验 kind 再 `retain_atom_handle`；
  `PrivateCallable` 先 `ObjectRef::from_borrowed_handle` + `as_callable` 复核再
  `retain_object_handle`；`Captured` 直接 `retain_var_ref_handle`；均净 1 条边归解码帧。

**释放与覆盖审计（显式 release）**

- `bindings.rs:38-51` `release_frame_binding`：三个变体分别走
  `release_atom_index`/`release_object_handle`/`release_var_ref_handle`。
- 覆盖点：`capture_frame_binding`（旧 `Private`/`PrivateCallable`）、
  `close_frame_binding`（旧 `Captured`）；`stack.rs` 的 take/replace/clear 路径
  已统一走 `release_binding`，无需改。
- `call/prepare.rs` 测试专用帧向量的错误出口补 release（非热路径）。

**读路径（零新分配）**

- 新增 `VarRefView::from_frame(runtime, VarRefId)`（`heap/roots.rs`，`pub(crate)`；
  文档化信任论证：边由帧绑定持有，视图与调用作用域同生命周期）。
- 视图替换点：`bindings.rs:173/249/271/351/433/700`、
  `run.rs:1276/1322/1344/1444`、`frame_operations.rs:261/434`、
  `environment_bindings.rs:57/150`、`private_bindings.rs:65/121/293/353`。
- 需物化 root 返回调用方的读路径：`private_bindings.rs:292`
  （`PrivateNameRef::from_borrowed_atom` + `brand`）、`:352`
  （`ObjectRef::from_borrowed_handle` + `as_callable`）；`driver.rs:1624`（测试）同样物化。
- `suspend.rs:109-119` 的 `Captured` 校验可直接用 id 访问 `heap.var_ref(*id)`，不建视图。

**挂起层简化（信任模型变更）**

- `suspension_records.rs:17-23`：`GeneratorFrameBinding::Private(Atom)` →
  `Private(AtomIdx)`；`is_null` 检查（`suspension_records.rs:212/457`、
  `object_storage.rs:1908`）改 `AtomIdx::is_null`。
- `suspend.rs:41-69` encode：直存 id，删除 3 处 `belongs_to` 校验；
  `:177-209` `atoms()` 对 `Private(idx)` 改 `table.brand(idx)?` 后入列。
- `gc.rs:2468-2473`：`Private(idx) => Some(*idx)`；`:2032-2043` 边集合不变
  （休眠记录不持边）。
- decode 的 kind/liveness 校验保留，`PrivateCallable` 继续 `as_callable` 复核（§4.3）。

**门禁与测试**

- 尺寸断言 + 重建 spike：`vm/bindings/representation_spike_tests.rs`
  （旧版 `git show 3e116f75:...`），镜像类型覆盖 现状 32B / 句柄化 24B /
  A4 反事实 16B / T2d 反事实 8B。
- 挂起 round-trip：capture→encode→thaw→release，比对 atom/object/var-ref
  strong 计数（仿 `suspend.rs` 的 `failed_thaw_releases_partial_roots...`）。
- 跨域/信任：decode 非 private atom、非 callable object、元数据不符均拒绝
  （沿用现有 invariant 测试并补 id 路径）。
- 帧密集微负载 + owned-storage 计数/RSS（§6 门禁）；`execution.rs:30` 与
  `call/prepare.rs:88-90` 的容量核算自动反映新步长。

### 4.6 提交切片（每片独立可回退）

1. `perf(vm): handle-ize frame bindings`：新表示 + 全部读写/构造/释放点 +
   覆盖审计 + 16B 断言；encode 暂以 `brand(idx)` 适配旧 `GeneratorFrameBinding`。
2. `perf(vm): store unbranded atom indices in dormant frames`：
   `GeneratorFrameBinding::Private(AtomIdx)` + encode/decode/`atoms()`/gc/校验简化 +
   删除 `belongs_to`（结果文档记录信任模型变更）。
3. `test(vm): rebuild frame-binding representation spike`：镜像类型 + 反事实断言。
4. `docs(perf): record T2 results`：owned-storage/RSS/微负载实测 + 信任模型变更 +
   门禁记录。

测量在 T1 收尾后串行；每片单独提交、单独回退（§6）。

### 4.7 实施结果（2026-09-23）

**提交**（均未推送，基线 `c662790e`）：

1. `dfe73e42 perf(vm): handle-ize frame bindings`
2. `5d60dc7b perf(vm): store unbranded atom indices in dormant frames`
3. `2f558a8f test(vm): rebuild frame-binding representation spike`

**表示尺寸（编译期断言 + spike 实测，`vm/bindings.rs`）**：

| 类型 | pre-T2 | T2 |
| --- | --- | --- |
| `FrameBinding` | 32B | **16B** |
| `Option<FrameBinding>` | 32B | **16B** |
| `JsValue`（参照） | 16B | 16B |

tag 由 rustc 打进 `JsValue` 判别值空位；`align_of` 保持 8B。
spike 镜像：pre-T2 rooted 形态 32B、A4 8B `Direct` + 现有句柄 16B、
T2d 整体打包 u64 可达 8B 但空槽须用保留 tag（`Option` 无 niche，16B）。

**owned-storage（帧密集微负载）**：脚本
`sum(n)=n+sum(n-1)`，200×100 递归，`--features profiling` 下同脚本
pre-T2（`c662790e`）与 T2 各跑一次：

| 指标 | pre-T2 | T2 |
| --- | --- | --- |
| `frames_prepared` | 20202 | 20202 |
| `maximum_slot_capacity`（槽） | 1024 | 1024 |
| `maximum_live_slots` | 208 | 208 |
| `maximum_frame_depth` | 103 | 103 |
| 槽容量隐含字节（容量 × 步长） | 32768B | **16384B** |

执行形状不变、槽数一致，容量字节随步长减半。RSS 在本微负载规模
（16KB 量级）不可分辨，未作为证据；正式 LTO 配对基准留待 A4 决策前
按 §6 协议与 T1 结果同批复测。

**信任模型变更（须记录）**：切片 ① 因 root 消失，encode 的 3 处
`belongs_to` 值品牌校验移除；跨域/kind 防护收敛到 decode 的
`brand`+kind/liveness 校验（切片 ② 后 `GeneratorFrameBinding::Private`
存无品牌 `AtomIdx`，仅在 `atoms()`/decode 边界 brand）。单线程
per-runtime 表下帧不可能持有外来句柄，该收敛符合 §2 边界原则。

**门禁记录**：`cargo fmt --all`、`cargo check --all-targets` 零警告；
`cargo test --locked --workspace --all-targets` 全绿（lib 2371 + 908 +
122 + …，0 failed）。切片 ① 期间发现并修复 capture 双 owner 单边的
回归（binding 与返回 root 共享一条 cell 边），由
`immediate_cell_writes_*`/`captured_reads_*`/test262 pinned 用例捕获。

**Test262 阶段收尾（`--full`，workers=12，2026-09-23）**：分类汇总与冻结
向量逐项对照——`fail-parse=7`、`fail-runtime=43`、`skipped-config-exclude=6700`、
`skipped-feature=11775`、`unsupported-feature=847`、`unsupported-module=121`
全部不变；`pass` 79982 → 80010（+28），`unsupported-negative-provenance`
2562 → 2534（−28）。+28 行经报告逐行核对，恰为 `00bb387f fix(lexer)` 新增的
14 条精确诊断契约 ×2 变体（28 行全部 pass），即该改进在 T1/T2 之前就已并入，
**T2 未引入任何 Test262 行级变化、零回归**。字节比对门禁因基线过旧退出 5：
冻结 receipt 的结果字节与源码身份仍停留在 `022e7b48`（`00bb387f` 的 +28
契约改进未随附 promotion），且任何源码演进都会改变工作区指纹（T1/T2 亦然）；
`--focused` 按设计拒绝过旧源码，未跑。基线 promotion（重跑 full → 派生
focused → 更新 `current.conf`/`docs/status.md`）留作独立事项。
`python3 scripts/checks/check-source-layout.py` 通过（698 个 reachable
Rust 文件）。证据：`target/s3-a-t2-test262-197162f5/`（full.log、TSV/JSONL、
status.json）。

### 4.8 T2 发布协议快速 A/B（2026-09-23）

**协议**：pre-T2 `c662790e` vs post-T2 `7cf2395e`，两侧同命令重建
（`cargo build --locked --release -p quickjs-oxide-cli --no-default-features`，
即 fat LTO + CGU=1、无 PGO/profiling；rustc 1.94.1）；`taskset -c 2` +
`perf stat -e cycles:u,instructions:u`，顺序轮换，stdout 断言，wall 为
普通进程运行；governor=powersave。二进制 sha256：pre `b496ecf1…`、
post `c4f1c433…`。这是 T2 增量快速判定，§6 双协议 scaling/V8/fixed-58
全量仍留 A4 决策点。

| 用例 | reps | insn pre→post (M) | insn Δ | cyc Δ | wall Δ |
| --- | ---: | ---: | ---: | ---: | ---: |
| locals（16 locals×20 万调用） | 5 | 4674→4141 | **−11.39%** | −13.84% | −13.44% |
| bigint32 | 7 | 4574→4175 | −8.71% | **+3.66%** | +2.89% |
| bigint64 | 7 | 5090→4691 | −7.84% | **+3.55%** | +1.67% |
| int_to_string | 3 | 4549→4307 | −5.32% | −7.25% | −7.03% |
| string_build1 | 3 | 2583→2448 | −5.22% | −7.14% | −6.48% |
| bigint256 | 7 | 7977→7575 | −5.05% | −0.44% | −2.14% |
| assign | 3 | 6655→6369 | −4.30% | −12.69% | −15.65% |
| add_one | 3 | 4833→4636 | −4.08% | −0.04% | −5.44% |
| concat | 3 | 1487→1430 | −3.87% | −3.67% | −0.98% |
| mul | 3 | 2846→2745 | −3.55% | −4.48% | −6.88% |
| frames（sum 递归 200×100） | 7 | 102→98 | −3.55% | +0.08% | +5.63%※ |
| string_build3 | 7 | 3838→3707 | −3.43% | −0.67% | −2.24% |
| string_build_large1 | 7 | 4238→4115 | −2.91% | −1.95% | −1.26% |
| map_delete | 3 | 537→530 | −1.28% | −1.06% | −1.01% |
| arguments_strict_read | 7 | 2736→2707 | −1.08% | −0.37% | +0.51% |
| arguments_read | 7 | 2833→2809 | −0.86% | +0.28% | −0.12% |

※ 10ms 量级用例，wall 噪声主导，仅指令数有效。

**机制探针**（同协议，3 reps）：`bigint_loop`（1M 次 BigInt 乘法，无帧/
局部访问）insn **0.00%**——BigInt 算术路径未变；`cell_loop`（顶层 `let`
1M 次自增）−12.1%；`func_locals`（函数内局部 1M 次自增）−12.3%；
`call0`/`call4`/`locals`（1/4/16 个局部 ×20 万次调用）insn −5.1%/−8.1%/
−11.4%，随局部数单调——与帧槽步长 32B→16B 及 `Captured` 借用视图
（`VarRefView::from_frame` 取代 root clone）的机制一致。

**判定**：预期收益确认且超出——16 个用例指令数**全部下降**（−0.9%~
−11.4%），帧密集用例最大；存储侧槽字节减半见 §4.7。无指令回退；多数
用例 cycles/wall 持平或改善。唯一混合信号：`bigint32`/`bigint64` cycles
**+3.6%/+3.5%**（7 次轮换稳定，IPC 3.05→2.69 / 2.95→2.62；指令与分支
均下降、branch-misses 不变），wall +2.9%/+1.7% 在噪声边界；判为小幅
回退风险而非结论，留 A4 决策矩阵用 nolto 对照/perf annotate 复核。
`bigint256` cycles 持平。限制：shipped LTO 协议下不排除部分差值来自
内联/布局移动（T1.6 先例）；未做 annotate 级因果证明；RSS 在该规模
不可分辨。

**证据**：`target/t2-ab/`（`measure_t2.py`、`recheck.py`、`raw-t2/`、
`raw-t2-recheck/`、`workloads/`、`probe/`）；pre 侧 worktree 与构建缓存
`/home/eric/.cache/opencode/t2-pre-wt`、`t2-pre-target`、`t2-post-target`。

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

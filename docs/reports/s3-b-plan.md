# S3-B 实施计划：守卫式专用执行跨度（B2 重立项）

> 状态（2026-09-24）：设计完成，待裁决。本文取代
> [s3-b-initial-plan.md](s3-b-initial-plan.md) 的「只读 QuickOp 投影」首批；
> 该首批已随 C 整体回退，本文给出重立项后的目标、机制、切片与门禁。
> 依据：[A4/D/B 决策报告](s3-a4-d-b-decision.md) §4.3、
> [性能架构](performance-architecture.md) §11、[阶段 D 计划](s3-d-plan.md)
> §4 的「native 编组链归 B」，以及本文 §2 的本轮新增实测归因。
> 基线：`feat/pr27-d`（PR #34）HEAD `3b759647`；测量二进制为
> fat LTO + CGU=1 的 `cargo build --release -p quickjs-oxide-cli`。

## 0. 结论摘要（TL;DR）

1. **B1 的负结果不是「quickening 不行」，而是「只加派发层、不减工作」**：
   只读投影在命中路径上仍做 bit 解码与二次分类，专用 handler 仍调用同一批
   可失败 helper，还额外引入 25–72% 的 canonical 回落（§1）。
2. **本轮决定性新证据（§2）**：热循环的成本大头既不是派发（跳转表
   <1%）也不是取指，而是**可失败 helper 链与操作数栈**：
   `copy_value` 的 `Ok(copied)` 一行占 `run` 采样 30–35%，
   `Result<_, Error>` 实测 80B、经隐藏内存槽返回，outlined helper 合计
   ~30%。`int_local` 每轮 15 条逻辑指令、11 个派发入口、1355 条机器指令。
3. **设计**：B2 分两步。**B2.1「守卫式专用跨度」**把编译器实际产生的热序列
   （循环条件、`s=s+1`、`s+=o.a`）折叠成**不可失败**的专用 handler：
   guard 通过后只做直接槽索引读写、`Number` 立即数运算、PC 更新，全程
   无 `Result`、无 owner 移动、无回调；guard 失败回到**同一 PC**执行完整
   canonical（不产生任何半提交）。B2.1 复用已认证的 `FusionPlan` 作为载体，
   **不建第二执行阵列、不引入第二 PC 空间、不做运行期解码**。
   **B2.2「自适应 per-PC 专用槽」（PEP 659）**仅在 B2.1 证明收益、且残留
   归因显示动态类型事实确有价值时启动（§4.3）。
4. **首批微目标**（决策报告 §3.2 的每操作成本）：
   `empty_loop` 607→目标 ≤200 insn/轮、`int_local` 1355→≤300、
   `prop_read` 1730→≤450（每轮指令数，QuickJS 折算 90/128/201，见 §2.1）。
5. **待裁决 4 项**见 §7；未裁决前不动默认执行路径。

## 1. B1 复盘：负结果的结构性原因

B1 首批（`s3-b-initial-plan.md`，实现见 `33a02659`，已随 C 回退到
`bcfb4fe5`）交付的是**发布期翻译 + 运行期解码**的只读 8B `QuickOp` 投影。
其负结果数字（`s3-c-negative-result.md`、`s3-bc-implementation.md` §7）：

| 指标 | 数字 |
| --- | --- |
| 认证后 `decode().expect(..)` 内联链占程序采样 | 16.55%–30.26%（六输入） |
| guard decline / canonical 回落比例 | 25.0%–72.5%；`prop_read` decline 100,006 次 vs Generic 242 |
| `.text` | +1.04%；`run` 主循环实例 +91% |
| 编译 | B1b projection-only +1.3%（单项最大 +4.3%） |
| 常驻内存 | 嵌套 cold RSS +1308 / +1124 KiB，超 `max(3%, 1MiB)` |
| 执行 | M2 相对 M0 全面变慢（多数输入 1.0–1.27） |

结构性教训（本文设计的直接输入）：

1. **派发便宜、工作昂贵**。`Instruction` 已在 `Rc<[Instruction]>` 中按 PC
   连续存放（12B/条，§2.3），`match` 编译为跳转表，实测派发本身只占
   `run` 采样 4–8%。把同一条指令换成一个 word 再 match 一次，省不下东西，
   只是加层。
2. **专用路径必须删掉可失败调用，而不是换一种表示**。B1 的热 handler 仍走
   `slots.local()?`/`slots.push()?` 等返回 `Result<_, Error>`（80B）的
   helper；隐藏内存返回、栈临时搬运、二次证明一个都不少（§2.2）。
3. **任何「先试专用、失败再 canonical」都必须让失败路径廉价**。B1 的
   decline 在 canonical 执行前还要多走一次分类与解码；一旦 decline 率
   上到 25–72%，税就超过收益。B2 的 guard 是**前置、无状态、几个指令**的
   绑定/类型检查，失败即原地回落，不解析、不重写、不记账（B2.1）。

## 2. 证据基线（2026-09-24 复核）

### 2.1 每轮机器指令数（本轮实测）

方法：`taskset -c 2 perf stat -e instructions,cycles target/release/qjs
target/a4db-decision/micro/<name>.js`（与决策报告同一负载几何：10M 轮循环）。
对 QuickJS 的折算比引用决策报告 §3.2（同一二进制/协议）。

| 微负载 | oxide insn/轮（本轮） | 对 QuickJS insn 比（报告） | QuickJS 折算 insn/轮 |
| --- | ---: | ---: | ---: |
| `empty_loop`（`for(j=0;j<n;j++){}`） | 607 | 6.74× | ~90 |
| `int_local`（`s=s+1`） | 1355 | 10.60× | ~128 |
| `prop_read`（`s+=o.a`） | 1730 | 8.59× | ~201 |
| `array_read`（`s+=a[j&3]`） | 2486 | 11.10× | ~224 |

### 2.2 源码行级归因（本轮新增，`perf annotate -l`）

`run::run` 符号内按 Rust 源码行聚合的采样占比（三负载各自归一到 `run`）：

| 归因行 | 含义 | empty_loop | int_local | prop_read |
| --- | --- | ---: | ---: | ---: |
| `stack.rs:1746` | `copy_value` 的 `Ok(copied)`（`#[inline(always)]`） | 34.6% | 32.0% | 30.1% |
| `stack.rs:0` | 同一内联区域的 std 代码 | 22.1% | 11.2% | 10.8% |
| `result.rs:2173/2174` | `Result` 构造/转换 | 15.6% | 12.3% | 18.0% |
| `run.rs:396` | 指令 match 派发 | 8.4% | 4.0% | 5.3% |
| `run.rs:1654` | `SetLocal` 的 `replace_local` | – | 6.2% | 5.7% |
| `index.rs:219/221` | 槽切片索引 | – | 2.8% | 1.1% |

符号级（`int_local`，`perf report`）：`run::run` 自时间 67.8%，
`push_current` 10.8%、`binary` 5.45%、`update_local` 4.66%、
`release_displaced` 2.45%、`replace_local_current` 2.43%、
`pop_current` 1.78%、`peek` 1.70%、`parameter_current` 1.62%。

**解读**：`copy_value` 的标量分支本身只做 `match` + 拷贝，但函数签名是
`Result<JsValue, Error>`；80B 返回值使每次调用都走**隐藏内存返回槽**，
再叠加调用点的 16B 值搬运。汇编中反复出现的
`lea 0x39(%rsp),%rdx`（返回槽指针）与 `movaps/movapd`（16B 值搬运）即此。

### 2.3 尺寸事实（本轮实测，`cargo test --lib` 临时探针）

| 类型 | 字节 |
| --- | ---: |
| `Instruction` | **12** |
| `Error`（`api/error.rs:166-172`：`ErrorKind` + `String` + `Option<Box<..>>` + `Option<SourceSpan>`） | **80** |
| `Result<(), Error>` | **80** |
| `Result<JsValue, Error>` | **80** |
| `RunExit` / `FrameBinding` / `JsValue` | 16 / 16 / 16 |

`Result<(), Error>` 已超过 SysV 16B 的寄存器返回上限，所以解释器内
**每一个 `?` 都是内存返回**。这是 §2.2 中 `result.rs` 与栈搬运成本的来源。

### 2.4 编译器的实际产物（内层函数字节码，本轮 dump）

`empty_loop`（内层 22 条，`max_stack=2`）：

```text
0: PushI32(0); 1: SetLocal(0); 2: Drop          ; j = 0
3: GetLocal(0)                                   ┐
4: GetArg(0)                                     │ 循环条件：4 条指令
5: Lt                                            │
6: IfFalse(18)                                   ┘
7: Goto(13)
8..12: Nop ×5
13: GetLocal(0); 14: PostInc; 15: PutLocal(0); 16: Drop   ; UpdateLocal span（已融合）
17: Goto(3)
18: GetArg(0); 19: Return
```

每轮 6 个派发入口（3/4/5/7/13/17）。

`int_local`（内层 29 条）：

```text
5: GetLocal(0); 6: GetArg(0); 7: Lt; 8: IfFalse(25); 9: Goto(15)   ; 条件
15: GetLocal(1); 16: PushI32(1); 17: Add; 18: SetLocal(1); 19: Drop ; s = s + 1（未融合）
20: GetLocal(0); 21: PostInc; 22: PutLocal(0); 23: Drop             ; UpdateLocal（已融合）
24: Goto(5)
```

每轮 11 个派发入口、15 条逻辑指令。注意 `PushI32(1); Add; SetLocal; Drop`
完全不在现有 `FusionPlan` 中：`local_add_span` 只接受「第二个操作数也是
`GetLocal`」或「`PushConst` 为 String」，数值常量右操作数没有形态
（`code/fusion.rs:99-132`，`run.rs:1388-1405`）。

`prop_read`（内层 34 条）：

```text
19: GetLocal(1); 20: GetLocal(2); 21: GetField(1); 22: Add; 23: SetLocal(1); 24: Drop
25: GetLocal(0); 26: PostInc; 27: PutLocal(0); 28: Drop   ; UpdateLocal
```

`array_read`（内层 39 条）：`j & 3` 展开为 `GetLocal(0); PushI32(3); BitAnd`
后接 `GetArrayEl`，同样全在操作数栈上来回搬运。

### 2.5 结论

- 派发、取指、跳转表都不是瓶颈（合计 <10%）。
- 瓶颈是**通用可失败 helper + 操作数栈**：局部读取经
  `local_current()?` → `copy_value()?` → `push()?`，每条都返回 80B
  `Result`、各做一次容量/空缺/所有权限检查；一次 `s = s + 1` 在栈上
  产生 4 次 push/pop 往返与 3 个 outlined 调用。
- 因此 B2 的专用 handler 必须**删除**这些调用（而不是替换派发表示），
  并在 guard 通过后保持**不可失败**。

## 3. 目标与不变量

### 3.1 目标

- **主指标（每片）**：`empty_loop` / `int_local` / `prop_read` /
  `array_read` 的每轮机器指令数；B2.1 完成后要求至少一项 ≥30% 下降、
  且 B2.1a 单独 ≥15%（kill criterion，见 §6）。
- **次指标**：同负载 cycles；D 的固定行
  （bigint32/64/256、scaling、V8、fixed 字符串簇）指令不退化 >2%。
- **正确性**：`cargo test --locked --workspace --all-targets`、
  Test262 全量零回归、`check-source-layout.py`。
- **机制约束**：默认构建在 B2 验收前不含任何自适应状态；
  B2.1 不新增每 PC 常驻数组（复用现有 `FusionPlan` flag）。

### 3.2 不变量（每条有执行依据，专用路径违反即视为设计错误）

1. **所有权恰好一次**：专用快路径只处理立即数（`Undefined/Null/Bool/
   Int/Float/ShortBigInt`）；引用类型（Object/String/BigInt/Symbol/…）
   一律 guard 失败回落 canonical。专用路径内**禁止** retain/release/
   drain/GC 入口：与 `copy_value` 的标量分支
   （`stack.rs:1730-1747`）同一语义域。
2. **guard 先于任何副作用**：检查失败时专用路径必须尚未写入任何槽、
   未改 `depth`、未改 `next_pc`、未动 owner。canonical 必须能从 span
   首 PC 完整重放（`code/fusion.rs:1-7` 的既有纪律）。
3. **PC/fault/resume 语义不变**：专用路径不执行可观察操作，因此不需要
   `publish_fault()`（`run/program_counter.rs:22-26`）；跨度成功时按
   canonical 跨度推进 `next_pc`；失败时不推进。错误与异常只由 canonical
   产生，fault PC 仍是 span 首 PC。
4. **跨度入口纪律**：专用跨度只在首 PC 进入，内部无控制流入口
   （复用 `FusionPlan::build` 的 `entries` 检查，`code/fusion.rs:28-41`）；
   被折叠的 `Goto` 必须按 `control_effect` 验证为无观察副作用。
5. **IC 语义不动**：属性 IC 仍按 canonical PC 索引
   （`property_ic.rs:360-407`），专用跨度只消费
   `property_ic_read_fast`（返回 `Option`，无 `Result`，
   `ordinary_storage/ic.rs:118`）或非拥有立即数读
   （`ordinary_storage.rs:985`）；不复制 rank/失效逻辑。
   失效仍靠 shape 指针 + `layout_revision` + `property_layout_epoch`
   三件套（`property_ic.rs:89-100`）。
6. **挂起/恢复 ABI 不变**：generator/async 保存 canonical PC +
   `FunctionBytecodeId`（`suspension_records.rs:27-51`）；B2.1 无可变状态，
   恢复后行为与 canonical 逐条等价。
7. **无 unsafe、无新增 GC edge**：专用状态（B2.2 若启动）只存索引/类型，
   不持有 owner；GC 不扫描（`unsafe_code = "forbid"`，workspace lint）。
8. **失败分类**：专用路径只有「命中/未命中」，未命中绝不合成
   `Error`；`Error::internal` 只由 canonical 在真实不变式破坏时产生。

## 4. 设计

### 4.1 载体：扩展已认证的 `FusionPlan`，不建第二派发层

`FusionPlan` 已经具备 B2.1 所需的全部纪律：按 canonical PC 的 u8 flag、
只在首 PC 进入、内部无入口、失败回退首指令。B2.1 只是把**更多编译器
实际产出的形态**纳入 flag，并把 handler 写成不可失败。

改动点：

1. `code/fusion.rs`：把散落的 flag 常量（16–31 UpdateLocal、32
   CompareBranch、64/65 AddStore、128–131 LocalAdd/ConstLocalAdd、
   160–167 MethodCall，`fusion.rs:39-211`）收敛为 `SpanKind` 枚举 +
   `flag(pc) -> Option<SpanKind>` 的单次查询；空闲值段
   （33–63、66–127、132–159、168–255）分配给新形态。编码仍在 u8 内，
   操作数从相邻 canonical 指令重读（普通 enum 字段读取，**不是** B1 的
   位域解码）。
2. `vm/run.rs`：`GetLocal`/`GetArg`/`PushConst`/比较臂把现有的
   `local_add_span`/`const_add_span`/`update`/`compare_branch` 多次查询
   合并为一次 `fusion.span(pc)` 查询，按 kind 分派；新增
   `run/specialize.rs`（或并入 `run/fusion.rs`）存放不可失败 handler。
3. `vm/stack/window.rs` + `vm/stack.rs`：新增只读/只写的立即数槽访问器
   （字段可见性所在）：
   - `fn immediate_local(&self, index: u16) -> Option<Number>`：
     bounds + `Direct` + `as_number_repr`，**无 Result**；
   - `fn store_number_local(&mut self, index: u16, value: Number) -> bool`：
     先验证旧 binding 是立即数（`primitive_release_owner`，`run.rs:305`），
     失败返回 false（不写、不 release）；成功原地替换，无 Result；
   - `fn immediate_parameter(...)` 同形（参数槽在 `parameters()`）。
4. `code/executable.rs`：`PublishedFunctionData` 不变（B2.1 无新增字段）；
   `FusionPlan` 仍随 `FunctionBytecodeData` 发布（`executable.rs:203`）。

**与 B1 的机制差异（必须写进提交信息/文档）**：

| 维度 | B1 只读投影 | B2.1 专用跨度 |
| --- | --- | --- |
| 派发 | 第二个 word 数组 + 二次 match | 无新增派发；沿用既有 match 臂 |
| 命中路径 | 运行期 `decode` + 二次分类 | 无解码；guard + 直接槽操作 |
| 失败路径 | 回落到 canonical 前要走完整 word 分类 | guard 失败即原地 canonical，零额外状态 |
| 专用 handler 语义 | 仍调用可失败 helper | 不可失败，无 `Result`/owner/回调 |
| 常驻内存 | 每 PC 8B（全函数） | 0（复用现有 u8 flag） |

### 4.2 B2.1 首批专用跨度

通用形态记法：`producer` = `GetLocal | GetLocalCheck | GetArg`；
`store` = `SetLocal | SetLocalCheck | PutLocal | PutLocalCheck`。
所有形态在 `FusionPlan::build` 里按「内部无入口 + 相邻指令精确匹配」
生成，与现有 span 相同。所有 handler 返回
`Option<usize>`（`Some(next_pc)` 命中，`None` 回落），不返回 `Result`。

#### S1 `LocalCompareBranch`（含可选 `Goto` 折叠）

- 形态：`producer(a); producer(b); op; IfTrue/IfFalse(t); [Goto(u)]`
  （`op` ∈ `Lt/Lte/Gt/Gte/Eq/StrictEq/Neq/StrictNeq`）。
- 实例：`empty_loop` PC3–7（`GetLocal; GetArg; Lt; IfFalse; Goto`）、
  `int_local` PC5–9、`prop_read` PC9–13。
- guard：`a`、`b` 两个槽的 binding 都是 `Direct` 且
  `as_number_repr()` 为 `Some`（参数槽走 `immediate_parameter`）。
- 快路径：直接比较两个 `Number`（**复用**
  `run/fusion.rs:55-66` 的同一比较语义，含 NaN/带符号零行为）；若
  `code[if_pc+1]` 是 `Goto(u)` 则 `next_pc = 命中? u : t`，否则
  `next_pc = 命中? if_pc+1 : t`。**全程不压栈、不弹栈**。
- 回落：返回 `None`，canonical 从 span 首 PC 执行（保留 coercion、
  TDZ、captured、错误顺序）。
- 保留现有 `CompareBranch`（`Lt;IfFalse` 无 producer）flag 作为兜底。
- 验收：`empty_loop` 每轮指令数、`ic_share_1prop/2prop` 不退化。

#### S2 `LocalAddConstStore`（`s = s + 1`）

- 形态：`producer(a); PushI32/PushConst(立即数 Int/Float); Add;
  store(a)[; Drop]`。
- guard：`a` 是 `Direct` 且 `as_number_repr` 为 `Some`；常量为
  `RawValue::Int/Float`（`BytecodeConstant::Value`，`run.rs:1151-1156`）。
- 快路径：`a = Number::add(a, c)`（`value/number/operations.rs:50`，
  不可失败），写回 `store_number_local`。`store` 为 `SetLocal` 时必须
  在同一 span 内紧跟 `Drop`（值被丢弃），生成器验证这一点后才折叠；
  否则不生成该 span（避免改变栈可观察性）。
- 回落：canonical（覆盖 String/BigInt 加法、BigInt、对象 `valueOf`、
  `SetLocal` 的保留语义）。
- 验收：`int_local` 每轮指令数；BigInt/字符串相关固定行不退化。

#### S3 `LocalFieldAddStore`（`s += o.a`）

- 形态：`producer(acc); producer(base); GetField(key); Add; store(acc)[; Drop]`。
- guard：`acc` 是 `Direct` 且 `Number`；`base` 是 `Direct` 且
  `JsValue::Object`；IC 命中且命中值为立即数 `Number`。
- 快路径：以非拥有方式取 IC 命中值（优先给
  `property_ic_read_fast` 增加 `property_ic_peek_immediate` 变体，
  返回 `Option<Number>`；现成兜底为
  `try_ordinary_field_immediate_read`，`ordinary_storage.rs:985`），
  `Number::add` 后写回。
- 回落：IC miss、非立即数值、getter/Proxy（`ordinary_receiver` 拒绝，
  `property_ic.rs:86-88`）一律 canonical。
- 注意：**不得**在 guard 阶段调用会 retain 的 `property_ic_read_fast`
  再丢弃 owner；peek 变体必须只读槽、不产生 owner 边。
- 验收：`prop_read` 每轮指令数；`prop_read`/`arguments_read` 固定行不退化。

#### S4 现有 `UpdateLocal`/`LocalAdd` 的不可失败快路径

- `UpdateLocal`（`run/fusion.rs:5-26`）当前经
  `update_number_local_current`（`stack/number.rs:47-98`）返回 `Result`；
  增加「binding 是 `Direct` 且为 Number」的前置快路径，直接
  `Number::update`（`operations.rs:93`）后写回，无 `Result`；其余走原路径。
- `LocalAdd`（现有 flag 128/129）同样加 Number 快路径。
- 验收：`empty_loop`/`int_local` 合并收益（S4 不单独立项。

#### 通用规则

- **producer 允许 `GetLocalCheck`**：guard 只接受 `Direct`；
  `Uninitialized` 时回落 canonical 以保留 TDZ 诊断。
- **captured 绑定**：guard 失败即回落，专用路径不碰 `read_run_cell`。
- **profiling**：命中时按 span 长度调用 `fusion::record_span`
  （`run/fusion.rs:80-87`），与现有 span 同一权重口径；不新增事件。
- **fault 发布**：专用路径无可观察操作，`ProgramCounter` 不变。
- **span 重叠**：新形态与现有形态都只在首 PC 置位；构建顺序保证
  同一 PC 只属于一个 span（先匹配更长的形态）。

### 4.3 B2.2 自适应 per-PC 专用槽（条件启动，PEP 659）

**启动门禁（须同时满足，否则记 backlog）**：

1. B2.1 至少一个微负载每轮指令数下降 ≥30%，且固定行门禁通过；
2. B2.1 后的残留 `perf annotate` 显示新材料成本来自「静态跨度无法覆盖的
   动态操作数/类型」或「guard 重复评估」，并有 ≥3% 指令的可归因缺口。

**数据结构**（复用 `PropertyReadCacheTable` 的位图 + rank 模式，
`property_ic.rs:360-407`）：

```rust
struct SpecializationTable {
    site_bits: Box<[u64]>,   // 静态候选 PC 位图（构建期生成）
    block_ranks: Box<[u32]>,
    sites: Box<[Cell<u64>]>, // 每候选站点 1 个 u64
}
```

- 候选 PC 在发布期按与 `FusionPlan` 相同的扫描生成；全函数无候选时
  不分配（`Option`/`OnceCell`）。
- `u64` 编码：tag(8) + 操作数位域（local u16 ×2、i32 常量等）；
  **不含任何 owning edge**，分支目标从 canonical 重读或 delta 编码。
- 挂在 `PublishedFunctionData`（`executable.rs:198-218`），与
  `property_read_ic` 并列，同函数所有闭包共享（`executable.rs:244-292`）。

**热计数与改写时机**：site 初值 0 = Generic。候选 PC 走 canonical 时
`count += 1`（`Cell` 读改写，无借用跨越）；`count` 达阈值（默认 16）
时按当次观察到的 binding 类/类型安装专用字。专用命中不计数。
canonical 路径只在候选 PC 付费，普通 PC 不变。

**去优化**：guard 失败时专用路径尚未产生副作用 → 同一 PC 立即执行
canonical，并把 site 置为 `Penalized{count}`；累计失败 ≥3 则 `Disabled`
（函数生命周期内不再特化），否则回 Generic 重计数。**禁止**在 canonical
通过后再尝试专用（B1 的 decline 二次分类税）。

**失效/GC/挂起**：形状失效沿用 IC 三件套（§3.2 第 5 条）；
site 不参与 GC 扫描；generator 恢复走 canonical PC + 同一
`PublishedFunctionData`，恢复后 guard 重新验证，无跨挂起状态。

**内存门禁**：沿用 B1b 的 `max(3%, 1MiB)`（`s3-c-b-next.md:419-434`），
并把候选站点数与 `size_of::<SpecializationTable>()` 单列入账。

**开关**：`oxide_specialize` 内部 cfg（非公共 API/CLI），默认关闭；
通过完整门禁后才讨论是否默认启用，默认构建在此之前不含该状态。

### 4.4 与现有机制的分工

| 机制 | 归属 | 边界 |
| --- | --- | --- |
| property IC | 不动 | 仍按 canonical PC 索引与失效；B2 只消费 `Option` 快路径 |
| 现有 `FusionPlan` 形态 | 保留 | UpdateLocal/LocalAdd/ConstLocalAdd/CompareBranch/AddStore/MethodCall 语义不变 |
| `AddStore`/`LocalAdd` 冷路径 | 不动 | 非 number 的转换与错误顺序仍归 conversion driver |
| native 调用编组链（map-int 22–30%） | **B2.3 定向子项** | 先写独立 spike（driver 选择复用、参数编组消除），不塞进 B2.1/B2.2 |
| C/TOS 缓存 | 已回退 | B 不与 C 耦合，不恢复 TOS facade |

### 4.5 设计问答（对应上一轮计划要求）

1. **热计数与改写时机**：B2.1 不设计数（形态静态、guard 每次重检，
   检查本身是寄存器级比较）；B2.2 按 §4.3，计数在 canonical 侧、
   安装在该 PC 的 canonical handler 末尾。
2. **专用编码与去优化**：B2.1 复用 u8 flag + canonical 操作数；
   B2.2 用 `Cell<u64>` 位域。guard 在任何副作用前；失败不重入、
   不半提交；B2.2 的降级策略见 §4.3。
3. **与 property IC 分工**：不复制、不重建 IC；S3 只以非拥有方式读 IC
   命中值；IC 仍是属性读的唯一缓存。
4. **可回退开关**：每片独立提交、独立回退；B2.1 按 span kind 门控
   （未验收 kind 不生成 flag）；B2.2 用 `oxide_specialize` 默认关闭。
5. **A/B 协议**：见 §6。

## 5. 实施切片与排程

| 切片 | 产物 | 依赖 | 关闭条件 |
| --- | --- | --- | --- |
| B2.0 | 证据冻结（§2 表格 + `target/s3-b-recon/`）；尺寸断言（`Instruction`==12、`RunExit`==16 编译期 `const` 断言；`Error`/`Result` 记为诊断指标不加断言）；`SpanKind` 单次查询重构实测不成立，已回退（§5.1） | 无 | 断言随 lib 测试编译；基线 A/A 复现 607.27/1355.27/1730.28 |
| B2.1a | S1 `LocalCompareBranch`（含 `Goto` 折叠） | B2.0 | `empty_loop` 每轮 ≤200；`ic_share_*` 不退化；Test262 绿 |
| B2.1b | S2 `LocalAddConstStore` + S4 UpdateLocal/LocalAdd 快路径 | B2.1a | `int_local` 每轮 ≤300；BigInt/字符串固定行不退化 |
| B2.1c | S3 `LocalFieldAddStore`（含 IC peek 变体） | B2.1b | `prop_read` 每轮 ≤450；属性固定行不退化 |
| B2.2 | 自适应 per-PC 专用槽（§4.3） | B2.1c + 启动门禁 | 启动门禁满足且内存门禁通过；否则记 backlog |
| B2.3 | native 编组链 spike（独立文档） | B2.1 冻结 | 给出定向方案与预估，不在本轮改代码 |
| B2.4 | 阶段文档收尾 + 默认路径裁决 | 全部 | 文档与 receipt 入库 |

排程：B2.0 → B2.1a → 裁决点（kill criterion）→ B2.1b/c → B2.2 门禁评估
→ B2.3 设计。所有测量串行（`taskset -c 2`），编码可并行。

### 5.1 B2.0 负结果：`SpanKind` 单次查询重构不成立

把 `update`/`local_add_span`/`const_add_span`/`compare_branch` 收敛为
`span(pc) -> Option<SpanKind>` 单次查询后，同一 fat-LTO 构建的每轮指令数：

| 负载 | 基线 | 重构后 | 变化 |
| --- | --- | --- | --- |
| `empty_loop` | 607.27 | 624.27 | **+2.8%** |
| `int_local` | 1355.27 | 1370.27 | +1.1% |
| `prop_read` | 1730.28 | 1735.28 | +0.29% |

`perf report` 显示 `SlotStore::parameter_current`（3.39% cycles）与
`SlotStore::push_current`（2.71%）被 out-line：`run` 的体量跨过 LLVM 内联
阈值，GetArg/操作数压栈从内联退化为调用。该重构违反 ±0.5% 门禁，已整体
回退（仅保留两个 `const` 尺寸断言）。**结论**：B2.1 不再预置统一解码层；
每个 kind 以「新增一个小 accessor + 臂内最小分支」增量加入；若新分支再次
触发同类内联翻转，优先用 `#[inline(always)]` 固定被 out-line 的既有小
helper，再评估。

## 6. 验收门禁

### 6.1 每片必跑

```bash
set -euo pipefail
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --all-targets
python3 scripts/checks/check-source-layout.py
```

Test262 全量在每片合并入默认路径前复跑，对照当轮冻结 receipt
（当前基线 `1ce1d4f2…`，pass 80010 / eligible 80060）；不得出现行级变化。

### 6.2 A/B 协议

- 构建：fat LTO + CGU=1、无 PGO/profiling（`cargo build --release
  -p quickjs-oxide-cli` 默认配置）；源码身份 = 提交哈希。
- 测量：`taskset -c 2`，5 样本中位；instructions 为主信号、cycles 为辅；
  比较对象是上一片提交的同一构建。
- 数据集：四个微负载（每轮指令数）+ D 的固定行
  （bigint32/64/256、scaling、V8、fixed 字符串簇）+ `a=a` 微负载 +
  `ic_share_1prop/2prop`。
- 归因：每片保留 `perf record -F 3997` + `perf annotate -l` 复测，
  确认成本迁移方向（`copy_value`/`Result` 占比应随之下降）。
- 不回退：固定行任一项劣化 >2% 时按 D §6 的 A/A 噪声规则复核后判定。

### 6.3 kill criterion

- B2.1a 若 `empty_loop` 每轮指令数下降 <15%，停止铺开 S2/S3，
  以负结果文档收口（B2.0 已证明预置统一解码层无收益，无需保留）。
- 任一 span kind 连续两轮有 profile 依据的调整仍不达标 → 回退该 kind，
  不阻塞其他。

## 7. 待裁决决策

1. **排序**：接受「B2.1 守卫式静态跨度先行、B2.2 自适应槽条件启动」，
   还是直接做 B2.2 可变槽？（推荐前者：先证明「删除可失败调用」的收益，
   再决定是否需要自适应状态。）
2. **首批集合**：S1/S2/S3 与 S4 是否同批纳入；S1 的 `Goto` 折叠是否首批
   （推荐：是，`empty_loop` 的收益主要来自这里）。
3. **错误通道尺寸**（独立可选项）：`Error` 实测 80B，是所有 `Result`
   走隐藏内存返回的直接原因（注意 `Result<JsValue, _>` 即使 `Error` 缩到
   8B 也仍是 24B、依旧内存返回，收益主要集中在
   `Result<(), _>`/`Result<bool, _>`/`Result<&T, _>` 这类 helper）。
   把 payload 装箱成 8B（`Box<ErrorData>` + 自定义 `Debug`）是一项与本
   计划正交、可能对所有负载生效的小切片（需自测 A/B，且触及公共 API
   表示）。是否单独立项？（推荐：立为 B2.5 独立 spike，不与 B2.1 混提。）
4. **B2.2 启动门禁**（§4.3）与失败降级策略（3 次禁用）是否接受。

## 8. 风险与回退

- **guard 漏检**：任何键落在引用类型上而未被 guard 拒绝 → 立即错误。
  缓解：每个 handler 的首行是类型穷举；`Direct` + 立即数双条件；
  差分测试覆盖 captured/TDZ/Proxy/getter/BigInt/字符串。
- **跨度误折叠**：`SetLocal` 无 `Drop` 却被折叠会改变栈。缓解：生成器
  显式验证尾随 `Drop` / 语义等价，否则不生成。
- **profiling 计数漂移**：新 span 必须调用 `record_span`，否则逻辑指令
  权重失真。缓解：`--features profiling` 下核对 span 计数测试。
- **性能回退**：新增匹配分支可能改变 `run` 布局（B1 的 `.text +1.04%`
  教训）。缓解：按 kind 独立提交、独立 A/B；单 kind 不达标即回退。
- **默认路径风险**：B2.2 自适应状态默认关闭；B2.1 无状态，但仍在默认
  构建中，所以每个 kind 都必须过完整门禁才保留。

## 9. 证据路径与复现

复现本轮归因：

```bash
cargo build --release -p quickjs-oxide-cli --locked
D=<前序工作区>/target/a4db-decision/micro            # 决策报告的负载目录
taskset -c 2 perf stat -e instructions,cycles target/release/qjs $D/int_local.js
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release -p quickjs-oxide-cli --locked
taskset -c 2 perf record -F 3997 -o /tmp/perf-int.data target/release/qjs $D/int_local.js
perf annotate -i /tmp/perf-int.data --stdio -l
```

本轮产物（gitignored）：`target/s3-b-recon/annotate-{int,empty_loop,prop_read}-src.txt`、
`target/s3-b-recon/attr.py`（按符号/源码行聚合采样）。

历史证据：[s3-c-negative-result.md](s3-c-negative-result.md)、
[s3-bc-implementation.md](s3-bc-implementation.md)、
[s3-full-rerun-results.md](s3-full-rerun-results.md)、
[s3-b-initial-plan.md](s3-b-initial-plan.md)（首批设计，已回退）。

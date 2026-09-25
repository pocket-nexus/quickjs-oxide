# 数值／数组跨度：冻结的实施规格 v1

> 设计基线：`6f09205c51f8b34e3c7a90ce406739fe1dd07c48`（PR #6，生产源码与 R0 `f531f605` 相同）。
> 本文取代 implementation.md 原 B0–B7 中“再选形态／意向接口”的部分。白名单、签名、接线位置和提交协议在此冻结；当前生产代码已实现全部 13 种形态，性能门禁尚未裁决。
> 已用 Rust 1.94.1 编译完整真实内核并取得四个目标函数的[发布后 dump 与 25 个站点 manifest](receipts/all-dense-6db6bfb0/README.md)。下面的符号序列仍是匹配器规范／源码推导，真实 PC 以 manifest 为准。静态站点不是动态覆盖或性能验收。

## 1. 已确定的产品边界

不重写 `Instruction`，不修改 canonical 指令、PC、异常表或恢复 ABI；`FusionPlan` 保持 `Option<Rc<[u8]>>`。只新增下表 13 种短跨度，最长 9 条。不引入每 PC descriptor、可变 QuickOp、通用 register IR、TOS facade 或 adaptive counter。

P2 实施 R0–R3；P3 实施 R4、A0–A3；P4 实施 W0–W3。这些是冻结的白名单，不是待实现者自由选择的方向。四函数里 A0–A3、W2、W3 静态站点为零，已记录而未擅自扩大语法；增加形态必须独立改规格、测试和 A/B。`this`、closure/global producer 本版明确不支持；B4 只记录其覆盖缺口，不安排未设计的 captured 写入。

### 1.1 Operand 的精确定义

| 记号 | 允许的 canonical 指令 | 发布期要求 | 运行期要求 |
| --- | --- | --- | --- |
| `L(x)` | `GetLocal(x)` 或 `GetLocalCheck(x)` | x 在 locals 内，`kind == Normal` | `FrameBinding::Direct`，拒绝 Captured/Uninitialized/空槽 |
| `P(x)` | `GetArg(x)` | 原发布验证器负责参数索引合法性 | 同样为 Direct；不能绕过现有 captured 参数臂 |
| `B(x)` | `L(x)` 或 `P(x)` | 同上 | 值为当前 Runtime 的 Object，进一步要求普通 dense Array |
| `K` | `PushI32(v)`；或 `PushConst(k)` 且 constants[k] 为 `Value(Int/Float)` | 常量索引及数值种类已验证 | 读取原 Int/Float 表示，不将 String/BigInt 当 Number |
| `N` | `L(x)`、`P(x)` 或 `K` | 同上 | `Number::Int/Float`；不做 ToPrimitive/ToNumber |
| `G(x)` | `L(x)` 或 `P(x)` | 用于下标自增／自减的绑定 | Direct Number |
| `PUT(x)` | local 对应 `PutLocal/PutLocalCheck`；argument 对应 `PutArg` | 必须与 G 的地址空间和编号相同；local 必须 `!is_const && kind == Normal` | 旧值仍是已验证的 Direct Number |
| `SET(x)` | local 对应 `SetLocal/SetLocalCheck`；argument 对应 `SetArg` | 同上；不得用 PUT 代替 SET | 同上 |
| `ACC(a)` | **仅** L(a) | 可写的 Normal local；本版不写 argument accumulator | Direct Number |
| `I` | `Add`、`Sub`、`BitAnd` | 精确枚举，不是任意 binary op | 只在 Number 域运行 |
| `A` | `Add/Sub/Mul/Div/BitAnd/BitOr/BitXor/Shl/Sar/Shr` | 精确枚举 | Number 运算；含现有 ToInt32 语义 |

最终下标 **只接受 `Number::Int(i)` 且 i≥0**，转换为 u32；Float（包括 -0、整数值 Float）、负数、String、BigInt 等均回落。本版不争取全部合法 JS property key，只保证接受域正确。`PushConst(Float)` 可参与算术并产生 Int；不靠 Rust `as` 把浮点数直接截断成数组下标。

本版不接受 `GetVarRef/GetVarRefCheck/PushThis/GetVar/GetField` 作为 producer，不跨 `Nop` 扫描，不包含 Call、getter、Proxy、ToPropKey、branch、catch、yield、await。`GetArrayEl2` 不在白名单；`GetArrayEl3` 仅允许 W3 的固定位置。

### 1.2 冻结的 flags、长度和栈契约

S 是进入首 PC 时已有的操作数前缀，下表不会读取或修改 S。peak 是 canonical 相对 S 的最高额外深度，不能只检查融合后净增长。

| flag / 名称 | 精确序列 | 长度 | peak | 结束栈 |
| --- | --- | ---: | ---: | --- |
| 1 / R0 Read | `B; N; GetArrayEl` | 3 | 2 | `S, result` |
| 2 / R1 ReadIndexBinary | `B; N; N; I; GetArrayEl` | 5 | 3 | `S, result` |
| 3 / R2 ReadPostUpdate | `B; G(x); PostInc/PostDec; PUT(x); GetArrayEl` | 5 | 3 | `S, result`，x 写新值 |
| 4 / R3 ReadPreUpdate | `B; G(x); Inc/Dec; SET(x); GetArrayEl` | 5 | 2 | `S, result`，x 写新值 |
| 5 / R4 ReadBinary | `B; N; GetArrayEl; N; A` | 5 | 2 | `S, result` |
| 6 / A0 AccPut | `ACC(a); B; N; GetArrayEl; Add; PUT(a)` | 6 | 3 | S，写 acc |
| 7 / A1 AccSetDrop | `ACC(a); B; N; GetArrayEl; Add; SET(a); Drop` | 7 | 3 | S，写 acc |
| 8 / A2 AccIndexPut | `ACC(a); B; N; N; I; GetArrayEl; Add; PUT(a)` | 8 | 4 | S，写 acc |
| 9 / A3 AccIndexSetDrop | `ACC(a); B; N; N; I; GetArrayEl; Add; SET(a); Drop` | 9 | 4 | S，写 acc |
| 10 / W0 Store | `B; N; N; Insert3; PutArrayEl; Drop` | 6 | 4 | S |
| 11 / W1 Copy | `B(dst); N; B(src); N; GetArrayEl; Insert3; PutArrayEl; Drop` | 8 | 4 | S |
| 12 / W2 StoreBinary | `B; N; N; N; A; Insert3; PutArrayEl; Drop` | 8 | 4 | S |
| 13 / W3 UpdateElement | `B; N; GetArrayEl3; N; A; Insert3; PutArrayEl; Drop` | 8 | 4 | S |

R2 取 old 作为下标，R3 取 `old.update(increment)` 作为下标；更新值暂存在 Rust 局部变量，读取成功和所有提交条件成立之前不写 x。PUT/SET 必须与表内 producer 同一绑定；不接受 `GetLocal(i); ...; PutArg(i)`。W0–W3 强制尾部 Drop，所以不消费仍被外部表达式使用的赋值结果；`a[i]=b[j]=v` 中内部赋值不会被误认为 W0。

**编号约束不能省略：**当前 `FusionPlan::update()` 用 `(flag & 16) != 0` 而不是精确区间识别旧 UpdateLocal。新 flags 1–13 均满足 `flag & 16 == 0`，且与 32–38、64/65、128–131、160–167 不相撞。本片不顺手改旧编码。测试遍历 1–13，断言所有旧 accessor（包括 update）均拒绝；14/15 暂不使用。

## 2. 真实源码对应、dump 与覆盖的边界

编译器证据固定在设计基线：`compiler/parser/expressions.rs::parse_member_suffix` 为 `[]` 发出 GetArrayEl；`lower_update_expression` 为 postfix 发出 PostInc/PostDec + Put，为 prefix 发出 Inc/Dec + Set；`emit_member_put` 对 computed assignment 发出 Insert3 + PutArrayEl；compound assignment 的 `promote_tail_member_get_for_compound` 发出 GetArrayEl3。后续名字解析决定 Local/Arg/VarRef，不能凭 JS 变量名推断编号或伪造 PC。

| 原始内核表达式 | 本版候选（符号化，非真实 dump） | 明确未覆盖部分 |
| --- | --- | --- |
| Crypto am3：`this_array[i] & 0x3fff` | R4：`L(this_array); P(i); GetArrayEl; K(0x3fff); BitAnd` | `this.array` 的初始化读取不在该跨度内 |
| Crypto am3：`this_array[i++] >> 14` | R2 处理到 GetArrayEl；随后的 `K(14); Sar` canonical | 不把 R2 与 R4 隐式拼成未编号长跨度 |
| Crypto am3：`w_array[j]` | R0：`L(w_array); P(j); GetArrayEl` | 乘法与 carry 链的跨表达式传播 |
| Crypto am3：`w_array[j++] = l & 0xfffffff` | RHS 可由既有 Number 路径执行 | 本版 W 不含 key-update；明确不融合整个赋值 |
| lin_solve：`x0[currentRow]`、`x[++currentRow]` | R0、R3；x/x0 是 argument 候选 | 多个数组读之间的整条求和链 |
| lin_solve：`x[currentRow] = x0[currentRow]` | W1 | 非直接 producer、跨语句写回 |
| project：`u[++nextValue] - u[++prevValue]` | 两个 R3，减法保持原顺序 canonical | `div[++currentRow]` 的写入 key-update 不在 W 白名单 |
| advect：`d0[i0 + row1]` | R1：`P(d0); L(i0); L(row1); Add; GetArrayEl` | 嵌套插值的整体重关联、FMA |

这些对应只支持“确有应覆盖的表达式与发射规则”，不支持静态条数、动态占比或性能数字。本版不提取内层函数重编译：必须对 pin `2034d98fc8c5f8044e186267593f5d5ea5232caf` 的**完整** crypto.js 和 navier-stokes.js 编译／发布，保住 FluidField 的闭包环境与参数布局。

完整探针见 [dump_numeric_spans.rs](probes/dump_numeric_spans.rs)，入口脚本见 [run_dump.py](probes/run_dump.py)。它在一个临时 detached worktree 里增加 test-only 模块，通过 `compile_in_realm → heap.function_bytecode` 递归读取发布后的 canonical 代码，记录目标函数全部 PC、constants、locals/args/closures 与 stack_contract；不运行 benchmark，不改公共 API。运行命令：

```sh
python3 docs/performance/probes/run_dump.py \
  --repo /absolute/quickjs-oxide \
  --source /absolute/js-engine-benchmark \
  --output /absolute/new-dump-directory \
  --rev 6f09205c51f8b34e3c7a90ce406739fe1dd07c48 \
  --toolchain 1.94.1
```

探针已完成 Rust 编译和真实运行；[收据](receipts/all-dense-6db6bfb0/README.md)包含四份完整函数内容、原始 PC 和生产 matcher 生成的 manifest。manifest 字段包括 source/function path、canonical 首末 PC、flag、slot/constant 编号、每条 opcode、peak/delta；R0 triad 的拒绝分类单独保存。不得由 JS 正则推算 manifest。

**B0 不再承担选设计的工作。**它验证上述冻结设计在真实输入上的实际覆盖；不匹配就报告具体断点并停止扩大，不把“先 dump 再决定 API”重新留给下一位实现者。

## 3. 类型与 API：完整声明及所有权

下面是本片已经实现的接口契约；设计基线没有这些新接口。生产代码没有另造未定义的 Value/NextPc。现有 `Number` 直接复用 `engine::value::number::operations::Number`，它是 `Copy` 的 Int/Float enum；通过既有 `From<Number> for JsValue` 写回，不新造数值表示。

### 3.1 编译期类型（code/fusion/dense.rs）

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectSlot { Local(u16), Argument(u16) }

#[derive(Clone, Copy, Debug)]
pub(crate) enum NumericSource { Slot(DirectSlot), I32(i32), Constant(u32) }

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DenseSpanKind {
    Read = 1, ReadIndexBinary = 2, ReadPostUpdate = 3,
    ReadPreUpdate = 4, ReadBinary = 5,
    AccPut = 6, AccSetDrop = 7, AccIndexPut = 8, AccIndexSetDrop = 9,
    Store = 10, Copy = 11, StoreBinary = 12, UpdateElement = 13,
}

// code/fusion/dense.rs 内；每种返回值的 len/peak/delta 必须等于 §1.2。
pub(super) fn candidate(
    rest: &[Instruction], locals: &[VariableDefinition],
    constants: &[BytecodeConstant], entries: &[bool],
) -> Option<DenseSpanKind>;

// code/fusion.rs 内；只在两个 producer 臂查询，不是全 opcode 前置层。
impl FusionPlan {
    #[inline]
    pub(crate) fn dense_span(&self, pc: usize) -> Option<DenseSpanKind> {
        DenseSpanKind::from_flag(self.flag(pc))
    }
}
```

DenseSpanKind 增加 `pub(crate) const fn len(self) -> usize`、`pub(crate) const fn peak(self) -> u8`、`pub(crate) const fn delta(self) -> i8` 和 `pub(crate) fn from_flag(flag: u8) -> Option<Self>`。code/fusion.rs 增加 `mod dense; pub(crate) use dense::{DenseSpanKind, DirectSlot, NumericSource};`，因此 VM 使用 `crate::engine::code::fusion::{...}`，不绕过私有子模块。DirectSlot/NumericSource 只作为构建／handler 栈内参数，不存储 per-PC descriptor，不持有 ObjectId、Number owner、Rc 或生命周期跨越的引用。

### 3.2 栈窗口 API（stack/window.rs，实现在 stack/number.rs）

```rust
#[derive(Clone, Copy, Debug)]
pub(in crate::engine::vm) enum NumericDestination { Push, Local(u16) }
#[derive(Clone, Copy, Debug)]
pub(in crate::engine::vm) struct NumberUpdate {
    pub slot: DirectSlot,
    pub value: Number,
}

impl<'frame> RunSlots<'frame> {
    pub(in crate::engine::vm) fn direct_value<'borrow>(
        &'borrow self, source: DirectSlot,
    ) -> Option<&'borrow JsValue>;

    pub(in crate::engine::vm) fn numeric_span_room(&self, extra_peak: u8) -> bool;

    pub(in crate::engine::vm) fn try_commit_number(
        &mut self, destination: NumericDestination, result: Number,
        update: Option<NumberUpdate>, extra_peak: u8,
    ) -> bool;
}
```

`stack.rs` 在现有 `pub(in crate::engine::vm) use window::{...};` 中追加 `NumericDestination, NumberUpdate`；`stack/number.rs` 从 `super` 导入它们，`run/fusion/dense.rs` 从 `crate::engine::vm::stack` 导入。所有新类型的可见性和使用路径至此明确。

`direct_value` 从 `window.locals()/parameters()` 用 get/as_ref 读取，只返回 Direct 的值借用；不 clone/retain，不把裸 ObjectId 当拥有者。返回生命周期是 `&self` 的短重借用 `'borrow`，**不是整个 `'frame`**；它存活时不能以可变方式提交／改写 RunSlots。不得把引用保存到 site cache 或 RunExit。

`numeric_span_room` 通过 checked_add 验证 `depth + peak` 不超过已认证 window，并检查对应 inactive 前缀为空；包括 R0 的 2、R2 的 3、W 的 4，而不是统一 +1。这样融合不能绕过测试构造的容量／空槽错误。

`try_commit_number` 在栈层完成一次全部预检：room、输出槽 None、目标／update 的旧 binding 为 Direct Number。Local destination 与 update 同时存在本版直接返回 false；该组合不在白名单。Push+update 则用 operands 边界的 `split_at_mut` 分开 binding 与输出，或等价的安全索引序列。所有可能返回 false 的检查在首次写入之前；其后只替换 Number、安装可选结果、修改 depth 及 profiling 的 live_slots/Move 计数。

它委托的新函数签名为：

```rust
impl SlotStore {
    pub(super) fn numeric_span_room_current(
        &self, window: &FrameWindow, extra_peak: u8,
    ) -> bool;
    pub(super) fn try_commit_number_current(
        &mut self, window: &mut FrameWindow,
        destination: NumericDestination, result: Number,
        update: Option<NumberUpdate>, extra_peak: u8,
    ) -> bool;
}
```

不调用 `push_current/replace_local_current/update_number_local_current` 再包一层；不返回 80B Error，也不生成跨层的宽 PreparedTransaction。既有 `immediate_local/immediate_parameter/store_number_local` 与 S1–S4 保持原样，避免影响 #44 的内联链。新提交 helper 可以 `#[inline]`，不全局 inline(always)；实际是否内联以 A/B 产物为准。

### 3.3 Runtime 叶 API（object/ordinary_storage.rs）

```rust
impl Runtime {
    pub(crate) fn peek_dense_number(
        &self, base: &JsValue, index: u32,
    ) -> Option<Number>;

    pub(crate) fn try_write_dense_number(
        &self, base: &JsValue, index: u32, value: Number,
    ) -> bool;
}
```

peek 的实现主体冻结如下（导入复用该模块已有类型）：

```rust
let JsValue::Object(id) = base else { return None; };
let state = self.0.state.try_borrow().ok()?;
let data = state.heap.object(*id).ok()?;
if !matches!(data.kind, ObjectKind::Array) { return None; }
match data.dense_array_value(index)? {
    RawValue::Int(value) => Some(Number::Int(*value)),
    RawValue::Float(value) => Some(Number::Float(*value)),
    _ => None,
}
```

返回 Number 不带 heap lifetime，`Ref<RuntimeState>` 在函数返回之前结束。保留 `heap.object` 的完整身份／generation 验证；其 HeapError 在这里表示未命中，最终诊断仍由 canonical 从原 PC 产生。peek 不使用 `operation()`、release-readiness、`try_borrow_mut`、`try_array_immediate_read_kind`、typed-array/arguments 分支或任何 retain/release。frame 借用保持真实 base owner 存活，所以不需要证明“随后释放这个临时 owner 是否会 drain”——本路径没有该临时 owner。

write 获取一次 `try_borrow_mut`，用 `heap.object_mut` 验证身份，要求 ObjectKind::Array + `ObjectPayload::Array { dense: Some(..) }`、索引在当前 dense 前缀内且旧元素为 Int/Float；通过后直接写同一槽的 RawValue::Int/Float。**不能调用** append、扩容、形状迁移、length 写入或一般 replace/cleanup 事务。dense 前缀“每项为默认属性的自有数据槽”是既有存储不变量，不新增稀疏 hole 表；descriptor/freeze 转慢后的对象必须拒绝。若这一不变量被后续源码修改，write 不得上线而必须追加显式 descriptor 验证。

现有不可写 length 不妨碍覆盖已有 writable 元素，不能误把 push 的 `writable_dense_length` 作为 overwrite 条件。加入 readonly element、freeze、seal、只读 length、原型 setter、Proxy 和同数组读写的专门测试。失配只返回 false；成功提交以后无 Result、无 owner release、无 cleanup/drain。

### 3.4 执行 API（run/fusion/dense.rs）

```rust
#[inline(never)]
pub(in crate::engine::vm::run) fn try_numeric_span(
    slots: &mut RunSlots<'_>, runtime: &Runtime,
    executable: &PublishedFunctionSnapshot, pc: usize,
    kind: DenseSpanKind, property_generation: &mut u64,
) -> Option<usize>;

fn read_numeric_source(
    slots: &RunSlots<'_>, executable: &PublishedFunctionSnapshot,
    source: NumericSource,
) -> Option<Number>;
fn index_from_number(value: Number) -> Option<u32>;
fn apply_number_binary(op: &Instruction, left: Number, right: Number) -> Option<Number>;
```

`Some(end_pc)` 表示完整提交；`None` 只是 guard miss，不能构造 `Error` 或改变 PC。不存在未定义的 NextPc/Value/error 类型。无新 RuntimeError/EngineError 穿过成功接口；既有 canonical/driver 的错误通道不变。借用冲突／槽空缺／非法索引等均是未提交的 miss，原路径决定正确错误位置与顺序，不把合法错误改为 panic。

`read_numeric_source` 的分支也固定，不另设转换回调：

```rust
match source {
    NumericSource::Slot(slot) => slots.direct_value(slot)?.as_number_repr(),
    NumericSource::I32(value) => Some(Number::Int(value)),
    NumericSource::Constant(index) => match executable.constant(index)? {
        BytecodeConstant::Value(RawValue::Int(v)) => Some(Number::Int(*v)),
        BytecodeConstant::Value(RawValue::Float(v)) => Some(Number::Float(*v)),
        _ => None,
    },
}
```

`index_from_number` 只做 `Number::Int(i) if i >= 0 => Some(i as u32)`，其余 None。`apply_number_binary` 精确复用 Number::add/sub/mul/div/int32；BitAnd/Or/Xor 用 int32 结果；Shl 用 i32::wrapping_shl(rhs.int32() as u32 & 31)，Sar 用有符号右移，Shr 先转 u32 再右移、以 `Number::compact(f64::from(result))` 返回。其余指令 None。禁止饱和 ToInt32、f64::mul_add 或重关联。不要把该 helper 注入旧 binary/S1–S4，避免给非目标指令改 codegen。

## 4. 逐函数改造与发布认证

| 函数／文件 | 唯一允许的改动 |
| --- | --- |
| `code/fusion.rs::FusionPlan::build` | 签名仍为 `(code, locals, constants)`；调用新 dense::candidate，优先选择已通过 entry 检查的候选；原 matcher/编码保留 |
| `code/fusion/dense.rs::candidate` | 实现 §1 表，最长优先；对每个候选检查整个 `entries[1..len]`，失败继续找更短候选 |
| `FusionPlan::dense_span` | `DenseSpanKind::from_flag(self.flag(pc))`；一次 u8 查询，无统一 span facade |
| `run/fusion.rs` | `mod dense; pub(super) use dense::try_numeric_span;`；既有 update_local、compare_branch、numeric_local_add、numeric_local_field_add 的签名／body／注解不变 |
| `run/fusion/dense.rs` | 单次 match kind，读取该长度内明确位置的 canonical enum operand；实现 read/index/update/acc/store 的标量数据流 |
| `stack.rs` | 仅增加两个 Number 提交类型的 re-export；旧 helper body 不变 |
| `stack/window.rs` | 新 direct_value、room、commit 薄转发；不迁移原 RunSlots 字段，不修改 FrameTransaction ABI |
| `stack/number.rs` | 实现 room 与一次性 Number 提交，按实际写入维护 profiling；不改既有 S4 |
| `object/ordinary_storage.rs` | 新共享数值读取和已有数值槽写入叶函数；旧 array-immediate 族保留为回落 |
| `vm/run.rs::run` | 只在 GetLocal/GetLocalCheck 和未被 captured guard 接走的 GetArg 两臂增加入口；W 成功同步 property_generation |
| `code/executable.rs`、发布器、Instruction、driver | **不变**；现有 bytecode.fusion 发布/共享已覆盖新 flags |

新增 matcher 的入口必须先按 `rest.first()` 拒绝非 GetLocal/GetLocalCheck/GetArg，避免对普通 PC 执行 13 次完整匹配。`candidate` 的 entries 参数是**从该 PC 起的相对切片**，不是全函数表；接入点为：

```rust
let dense_candidate = dense::candidate(rest, locals, constants, &entries[pc..])
    .map(|kind| (kind as u8, kind.len()));
let candidate = dense_candidate.or(old_candidate);
// old_candidate 是原 local_compare.or(method)... 的完整结果，语义不改。
// 沿用后面的 flags.resize/code.len/flags[pc] 发布；不另建数组。
```

新 matcher 内对每个长度执行 `entries.get(1..length)?` 检查后才返回；外层既有 entry 检查仍可保留，新 flags 不享受 flag34 的特殊处理。NumericSource/DirectSlot 的解码只匹配表内固定位置的 Instruction enum 字段，无位域 word 解码，无通用 bytecode evaluator。

构建器优先级冻结为：A3/A2/A1/A0 → W1/W2/W3/W0 → R4 → R2/R3/R1 → R0 → 原候选链。先验证一个长候选的 entries，再考虑选择，**不能先 `.or` 选中长候选、最后检查失败就把同 PC 可行的短候选丢掉**。只有旧 S1 flag34 的 Goto 保留原有特殊 entry 豁免；新跨度无任何豁免。

发布期以 `Instruction::stack_contract()` 从相对深度 0 执行每个候选，断言无下溢、peak/delta 与 §1.2 一致；任何失败不发布新 flag。写 target 的 local metadata 必须可写 Normal；参数写仅来自已验证 PutArg/SetArg 并在运行期拒绝 Captured。构建阶段不猜类型、不执行用户程序。

不同 PC 的跨度区间可以重叠：长跨度命中就跳过其内部 PC，未命中则 canonical 继续并允许内部短跨度之后独立尝试。只要求每个首 PC 有一个 flag，不全局清空内部 flag。编译字节码不做重写，不增加第二 PC 空间。

## 5. run 接线与数据流提交

两个臂使用同一片接线模板（pc 是现有 ProgramCounter 的本地工作状态）：

```rust
if let Some(kind) = executable.fusion.dense_span(pc.fault) {
    if let Some(end) = fusion::try_numeric_span(
        &mut slots, runtime, executable, pc.fault, kind,
        &mut frame.property_generation,
    ) {
        #[cfg(feature = "profiling")]
        fusion::record_span(&executable.code[pc.fault..end], observed_depth);
        pc.resume = end;
        continue;
    }
}
```

GetLocal 入口放在现有 S1/S2/S3/S4 块之后、`match slots.local(*index)?` 之前；GetArg 放在现有最终 `Instruction::GetArg(index)` 臂中、parameter/copy/push 之前。前面带 Captured guard 的参数臂保持原顺序。profiling 下在那里可额外记录 eligible-but-captured，普通构建不加分支。不能在每轮总 match 前检查，也不能把这片检查迁移到旧 S3 内。

### 5.1 R0 的完整边界

```rust
// handler 已从 kind 和 code[pc..pc+3] 解出 base_slot 与 key_source。
let key = index_from_number(read_numeric_source(slots, executable, key_source)?)?;
let value = {
    let base = slots.direct_value(base_slot)?;
    runtime.peek_dense_number(base, key)?
}; // base 的短借用和 Runtime 的 heap 借用均已结束
if !slots.try_commit_number(NumericDestination::Push, value, None, kind.peak()) {
    return None;
}
Some(pc + kind.len())
```

它可以先做无副作用的 key／heap 检查再做 room：任何失败都会从首 PC 重放原 canonical；没有提前抛错、getter 或状态改变。room 不通过时不能仍返回成功。不要把结果重新传给 `array_immediate_read_current`，否则目标工作没有被删除。

### 5.2 R1–R4 与 A0–A3

R1 先以 Number::add/sub/int32 算出 key，再与 R0 相同；R4 读到 Number 后读取尾部 N、以 A 计算，最后只 push 一个 Number。R4 的 RHS 只读直接标量，所以提前／延后准入不会调用转换或 getter。

R2/R3 读取 old，计算 next，选择 old/next 作为 key；`NumberUpdate {slot, value: next}` 暂存。peek、输出 room 或任何类型 guard 失败，x 保持 old。只有 `try_commit_number(Push, result, Some(update), peak)` 一次提交 x 与输出。getter/hole 测试必须看到 canonical 自增一次，不能“先增加再失败回首 PC”。

A0–A3 读取 acc、base、index，只在两个加数都 Number 且 acc 仍为 Direct Number 时调用 `try_commit_number(Local(a), acc.add(value), None, peak)`。不 push/pop 中间值，不返回持有 old owner 的 FrameBinding。acc 与 index 同槽合法：读完旧值后唯一写回；acc 与 base 同槽无法同时为 Number/Object，应 miss。

### 5.3 W0–W3

先 `numeric_span_room(peak)`，再 `property_generation.checked_add(1)`；任一失败直接 None。随后完成全部 producer/Number/key 读取。W1 先读取源元素；允许 src==dst 和相同索引，因为 Number 已复制且共享 heap 借用已结束，之后才借可变 heap。W2 按原左到右 A 运算；W3 先读旧元素、再读 RHS、最后执行同一 A。

最后一次可失败动作是 `try_write_dense_number(base, key, result)`。返回 false 时没有写过堆。返回 true 后只能执行 `*property_generation = next_generation`、记录已完成事件并返回 Some(end_pc)；不能再调用 slots.push、retain/release、分配、可失败转换或其他 guard。

PutArrayEl 在 run 中原本递增 property_generation；融合跳过 opcode 不代表可以跳过它。新写叶不会修改 shape/layout revision/length，因为只覆盖已有 Number 数据值。若任何新写形态需要改变布局，必须另案设计失效协议，不混入 W0–W3。

## 6. 错误、生命周期及回退的不变量

运行前后只允许两种状态：None 时 frame slots、depth、owner、heap、property_generation、fault/resume 都与进入前相同；Some 时与整个 canonical 跨度执行完毕相同。所有成功操作均为不可观察的 Number 读写或已存在的自有数值槽写入；不合成 JS 异常。内部故障／资源错误由原路径处理。

`&JsValue` 与任何 `Ref/RefMut<RuntimeState>` 不得进入 site、返回值、driver、generator 保存状态或闭包。读取函数只输出 Copy Number；no-retain 是因为真实 frame owner 仍在，不是“ObjectId 是 Copy 所以它会活着”。不省 generation，不改变 foreign Runtime 信任模型；入口只接受与 executable/FrameTransaction 同域的内部值，公共 API 不可调用此快路径。

read/acc/update 的所有 mutable slot 操作都在唯一 commit 内；W 的唯一 heap 提交之后再无可失败步骤。新形态不得跨越已提交状态后重新从跨度首 PC 回落。需要多次 store 或捕获写的长块不在本版范围。

## 7. 必须落实到测试名称的验收

| 测试 | 必须断言 |
| --- | --- |
| dense_flags_do_not_alias_legacy_accessors | 1–13 唯一、bit16 清零；旧 UpdateLocal/Compare/Add/Method accessor 全拒绝 |
| dense_manifest_matches_stack_contracts | 13 行的长度/peak/delta 用生产 stack_contract 复核，而非另一份手写表 |
| dense_candidate_prefers_valid_longest | 长候选内部跳转被拒绝后仍能选合法短候选；每个内部 PC 含尾 Drop 的 entry 均拒绝 |
| dense_producer_and_store_whitelist | Local/Check/Arg/K 的正例；VarRef/this/field、错空间 PUT、const、TDZ 等负例 |
| dense_read_preserves_single_owner | receiver strong=1 也可非拥有读；计数、identity 和延迟队列不变化 |
| dense_update_miss_is_not_committed | postfix/prefix 的 hole、getter、non-number、capacity miss，x 和 frame 未预改；canonical getter 只见一次更新 |
| dense_peak_capacity_not_net_delta | 给 R0/R2/A3/W3 分别少一个 canonical peak 槽；必须原地 miss，不绕过容量错误 |
| dense_numeric_semantics | Int 溢出、-0、NaN/Infinity、移位 0/31/32/负数、uint32 大结果；与旧 Number/完整 canonical 一致 |
| dense_write_metadata_and_alias | generation +1、overflow 原地 miss；src==dst、同索引、readonly/freeze/length、prototype setter 和 Proxy |
| dense_kept_assignment_and_compound | W 强制 Drop；赋值结果被使用时不误融合；GetArrayEl3 只在 W3 接受 |
| dense_canonical_failure_position | 非法内部 slot、getter throw、conversion throw、try/finally 的顺序和 fault PC 不变 |
| dense_real_kernel_sites | 四函数完整发布后 dump、真实 PC manifest，明确覆盖数与拒绝分类；无站点不得假通过 |

测试模式增加 canonical-only 对照只控制新候选的生成，不能改 JS body；保留旧融合用于 Base/Parent 比较。生产构建不保留每操作调试 toggle。

原分片计划要求 P2 先 R0、再 R1、再 R2/R3，P3 后接 R4 与 acc，P4 最后接 store。当前实现已在全部 handler 完成后发布 13 个 flag；[真实 manifest](receipts/all-dense-6db6bfb0/README.md)记录零覆盖形态。[四方正式测量](receipts/fourway-2026-09-25/README.md)记录动态命中和净效果。性能未过门或不命中只记录和归因，不扩大到任意“numeric block”。

## 8. 固定源码入口与本次验证

源码依据均固定到设计基线，而非会变化的 main：

- [FusionPlan/build/accessors](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/code/fusion.rs)：entry 规则、flags 与旧 accessor。
- [表达式发射规则](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/compiler/parser/expressions.rs)：prefix/postfix、GetArrayEl3、Insert3/PutArrayEl。
- [RunSlots](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/vm/stack/window.rs)、[Number 提交](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/vm/stack/number.rs)、[数值语义](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/value/number/operations.rs)。
- [现有数组叶路径](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/object/ordinary_storage.rs)、[dense 前缀存储](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/heap/object_storage.rs)、[run 接线](https://github.com/Eric-Song-Nop/quickjs-oxide/blob/6f09205c51f8b34e3c7a90ce406739fe1dd07c48/src/engine/vm/run.rs)。
- [完整 Crypto 源码](https://github.com/ahaoboy/js-engine-benchmark/blob/2034d98fc8c5f8044e186267593f5d5ea5232caf/v8-v7/crypto.js)、[完整 NavierStokes 源码](https://github.com/ahaoboy/js-engine-benchmark/blob/2034d98fc8c5f8044e186267593f5d5ea5232caf/v8-v7/navier-stokes.js)。尤其 project 使用的是前缀索引更新，不应把等价手写 `row±1` 当原始语料。

### 本次设计验证与剩余实测

已核对源代码接口、符号序列的发射规则、flags 冲突、13 行独立栈代数和父 PR 身份。Python runner 通过语法／help 检查，以及模拟编译器的完整输出、零匹配、缺目标、编译失败四种工作流测试；四种均检查临时 worktree 清理和原工作区不变。这不是生产 stack_contract 测试或真实 Rust capture；Rust 探针只做源码 API 对照。本环境无 cargo/rustc，未执行 probe、Rust signature compile、Test262、性能或完整四函数 dump。

因此本版关闭的是“白名单／API／函数级改造待决定”，不是把不存在的运行结果填进计划。真实 dump、生产 matcher manifest、签名编译测试和每片 A/B 仍是精确、可执行的验收工件；不得以本文档替代这些工件。

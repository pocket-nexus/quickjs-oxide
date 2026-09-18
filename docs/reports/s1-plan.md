# S1 计划：可信快路（trusted fast path）

本文是 S1 的实现计划，尚未实施。S1 只改**热路径的开销**，不改任何 JS
可观察行为，也不换 GC 模型（继续 plain RC + 循环回收）。

## 1. 目标与非目标

**目标**：消除 S0 定位到的三大开销来源，即热路径上的

1. `Result` / `Option` 管道分支；
2. 全局 `RefCell<RuntimeState>` 的**可变**借用（`try_borrow_mut`）；
3. 可避免的 `validate_slot_identity` 校验。

**非目标**：

- 不删除 `Value` 的 runtime `Rc`（属于 S3 值瘦身）。
- 不改 GC 回收时机（保持即时 RC）。
- 不为 public API 或未可信输入放宽错误返回。
- 不处理 Symbol 的 atom 表可变借用（S1 走回退路径，见 4.5）。

## 2. 热点路径与证据（来自 S0）

S0 测得 `prop_read_int` 的一次循环迭代：

| 步骤 | 调用链 | 成本 |
| --- | --- | ---: |
| 读捕获对象 `o` | `read_run_cell` → `read_immediate_cell`(shared，decline) → `try_read_owned_var_ref`(`try_borrow_mut`) | 6.47%（其中 `Result::map` 6.29%） |
| 读属性 `o.a` | `stack::property_ic_read_current` → `try_property_ic_read_owned`(`try_borrow_mut`) | int 11.36% / obj 24.98%（`Result::branch` 12.54%） |
| 写立即数 `s`/`i` | `try_write_immediate_cell` → `try_replace_immediate_var_ref_value`(`try_borrow_mut`) | 6.92% |
| `run` 内联 | `Result::branch` + `copy_value` | 14.85% + 7.96% |

结论：主导成本是 **`Result`/`Option` 分支**，其次是可变借用与校验，而不是
引用计数本身。

## 3. 根因

热路径每一步都走可失败、需要 `&mut` 的通用接口：

- `Heap::live_node` / `live_node_mut` / `validate_slot_identity` 返回 `Result`；
- `Node.strong` 是裸 `u32`，retain 需要 `&mut`；
- `Runtime::retain_raw_root` 返回 `Result`（Object 走 heap retain，Symbol 走 atom retain）；
- `Runtime::take_owned_raw_value` 返回 `Result`（拒绝内部哨兵）；
- 于是 `try_*` 层层用 `Result<Option<_>>` 包裹，每次调用一个分支。

这些 `Result` 绝大多数是**防御性不变量**（stale handle / refcount overflow /
内部哨兵），在「句柄已由活对象持有」的热路径上不可能发生。

## 4. 设计：可信快路

核心原则：**内部已证明存活的句柄走不失败、可共享借用的快路；通用可失败
接口保留给边界与冷回退。**

### 4.1 refcount 改为 `Cell`

- `Node { strong: u32, data }` → `Node { strong: Cell<u32>, data }`
  （`src/engine/heap/mod.rs`）。
- `Heap::retain_raw(&self, id)` 改为 `&self` 且不返回 `Result`；溢出按
  QuickJS 语义**饱和为 immortal**（`u32::MAX`），`debug_assert` 记录异常。
- `Heap::retain_object(&self, id)` 同步改为 `&self` / `()`。
- release 的「归零入队」仍需 `&mut`，保持现状：`release_raw_no_drain(&mut self)`。
- 更新全部 `.strong` 读写为 `.get()`/`.set()`（`gc.rs`、`arena.rs`、
  `slot_ownership.rs`、`roots.rs`、`mod.rs`，共约 32 处）。

### 4.2 可信访问器（无 `Result`、无 generation 校验）

在 `Heap` 上新增仅供**内部已证明存活句柄**使用的访问器：

```rust
impl Heap {
    /// Trusted: caller holds a live owning edge. Panics on heap invariant
    /// violation; release builds elide the generation check.
    #[inline]
    pub(in crate::engine::heap) fn live_node_fast(&self, id: RawId) -> &Node {
        let index = id.index() as usize;
        debug_assert!(self.validate_slot_identity(id).is_ok());
        match &self.slots[index].state {
            SlotState::Live(node) => node,
            _ => unreachable!("trusted handle reached a non-live slot"),
        }
    }
    #[inline]
    pub(in crate::engine::heap) fn var_ref_fast(&self, id: VarRefId) -> &VarRefData { ... }
    #[inline]
    pub(in crate::engine::heap) fn object_fast(&self, id: ObjectId) -> &ObjectData { ... }
}
```

保留现有 `live_node`/`validate_slot_identity` 作为边界与测试路径。

### 4.3 无失败根转换

- 新增 `Runtime::take_owned_raw_value_fast(&self, raw: RawValue) -> Value`：
  仅接受公开变体（Undefined/Null/Bool/Int/Float/BigInt/String/Symbol/Object），
  内部哨兵走 `debug_assert` + panic。已有逻辑见
  `src/engine/heap/roots.rs:345`。
- 新增 `Runtime::retain_object_root_fast(&self, id: ObjectId)`：直接调
  `heap.retain_object`（已是 `&self`），不返回 `Result`。

### 4.4 快路函数与调用点改写

新增（不删除原函数，原函数作为冷回退）：

- `Runtime::read_owned_cell_fast(&self, root) -> Option<Value>`
  - 共享借用 `state.try_borrow()`；
  - 用 `heap.var_ref_fast` 读单元；
  - 仅处理 Object/String/BigInt（String/BigInt 克隆自带所有权，Object 用
    `retain_object_root_fast`）；
  - Symbol 与标量 → `None`，回退原 `try_read_owned_var_ref`。
- `bindings::read_run_cell_fast(runtime, root) -> Option<Value>`
  - `read_immediate_cell`（现有，shared）→ else `read_owned_cell_fast`。
- `Runtime::property_ic_read_fast(base, executable, pc, key, keep_receiver, native) -> Option<Value>`
  - 复制 `try_property_ic_read_owned` 的逻辑（`ic.rs:12`），但：
    - 共享借用；
    - `heap.object_fast`、`cache.read`、`slot_object_release_readiness` 均为 `&self`；
    - Object/String/BigInt 用可信 retain；
    - Symbol 或需要 atom 变更的情形 → `None` 回退。

调用点改写：

- `src/engine/vm/bindings.rs:64` `read_run_cell` 优先调用 fast 版本。
- `src/engine/vm/run.rs` 的 `GetVarRef`/`GetArg` captured 分支改用
  `read_run_cell_fast`，去掉 `?`。
- `src/engine/vm/stack.rs:114` 与 `src/engine/object/ordinary_storage/ic.rs`
  的属性读调用点改用 `property_ic_read_fast`，失败再走原可失败路径。
- `try_replace_immediate_var_ref_value` 保持 `bool`；去掉其中可省略的
  `validate_var_ref_value` 重复调用（同一谓词已在前面判过），并优先用
  `var_ref_fast_mut`。

### 4.5 Symbol / 立即写的原因与边界

- **Symbol**：`AtomTable::retain` 需要 `&mut`（atom 表可变），无法在共享借用
  下完成。S1 让 Symbol 读回退到现有 `try_read_owned_var_ref`。若后续需要，
  可将 atom 的 `ref_count` 也做成 `Cell`（S1b，暂不做）。
- **立即写**（`i`/`s`）：要修改 `VarRefData.value`（`RawValue`，非 `Copy`），
  仍需要 `&mut`。S1 只降低其校验与 `Result` 开销，不消除可变借用；彻底消除
  需要把单元值改成 `RefCell<RawValue>`（S1b）。

## 5. Scope 清单

| 文件 | 改动 |
| --- | --- |
| `src/engine/heap/mod.rs` | `Node.strong: Cell<u32>`；`SlotState::strong` 读写 |
| `src/engine/heap/gc.rs` | `retain_raw` 改 `&self`/饱和；trial/finalize 读 `.get()` |
| `src/engine/heap/arena.rs` | 新增 `live_node_fast` 等可信访问器 |
| `src/engine/heap/slot_ownership.rs` | `.strong.get()` |
| `src/engine/heap/roots.rs` | `take_owned_raw_value_fast`；`read_owned_cell_fast` |
| `src/engine/heap/binding_storage.rs` | 精简 `try_replace_immediate_var_ref_value` |
| `src/engine/heap/runtime/mod.rs` | `retain_object_root_fast` |
| `src/engine/vm/bindings.rs` | `read_run_cell_fast` |
| `src/engine/vm/run.rs` | captured 读调用点 |
| `src/engine/vm/stack.rs` | 属性读调用点 |
| `src/engine/object/ordinary_storage/ic.rs` | `property_ic_read_fast` |
| 若干 `heap/tests.rs` | `.strong` 测试适配 |

## 6. 算法草图

```rust
// heap/gc.rs
#[inline]
pub(super) fn retain_raw(&self, id: RawId) -> () {
    let node = self.live_node_fast(id);
    let next = node.strong.get().saturating_add(1);
    debug_assert!(next != u32::MAX, "refcount saturated to immortal");
    node.strong.set(next);
}

// heap/roots.rs
#[inline]
pub(crate) fn read_owned_cell_fast(&self, id: VarRefId) -> Option<Value> {
    if self.0.deferred_references.has_pending() { return None; }
    let state = self.0.state.try_borrow().ok()?;      // shared, 非 mut
    if !state.heap.zero_queue.is_empty() { return None; }
    let cell = state.heap.var_ref_fast(id);
    match &cell.value {
        RawValue::Object(object) => { state.heap.retain_object(*object); Some(Value::Object(ObjectRef::from_owned_handle(self.clone(), *object))) }
        RawValue::String(s) => Some(Value::String(s.clone())),
        RawValue::BigInt(b) => Some(Value::BigInt(b.clone())),
        _ => None,                                     // Symbol/标量回退
    }
}

// object/ordinary_storage/ic.rs
#[inline]
pub(crate) fn property_ic_read_fast(...) -> Option<Value> {
    let atom = linked_field_atom(self, executable, key_index)?;
    let Value::Object(object) = base else { return None };
    if !object.belongs_to(self) { return None; }
    let cache = executable.property_read_ic.site(pc)?;
    let state = self.0.state.try_borrow().ok()?;
    if state.heap.has_pending_zero_cleanup() { return None; }
    let receiver = object.object_id();
    let raw = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)?.clone();
    match raw {
        RawValue::Object(id) => { state.heap.retain_object(id); Some(Value::Object(ObjectRef::from_owned_handle(self.clone(), id))) }
        RawValue::String(s) => Some(Value::String(s)),   // Rc clone 已在 clone() 中
        RawValue::BigInt(b) => Some(Value::BigInt(b)),
        _ => None,
    }
}
```

（`keep_receiver` / native 选择等分支保留在快路内，否则回退原函数。）

## 7. 验证与度量

1. `cargo fmt --check`、`cargo clippy --all-targets -D warnings`。
2. workspace 测试 + Test262 冻结向量（pass/eligible 不得倒退）。
3. 重跑 `scripts/benchmark/property_read_probe.py`，与 S0 基线对比。
4. 对比 profiling 构建的 `heap_root_copies` 计数，确认复制次数未异常变化。

## 8. 风险与待确认

- **失败模式改变**：可信快路对 stale/哨兵改为 panic（debug_assert + release
  panic）。这是把「不变量破坏」当 bug 处理。需确认可接受。
- **refcount 饱和**：溢出改为 saturate-immortal，与 QuickJS 的 immortal 语义
  一致，但需确认不影响现有溢出测试的预期。
- **共享借用冲突**：`try_borrow()` 与任何活动 `borrow_mut` 冲突时快路返回
  `None` 回退，行为与现有 `try_borrow_mut` 回退一致。
- **S1b 待定**：Symbol atom 的 `Cell` 化、`VarRefData.value` 的 `RefCell` 化
  是否纳入 S1 或另开阶段。

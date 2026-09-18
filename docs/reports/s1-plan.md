# S1 计划：可信快路（trusted fast path）

本文是 S1 的实现计划，尚未实施。S1 只改**热路径的开销**，不改任何 JS
可观察行为，也不换 GC 模型（继续 plain RC + 循环回收）。

## 0. 已决定的取舍

| 项 | 决定 |
| --- | --- |
| 可信快路遇到 stale handle / 内部哨兵 | **panic**（`debug_assert` + release panic），视为不变量被破坏的 bug |
| 可信快路的 refcount 溢出 | **饱和为 immortal**（`u32::MAX`），不返回错误 |
| 现有可失败 `retain_*` 的溢出 | **保持返回 `Err`**，签名与语义不变（不动现有溢出测试） |
| S1b（Symbol atom `Cell` 化、`VarRefData.value` `RefCell` 化） | **留到 S1 验证后再做** |

关键点：**快路与通用可失败路径的溢出行为不同**——快路饱和、通用路径报错。
快路只用于计数很小的已证明存活句柄，饱和在实践中不可达，因此
`src/engine/heap/tests.rs` 里 `retain_edges_transactionally` 的溢出用例无需改动。

## 1. 目标与非目标

**目标**：消除 S0 定位到的三大开销来源，即热路径上的

1. `Result` / `Option` 管道分支；
2. 全局 `RefCell<RuntimeState>` 的**可变**借用（`try_borrow_mut`）；
3. 可避免的 `validate_slot_identity` 校验。

**非目标**：

- 不删除 `Value` 的 runtime `Rc`（属于 S3 值瘦身）。
- 不改 GC 回收时机（保持即时 RC）。
- 不为 public API 或未可信输入放宽错误返回。
- 不改 Symbol 的 atom 表可变借用（S1 走回退路径，见 4.5）。
- 不改现有可失败接口的签名与失败语义。

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

### 4.1 refcount 改为 `Cell` + 新增快路 retain

- `Node { strong: u32, data }` → `Node { strong: Cell<u32>, data }`
  （`src/engine/heap/mod.rs`），`SlotState::strong()` 用 `.get()`。
- **现有** `Heap::retain_raw(&mut self, id, additional) -> Result` 及其所有 caller
  **签名与错误语义不变**，只把字段读写改成 `.get()/.set()`。溢出仍返回 `Err`
  （保住 `retain_edges_transactionally` 的现有测试）。
- **新增** 快路：
  ```rust
  #[inline]
  pub(in crate::engine::heap) fn retain_raw_fast(&self, id: RawId) {
      let node = self.live_node_fast(id);
      node.strong.set(node.strong.get().saturating_add(1));
  }
  ```
  仅供快路使用，饱和后不可回退（immortal）。
- release 的「归零入队」仍需 `&mut`，`release_raw_no_drain(&mut self)` 保持不变；
  release 读/写 `Cell` 用 `.get()/.set()`，`u32::MAX` 视为 immortal（不递减）。
- 更新全部 `.strong` 直接字段访问（`gc.rs`、`arena.rs`、`slot_ownership.rs`、
  `roots.rs`、`mod.rs`、`tests.rs`，约 32 处）。

### 4.2 可信访问器（无 `Result`、省 generation 校验）

在 `Heap` 上新增仅供**内部已证明存活句柄**使用的访问器：

```rust
impl Heap {
    /// Trusted: caller holds a live owning edge. Bounds-checked index; the
    /// generation/state check runs in debug builds only.
    #[inline]
    pub(in crate::engine::heap) fn live_node_fast(&self, id: RawId) -> &Node {
        debug_assert!(self.validate_slot_identity(id).is_ok());
        match &self.slots[id.index() as usize].state {
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

保留现有 `live_node`/`validate_slot_identity` 作为边界与测试路径。注意 release
构建仍会做 `Vec` 越界检查（安全 Rust，不用 `get_unchecked`）。

### 4.3 无失败根转换

- 新增 `Runtime::take_owned_raw_value_fast(&self, raw: RawValue) -> Value`：
  仅接受公开变体（Undefined/Null/Bool/Int/Float/BigInt/String/Symbol/Object），
  内部哨兵走 `debug_assert` + panic。逻辑复制自 `src/engine/heap/roots.rs:345`。
- 新增 `Runtime::retain_object_root_fast(&self, id: ObjectId)`：调用于共享借用
  下的 `heap.retain_raw_fast(RawId::Object(id))`，不返回 `Result`。

### 4.4 快路函数与调用点改写

新增（原函数保留为冷回退）：

- `Runtime::read_owned_cell_fast(&self, root) -> Option<Value>`
  - `state.try_borrow()`（共享）；
  - `heap.var_ref_fast` 读单元；
  - 仅处理 Object/String/BigInt（String/BigInt 克隆自带所有权，Object 用
    `retain_object_root_fast`）；
  - Symbol 与标量 → `None`，回退 `try_read_owned_var_ref`。
- `bindings::read_run_cell_fast(runtime, root) -> Option<Value>`
  - `read_immediate_cell`（现有，shared）→ else `read_owned_cell_fast`。
- `Runtime::property_ic_read_fast(base, executable, pc, key, keep_receiver, native) -> Option<Value>`
  - 复制 `try_property_ic_read_owned`（`ic.rs:12`）的**数据属性命中**分支：
    - 共享借用；`heap.object_fast` / `cache.read` / `slot_object_release_readiness` 均 `&self`；
    - Object/String/BigInt 用可信 retain；
    - `keep_receiver` / native 选择 / accessor / 描述符等情形 → `None` 回退原函数。

调用点改写：

- `src/engine/vm/bindings.rs:64` `read_run_cell`：先 `read_immediate_cell`，再
  `read_owned_cell_fast`，最后才回退 `try_read_owned_var_ref`；不再包 `Result`。
- `src/engine/vm/run.rs` 的 `GetVarRef`/`GetArg` captured 分支改用
  `read_run_cell_fast`，去掉 `?`。
- `src/engine/vm/stack.rs:114` 与 `src/engine/object/ordinary_storage/ic.rs`
  的属性读调用点：先 `property_ic_read_fast`，`None` 再走原可失败路径。
- `try_replace_immediate_var_ref_value` 保持 `bool`；删除其中与前置判断重复的
  `validate_var_ref_value` 调用，并用 `var_ref_fast_mut`（新增，可信 `&mut`）。

### 4.5 Symbol / 立即写的原因与边界

- **Symbol**：`AtomTable::retain` 需要 `&mut`（atom 表可变），无法在共享借用
  下完成。S1 让 Symbol 读回退到 `try_read_owned_var_ref`（S1b 可把 atom 的
  `ref_count` 也 `Cell` 化）。
- **立即写**（`i`/`s`）：要修改 `VarRefData.value`（`RawValue`，非 `Copy`），
  仍需要 `&mut`。S1 只降低其校验与 `Result` 开销，不消除可变借用；彻底消除
  需要把单元值改成 `RefCell<RawValue>`（S1b）。

## 5. Scope 清单

| 文件 | 改动 |
| --- | --- |
| `src/engine/heap/mod.rs` | `Node.strong: Cell<u32>`；`SlotState::strong` |
| `src/engine/heap/gc.rs` | `.strong.get()/.set()`；新增 `retain_raw_fast` |
| `src/engine/heap/arena.rs` | 新增 `live_node_fast` / `var_ref_fast_mut` 等可信访问器 |
| `src/engine/heap/slot_ownership.rs` | `.strong.get()` |
| `src/engine/heap/roots.rs` | 新增 `take_owned_raw_value_fast`、`read_owned_cell_fast` |
| `src/engine/heap/binding_storage.rs` | 精简 `try_replace_immediate_var_ref_value`；`var_ref_fast_mut` |
| `src/engine/heap/runtime/mod.rs` | 新增 `retain_object_root_fast` |
| `src/engine/vm/bindings.rs` | 新增 `read_run_cell_fast` |
| `src/engine/vm/run.rs` | captured 读调用点改快路 |
| `src/engine/vm/stack.rs` | 属性读调用点改快路 |
| `src/engine/object/ordinary_storage/ic.rs` | 新增 `property_ic_read_fast` |
| `src/engine/heap/tests.rs` | `.strong` 测试适配 |

预计 12 个源文件，不新增模块，不改 public API。

## 6. 算法草图

```rust
// heap/gc.rs
#[inline]
pub(in crate::engine::heap) fn retain_raw_fast(&self, id: RawId) {
    let node = self.live_node_fast(id);
    node.strong.set(node.strong.get().saturating_add(1));
}

// heap/roots.rs
#[inline]
pub(crate) fn read_owned_cell_fast(&self, root: &VarRefRoot) -> Option<Value> {
    if !root.belongs_to(self) || self.0.deferred_references.has_pending() {
        return None;
    }
    let state = self.0.state.try_borrow().ok()?;      // shared, 非 mut
    if !state.heap.zero_queue.is_empty() { return None; }
    let cell = state.heap.var_ref_fast(root.id());
    match &cell.value {
        RawValue::Object(object) => {
            state.heap.retain_raw_fast(RawId::Object(*object));
            Some(Value::Object(ObjectRef::from_owned_handle(self.clone(), *object)))
        }
        RawValue::String(s) => Some(Value::String(s.clone())),
        RawValue::BigInt(b) => Some(Value::BigInt(b.clone())),
        _ => None,                                     // Symbol/标量回退
    }
}

// object/ordinary_storage/ic.rs（数据属性命中分支）
#[inline]
pub(crate) fn property_ic_read_fast(
    &self, base: &Value, executable: &PublishedFunctionSnapshot,
    pc: usize, key: u32, native: &mut Option<LinkedNativeSelection>,
) -> Option<Value> {
    let atom = linked_field_atom(self, executable, key)?;
    let Value::Object(object) = base else { return None };
    if !object.belongs_to(self) { return None; }
    let cache = executable.property_read_ic.site(pc)?;
    let state = self.0.state.try_borrow().ok()?;
    if state.heap.has_pending_zero_cleanup() { return None; }
    let receiver = object.object_id();
    let raw = cache.read(&state.heap, self.domain_id(), executable.realm, receiver)?.clone();
    match raw {
        RawValue::Object(id) => {
            state.heap.retain_raw_fast(RawId::Object(id));
            Some(Value::Object(ObjectRef::from_owned_handle(self.clone(), id)))
        }
        RawValue::String(s) => Some(Value::String(s)), // Rc clone 已在 clone() 中
        RawValue::BigInt(b) => Some(Value::BigInt(b)),
        _ => None,
    }
}
```

## 7. 分步提交计划

1. `perf(heap): make refcounts Cell-backed` —— 仅 4.1 的字段机械化改造 +
   新增 `retain_raw_fast`，行为不变，测试通过。
2. `perf(heap): add trusted non-fallible accessors` —— 4.2/4.3，未被调用，
   行为不变。
3. `perf(vm): add shared-borrow fast paths for captured reads` ——
   `read_owned_cell_fast` / `read_run_cell_fast` + `run.rs` 调用点。
4. `perf(object): add property-read fast path` —— `property_ic_read_fast` +
   `stack.rs`/`ic.rs` 调用点。
5. `perf(heap): slim immediate var-ref write validation` ——
   `try_replace_immediate_var_ref_value` 精简 + `var_ref_fast_mut`。
6. `docs(perf): record S1 measurements` —— 更新 S1 报告数据。

每步独立可编译、可测；若某步无收益可单独回退。

## 8. 验证与度量

1. `cargo fmt --check`。
2. `cargo clippy --locked --all-targets -- -D warnings`。
3. `cargo test --locked --workspace`。
4. Test262 冻结向量：`pass=79982 / eligible=80032 / total=102037` **不得倒退**。
5. `python3 scripts/checks/check-source-layout.py` 及 rust-only 门禁。
6. 重跑 `scripts/benchmark/property_read_probe.py`，与 S0 对比。
7. profiling 构建对比 `heap_root_copies`，确认复制次数未异常变化。

## 9. 实施结果

已按本计划实现（`Cell` refcount、可信访问器、无失败根转换、captured/属性读快路、
立即写校验精简）。行为不变。

### 计时对比（`property_read_probe.py`，N=5,000,000，median，同机）

| case | engine | S0 ns/op | S1 ns/op | 变化 |
| --- | --- | ---: | ---: | ---: |
| prop_read_int | plain | 222.55 | 189.95 | −14.6% |
| prop_read_obj | plain | 270.94 | 243.62 | −10.1% |
| prop_read_string | plain | 299.68 | 273.83 | −8.6% |
| prop_read_int | profiling | 343.98 | 309.44 | −10.0% |
| prop_read_obj | profiling | 409.00 | 355.74 | −13.0% |
| prop_read_string | profiling | 437.11 | 406.46 | −7.0% |

### perf 归属变化

- `run::run` 自耗时从 42.50% 降到 36.29%（int）/ 33.99% 降到 29.13%（obj）。
- 原 `try_property_ic_read_owned` 的 `Result::branch` 主导项消失；快路内联后
  归属到 `RunSlots::property_ic_read`，不再看到成片的 `Result::branch` 子项。
- `try_replace_immediate_var_ref_value` 去掉重复 `validate_var_ref_value`。

### 验证

- `cargo test --locked --workspace --all-targets`：通过（lib 2278、oracle 907、
  CLI 32、rust-only 4、unsupported-diagnostics 6 等，0 失败）。
- `cargo fmt`：通过。
- clippy：本次改动未新增 lint；1.88/1.94 下报出的均为仓库既有 lint
  （`collapsible_if`、`manual_is_multiple_of`、测试 cfg 的 unused/dead_code）。
- Test262 冻结门禁：因为 `engine_semantics_trees=src`，任何 `src` 改动都会使
  冻结基线变为 stale，需走 milestone promotion 重新冻结；本 PR 不改基线
  （符合 README 的“不得为性能改动修改基线”）。行为不变由全量测试与 oracle
  907 例覆盖。

## 10. 预期与风险

- **预期**：消除读路径的 `Result` 分支与可变借用。S0 显示 `Result` 管道合计
  >30%，其中读路径占大部分；S1 目标是把其中可移除的部分拿掉，期望整体有
  可测的正收益（量级待测，不承诺具体倍数）。
- **风险**：
  - 可信快路的 panic 可能暴露此前被静默返回错误的真实不变量 bug —— 由
    Test262 与 workspace 测试兜底。
  - 共享借用与活动 `borrow_mut` 冲突时快路返回 `None` 回退，行为与现状一致。
  - `property_ic_read_fast` 只覆盖数据属性命中；其余走原路径，正确性不受影响。

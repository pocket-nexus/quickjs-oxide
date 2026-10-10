# 第 5 项 5e-2 设计：条件分支循环形专门化循环（2026-10-10）

前置：5e-1 清单（runtime-item5-loop-inventory.md）选定先拆条件分支循环形；
计划文档 §6 第 5 项计划调整。本文档是动手前的设计回执。

## 1. 循环解剖结论（execute.rs 精读）

`execute_admitted_in_state`（基线 64017 字节，`nm --print-size` 于
target/release/qjs）的结构：

```
loop {                                    // 每 turn 一次
    let FrameTurn { transaction, fault_pc, resume_pc, pending, owners,
                    executable, active_frame, .. } = segment.frame();
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, poisoned);
    'dispatch: loop {                     // 每条指令一次
        let pc = cursor.begin();         // fault = resume
        let decoded = executable.exec.decode_published(pc)?;   // Result 包装
        match decoded.opcode { ... }     // arm: cursor.advance(next) / continue
                                        // 或 break 'dispatch Ok(VmAction)
    }
    match action { ... }                 // 循环外落点；多数 return，少数 continue
}
```

关键机制：

- **FrameCursor**：`begin()` 令 `fault = resume`；`advance(next)` 更新局部
  `resume`；`Drop` 把 `fault`/`resume` 发布到帧。错误路径的 fault PC 可观察，
  语义是"当前指令 pc"。`publish_fault` 额外物化到激活帧。
- **FrameTurn/FrameTransaction**：`transaction` 独占 `&mut SlotStore` +
  `&mut FrameWindow`；一切槽位操作走 `cursor.with_slots(|slots| ...)` 闭包，
  经 `FrameSlots` 助手（`peek_current`/`push_current`/`binary_number`…），
  每次都从 `window.depth`、区界重新计算索引。
- **发布字流**：`ExecCode.words: Rc<[Cell<u32>]>`；`decode_published` 已
  `#[inline(always)]`，返回借用了 words 切片的 `PublishedDecoded`。header
  布局（exec.rs 私有常量）：`opcode = (word>>16) & 0x03ff`，
  `count = (h>>10)&3`，`first_wide = h & 0x1000`，`next = pc + ((h>>13)&3) + 1`；
  `operand(0)` 在非 wide 时内嵌于 `word` 低 16 位。
- **空循环探针真实热路径**（scratch 测试 dump `decode_published`，已还原）：
  每迭代 6 分派：`CompareBranchLocalLt(i<n)` → `Goto 16` → `GetLocal i` →
  `PutLocal s` → `UpdateLocalDiscard(i++)` → `Goto 2`。融合 op 的
  回退展开（GetArg/Lt/IfFalse/PostInc/PutLocal/Drop）是冷路径。
- **75 Ir/分派的成本构成**：每条指令重复 解码头+Result、`with_slots` 闭包
  往返、`FrameSlots` 助手的区界重算与 `window.depth` 读写、巨型 match 的
  取指。没有任何单个 arm 是热点（execute.rs 内最热单行 0.16%）。

## 2. 设计：专门化循环函数

新文件 `src/engine/vm/execute/specialized.rs`（execute/ 子模块已存在），
核心函数：

```rust
#[inline(never)]
fn conditional_shape_loop(
    cursor: &mut FrameCursor<'_>,
    runtime: &Runtime,                    // 仅 cfg(feature="profiling") 使用
    executable: &PublishedFunctionSnapshot,
) -> Result<SpecializedExit, Error>
```

`SpecializedExit { Fallback /* 5e-3 预留 Action(VmAction) */ }`。

### 2.1 热状态局部变量（不经通用辅助函数）

进入时一次性物化，退出时一次性写回：

```rust
let window  = &mut *cursor.transaction.window;   // 需要 pub(in vm) 访问权
let store   = &mut *cursor.transaction.store;
let slots   = store.hot_slots();                  // &mut [Option<FrameBinding>]
let mut depth          = window.depth;            // hot_depth()
let params_start       = window.parameters().start;
let locals_start       = window.locals().start;
let operands_start     = window.locals_end;
let operand_room       = window.end - window.locals_end;
let words              = executable.exec.published_words();
let mut pc             = cursor.resume;           // 等价 begin()
```

槽位操作直接切片索引：`slots[locals_start + i]`、
`slots[operands_start + depth - 1]`；push 复刻
`operand_push_index` 的两条检查（`depth < operand_room`、
目标槽为 `None`），错误文本与通用路径一致。opcode 判别用 header 原始
u16 与常量 bitset（`matches!` 范围式），不做 `Opcode::from_raw`。

### 2.2 快集（state-free 闭集，5e-2）

入选标准：快路径**不触碰 RuntimeState、owners、pending、selected_\***，
守卫失败一律 sync+Fallback 由通用循环重执行该指令（语义逐操作等价）。

| 指令 | 快路径 | 守卫失败 |
|---|---|---|
| Nop / MarkSuperCall | 无操作 | — |
| PushI32 / Undefined / Null / PushFalse / PushTrue | 标量 push（容量+空槽检查） | 容量错误→Error |
| PushConst | Int/Float/Undefined/Null/Bool/ShortBigInt 常量 | String/BigInt→Fallback |
| GetLocal / GetLocalCheck / GetArg | direct 标量 binding 复制 push | 非标量（owner 复制需 state）→Fallback |
| PutLocal / SetLocal /(Check) | DirectNumber 目标 + 数字栈顶（store_proven_number_operand 语义） | 其余→Fallback |
| UpdateLocalDiscard(/Check) | direct 数字 local 更新 | 其余→Fallback |
| NumberLocalInc / NumberArgInc | direct 数字 + 容量 → push 增量 | 其余→Fallback |
| Add…BitXor、Eq…Gte、StrictEq/StrictNeq | 双数字栈顶 binary_number_result | 非数字→Fallback |
| Neg/Plus/BitNot/Inc/Dec/PostInc/PostDec | 数字栈顶 unary | 其余→Fallback |
| Not | 立即栈顶 → bool | 其余→Fallback |
| Drop / Nip | 立即栈顶 → pop（无 release） | 非立即→Fallback |
| CompareBranchStack / Local / Arg / LocalLt / ArgLt | 双数字/direct 数字比较分支 | 其余→Fallback |
| Goto | pc = 操作数 | — |
| IfTrue / IfFalse | 立即栈顶 → pop + bool | 非立即→Fallback |

快集没有 Action 出口（所有 fast path 都 continue）；5e-3 扩展字段读/调用时
再加 `Action` 变体（sync：depth 写回、`fault = pc`、`resume = next_pc`、
`*pending = ...`，逐 arm 复刻通用 advance+break 顺序）。

### 2.3 同步纪律（Fallback / Error 的唯一出口）

```rust
// Fallback：未处理/守卫失败指令位于 pc
window.set_hot_depth(depth);
cursor.fault = pc;
cursor.resume = pc;      // 通用循环下一次 begin() 得 fault=resume=pc
return Ok(SpecializedExit::Fallback)
```

Error 出口同 Fallback 后 `return Err(e)`——与通用路径"begin 后 arm 内 `?`"
发布 `fault = 当前指令 pc` 的语义一致（resume 未被 advance）。

### 2.4 入口预检（'dispatch 顶部）

```rust
'dispatch: loop {
    let pc = cursor.begin();
    if specialized::is_fast_word(executable, pc) {     // 1 load + shift/and + bitset
        match specialized::conditional_shape_loop(&mut cursor, runtime, executable)? {
            SpecializedExit::Fallback => {}
        }
        let pc = cursor.begin();      // specialized 已同步 fault=resume=pc'
    }
    let decoded = executable.exec.decode_published(pc as u32)?;   // 原样
    ...
}
```

`is_fast_word`：`words.get(pc)` 越界→false；header 原始 u16 进
`matches!` 范围 bitset。非快指令代价 ≈ 6–8 Ir/分派（1 load + 移位与
bitset 测试），这是对通用路径的唯一新增开销，DeltaBlue 这类调用密集负载
是否有感必须八项实测（风险与对策见 §5）。

### 2.5 需要的访问权增量（全部 pub(in crate::engine::vm)）

- `ExecCode::published_words(&self) -> &[Cell<u32>]`（pub(crate)，与
  `PublishedDecoded::operand` 相同的直接索引约定）。
- `SlotStore::hot_slots(&mut self) -> &mut [Option<FrameBinding>]`。
- `FrameWindow::hot_depth/set_hot_depth` + 区界只读访问（或一个
  `hot_geometry()` 元组）。
- profiling 计数对等：`record_execution_dispatch/outcome` 在快 arm 中原位
  保留；storage cost 记录（`Cost::Move/Clear`、`record_occupancy`）若私有，
  加同名 passthrough，保证 residuals.py 计数不漂移。

## 3. 正确性不变式

1. 每条 fast arm 与通用 arm 快路径**逐操作等价**：同一守卫顺序、同一错误
   文本、同一 profiling 记录、同一 advance 目标（含 NumberLocalInc 的
   `next+2`、UpdateLocalDiscard 的 `next+{2,3}`、比较分支的
   `decision == 谓词位` 目标选择）。
2. 槽位不变式：operands `[operands_start, +depth)` 为 Some、其后至
   room 为 None；locals/parameters 区直接 FrameBinding。fast push 复刻
   `operand_push_index` 双检查。
3. 专用循环内 pc 只来自 `next_pc` 算术与发布时验证过的静态目标；words 直接
   索引与 `PublishedDecoded::operand` 相同的越界 panic 约定。
4. 不引入新 owner/释放语义：快集只移动标量（Int/Float/Bool/Undefined/
   Null/ShortBigInt 与 Number），heap owner 一律 Fallback。
5. 既有语义改动对照 Node（工作约定）；每提交跑 lib 全量测试（2 个既存
   失败除外）。

## 4. 测量计划（验收对齐计划文档 §6/§7）

- 容器（oxide-vg:1.88，CARGO_PROFILE_RELEASE_DEBUG=1）：empty_loop 探针
  572 Ir/迭代基线 → 目标每分派 ≤40（6 分派 → ≤240 + turn 开销）；八项
  Ir/Dw，非目标负载 ≤0.5% 门槛。measure_docker.py --workdir
  ~/.cache/oxide-measure。
- 行级归因：callgrind_annotate 传 execute.rs 与新 specialized.rs 源路径。
- 函数大小记录：execute_admitted_in_state 基线 64017 B；提交记录
  before/after + 新函数大小 + 栈帧/栈槽（objdump -d 与 readelf 帧信息）。
- 原生 A/B：收口时 build.py + iterate_v8.py（ governor=performance、
  boost 关、停 Docker Desktop）；i-cache 敏感，Ir 赢不算赢。
- 侧分支净收益：Fallback 只在守卫失败/非快 opcode 时发生；预检给通用
  指令的 +6–8 Ir 是唯一的普遍侧成本，八项里任何非目标回退超 0.5% 即
  阻塞并复测。

## 5. 风险与对策

| 风险 | 对策 |
|---|---|
| 预检给通用分派 +6–8 Ir，调用密集负载（Dw）回退 | 八项实测；若超门槛，预检改挂到分支/turn 入口（每 turn 一次），或仅在向后跳转目标处使能 |
| 新函数 icache 占用 | #[inline(never)] 独立函数（估 1–2 KB），不占巨型函数；调用密集时它整体驻留 L1i |
| 布局噪声 ±3–8% | 判据以 Combined 与"同代码关新路径"对照为准（计划 §7 布局对照规则） |
| profiling 计数漂移 | 快 arm 原位保留 record_execution_dispatch/outcome；residuals.py 复核 |
| 5a/5c 教训重演（侧分支抵消） | 快集全部 state-free、无 Action 出口；任何可失败/可观察操作都 Fallback；合入前八项实测净收益 |

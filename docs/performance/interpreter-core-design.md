# 5a 解释器核心重写设计（2026-10-08）

第 5 项第一步（5a）的设计。依据是第 4 项收口和 #104 中的实测（aarch64 Callgrind，`item5-c6` 构建），目标与验收见第 5 节。

## 1. 现状

解释器核心是一个函数 `execute_admitted_in_state`：约 55 KB 机器码、2,300 行 Rust，所有 opcode 的处理都内联在一个 `match` 里。

**每次分派的公共路径约 30–34 条指令**（QuickJS 约 5 条）。空循环 `s = s + i` 每轮 9 次分派，在解释函数里花 740 条，平均每次约 82 条：

| 公共路径的步骤 | 指令 | 原因 |
|---|---:|---|
| 从栈上重新加载 `executable`、字流指针和长度，检查 `pc < len` | 8 | 寄存器压力：循环不变量被溢出到栈上 |
| 解码字头、计算下一 PC、opcode 范围检查（两次）、查跳转表 | 18 | 解码在每次分派重做 |
| 把 fault/resume PC 写回栈上的 `FrameCursor`，回到循环顶部 | 8 | 游标字段在内存里，每次分派写两次 |

各 opcode 自身也要从栈上重新读窗口字段（base、depth、end）再做边界检查，`GetLocal` 这类指令约 25 条。函数入口还有约 100 条指令在预先计算几十个栈地址再溢出，所以每次退出后重入都要付这笔钱（第 4 项 4f 实测重入固定成本约 930 条）。

**按用例：**

| 用例 | 解释函数 self | 公共分派路径 | 每次分派平均 |
|---|---:|---:|---:|
| Richards | 35.8% | 10.7% | 约 318 条（含重量级 opcode） |
| DeltaBlue | 33.3% | 8.3% | — |

Richards 最常见的 opcode：`BorrowedFieldThis` 17.6%、`CompareBranchStack` 6.8%、`PutField` 6.9%、`PushThis` 6.3%、`GetField2Cached` 6.2%、`Null` 5.8%、`GetVar` 5.6%、`Return` 5.5%、`CallMethod` 5.5%。
NavierStokes、Crypto 以 `GetLocal`（约 22%）、`GetArg`、`PushI32`、`Add`、`Mul`、`NumericArray*` 为主。

**E 实验与 #104 的教训：** 在巨型函数里做局部优化会被寄存器溢出抵消（E3 指令减少、周期上升）；`instanceof` 在解释循环里加一个分支，其余七项全部回升 0.5%–1.8%。所以必须先拆函数，不能继续在里面加东西。

## 2. 结构

拆成两层：

- **热循环 `run_hot`**：一个很小的 `#[inline(never)]` 函数。热状态全部是局部变量，小到编译器能把它们一直留在寄存器里：
  - 字流切片 `words: &[Cell<u32>]` 与当前 PC；
  - 当前帧窗口的切片 `&mut [Option<FrameBinding>]`，以及局部变量、参数和操作数区的起点、操作数深度、操作数容量；
  - `&mut RuntimeState` 与只读的执行事实（常量池、读写缓存表）。

  只处理热 opcode。遇到其他 opcode、未命中或需要分配与回调的情况，把 PC 和深度写回帧（显式同步），返回一个小的出口值。
- **冷路径**：现有的 `execute_admitted_in_state` 保留为冷路径，从同步后的 PC 继续执行。下一条又是热 opcode 时，交还热循环（见第 3 节）。

不变的部分：帧与窗口的所有权规则（每个槽拥有自己的边）、`ready` 循环与冷驱动的协议、物化规则（热循环不观察活动帧；调用与返回照旧交给安装器和返回路径）、poison 与错误传播。

## 3. 两层之间的切换

1. `execute_admitted_in_state` 进入后先调用 `run_hot`。
2. `run_hot` 返回"冷 opcode"时，现有分派循环执行这一条（以及之后连续的冷 opcode）。每次分派的开头多一个"下一条是否热"的检查，命中就回到第 1 步。这是唯一一处改动现有循环的地方，第一批先单独测它的成本。
3. 调用、返回、`VmAction` 出口照旧经由现有循环交给 `ready` 循环。

切换要便宜：热循环的入口只装载局部变量，没有地址预计算；出口只写回 PC 和深度。目标是一次往返 ≤30 条，所以对重量级 opcode 交错的 Richards、DeltaBlue 也划算。

## 4. 分批迁移

| 批次 | opcode | 主要受益用例 |
|---|---|---|
| B0 | 框架：`run_hot`、出口协议、冷循环中的"回到热循环"检查；只迁 `Nop`、`Goto` | 验证切换成本与正确性 |
| B1 | `GetLocal`/`PutLocal`/`SetLocal`/`GetArg`/`PutArg`、`PushI32`/`PushConst`（立即数）/`Undefined`/`Null`/`PushTrue`/`PushFalse`、`Drop`/`Dup`/`Nip`、`IfTrue`/`IfFalse`、`CompareBranch*`、`UpdateLocalDiscard`、整数与浮点的 `Add`/`Sub`/`Mul`/比较 | 空循环、数值探针、NavierStokes、Crypto |
| B2 | `PushThis`、`BorrowedFieldThis`/`Local`/`Arg`、`GetFieldCached`/`GetField2Cached` 命中、`PutField`/`DefineField` 命中（缓存探测与写入调用现有的外置函数） | Richards、DeltaBlue、RayTrace |
| B3 | `GetVar`（受信全局读取）、`GetVarRef`/`PutVarRef`、`Call`/`CallMethod` 的调用站点缓存命中直接交给安装器、`Return` | Richards、DeltaBlue、EarleyBoyer |
| B4 | `NumericArray*`、`Dense*` 等融合指令；已迁移分支从冷函数中删除 | NavierStokes、Crypto；缩小冷函数 |

每批：库测试（profiling 与默认）、八项 Ir、探针、两个函数的尺寸；Ir 改善的批次再用 `perf stat`（周期、L1i 未命中）对比。任何一项回升超过 0.5% 先查原因，不累积到下一批。

## 5. 目标与验收

- **公共分派路径 ≤10 条/分派**（现约 30–34）；空循环每轮 ≤350 条（现 740，解释函数部分）。
- **退出重入 ≤300 条**（第 4 项划转的验收；探针：被调函数内写一次全局变量，减去 `f(i)`）。
- **八项**：Richards、DeltaBlue Ir 各降 ≥15%；其余各项不回升超过 0.5%；`perf stat` 周期不回退。
- 第 5 项原有目标不变：≤40 条/分派、Dw 2–3/分派；完成后核对 R/D 读缓存与解释衔接合计 ≤8%。

## 6. 风险

- **状态同步：** 热循环与冷路径之间漏同步 PC 或深度，会造成错误 PC 或槽位泄漏。对策：出口只有一个写回函数；debug 构建在冷路径入口校验窗口和 PC（同 `admit`）；测试覆盖每个迁移 opcode 的抛错与 GC 路径。
- **代码生成：** 热循环本身也会长大。每批记录 `run_hot` 的尺寸和栈帧；栈帧出现溢出时，先把冷的出口路径外置。
- **重复语义：** 迁移期间同一 opcode 有两份实现。迁移完成即从冷函数删除；B4 结束前冷函数里只保留真正冷的分支。

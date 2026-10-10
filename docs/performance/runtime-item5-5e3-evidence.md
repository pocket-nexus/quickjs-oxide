# 第 5 项 5e-3（修订版）取证：字段读命中链的逐行构成（2026-10-10）

5e-2 否证"拆循环"后按重启条件转攻字段读命中链的 **arm 工作**。方法：
`oxide-vg:1.88` 容器内 `CARGO_PROFILE_RELEASE_DEBUG=1` 构建基线
（954de009，与 826cde3c 代码相同），对 pinned V8-v7 固定工作量驱动跑
`valgrind --tool=callgrind`（无 cache-sim），`callgrind_annotate` 逐行。
原始归因文件：`~/.cache/oxide-measure/item5/5e2-base.work/{deltablue,raytrace}.{execute.rs,ic.rs,window.rs}.annot`。

## 每命中成本构成（BorrowedFieldThis，DeltaBlue）

每命中 ≈87 Ir（命中数 ≈4M/轮 ×10 轮）。arm 块合计 349.6M Ir（DeltaBlue
总量 3.89B 的 9.0%；RayTrace 对应块 238.5M）：

| 段 | Ir/命中 | 来源行（基线行号） |
|---|---:|---|
| select_linked_data_into 进入 + 守卫链 | ~13 | ic.rs:45/56/60/65 |
| site(pc) 两级查表（offsets+block_ranks+sites 三次边界检查 load） | ~15 | property_ic.rs:732–741 |
| read_inline 守卫（kind/entry Cell、shape、layout_revision、epoch、depth、命中计数器） | ~14 | property_ic.rs:180–287 |
| **promote_field_in_state（retain + native 探测 + 包装）** | **~32** | ic.rs:256–320；其中 `*native = selected.map(...)` 每命中写 24B None ≈4 Ir |
| **commit_owned（#[inline(never)] 共享出线调用，命中即付）** | **~9** | execute.rs:614（callee 含 try_push+install） |
| arm 自身守卫/收尾（capacity/ready/miss init/or_else/selected if） | ~8 | execute.rs:569–612 |

放大因子：同一条 select/promote 链还服务 GetFieldCached/命名读路径
（DeltaBlue 内另 ≈290M），字段读选择合计 ≈10–11% 总量。

## 判定：有料（瘦肉型）

可达的削减与预期：
1. `promote` 仅在选中时写 `*native`（合同：调用方都传新 None）→ −4 Ir/命中。
2. 两个 BorrowedField arm 的 commit 改内联（5e-2 已证函数尺寸中性，
   rejected 路径保持单个 cold 出线符号）→ −6~8 Ir/命中。
3. site() 扁平化与 read_inline 计数器暂未动（适应机制与表布局，风险收益
   不成比例）。

预期合计 −10~12 Ir/命中（arm 的 ~12–14%），换算 DeltaBlue −1.5~2.5% Ir、
RayTrace −1~2% Ir。达不到第 5 项原预算，但满足第 7 节验收（目标用例 ≥1%
且其余不回退）。

## 实施与验证

- e466529b：上述 1+2。execute_admitted_in_state 61864→64371 B（+4.0%），
  栈帧 1784→1832 B，生成代码已检查。lib 全量无新增失败。
- e465529b 之后的 site 扁平化：IC 站点表从 offsets(u8)+block_ranks(u32/64)
  改为每字一个 u32 直索引（u32::MAX=无站点），热点探测从两次依赖 load 降为
  一次；append_ic 的 pc_words 语义保留。
- 容器 A/B（`~/.cache/oxide-measure/item5/5e3-base-db.json`/`5e3-base.json`
  为基线 826cde3c，`5e3-flat.json` 为合并提交）：

| 用例 | Ir | Dw |
|---|---:|---:|
| deltablue | **−1.42%** | −1.84% |
| raytrace | **−1.01%** | −1.22% |

13 个固定探针无一回退（s=o.x −1.9%、o.m(i) −0.8%、o.x=i −0.6% 为改善）。
中间态（仅前两刀）deltablue −0.78%/raytrace −0.56%，归因确认机制：
commit_owned 总量 78.7M→39.5M（内联生效），promote −20.4M（死存储消除）。

**判定：采纳**（第 7 节：目标用例 Ir 下降 ≥1% ×2，其余无回退）。
focused Test262：6844/6844 通过（2026-10-10，收口前语义门槛）。
非目标 V8 用例（richards/navier 等）未在本子任务复测，依赖收口测量；
原生 ABBA 由参考机会话在收口时复核（i-cache 规则不变）。

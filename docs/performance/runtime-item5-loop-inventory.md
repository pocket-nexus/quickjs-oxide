# 第 5 项 5e-1：专门化循环清单（2026-10-10）

方法：`oxide-vg:1.88` 容器内 `CARGO_PROFILE_RELEASE_DEBUG=1` 构建（release profile 的
`debug = false` 会覆盖 `RUSTFLAGS -Cdebuginfo`，必须用 cargo profile 环境变量），对
pinned V8-v7 固定工作量驱动跑 `valgrind --tool=callgrind`（无 cache-sim），
`callgrind_annotate OUT.callgrind /src/src/engine/vm/execute.rs` 出逐行 Ir。
基线提交 `4889ebf5`（7a 后，与第 5 项基线同代码）。注意 debuginfo 构建内联分布与
生产构建不同，本清单用于**循环形状的相对份额**，总量以生产容器报告为准。

## 基线

- 空循环探针：572 Ir/迭代（循环体 7–8 分派，≈75 Ir/分派）；预算 ≤40。
- execute.rs 本体内逐 opcode arm 份额（含 arm 之下的共享脚手架行，arm 间脚手架归前一 arm）：

| arm | DeltaBlue | RayTrace |
|---|---:|---:|
| BorrowedFieldThis | 8.3% | 4.2% |
| IfTrue | 3.2% | 9.3% |
| Gt/Gte/Lt | 2.7% | 3.1% |
| BorrowedFieldLocal/Arg | 0.7% | 4.2% |
| CompareBranchLocal | 0.4% | 0.3% |
| SetLocal/SetName/Drop/PostInc 等 | ~1.2% | ~1.9% |

调用的成本不在 execute 本体内（走 `enter_ordinary`/安装链，见 7b 记录）。

## 候选循环形（按合并权重排序）

1. **字段读命中链**（BorrowedFieldThis/Local/Arg）：deltablue 12.6%、raytrace 8.2%
   的 execute 本体，单项最大。即 5a/5c 重启条件明文要求的"读写字段命中"覆盖对象。
2. **条件分支循环形**（IfTrue + Gt/Gte + CompareBranch + 局部读写的连续段）：
   6–12%。空循环探针的形状，分派脚手架压缩的直接度量。
3. **调用循环**（Call/CallMethod arm 的处理不退出专门化循环，侧分支进安装链）：
   覆盖条件之二；成本主体在安装链（7b 已 State 化）。

## 5e-2 选择

先拆**条件分支循环形**（候选 2）：形状最闭合、侧分支最少，用来验证拆循环机制本身
（热状态局部变量、冷路径显式同步）在容器内能把 75 Ir/分派压向哪个方向；字段命中链
（候选 1）作为 5e-3 主体跟进——它同时满足重启条件的字段覆盖，但 arm 内部的 IC 读取
分支更多，不适合作为第一个拆分对象。调用覆盖随 5e-3/5e-4 扩展。

## 规矩复述

改动 `execute_frame_in_state` 的提交记录函数大小、栈帧、栈槽；每个子任务容器验证；
侧分支（decline/等待/异常）整体净收益后才合入；预解码（5e-4）单独验证。

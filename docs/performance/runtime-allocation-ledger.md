# 对象分配成本账本

本页是[新顺序第 1 项](runtime-bc-plan.md#第-1-项对象分配任务清单)的准备工作：
把对象字面量和 `new` 每次迭代的指令拆成互斥的成本块，作为任务清单的依据。
数据见 [runtime-allocation-ledger.json](runtime-allocation-ledger.json)。

## 测量对象与方法

- 运行时：B2a `f2501839`，Rust 1.88.0 普通 release（fat LTO、CGU=1、无 PGO、无 profiling feature）。
- 参考：QuickJS `535a7c25`，上游 Makefile 构建，只用于每次迭代的总指令数。
- 探针（`docs/performance/probes/allocation/ledger.py` 生成，输出逐次校验）：

  ```js
  function work(n){ var s; for (var i=0;i<n;i++){ s=i; } return s; }                 // empty_loop
  function work(n){ var s; for (var i=0;i<n;i++){ s={a:i,b:i}; } return s.a; }      // object_literal
  function P(x){ this.a=x; this.b=x; }
  function work(n){ var s; for (var i=0;i<n;i++){ s=new P(i); } return s.a; }       // constructor
  ```

- 每次迭代成本 = `(Ir(2N) − Ir(N)) / N`，N = 100,000；`(Ir(4N) − Ir(2N)) / 2N` 核对线性，三项完全一致。
- 账本：Callgrind `--separate-callers=30` 在 N 和 2N 各跑一次。每个“函数 + 完整调用链”上下文的边际
  **self** 成本按调用链归入一个阶段，按自身函数归入一个机制。每条指令只属于一个上下文，
  所以各格相加等于总量。账本合计比 Cachegrind 斜率少 10–16 Ir（≤0.13%），来自两种工具的计数差异。
- 阶段和机制的匹配规则写在工具里；函数名在比较前去掉 rustc 版本相关的括号和 `impl` 写法。

复现：

```sh
python3 docs/performance/probes/allocation/ledger.py \
  --qjs "$OXIDE_QJS" --reference "$QUICKJS_QJS" --output "$OUT"
# 调整分类规则后，可用 --reuse 在同一 $OUT 上重新归类，不重跑 Valgrind
```

## 总量

| 探针 | oxide Ir/迭代 | QuickJS Ir/迭代 | 倍数 | oxide 扣除空循环后 |
| --- | ---: | ---: | ---: | ---: |
| 空循环 | 647 | 135 | 4.8× | — |
| 对象字面量 `{a:i,b:i}` | 12,215 | 1,392 | 8.8× | 11,568 |
| 构造 `new P(i)`（两次 `this.x = …`） | 12,833 | 1,973 | 6.5× | 12,186 |

## 对象字面量：12,215 Ir/迭代

| 阶段 | Ir/迭代 | 占比 | 主要机制 |
| --- | ---: | ---: | --- |
| **D 字面量字段定义（离开解释循环）** | **6,791** | 55.6% | slot 追加 1,494；查找/准入/校验 1,163；公共包装与协议对象 1,150；引用计数与释放 1,050；边与 atom 引用 740；shape 转移 679；驱动 290；零散 225 |
| R 退出与重入解释循环 | 1,474 | 12.1% | 帧 materialize 378、`run_frames_with_state` 310、`ready::run` 272、重入准入 176、`deferred_action` 130、`cold::dispatch` 110 |
| I 解释循环自身 | 1,385 | 11.3% | 其中空循环约 647 |
| A 对象分配 | 1,297 | 10.6% | 查找/校验 381（含 `validate_object_layout` 131、`get_or_create_shape` 125）；引用计数与释放 287；边与 atom 210；`ObjectData` 拷贝 90 |
| F 覆盖 `s`：释放上一个对象 | 913 | 7.5% | `finish_resident_node` 280、零引用队列 143、`release_raw_no_drain` 118、边重新计算 102 |
| S 局部写入与值复制 | 251 | 2.1% | |
| T 其他释放 | 88 | 0.7% | |

**两个字段的定义占一半以上，每个字段约 3,400 条。** 每个 `DefineField` 都离开解释循环，
经过 `cold::dispatch` → `construct_driver::define_property` → `start_public_field` →
公共 `Runtime::try_define_owned_property` → `try_define_own_property_in_state` →
`define_raw_property_with_poison` → `append_selected_missing_slot` →
`append_slot_with_owned_shape_input`。路上构造并释放 `PropertyKey`、`ObjectRef`、
`OwnedPropertyDescriptor`，并触发帧 materialize。R 的 1,474 条也全部由这两次离开循环产生。

## 构造：12,833 Ir/迭代

| 阶段 | Ir/迭代 | 占比 | 主要机制 |
| --- | ---: | ---: | --- |
| **W `this.x = …` 追加字段** | **4,741** | 36.9% | slot 追加 1,691；查找/准入/校验 1,321（`select_set_slot` 522、`locate` 435、`select_missing_prototypes` 292）；引用计数与释放 802；shape 转移 660；边与 atom 232 |
| I 解释循环自身 | 2,530 | 19.7% | 两个帧的指令，含空循环约 647 |
| C 构造调用与返回 | 1,979 | 15.4% | `enter_constructor` 744、帧回收与释放约 650、窗口准备 128、目标选择 87 |
| A 对象分配 | 1,303 | 10.2% | 与字面量相同的分配路径，另有 `strong_count` 159 |
| S 局部写入与值复制 | 1,172 | 9.1% | `insert_copy_in_state` 267、`retain_raw_root` 215、`dup_jsvalue` 176、`rotate_operands` 138 |
| F 覆盖 `s`：释放上一个对象 | 913 | 7.1% | 与字面量相同 |
| T、R | 185 | 1.4% | |

`this.x = …` 留在解释循环内，没有离开循环的成本；但每个追加仍约 2,370 条。
其中约 1,250 条是缺失属性的写入选择：沿原型链确认没有 setter 和只读属性（`select_set_slot`、
`locate`、`select_missing_prototypes`），每次都重做。

## 两条路径共用的追加内核

字面量和 `this.x = …` 最终都调用 `append_slot_with_owned_shape_input`。下表六项合计每个字段约 1,360 条，两条路径相同：

| 工作 | 每字段 Ir | 代码位置 | 说明 |
| --- | ---: | --- | --- |
| 追加主体 | 约 395 | `heap/runtime/mod.rs:635` | 调用方先取得临时 shape 引用，这里释放并 `apply_cleanup` 两次 |
| 后继 shape 查找 | 约 133 | `canonical_successor`（`heap/runtime/mod.rs:395`） | 嵌套 `HashMap` 查找，再校验目标 shape |
| 转移记录 | 约 146 | `record_transition`（`heap/runtime/mod.rs:420`） | 每次命中都重新 `entry().or_default().insert` |
| 新增 slot 发布 | 约 168 | `append_selected_missing_slot` 与 `heap/object_storage.rs:1349` | 含后继 shape 与旧 shape 条目的 O(n) 前缀比较 |
| 边与 atom | 约 120 | `retain_edges_transactionally`、`retain_slot_atoms` | atom 列表每次分配 `Vec` |
| 释放与清理 | 约 400 | `release_and_drain`、`release_raw_no_drain`、`apply_cleanup` | 释放临时 shape 与旧 shape 引用 |

QuickJS 在命中已有 shape 转移时，一次追加只需要一次哈希查找、一次 shape 引用交换和一次槽位写入。

## 任务清单与预期

任务定义见[计划第 1 项](runtime-bc-plan.md#第-1-项对象分配任务清单)。按本账本估算
（预算是目标，需逐项实测）：

| 任务 | 处理的成本块 | 字面量 | 构造 |
| --- | --- | ---: | ---: |
| 1a 追加站点缓存 | D 的 slot/shape/边/引用计数（约 3,960）；W 全部（4,741） | −3,660 | −4,440 |
| 1b 字面量字段留在循环内 | D 的包装/准入/驱动（约 2,830）；R（1,474） | −4,300 | — |
| 1c 分配路径 | A（约 1,300 → ≤400） | −900 | −900 |
| 1d 释放路径 | F（913 → ≤250） | −660 | −660 |
| 1e `this.x` 值传递 | S（1,172 → ≤200） | — | −970 |
| **完成后估计** | | **约 2,700（扣除空循环约 2,050）** | **约 5,850（扣除空循环约 5,200）** |

构造路径剩余的大头是 C（1,979）和解释循环（2,530），分别属于第 4 项和第 5 项。

## 局限

- 归类按“函数 + 调用链”，不是按源码行。内联进某个函数的代码计入该函数。
- 机制标签按函数名匹配，用于定位，不等于源码职责的精确划分；阶段划分才是互斥账本的主口径。
- 指令数不等于时间。第 1 项的任务完成后，仍要看 Dw、R/D/NS 计数和收口时的原生 ABBA。
- 账本基于 Rust 1.88 构建。Rust 1.97 构建的同一源码，字面量为 11,580 Ir/迭代、构造为 12,621，
  各阶段比例相近。

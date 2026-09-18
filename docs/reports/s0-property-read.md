# S0：`a.b` 属性读路径成本采样

本报告是性能改造的第一个阶段（S0）。目标不是改行为，而是**量化**
属性读路径的成本构成，为后续 S1–S3 排优先级。所有数据由
[`scripts/benchmark/property_read_probe.py`](../../scripts/benchmark/property_read_probe.py)
复现。

## 方法

- 三个单态（monomorphic）数据属性读负载，各固定迭代 N 次：
  - `prop_read_int`：`s += o.a`（int 结果）
  - `prop_read_obj`：`s += o.a.x`（对象结果，走 retain 路径）
  - `prop_read_string`：`s += o.a.length`（字符串结果，走 Rc 叶子）
- 记录**整进程 wall time** 的中位数与 ns/op（含启动、编译、执行、退出）。
- `perf record -g` 采样，`perf report --no-children` 取符号自耗时。
- 对比两个构建：`plain`（release + debuginfo，无 profiling）与
  `profiling`（`--features profiling`）。

复现命令：

```sh
python3 scripts/benchmark/property_read_probe.py \
  --engine plain=target/s0-plain/release/qjs \
  --engine profiling=target/profile-feature/release/qjs \
  --iterations 5000000 --repeat 5 --perf --perf-engine plain \
  --output target/s0-property-read
```

## 计时结果（N=5,000,000，median）

| case | engine | median ms | ns/op |
| --- | --- | ---: | ---: |
| prop_read_int | plain | 1112.73 | 222.55 |
| prop_read_obj | plain | 1354.70 | 270.94 |
| prop_read_string | plain | 1498.38 | 299.68 |
| prop_read_int | profiling | 1719.90 | 343.98 |
| prop_read_obj | profiling | 2045.00 | 409.00 |
| prop_read_string | profiling | 2185.56 | 437.11 |

profiling 构建的插桩（`record_owned_execution_event`、`record_owned_storage`）
本身带来约 50% 的额外开销，因此 perf 归属分析使用 plain 构建。

## perf 归属（plain，符号自耗时）

`prop_read_int`：

| 符号 | 占比 |
| --- | ---: |
| `vm::run::run` | 42.50% |
| └ `Result::branch`（内联） | 14.85% |
| └ `copy_value`（内联） | 7.96% |
| └ `Option::map`（内联） | 4.70% |
| `try_property_ic_read_owned` | 11.36% |
| `run::binary` | 8.48% |
| `try_replace_immediate_var_ref_value` | 6.92% |
| `try_read_owned_var_ref` | 6.47% |
| `SlotStore::push_current` | 5.31% |

`prop_read_obj`：

| 符号 | 占比 |
| --- | ---: |
| `try_property_ic_read_owned` | 24.98% |
| └ `Result::branch`（内联） | 12.54% |
| └ IC site 查找 (`PropertyReadCacheTable::site`) | 1.37% |
| └ `linked_field_atom` / `belongs_to` / `Rc::as_ptr` | 1.67% |
| `vm::run::run` | 33.99% |
| └ `Result::branch`（内联） | 10.50% |
| └ `copy_value`（内联） | 6.95% |
| └ `Option::map`（内联） | 3.84% |

## 结论

1. **最大成本是 `Result`/`Option` 管道，而不是引用计数本身。**
   `Result::branch`、`Result::map`、`Option::map` 在各处合计约占
   **总时间的 30% 以上**。对象的属性读里，`try_property_ic_read_owned`
   有**一半**（12.54% / 24.98%）花在 `Result` 分支上。
2. **`copy_value` 占约 7–8%**，对应非 `Copy` 的 32B `Value` 每次搬运。
3. **属性读 IC 路径随结果类型从 11% 涨到 25%**：int 结果便宜，对象/字符串
   结果触发 retain/borrow/validate。
4. **局部变量走堆 binding 存储**：`try_read_owned_var_ref` +
   `try_replace_immediate_var_ref_value` 合计约 **13%**。循环变量 `i`/`s`
   每次读写都经过 var-ref 单元与运行期状态借用。
5. **`push_current`/`install_operand` 约 5%**。

## 对后续阶段的含义

- S1（热元数据去全局借用 + 免失败快路）应优先，因为它同时命中
  「`Result` 管道」和「binding/堆借用」两大块。
- S2（热路径去 generation 校验）与 S1 同源，紧随其后。
- S3（`Value` 瘦身）对应 `copy_value` 的 7–8%，收益确定但依赖前两步。

## 局限

- 整进程 wall time 包含启动/编译/退出；ns/op 是派生值，不是单操作延迟。
- 尚未构建 pinned QuickJS oracle，因此本报告只有**绝对成本构成**，
  没有相对 QuickJS 的比值。
- perf 只做符号级归属；`Result`/`copy_value` 等以「内联子项」形式给出，
  不能直接映射到源码行。

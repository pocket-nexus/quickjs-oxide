# 诊断探针

## 固定工作量矩阵

[`build_fixed_matrix.py`](build_fixed_matrix.py) 生成项目自有的 20 份 JavaScript 程序和 [`fixed/manifest.json`](fixed/manifest.json)。manifest 保存每份源码的 SHA-256 与预期 stdout，供 `scripts/benchmark/fixed.py` 在相同 guest 工作量下比较两个引擎构建。

| 路径 | Case |
| --- | --- |
| 数值局部计算与转换 | `numeric_local_loop`、`numeric_coercion_loop` |
| 普通局部读取 | `local_read_with_setup`、`local_read` |
| 值与对象操作 | `bigint_32/64/256`、`prop_read/write`、`array_read/write`、`call0`、`string_bridge` |
| 绑定写入 | `local_move`、`argument_move`、`number_owner_fallback`、`object_move`、`empty_loop` |
| 异常语义 | `type_error`、`tdz` |

生成器从 [`ordinary-writes/`](ordinary-writes/) 读取绑定写入程序，其余程序由模板产生。每项都以固定输出校验结果。`--check` 核对 `fixed/` 中的字节与生成结果一致；`--smoke` 使用八轮循环，`--oracle node` 通过 Node 核对生成器声明的输出。

```sh
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 docs/performance/probes/build_fixed_matrix.py --smoke --output "$OUTPUT/smoke" --oracle node
python3 scripts/benchmark/fixed.py --manifest docs/performance/probes/fixed/manifest.json \
  --workload-dir docs/performance/probes/fixed \
  --engine reference="$REFERENCE_QJS" --engine candidate="$CANDIDATE_QJS" \
  --repeat 8 --order abba-baab --output "$OUTPUT/fixed"
```

正式测量使用普通 release 构建，记录两侧的源码、二进制与负载身份。逻辑完成与交接次数由单独的 profiling 构建采集。采样和成本字段见[测量方法](../measurement.md)。

## Callgrind 探针

[`allocation/ledger.py`](allocation/ledger.py) 生成对象字面量、`new` 与空循环探针，在 N、2N、4N 下用 Callgrind 的调用链上下文把每次迭代的指令拆成互斥的阶段与机制。

[`runtime/probes.py`](runtime/probes.py) 在上述三个探针之外加入绑定、属性、数组、调用与构造探针（`s=o`、`s=o.x`、`o.x=i`、`o.x=p`、`a[i&1023]=i`、`a[i&15]=p`、`f(i)`、`o.m(i)`、`g(p)`、`new E()`）。每项校验输出，按 N/2N/4N 斜率报告每次迭代的 Ir、扣除空循环后的净值，以及按函数 self 成本的 2N−N 差值；aarch64 上内联使调用链阶段拆分失真，函数 self 差值是主要拆分。`--v8` 加入固定迭代的 V8 v7 用例，报告 Ir/Dw、引用计数 self 占比和每个函数的 self 成本；`--v8-case CASE[=ITERATIONS]` 选择用例与迭代数。

```sh
python3 docs/performance/probes/runtime/probes.py --qjs "$CANDIDATE_QJS" \
  --v8 "$V8_V7_DIR" --output "$OUTPUT/probes"
```

[`runtime/classify.py`](runtime/classify.py) 把 `probes.py` 报告中 V8 用例的函数 self 成本归入读缓存、dense 数组、属性写入、调用与返回、释放/GC/分配、RegExp 与内建、解释循环与数值等类别；`--other` 列出最大的未分类函数。

[`runtime/ic_events.py`](runtime/ic_events.py) 用 profiling 构建运行八项 V8 用例，统计读缓存事件的种类（单态与多态首项/后项命中及其原型命中、guard 失败、超容量跳过、访问器站点、冷站点、未命中），并写出 `ic.json`。

```sh
python3 docs/performance/probes/runtime/classify.py "$OUTPUT/probes/probes.json"
python3 docs/performance/probes/runtime/ic_events.py "$PROFILING_QJS" "$V8_V7_DIR" "$OUTPUT/ic"
```

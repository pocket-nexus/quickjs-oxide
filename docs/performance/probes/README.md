# 固定工作量诊断

[`build_fixed_matrix.py`](build_fixed_matrix.py) 生成项目自有的 20 份 JavaScript 程序和 [`fixed/manifest.json`](fixed/manifest.json)。manifest 保存每份源码的 SHA-256 与预期 stdout，供 `scripts/benchmark/fixed.py` 在相同 guest 工作量下比较两个引擎构建。

| 路径 | Case |
| --- | --- |
| 数值局部计算与转换 | `numeric_local_loop`、`numeric_coercion_loop` |
| 普通局部读取 | `local_read_with_setup`、`local_read` |
| 值与对象操作 | `bigint_32/64/256`、`prop_read/write`、`array_read/write`、`call0`、`string_bridge` |
| 绑定写入 | `local_move`、`argument_move`、`number_owner_fallback`、`object_move`、`empty_loop` |
| 异常语义 | `type_error`、`tdz` |

生成器从 [`ordinary-writes/`](ordinary-writes/) 读取绑定写入程序，其余程序由模板产生。每项都以固定输出校验结果。`--smoke` 使用八轮循环，并通过 Node 核对生成器声明的输出。

```sh
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 docs/performance/probes/build_fixed_matrix.py --smoke   --output /tmp/oxide-fixed-matrix-smoke --oracle node
python3 scripts/benchmark/fixed.py   --manifest docs/performance/probes/fixed/manifest.json   --workload-dir docs/performance/probes/fixed   --engine reference="$REFERENCE_QJS" --engine candidate="$CANDIDATE_QJS"   --repeat 8 --order abba-baab --output "$OUTPUT/fixed"
```

正式测量使用普通 release 构建，记录两侧的源码、二进制与负载身份。逻辑完成与交接次数由单独的 profiling 构建采集。采样和成本字段见[测量方法](../measurement.md)。

[`allocation/ledger.py`](allocation/ledger.py) 生成对象字面量、`new` 与空循环探针，用 Callgrind 的调用链上下文把每次迭代的指令拆成互斥的阶段与机制，结果见[对象分配成本账本](../runtime-allocation-ledger.md)。

[`runtime/probes.py`](runtime/probes.py) 在上述三个探针之外加入第 2–4 项的探针（`s=o`、`s=o.x`、`o.x=i`、`o.x=p`、`a[i&1023]=i`、`a[i&15]=p`、`f(i)`、`o.m(i)`、`g(p)`、`new E()`），每项校验输出，按 N/2N/4N 斜率报告每次迭代的 Ir、扣除空循环后的净值，以及按函数 self 成本的 2N−N 差值（aarch64 上调用链阶段拆分不可靠，以此为主要拆分），可选地附带固定迭代的 Richards/DeltaBlue/NavierStokes 的 Ir/Dw 与引用计数 self 占比。[`runtime/measure_docker.py`](runtime/measure_docker.py) 在 `oxide-vg:1.88` 容器中构建给定源码或修订并运行整套测量，输出 JSON 供 A/B 比较：

```sh
python3 docs/performance/probes/runtime/measure_docker.py --source . --rev <commit> \
  --target-subdir item2-a --json /private/tmp/item2/a.json
```

`runtime/probes.py` 的 `--v8-case CASE[=ITERATIONS]` 可选择八项中的任意用例，报告保留每个函数的 self 成本；
[`runtime/classify.py`](runtime/classify.py) 把它们归入读缓存、dense 数组、属性写入、调用与返回、释放/GC/分配、RegExp 与内建、解释循环等类别。
[`runtime/ic_events.py`](runtime/ic_events.py) 用 profiling 构建统计八项中读缓存命中的种类（单态、多态首项/后项、原型命中、未命中）。

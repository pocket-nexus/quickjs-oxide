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

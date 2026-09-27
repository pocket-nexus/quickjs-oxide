# 固定工作量 VM 诊断矩阵

[`build_fixed_matrix.py`](build_fixed_matrix.py) 生成项目自有的 JS 负载与带 SHA-256、预期 stdout 的 [`fixed/manifest.json`](fixed/manifest.json)。这些负载用于比较相同工作量下的引擎成本，不代表原版 V8 benchmark 的子项或总分。计数是首轮估值，须在空载主机上试运行后校准到约 0.1–0.5 秒；校准须改变生成器和负载哈希，并为比较双方重新生成同一矩阵。

| 类别 | Case | 要观察的路径 |
| --- | --- | --- |
| 数值专用操作 | `fusion_hit`、`fusion_dynamic_miss` | Number 计算与 Object 转换的不同成本；需核对当前发布的 opcode 与动态路径 |
| 普通局部读取 | `fusion_flag0`、`fusion_no_plan` | 两者循环正文相同，前者多一次循环外的数值加法；名称保留历史输入身份 |
| 基础执行 | `bigint_32/64/256`、`prop_read/write`、`array_read/write`、`call0`、`string_bridge` | 检查未命中及广泛路径的新增成本；`string_bridge` 覆盖字符串加法 |
| 异常 | `type_error`、`tdz` | 每轮捕获并检查异常类型，防止优化改变 TypeError/TDZ 路径 |
| 历史债务 | `local_move`、`argument_move`、`number_owner_fallback`、`object_move`、`empty_loop` | 原样复用 [普通 Number 写入探索收据](../receipts/ordinary-number-writes-2026-09-26/README.md)中的五份源码 |

`fusion_*` 沿用旧实验名称，**不是单靠 JS 源码就已证明的发布后站点分类**。
#52 没有 `FusionPlan`、逐 PC flag 或 fusion sidecar。性能归因前应检查当前
`ExecCode` 的普通／专用 opcode、执行字 PC 和实际命中／回落路径；现有计数器的
覆盖限制见 [profiling](../../profiling.md)。静态上不存在的候选要单独报告，
不应被“命中率”分母掩盖，也不能把旧名称解释为当前编译器的保证。

2026-09-26 的历史修订：原来两份非候选负载使用 `while(n>0)`，独立的比较与分支也可发布融合候选，因此旧名称不能证明整个函数无计划。两者改为相同的 `while(n)` 热循环，并由当时 `fusion.rs` 的发布器测试检查 `FusionPlan`：`fusion_no_plan` 为 `None`，`fusion_flag0` 为 `Some` 且热循环局部读取的 flag 为零。这仅描述旧实现的认证。旧测量收据保留原样；修订改变下表两项源码哈希，其余 18 项未变。

| Case | 旧 JS SHA-256 | 新 JS SHA-256 |
| --- | --- | --- |
| `fusion_flag0` | `1b48f28344e148dcb425857a008881ec8aebf2d09d4c82f1b0ec4ebc52729ef6` | `92df3e1420dafee42a615e883679737deb0021eb081e2bc05aafd63ec45829f1` |
| `fusion_no_plan` | `7d41f8cf6f6e823a498a4f78de7ae0ac6173d430cabdcd4fe1448a77e2008d80` | `b4eeccb8624c086b664f7cb26741e82a946ca155fb2e738fbba46e372f172b7c` |

生成与核对：

```sh
python3 docs/performance/probes/build_fixed_matrix.py
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 docs/performance/probes/build_fixed_matrix.py --smoke \
  --output /tmp/oxide-fixed-matrix-smoke --oracle node
```

`--smoke` 只把每项循环缩至八轮，通过 Node 核对生成器声明的预期输出，不计时也不验证 VM。正式固定工作量使用 `scripts/benchmark/fixed.py`，例如 `--manifest docs/performance/probes/fixed/manifest.json --workload-dir docs/performance/probes/fixed`；该 runner 会核对每份 JS 的 SHA-256 和 stdout，并交错执行所给引擎。`fixed.py` 报告的是进程 wall time；退休指令、cycles、分支和 cache 事件另按 [测量协议](../measurement.md)采集，同次记录完整二进制、构建和主机身份。先做 A/A，再做交错 A/B，同时保存逐项与累计结果。

外部 V8 子项仍使用独立固定版本的 checkout；这里没有复制其源码，也不能由这个矩阵推导 V8 总分。

`run_dump.py` 与 `dump_numeric_spans.rs` 保留作历史 `FusionPlan` 实验的复现工具，
依赖对应收据的旧源码，不能直接用于 #52。编译 IR capture 也不等于发布后的
`ExecCode` dump；新站点需要当前编码的独立检查或诊断支持。

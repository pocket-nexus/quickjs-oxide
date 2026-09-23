# S3 全量同协议重测结果（第 1 轮，本机）

> **2026-09-23 后续裁决：** 本文件数据（连同
> [B/C 实施记录](s3-bc-implementation.md) §7 的 profile）已用于关闭阶段 C、
> 整体回退 B/C 实现，见 [阶段 C 负结果与撤回落](s3-c-negative-result.md)。

对应计划：`docs/reports/s3-full-rerun-plan.md`。本文件只记录第 1 轮实测，
不把单轮本机结果当作正式验收结论。

## 1. 身份与协议

| 项 | 值 |
| --- | --- |
| 源码 | `e0c506f8`（干净工作区，Clang 清理后） |
| 冻结导出 | `target/s3-bc-repair/round10-full/source.json`（1273 文件，空 patch） |
| 工具链 | rustc 1.94.1（2026-03-25），release，fat LTO，CGU=1，无 PGO，无 profiling |
| 测量主机 | 本机 16 逻辑核，governor=powersave，`taskset -c 2`，串行 |
| 轮次 | 第 1 轮，2026-09-22T13:39Z–20:20Z（约 6.7h，含 original 3h 超时） |

引擎（SHA-256 见 `summary.json` 与计划文档）：

| 名称 | 说明 |
| --- | --- |
| `pre_a` | 已认证 pre-A 发布二进制（`17694ed4…`） |
| `m0` | 当前默认 canonical（`2547a85e…`） |
| `m1` | `--cfg oxide_quick_projection`（`8380a1a4…`） |
| `c` | scalar+owned TOS + store-drop（`fd370b76…`） |
| `bc` | C + quick dispatch（`748a3e6d…`） |
| `quickjs` | pinned QuickJS 2026-06-04（`d0f8966b…`） |

## 2. A/A 噪声门（同二进制双标签，scaling 84 组合）

`m0_b/m0_a`：几何均值 **0.9987**，p50 1.0008，min 0.875，max 1.063。

- 聚合层面偏差 <0.2%，可用于套件级判定；
- 单项噪声可达 −12%~+6%，因此单项 3–5% 的差异不足以单独判定，
  本文件对单项只列方向、不宣称收益。

## 3. 总表（对 pre_a 的几何均值比）

时间类指标（scaling/fixed/original/property/compile/microbench）：**<1 表示比 pre_a 快**；
V8 Score（越高越好）：**>1 表示比 pre_a 快**。

| 套件 | m0 | m1 | c | bc | quickjs | 说明 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| scaling 86 | **0.960** | 0.990 | 0.989 | 0.990 | 0.117 | QuickJS 快 8.6× |
| scaling-regexp | **0.948** | 0.936 | 0.957 | 0.937 | 0.186 | QuickJS 快 5.4× |
| property 3 项 | **0.736** | 0.741 | 0.742 | 0.749 | 0.095 | QuickJS 快 10.6× |
| microbench 8 项 | 0.750 | 0.688 | 0.688 | 0.688 | 0.077 | 50ms 量化，仅作参考 |
| V8 isolated（Score） | **1.045** | – | 1.017 | 1.013 | 17.106 | QuickJS 快 17.1× |
| V8 combined（Score） | **1.035** | – | 1.000 | 1.000 | 16.575 | QuickJS 快 16.6× |
| fixed 58 | **0.912** | 0.895 | 0.895 | 0.890 | 0.088 | QuickJS 快 11.4× |
| compile 67 | **0.993** | 1.006 | – | – | – | m1/m0 = 1.013 |
| original 9（wall） | **0.977** | 0.983 | 0.983 | 0.981 | 0.191 | QuickJS 快 5.2× |

**方向修正**：QuickJS 在这些套件上均比 quickjs-oxide 快（scaling 8.6×、property 10.6×、
fixed 11.4×、original 5.2×、V8 17.1×、microbench ~13×）。此前沟通中把该方向说反，
以本表为准：比值 <1 表示该引擎耗时是 pre_a 的比例，quickjs 的 0.117 意味着
QuickJS 耗时只有 pre_a 的 11.7%。

## 4. 分套件明细

### 4.1 scaling（86 组合，32768 ops，repeat 5）

m0/pre_a：geo 0.960，p50 0.975，min 0.737，max 1.171（84 项）。
m1/c/bc 都在 0.989–0.990，比 m0 慢约 3%，但仍优于 pre_a。
说明 C 修复的收益在 m0 上体现最明显，投影/派发实验档在这批数据负载上没有增量。

### 4.2 V8-v7 isolated（Score，越高越好）

| case | m0 | c | bc | quickjs |
| --- | ---: | ---: | ---: | ---: |
| crypto | 1.184 | 1.181 | 1.204 | 21.28 |
| deltablue | 1.028 | 0.973 | 0.969 | 17.45 |
| earley-boyer | 0.984 | 0.959 | 0.943 | 27.59 |
| navier-stokes | 1.071 | 1.079 | 1.018 | 10.29 |
| raytrace | 1.010 | 0.980 | 0.990 | 27.27 |
| regexp | 1.033 | 1.014 | 1.019 | 7.04 |
| richards | 1.065 | 1.020 | 1.020 | 24.58 |
| splay | 0.994 | 0.952 | 0.964 | 14.75 |

m0 几何均值 1.045：crypto/navier/richards 领先 pre_a，earley-boyer/splay 略负。
c、bc 相对 pre_a 仍为正（1.017/1.013），但低于 m0。

### 4.3 fixed 58（repeat 10）

m0/pre_a：geo 0.912，p50 0.970，min 0.570，max 1.204。

- 领先：`prop_read` 0.570、`array_length_read` 0.572、`string_length` 0.597、
  `global_write` 0.690、`prop_clone` 0.701；
- 回退：`int_to_string` 1.204、`string_build3` 1.178、`string_build1` 1.170、
  `string_build_large1` 1.170、`map_delete` 1.128。
  字符串构造/数字转字符串是当前默认档相对 pre-A 的集中回退点，需归因。

### 4.4 compile 67（compile probe，repeat 10）

- m0/pre_a 0.993（min 0.958，max 1.028）→ 编译成本与 pre-A 持平；
- m1/pre_a 1.006，m1/m0 **1.013**（max 1.043）→ B 门槛 geomean≤1.02 通过，
  但单项最大 +4.3%，接近 3% 复核线，第 2 轮需关注。

### 4.5 original 9（wall，repeat 5；`all` 项由 `original-rest*` 补跑合并）

m0/pre_a：geo 0.977，p50 0.990，min 0.853（crypto），max 1.037（deltablue）。
单项 m0：crypto 0.853、all 0.955、navier-stokes 0.986、earley-boyer 0.988、
regexp 0.990、richards 0.994、raytrace 1.001、splay 1.001、deltablue 1.037。

### 4.6 BigInt 固定工作量（3 次墙钟 + perf `:u` + rss_wait4）

| 工作量 | m0 wall/pre_a | m0 instructions/pre_a | RSS pre_a→m0 (KiB) |
| --- | ---: | ---: | --- |
| bigint32 | 0.79–0.84 | 0.98 | 9028→8840 |
| bigint64 | ~0.90 | 1.12 | 9128→9012 |
| bigint256 | **1.21** | **1.43** | 9140→8940 |

bigint256 的回退（墙钟 +21%，指令数 +43%）自阶段 A 起未关闭，仍是当前最大单项回退。

### 4.7 首次执行与资源 RSS（B 专用输入，`quick-resource-inputs-v2`）

- first-execution probe（5 输入 × 5 次）：
  - compile_ns：m0 对 pre_a −6%~+4%，m1 对 m0 +1%~+4.5%；
  - first_execute_ns：m0/pre_a 0.90–1.10，m1/m0 0.96–1.07；
  - `repeated_eval_release`：m0 662ms 对 pre_a 698ms；`one_large_function`：
    m0 1.12ms 对 pre_a 2.22ms。
  - 未观察到投影发布导致的系统性首次执行回退。
- resource RSS（5 输入 × 3 次，`rss_wait4` ru_maxrss）：
  - m0 对 pre_a：+56~+252 KiB（约 +0.3%~0.9%）；
  - m1 对 m0：−156~+92 KiB；bc 对 m0：±~150 KiB。
  - 这批输入未复现 >1 MiB 的 RSS 增量；与 B1b 早前失败门禁所用输入/口径不同，
    **不能据此关闭 B1b 内存门禁**。

## 5. 结论

1. **相对 pre_a**：当前默认 m0 在 scaling（−4.0%）、fixed（geo −8.8%，median −3.0%）、
   V8 isolated（Score +4.5%）、property（−26%）、original（−2.3%）、compile（−0.7%）
   全面不劣于 pre-A；集中回退为 bigint256（+21%）与 fixed 中的字符串构造类单项。
2. **实验档**：c/bc 在 scaling 上比 m0 慢约 3%、V8 上略低但仍优于 pre_a；
   m1 的编译代价 +1.3%、首次执行与 RSS 无显著代价。B1e 的“默认启用”决定
   需要第 2 轮与归因证据，当前数据不足以支持启用。
3. **对 QuickJS**：全部套件仍明显落后（5–17×），是后续阶段的主目标。
4. **限制**：单轮；本机非 PocketLab；未跑当前源码的 Test262；governor=powersave；
   单项噪声 ±6–12%；`original` 套件因 3h 超时用补跑合并（已完成 9/9）。
   正式验收仍需目标机、双轮与 Test262 receipt。

## 6. 证据路径

- 汇总：`target/s3-full-rerun/round1/summary.json`
- 状态与日志：`target/s3-full-rerun/round1/status.jsonl`、`logs/`
- 各套件原始样本：`target/s3-full-rerun/round1/<suite>/`
- 探针收据：`target/s3-full-rerun/probes/*/build.json`
- 驱动与分析：`target/s3-full-rerun/run_round.py`、`analyze.py`

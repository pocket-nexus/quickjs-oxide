# S3 全量同协议重测计划（C/B 后）

状态：执行中。本文记录本轮“全量同协议重测”的冻结身份、引擎集合、矩阵、命令与判定口径；
最终数据另写结果文档，不把计划当结果。

## 1. 目的

在 C1–C4、B1a–B1d 集成并完成 §9 修复（提交 `33a02659`）与 Clippy 清理（提交 `e0c506f8`）
之后，重新跑一遍与阶段 A 相同协议的全量矩阵，回答：

1. 当前默认引擎（M0）相对已认证 pre-A 发布二进制（`pre_a_release`）的累计回退；
2. 相对 pinned QuickJS oracle 的外部对照；
3. C 组合（`c`）与 C+B 组合（`bc`）相对 M0 的收益/代价；
4. B1 投影（`m1`）在编译与首次执行上的代价；
5. A/A 噪声与本机测量分辨率，判定上述差异是否可辨。

## 2. 冻结身份

| 项 | 值 |
| --- | --- |
| 源码 | `e0c506f8`（干净工作区），冻结导出 `target/s3-bc-repair/round10-full/source` |
| 导出身份 | `target/s3-bc-repair/round10-full/source.json`（1273 文件，空 patch） |
| 工具链 | 本机默认 rustc 1.94.1（2026-03-25），release，fat LTO，CGU=1，无 PGO，无 profiling |
| 目标 | `x86_64-unknown-linux-gnu` |
| 测量主机 | 本机（Ryzen 7 7840HS 级别，16 逻辑核），governor=powersave |
| 固定 CPU | `taskset -c 2`（与 C1 矩阵一致） |
| 执行方式 | 串行，计时期间不跑构建/测试；每套件独立输出目录与日志 |

### 引擎

| 名称 | 路径 | SHA-256 | 说明 |
| --- | --- | --- | --- |
| `pre_a` | `target/s3-c-b-next/pre-a/x86_64-unknown-linux-gnu/release/qjs` | `17694ed4…` | 已认证 pre-A 发布二进制 |
| `m0` | `target/s3-bc-repair/round10-full/m0/plain/release/qjs` | `2547a85e…` | 当前默认（canonical） |
| `m1` | `target/s3-bc-repair/round10-full/m1/plain/release/qjs` | `8380a1a4…` | `--cfg oxide_quick_projection` |
| `c` | `target/s3-bc-repair/round10-full/c/plain/release/qjs` | `fd370b76…` | scalar+owned TOS + store-drop |
| `bc` | `target/s3-bc-repair/round10-full/bc/plain/release/qjs` | `748a3e6d…` | C + quick dispatch |
| `quickjs` | `target/oracle/quickjs-2026-06-04/qjs` | `d0f8966b…` | pinned 外部对照 |

编译/首次执行探针（`oxide-compile-probe` / `oxide-first-execution-probe`）：
`pre_a`、`m0`、`m1` 三份，同 rustc、同 LTO/CGU 环境，`m1` 额外 `RUSTFLAGS=--cfg oxide_quick_projection`。

## 3. 输入

- 86 项 scaling：`scripts/benchmark/scaling_workloads.py` 的 21 个 case × {32,128,512,2048}
  + `regexp-groups` × {32,128}；`--operations 32768 --repeat 5`。
- 58 项 fixed：`target/s3-c-opening/inputs/data-structure-fixed-final.json`
  + `fixed-workloads/`，`--repeat 10`。
- 67 项 compile、9 项 original：`target/s3-c-opening/inputs/recovery-receipt.json`
  + `replay-workloads/`。
- property 诊断：`property_read_probe.py --iterations 5000000 --repeat 7`。
- microbench：pinned `target/oracle/quickjs-2026-06-04/tests/microbench.js` 8 项，`--repeat 5`。
- V8-v7：`/tmp/opencode/js-engine-benchmark`（commit `c8e13f1e…`，工作区干净），
  isolated 8 项 + combined，`--repeat 5`。
- BigInt 诊断：`target/s3-c-opening/inputs/bigint-workloads/{bigint32,bigint64,bigint256}.js`
  + `perf stat`（cycles/instructions/branches/branch-misses, `:u`）+ 冻结的
  `rss_wait4`（`target/s3-c-b-next/rss_wait4`，wait4 ru_maxrss）RSS，各 3 次墙钟重复。
- A/A：`m0` 同二进制双标签跑 scaling 全矩阵，检验噪声门。

## 4. 矩阵与顺序（第 1 轮）

1. scaling（6 引擎：pre_a/m0/m1/c/bc/quickjs）
2. scaling-regexp（同 6 引擎）
3. property（6 引擎）
4. microbench（6 引擎）
5. v8 isolated（5 引擎：pre_a/m0/c/bc/quickjs）
6. v8 combined（同 5 引擎）
7. fixed 58（6 引擎）
8. compile 67（pre_a/m0/m1 探针）
9. original 9（6 引擎）
10. BigInt 32/64/256 perf+RSS（pre_a/m0/c/bc）
11. first-execution 探针（pre_a/m0/m1）
12. A/A（m0 双标签）

第 2 轮独立复现：引擎顺序反转后重跑 1–9，用于验收；是否执行由第 1 轮噪声与时间决定。

## 5. 输出

- 根目录：`target/s3-full-rerun/round1/`
- 每套件独立子目录（`scaling/`、`v8-isolated/`…）与 `logs/<suite>.log`；
- 进度与退出码：`status.jsonl`；
- 构建/探针收据：`target/s3-full-rerun/probes/*/build.json`；
- 汇总（后续）：`summary.json`、结果文档 `docs/reports/s3-full-rerun-results.md`。

## 6. 判定口径

- 以各套件 `results.json` 的每样本耗时为原始数据；比较用几何均值比（after/before），
  并保留单项与原始样本，不只报 geomean。
- 可辨性：先看 A/A 同二进制比值的离散度；差异小于 A/A 噪声或落在单项正负混杂内的，
  记为 inconclusive，不记收益。
- 失败/超时样本不计入分母，但必须在结果中列出数量与原因。
- QuickJS 对照仅作外部参照，不参与“回退”判定。

## 7. 已知限制

- 本机不是计划指定的 PocketLab 静默测量机；本轮结果按“本机同协议诊断”记录，
  不冒充正式验收结论，正式验收仍需目标机复跑。
- governor=powersave，与阶段 A 同协议；若与历史记录不同需在结果文档标注。
- V8 套件按历史耗时约 23 分钟/引擎，是总时长主项。
- original 套件实测极慢：crypto≈53s、raytrace≈25s、earley-boyer≈89s、`all` 约数分钟
  （每引擎每轮），完整 9 项 × 5 轮 × 6 引擎可达数小时；首轮若被超时中断，
  用 `original/samples.jsonl` 中已完成的样本 + 补跑 `original-rest/` 合并，
  不把中断当失败丢弃。

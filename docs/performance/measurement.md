# 性能测量方法

本页使用仓库中的 [`scripts/benchmark`](../../scripts/benchmark/README.md) 工具和 [固定工作量矩阵](probes/README.md)。每次比较记录候选与对照的完整源码 SHA、工具版本、编译配置、二进制 SHA-256、负载 SHA-256、主机信息、原始样本和输出校验。

## 构建与采样

普通 release 构建用于时间和硬件计数。profiling 构建用于逻辑事件与生命周期。双方使用相同工具链和构建配置，各自使用独立 target 目录。`build.py` 生成源码、工具和二进制身份回执。

```sh
python3 scripts/benchmark/build.py --repo "$SOURCE_TREE"   --plain-only --jobs 2 --plain-target "$OUTPUT/plain"
python3 docs/performance/probes/build_fixed_matrix.py --check
python3 scripts/benchmark/fixed.py   --manifest docs/performance/probes/fixed/manifest.json   --workload-dir docs/performance/probes/fixed   --engine reference="$REFERENCE_QJS" --engine candidate="$CANDIDATE_QJS"   --repeat 8 --order abba-baab --output "$OUTPUT/fixed"
```

采样先用同一二进制的 A/A 对照确认时间分辨率，再用交错 A/B 对照。每份程序固定 guest 工作量，逐次验证 stdout、stderr 和退出状态。包含写入的应用每次使用新的应用状态；属性实验在记录的缓存策略下开始。固定迭代、完整进程耗时、编译探针、正式 V8 Score 和 profiling 计数分别命名。

## 结果字段

| 成本 | 记录内容 |
| --- | --- |
| 执行 | 固定工作量耗时、可用的退休指令与 cycles、热点 PC 和完成/交接计数 |
| 准备 | 编译耗时、分配次数与字节、发布耗时 |
| 表示 | 执行字、元数据、生成代码及二进制体积 |
| 生命周期 | 帧与堆容量、owner 保留与释放、回收队列和峰值内存 |

结果表逐项列出样本数、中位数、配对比值和 A/A 范围。属性读取可分别测冷、稳定、访问器及形态变化；普通赋值覆盖调用结果和属性结果。编译与执行使用各自的测量入口。可复核的当前样本见[当前测量](current-measurement.md)。

## 语义核对

在相同负载下比较输出、异常身份、效果顺序和资源清理。Rust workspace 测试、Test262 与定向所有权测试覆盖语言行为；性能样本使用普通构建并保留逐次原始数据。当前命令见[状态页](../status.md#verification)和[benchmark 工具说明](../../scripts/benchmark/README.md)。

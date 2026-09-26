# 十分钟固定迭代 V8 runner 静态审查

审查身份：仓库 `5e3dedc9e3bfd717fe50f7187d08db1247c224d0`；`iterate_v8.py` SHA-256 `5abd0cb82d0f13ff03101731754e29778a990e13b635c4b3029854210bb79cff`，`test_iterate_v8.py` SHA-256 `24a56956b0562a93f4b5c8f227501d29d72b0ab86b66f3defecca5aa533c42c8`。上游 V8 checkout 为 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。仅阅读源码、测试和上游 pinned JS；没有运行引擎、测试或 benchmark。

## 结论

未发现会把失败、超时或缺失样本算进九项总体比值的缺陷。冻结工作量和构建身份的关键检查都有实现。它交付的是八个 isolated 及 combined 的固定迭代整进程对照，**不是原版自适应 V8-v7 Score**。单次调用的采样共享 600 秒截止时间；源码不能保证 CLI 从入口到返回严格不超过 600 秒，也不保证任意机器能在截止前完成九项。

## 逐项核查

| 条目 | 代码证据与判断 |
|---|---|
| 截止时间 | `iterate_v8.py:330` 建立唯一 monotonic 截止；pilot (`:375-400`) 与 formal A/A、A/B (`:435-464`) 的每次 `run_one` 共用它。`run_one:181-195` 在身份检查前后检查剩余时间，并把单样本 timeout 限为 `min(sample_timeout, remaining)`；`run.py:231-262` 的 watchdog 超时杀整个进程组。校准只在 pilot 后用剩余预算，预算不足降为完整 ABBA 或返回失败。 |
| 不完整结果 | `summarize:222-269` 要求样本数量、逐项顺序和全部 `status == ok` 同时成立才计算 aggregate。pilot 缺项或失败在 `:393-419` 返回 `aggregate: null`；formal 截止/错误在 `:451-464` 保留原样本并以非零退出。合格样本还要 `:205-218` 精确比对 stdout marker、空 stderr 和 Darwin 计数。 |
| 原始 JS 行为 | `profile_v8.py:32-55` 锁定外部 checkout commit、clean tracked tree 和 `run.js` 中 base + 八项顺序。`iterate_v8.py:115-145` 逐字节读取 `base.js` 和各 body，按原顺序生成 isolated/combined；`:95-109` 对每个原始 Benchmark 执行一次 Setup、固定 warmup 次 run、冻结次数的 run、一次 TearDown。原版 `base.js:247-279` 也是每个 Benchmark 一次 Setup/TearDown，正文中的抛错和校验仍运行。默认 warmup 为零、迭代数冻结，故与原版自适应执行次数不同，脚本已明确标注。 |
| RNG | pinned `base.js:84-99` 加载时初始化确定性 `Math.random`；生成输入每进程只加载一次 base，不插入 ResetRNG。上游该 pin 无 ResetRNG；combined 按 `run.js` 顺序推进 RNG，isolated 各自在新进程从初始 seed 开始，符合其独立运行口径。 |
| 冻结重放 | `:272-291` 检查 freeze schema、源 commit/tree/文件路径与 hash、工具 hash、次序、重复数、warmup、每项重新生成 JS 的完整元数据及 hash；`:183-186` 每个样本前复核工具、源、生成 JS 与 engine bytes。`:365-374` 复制同一 freeze-plan 字节到新输出，计数不按候选重新校准。 |
| A/A、A/B | `:165-176` 每 case 先 same-binary A/A 后 baseline/candidate A/B；`run.py:47-60` 在 repeat=4、`abba-baab` 下形成 ABBA-BAAB，在 repeat=2、`abba` 下形成 ABBA。`test_iterate_v8.py:56-71` 覆盖默认 144 jobs 和首个完整块。 |
| plain 拒绝 | `:48-72` 要求经 binary SHA 绑定的 build receipt、`mode=plain`、空 features、成功 release/干净 source，并比较 rustc、cargo、目标和 codegen；`run.py:64-76` 验证 binary SHA 与 receipt 一致。profiling 构建 receipt 的 mode/features 不满足条件。 |

## 有界限制与测试缺口

1. **“600 秒硬截止”是采样启动与单样本 watchdog 的界限。** 截止钟在 `main:330` 开始，但命令元数据收集、生成文件、`Popen` 建立、watchdog 回收、结果写盘的耗时可能使整个 CLI 略超 600 秒。不会据此产生成功的未完成总体；对外可称“所有采样共享 600 秒截止”，避免把它写成操作系统级整命令时限。
2. 成功样本的 stdout 与预期完成标记逐字节相等，stderr 必须为空；`samples.jsonl` 存原始文件路径，但 stdout/stderr 没有采样时 SHA。后续独立审计原始文件完整性时，需依赖冻结目录的外部封存。该缺口不改变当前成功判定。
3. 合成测试覆盖生成字节、次序、重放、缺项/超时拒绝和过期不启动；**没有直接单测** `require_plain_engine` 对 profiling receipt 的拒绝、含 Darwin 计数的 marker 拒绝路径或完整 CLI 入口。以上是覆盖缺口，不是本次发现的生产逻辑失败；若后续扩展工具，优先补纯合成测试。

审查没有执行动态正确性验证，因此不能由此证明当前 Rust 二进制会完成所有 pinned V8 body。实际结果仍以逐样本 `status`、completion marker、原始输出及完整矩阵为准。

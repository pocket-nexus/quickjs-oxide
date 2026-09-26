# 2026-09-26 普通 Number 写入：探索性固定工作量测量

本收据记录本机 `/tmp/oxide-number-write-29560e08/{aa,ab2,ab3}` 的结果。基线是干净的 detached checkout `29560e08e37e7e3465fb355be5641b26da374576`，CLI SHA-256 为 `fa5e1f681def114297c3ce7fb83945e080981bfa13c569e58fae32736cdad03e`。候选 CLI SHA-256 为 `ea0c7f926c0dc66942809b318260d879147bf0abb72244140f157598d6b8052b`；测量元数据记录候选仓库 HEAD 同为 `29560e08`，但工作区有未提交的文档、benchmark 和 VM 改动，不能由该 commit 单独还原候选源码。

两份二进制均位于 `target/release/qjs`，两棵源码树的 `[profile.release]` 均指定 fat LTO、`codegen-units = 1`；本机核对时 `rustc` 为 1.96.0。测量元数据中的 `build` 均为 `null`，没有构建命令、编译器或有效 Cargo 覆盖参数回执，因此不能独立证明构建时实际使用的工具链和 flags。主机记录为 macOS 26.6.2 arm64、8 个逻辑 CPU，未记录 CPU 型号或亲和性。

五个完整 JS 负载保存在 [workloads](workloads/)；原 manifest SHA-256 为 `e87f47ca78b2d89d2b3d669fdf9b9f3794795c46950c37df897d80eb00aa78fb`。`aa` 用同一基线二进制的两个名字交错执行，每边每项 5 个独立进程；`ab2` 和 `ab3` 是两轮独立的基线／候选交错 A/B，每轮每边每项 7 个独立进程。50/50 个 A/A 与两轮各 70/70 个 A/B 样本的退出码、预期 stdout、空 stderr 均核对通过。下表是**整个进程 wall time 的中位数**，单位 ms；括号内为后者／基线的耗时比，低于 1 表示更快。

| 固定负载 | A/A：base → base_again | A/B 第 1 轮 `ab2`：base → candidate | A/B 第 2 轮 `ab3`：base → candidate |
| --- | ---: | ---: | ---: |
| `local_move`，600 万轮 | 494.274 → 496.041（1.0036） | 495.224 → 407.678（0.8232） | 495.032 → 407.320（0.8228） |
| `argument_move`，600 万轮 | 298.005 → 298.123（1.0004） | 298.318 → 253.808（0.8508） | 297.819 → 253.843（0.8523） |
| `number_owner_fallback`，100 万轮 | 134.577 → 133.881（0.9948） | 134.509 → 131.721（0.9793） | 134.442 → 131.493（0.9781） |
| `object_move`，100 万轮 | 90.717 → 90.006（0.9922） | 91.114 → 92.926（1.0199） | 90.364 → 91.334（1.0107） |
| `empty_loop`，1000 万轮 | 120.594 → 120.633（1.0003） | 121.496 → 120.742（0.9938） | 120.375 → 120.903（1.0044） |

两轮 `local_move` 与 `argument_move` 的候选耗时分别降低约 17.7% 和 14.8%；`number_owner_fallback` 降低约 2.1%。`object_move` 两轮分别增加约 2.0% 和 1.1%，是需要继续调查的对象路径回退信号。A/A 中位数差异为 −0.78% 至 +0.36%，`empty_loop` 两轮方向相反；小幅变化不能凭这份数据作稳定收益或回退裁决。

这是 dirty candidate 的本地 arm64 wall-time 微负载测量，没有退休指令、cycles、正式 V8 Score、完整构建回执或完整性能门禁。原始 `results.json`、`metadata.json` 与逐次 stdout/stderr 保留在上述本机目录；本收据只保留摘要和负载源码，不把它用作默认开启或总体性能结论。

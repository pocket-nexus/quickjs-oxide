# 五项优化计划：最终候选验收记录

**验收状态：产品源码 `b280ec8b77b9ff4d387118d31cfd5cbee644f477` 的正确性工具门禁、640/640 固定负载及四批各 72/72 的九项 V8 固定工作量对照均完成。固定工作量退休指令裁决完成；wall／cycles 受同机干扰，整体时间性能未裁决。** 候选 plain release 二进制 SHA-256 为 `fd98515d63dbcf001fe836f6510e6315f2c0ed68fbeba0f2d0708012a0542249`。[验证收据](validation-summary.json)记录 profiling 库 **2,158/2,158**、Rust 1.88 workspace/all-targets/profiling Clippy、格式和 626 个 reachable Rust 文件结构检查通过；产品的 fast 与 test262-focused CI 均成功；test262-full 被跳过，不能移用早期 full Test262 结果。[CI 身份](ci.json)。

协议固定外部 V8-v7 源 commit `2034d98fc8c5f8044e186267593f5d5ea5232caf`、原始八项 body 和顺序，Parent pilot 冻结每项迭代数，再重放同一 [freeze plan](freeze-plan.json)。每批八 isolated 加 combined，A/A 与 A/B 每侧 2 次 ABBA，单次调用 600 秒采样截止、单样本 90 秒上限。报告的是固定迭代**整进程** wall／cycles／退休指令，**不是原版自适应 V8 Score**；原版 Score 本轮按用户约十分钟完成全八项迭代的要求停止，也没有证明最初的总分 3–4 倍目标。[测量协议](measurement-decisions.md)。

## 五项结论

| 原计划 | 结果与决定 |
| --- | --- |
| **1. Parent／R0／B37 裁决** | 640 个固定负载样本分四组完整；对 Parent 和历史 c7 均无超过 2% 的退休指令回退，RSS 没有门限告警。Parent、c7、R0、B37 四批 V8 各 72/72 完成，四组都无 >2% 的逐项退休指令回退；多批 A/A 时间跨度很大，整体时间性能未裁决。 |
| **2. 真实负载成本与覆盖** | 八项中静态未融合 direct-read 站点占 **79.2%～95.1%**，动态 flag=0 访问占 **58.7%～99.2%**；Crypto 曾记录 **711,446** 次 ordinary-array materialized miss。[成本画像](call-cost-evidence.md)。符号 self% 不作为机制收益上界。 |
| **3. 发布事实传递到执行器** | 保留入口 choice 和已发布的首操作数／增减方向等 dense 静态事实。plain ARM64 中普通 local／parameter 读 **21** 处、写 **8** 处 out-of-line helper 调用消失，原有边界、TDZ、owner 与错误检查仍在；静态代码增大，固定工作量已验收指令成本，时间影响未裁决。[读机器码](codegen-read-current.md)、[写机器码](codegen-write-inline.md)。 |
| **4. Crypto 数组路径** | 保留现有有界 dense 恢复：窄撤回使 Crypto 固定工作量退休指令增加约 **38.7%**；一次 profile 记录 66 次 gap-write materialize 与 60 次恢复成功，不能相除为对象恢复率。[数组收据](profile-evidence.md)。 |
| **5. 调用路径** | method receiver 单 owner 性能转移两版均未通过累计 Object cycles 门禁，已撤回；只保留 copied receiver 立即进入 `CallInput` RAII 与 named-local 失败清理。[撤回审查](codegen-receiver-withdraw.md)。未启动 callsite cache 或帧重写。 |

## 完整固定负载：b280 相对 Parent 和 c7

下表为同一工作量下候选相对旧版的每侧中位数变化，负数表示候选更少；cycles 受同机干扰，仅作观察。A/A、Parent、c7 与直接切片四组各 160 个成功样本，合计 **640/640**；对 Parent、c7 均无 >2% 的退休指令回退，最大 RSS 与 footprint 均未触发 `max(3%, 1 MiB)` 复核线。[完整固定汇总](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/write-inline-fixed-summary.json)（SHA-256 `53cd9f7ddac0aa80128d292bf6f01b26847cd532b6842e6e50b5f386da0caef0`）。

| 固定负载 | 对 Parent 指令 | 对 c7 指令 | 对 Parent cycles | 对 c7 cycles |
| --- | ---: | ---: | ---: | ---: |
| `argument_move` | −25.49% | −6.65% | −23.35% | −7.14% |
| `array_read` | −4.05% | −0.01% | −2.28% | +0.51% |
| `array_write` | −4.93% | −1.93% | −3.28% | −2.34% |
| `bigint_256` | −2.85% | −1.66% | −8.39% | −3.01% |
| `bigint_32` | −6.69% | −3.93% | −11.44% | −2.73% |
| `bigint_64` | −3.53% | −2.07% | −7.38% | −4.37% |
| `call0` | −5.34% | −1.11% | −14.61% | −2.07% |
| `empty_loop` | −6.05% | +0.01% | −2.26% | +0.11% |
| `fusion_dynamic_miss` | −2.10% | −1.55% | −4.25% | −2.45% |
| `fusion_flag0` | −24.56% | −7.78% | −26.64% | −10.69% |
| `fusion_hit` | −16.57% | −5.49% | −17.40% | −6.02% |
| `fusion_no_plan` | −24.27% | −7.93% | −27.46% | −10.19% |
| `local_move` | −29.00% | −8.41% | −27.19% | −10.40% |
| `number_owner_fallback` | −9.31% | −6.71% | −11.38% | −8.67% |
| `object_move` | −5.57% | −4.52% | +0.16% | −6.99% |
| `prop_read` | −2.49% | −0.01% | +0.31% | −0.08% |
| `prop_write` | −3.36% | −1.18% | −1.42% | −1.70% |
| `string_bridge` | −0.33% | −0.61% | −5.55% | −7.30% |
| `tdz` | −0.40% | −0.13% | −0.63% | +0.44% |
| `type_error` | −0.48% | −0.23% | −1.21% | −0.08% |

`object_move` 对 Parent cycles **+0.16%**，不能宣称稳定时间收益；此前约 +7% 的信号未在该完整批次复现。其余小幅时间变化也受 A/A 与同机噪声限制。

## 固定 V8：九项与累计对照

Parent 首批 [原始结果](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/v8-write-inline-parent/results.json) **72/72 成功**。wall 列为 Parent／候选，指令与 cycles 列为候选相对 Parent 变化；A/A 跨度是四个同二进制 wall 样本的 min/max 相对中位数范围。RayTrace 的 wall A/A 跨度 **29.35%**，其 cycles +2.16% 不能裁定为稳定回退；combined wall 1.1641× 也不能据此宣称稳定收益。

| V8 固定项 | wall 旧/新 | 指令变化 | cycles 变化 | A/A wall 跨度 |
| --- | ---: | ---: | ---: | ---: |
| `richards` | 1.0546× | −0.18% | −1.63% | 3.09% |
| `deltablue` | 1.0097× | −0.19% | −0.21% | 7.60% |
| `crypto` | 1.3450× | −33.33% | −35.07% | 5.37% |
| `raytrace` | 0.9565× | −0.39% | +2.16% | 29.35% |
| `earley-boyer` | 1.0810× | −0.39% | −2.10% | 6.76% |
| `regexp` | 0.9906× | +0.09% | −0.21% | 2.96% |
| `splay` | 1.0328× | −0.37% | −2.33% | 2.19% |
| `navier-stokes` | 1.0731× | −7.12% | −5.67% | 3.09% |
| `combined` | 1.1641× | −6.15% | −8.36% | 3.86% |

四批 [Parent](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/v8-write-inline-parent/results.json)、[c7](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/v8-write-inline-previous/results.json)、[R0](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/v8-write-inline-r0/results.json)、[B37](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/v8-write-inline-b37/results.json) 各 **72/72 成功**，均重放 SHA-256 `ad6c98a6cd4f481fc9cb426359cff93600b8d02bc6b3cc916c5b9bf0763f9687` 的同一 freeze plan，候选均为 `b280ec8b` 的同一 plain binary。旧版源码依次为 Parent `29560e08`、c7 `c7fb5b69`、R0 `f531f605`、B37 `3341ac45`；[批次命令与时长](commands.json)均退出 0。下表 wall 为旧/新；指令和 cycles 为候选相对旧版变化。**固定指令裁决完成：四个分母的九项均无 >2% 指令回退。** wall／cycles 属受干扰观察，不能据此宣称稳定时间收益或回退。

| 对照（旧→b280） | 八 isolated wall 几何比† | combined wall 比† | combined 指令变化 | combined cycles 变化† | combined A/A wall 跨度 | 完整样本 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Parent→b280 | 1.0626× | 1.1641× | −6.15% | −8.36% | 3.86% | 72/72 |
| c7→b280 | 0.9441× | 0.7578× | −0.65% | +14.89% | 29.14% | 72/72 |
| R0→b280 | 1.1676× | 1.1428× | −10.42% | −10.52% | 8.49% | 72/72 |
| B37→b280 | 1.0953× | 1.6099× | −10.84% | −18.77% | 39.51% | 72/72 |

†c7 批次 combined wall A/A 约 **8.99–11.89 秒**，A/B baseline **7.75–12.93 秒**、candidate **10.63–16.66 秒**，cycles 也明显分散。B37 combined A/A wall 跨度 **39.51%**，Splay 单项跨度 **107.11%**；Parent RayTrace 单项跨度 **29.35%**。四组的时间比率保留原值但**整体时间性能未裁决**，不能把 Parent combined 1.1641× 或 B37 1.6099× 称为稳定收益，也不能把 c7 0.7578× 称为稳定回退。八 isolated 几何比与 combined 是两个独立指标。

各项固定工作的退休指令变化如下。值是候选相对对应旧版的 A/B 中位数差；负号表示候选更少。上段四个原始结果文件保留完整时间、cycles、RSS 和输出状态；其 SHA-256 依 Parent／c7／R0／B37 顺序为 `1874290c295c43145e4dced9ee7889256ba1282b1c25c0f69709989c2af49051`、`e6ce33748b19c26a86ecc5ac04ef9cc1e3111f4807055597f2a4624c0895a7e5`、`d465d549d07f815a39384c4b55fa9059f329124c3c7290950d5d8dfe4729c083`、`e92cfdfb1139da81766485a94c0eadadd1287063c63148f10ba9c88c5dc0ea76`。

| V8 固定项指令变化 | Parent→b280 | c7→b280 | R0→b280 | B37→b280 |
| --- | ---: | ---: | ---: | ---: |
| `richards` | −0.18% | +0.48% | −5.49% | −6.38% |
| `deltablue` | −0.19% | −0.16% | −5.20% | −6.61% |
| `crypto` | −33.33% | −3.05% | −36.66% | −36.80% |
| `raytrace` | −0.39% | −0.35% | −5.14% | −5.60% |
| `earley-boyer` | −0.39% | −0.40% | −1.93% | −2.93% |
| `regexp` | +0.09% | +0.11% | −0.59% | −0.55% |
| `splay` | −0.37% | −0.33% | −3.28% | −3.29% |
| `navier-stokes` | −7.12% | −2.18% | −36.55% | −36.67% |
| `combined` | −6.15% | −0.65% | −10.42% | −10.84% |

## 被否决的候选与证据边界

| 候选 | 关键负结果 | 决定 |
| --- | --- | --- |
| ordinary Number 撤回、early-`keep`、Number／Ready 拆分 | Number 撤回对 Parent `object_move` cycles 仍 **+6.478%**；另外两版也未修复约 7% 信号。[门禁](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/accepted-parent-gate-summary.json)。 | 保留 Number 快路。 |
| receiver 转移 `2d0eca7f` 与 compact `b19003ea` | 对 Parent Object cycles **+6.648%／+6.449%**；compact `install` 栈从 `0x320` 升至 `0x330`。[机器码](codegen-compact.md)。 | 撤回性能转移，保留 RAII／失败清理。 |
| 第一轮读取内联 `708afeb6` | 21 个机器调用只改被调符号名，没有消失。[核查](codegen-read-inline.md)。 | 按机器码条件停止该版。 |
| 无入口 choice 的读取内联 `8d3875dc` | 对历史 c7 的 `empty_loop`、`array_read`、`prop_read` 指令 **+6.43%／+2.72%／+2.61%**。[固定矩阵](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/read-current-fixed-summary.json)。 | 未启动其 R0／B37 批次；恢复入口 choice 后另测组合。 |
| 只恢复入口 choice 的 `be855322` | Object 对 Parent cycles 复测仍 **+3.128%**；只删除了读边界，写边界还在。[组合门禁](/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/entry-read-parent-gate-summary.json)。 | 未运行其 V8；加上窄写回内联后另测组合。 |

同机其他工作被允许，wall／cycles 不宜解读小幅差异。退休指令不是 cache 或分支预测计数；既有 ARM64 核查也没有证明 Object 周期变化的硬件原因。最终收据应保留构建、工具、输入、原始样本与结果文件的路径和 SHA-256，不从缺失样本补零。历史 c7 的累计收益只作为分母身份，不与 b280 的新结果混用。

## 可复核收据

[结构化测量汇总](measurements.json)保存样本数、源码／树／二进制／编译器身份、每项整进程指标的中位数与分布，以及外部原始 results、metadata、samples 和冻结计划的 SHA-256。[外部证据索引](external-evidence.json)保存分析与批次脚本、日志和负结果摘要的路径及哈希。原始大样本保留在测量目录，不从缺失结果补零。

[调用失败路径审查](final-correctness-review.md)针对保留的 RAII 与 named-local 失败清理；其后组合只恢复已验证的入口选择并增加内联注解。被拒绝的 receiver 证明与机器码文件均标为历史候选，不是最终实现说明。

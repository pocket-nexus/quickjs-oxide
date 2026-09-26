# 性能证据账本：issues #41–#44

> 原账本整理日期：2026-09-25；历史主仓快照 R0：`f531f6052cb497ce4707f01c276e8642e5e26788`。下列 E41–E44 的旧表格保留原维护者实验身份，不冒充当前组合版测量。
> 实施更新：#41 已以新身份 `04bb1a74` 恢复并合入当前集成分支；13 种数值数组跨度已全部实现。[#41 复验](receipts/gates-2026-09-25/README.md)、[四函数 25 站点 manifest](receipts/all-dense-6db6bfb0/README.md)及[四方正式 benchmark／profile](receipts/fourway-2026-09-25/README.md)分别记录独立门禁、静态覆盖和组合实测。组合版八项隔离几何平均相对 R0 为 +12.1%，原版 combined 为 +13.0%；不等于原计划 3–4 倍目标已达成。

## E0. 版本与证据可取得性

| 记录 | 被测版本／环境 | 可以支持什么 | 仍缺什么 |
| --- | --- | --- | --- |
| [E41] | 历史报告候选 `0cd4acee`；新恢复候选 `04bb1a74` 另见[复验](receipts/gates-2026-09-25/README.md) | 宽错误载体改小后的 ABI 和固定指令变化；新复验发现 cycles／错误分配／RSS 回退；[四方 V8](receipts/fourway-2026-09-25/README.md)另见完整 Score | 第二次独立正式轮和当前四方固定微负载矩阵未测 |
| [E42] | `8a4b89d4` → `f531f605`，Rust 1.88.0；固定 10M 轮，CPU 2、5 样本中位 | 数组路径剩余工作明显多于其他三个循环 | 四探针原来未入库；V8 captured miss 未测；无总分增益结果 |
| [E43] | `8a4b89d4` → `f531f605`，Rust 1.94.1；release perf 自时间与独立 profiling 计数 | 四个真实 V8 子项上的调用路径不是主要高倍数来源 | 上游 checkout 为 `2034d98`，不是旧文档的 pin；仅四子项，无完整调用点类型分布 |
| [E44] | `f531f605`；同机串行 release A/B；固定指令重复一致 | 便宜 guard 前移的候选在所测分布上净亏 | 更大 V8 语料的失败比例仍未知；wall 受共享主机影响 |

R0 已合入 PR #48 的无状态 Test262 gate。#41 评论提到“合并后晋升源码指纹”，不能直接沿用为新计划要求：当前 gate 比较结果正文，性能改动没有改变结果正文时，不需要为每个 commit 晋升语义基线。以 [Test262 当前契约](../test262.md) 为准。

`3341ac456ea2719858fd6173e8dcd9123ad9e660` 是 [PR #37] 的真实 head（文档／receipt 提交），不是一个虚构基线；只是不能把它说成实现提交。PR #37 的 merge 为 `26a5726a19efb0088c59d0325f6c3db3699b7048`，最终写回调整包含 `2fa370a7`。#42 报告指出 R0 与 PR #40 `fd9b4eac` 的引擎源码相同。新实验仍需记录完整源码与构建身份，不用这些关系替代 receipt。

原整理时 `0cd4acee` 不可解析，因此未把旧 hash 冒充当前实验。现已独立恢复 `04bb1a74`，本仓只保留 R0 复验的[结果总结](receipts/gates-2026-09-25/README.md)；原始样本与构建文件不进入 PR。#44 的历史报告仍按其原身份阅读。

<a id="e41"></a>
## E41. 紧凑错误载体：历史机制与当前负结果

[E41] 的候选是约 60 行、两个源文件的 `Error(Box<ErrorData>)`，不是全仓库 API 重构。记录如下。

| 指标 | 基线 | 候选 | 变化 |
| --- | ---: | ---: | ---: |
| `Error` | 80 B | 8 B | payload 冷置 |
| `Result<(), Error>` | 80 B | 8 B | 该测量产物寄存器返回 |
| `Result<bool, Error>`、`Result<&T, Error>`、`Result<JsValue, Error>` | 80 B | 16 B | 该测量产物 RAX:RDX 返回 |
| `.text` | 8,258,940 B | 7,776,372 B | −5.84% |
| `run::run` 栈帧 | 3,752 B | 2,184 B | −41.8% |
| 指定宽结果判别写入模式的站点数 | 637 | 182 | 静态机器码模式计数，不是运行次数 |

**纠正旧推论：本次 `Result<JsValue, Error>` 实测为 16B，而不是先前预计的 24B。** 类型布局、niche 与 ABI 必须在实际类型／目标／编译器上测量；不能普遍声称“16B 一律走寄存器”，更不能按 `?` 个数乘 80B 估算搬运。

装箱后不再保留独立调用边界的包括 `push_current`、`local_current`、`parameter_current`、`replace_local_current`、`pop_current` 以及部分数组／属性包装 helper。`binary_number` 和 `update_number_local_current` 在基线就已经内联，不能算成此次“消失的调用”。S1/S2/S3 等已直接处理立即数的路径收益很小，不能再按旧 `copy_value` 采样占比推算剩余空间。

| 固定负载 | 退休指令变化 | 固定负载 | 退休指令变化 |
| --- | ---: | --- | ---: |
| empty_loop | −0.57% | int_local | −0.22% |
| prop_read | −0.53% | call0 | −6.16% |
| array_read | −4.59% | array_write | −4.61% |
| prop_write | −4.53% | string_build1 | −3.02% |
| bigint32 | −9.14% | bigint64 | −4.37% |
| bigint256 | −4.31% | throw_catch | −1.20% |
| tdz_catch | −0.79% | | |

原报告中的 instructions A/A 为 0.00%；cycles A/A 却为 −3.4%～+10.5%。当前 R0 复验已补上两轮配对：`prop_write` cycles 中位数 +10.87%，`string_build1` +4.09%；每 100,000 次错误构造／传播由 100,000 次／1.3 MB 分配升至 200,000 次／9.3 MB，保留模式 RSS HWM 中位数增加 2,340 KiB。[复验总结](receipts/gates-2026-09-25/README.md)记录方法和结果，原始样本不入库。**不能以指令下降抵消已测得的时间和错误路径成本。**

报告称 14 个错误场景、CLI 输出／退出码、3009 workspace tests、2048 test262-host tests、完整 Test262 80010/80060 等通过。这是原候选的历史验证记录，不替代恢复版或组合版的重放；恢复版的独立验证见[#41 报告](../reports/issue-41-error-carrier.md)。

原门禁据此拒绝单独接纳 #41；当前用户要求继续完成全部代码并记录未达门禁的结果，故它已进入集成分支。组合版表现须重新测量，不能用历史正结果或单独负结果直接替代。

<a id="e42"></a>
## E42. 数组差距：已复现，但不能外推为总分

[E42] 同协议 base → R0 的固定 10M 轮结果：

| 负载 | base 指令／轮 | R0 指令／轮 | 变化 | QuickJS 指令／轮 | R0／QuickJS |
| --- | ---: | ---: | ---: | ---: | ---: |
| empty_loop | 623.3 | 354.3 | −43.2% | 90.1 | 3.93× |
| int_local | 1386.3 | 461.3 | −66.7% | 128.1 | 3.60× |
| prop_read | 1779.3 | 764.3 | −57.0% | 201.1 | 3.80× |
| array_read | 2528.3 | 2328.3 | −7.9% | 227.1 | 10.25× |

数组 cycles／轮为 786.67 对 44.73，即 17.59×；其他三项 cycles 比为 3.64–3.93×。QuickJS 是 pinned 2026-06-04、gcc `-O2` 构建；该跨引擎比较只用于结构诊断，不能当作同源码优化 A/B 的验收。

冻结 H1 数字 2486.27→2330.28（−6.27%）仍是有效历史系列；本轮重建是 2528.3→2328.3（−7.9%）。两组方向一致，但不能拿其中一个 base 除另一个 HEAD。新测量中部分 base 比历史值高 1.7–2.8%，在没有汇编与编译身份对照前，不能将确定性的指令差异简单归为运行时“布局噪声”。

源码对应：[array_immediate_read_current] 仍从拥有式栈取 base/key，替换输出槽，然后 `release_jsvalue(key)` 和 `release_jsvalue(base)`。已由 frame 保活的数组在短块中可作为非拥有读取候选；这个机制可行性不等于已经证明所有释放都可以省略。

Crypto 使用 JS 数组数值内核，实际初始化为 `setupEngine(am3, 28)`；NavierStokes 用普通 `Array` 存场量并执行 `project`／`lin_solve`／`advect`。因此优先研究 dense 数组与 Number／位运算，而不是优化 Rust BigInt 库。外部语料与生成产物仍须重新 pin。

未测：八子项的 span 动态覆盖、captured binding 拒绝比例、数组空洞／慢形态比例、执行块前后正式 Score。新计划把这些列为继续投入的门槛。

<a id="e43"></a>
## E43. 调用画像：下调投入，而非继续寻找理由扩大范围

[E43] 的 `call0` 固定工作量是 4151.28→3926.27 指令／轮（−5.42%），R0／QuickJS 为 10.12×。这证明微负载差距，**不证明真实程序花了同样比例的时间在调用入口**。

| V8 子项 | JS 调用数占比 | 选择／domain／认证／enter + JS frame 自时间 | 窄口径 native 编组自时间 | ownership／RC 符号组自时间 |
| --- | ---: | ---: | ---: | ---: |
| Richards | 99.999% | 5.92% | 0.68% | 16.60% |
| DeltaBlue | 96.205% | 10.46% | 1.06% | 16.32% |
| Splay | 98.017% | 3.10% | 0.36% | 8.26% |
| Crypto | 94.02% | 0.96% | 0.66% | 14.74% |

原画像的 Richards “native 进入”组混有属性驱动 `proxy_get_driver::drive`；上表使用评论明确给出的窄 native 口径，不能把属性驱动全算成原生调用。四行不是完整八子项画像。

作为对照，`call0` 的 JS 调用组约 23.31%；Map-get 与 Math-min 的 native 编组组为 25.33%／36.76%。Map 微负载的 22–30% 机制线索不能移植为 V8 占比。

普通函数认证缓存命中 99.995–100%，native argv 全程容量增长至多一次（启动的 16–32 B），buffer 计数 `values_copied=0`、`values_moved=实参数量`。因此取消“已有函数级缓存再造一遍”和“消除每次 native argv 分配”的立项依据。

**函数级 auth 命中率不等于调用点单态率。** 同一 PC 可以在多个已认证函数间轮换而保持 auth 全命中；若重开 callsite cache，必须新增按 `(FunctionBytecodeId, canonical PC)` 的实际 callee 身份／变化率计数，不能复用本表冒充。

这些时间比例来自无 debug info／帧指针的 release 符号 self% 归组；内联成本可能归到 `run` 或 driver，分组不是完整且精确的语义成本分割。对上述被测分组，假设全消除 DeltaBlue 的 10.46% 且其他成本不变，Amdahl 算术为约 1.117 倍；这不是整个调用语义的绝对上限，更不是可兑现收益。ownership／RC 组同样不能全算作可删除成本。

裁决：不扩大通用 JS/native 调用专项；只有在统一语料、#41 后重新归因及定向 A/B 支持下，允许 DeltaBlue 的窄 frame／方法路径实验。

<a id="e44"></a>
## E44. 检查顺序负结果

[E44] 将 acc 数字检查前移，契约上安全，但实际机器码改变了。单次 attempt 的 candidate − baseline：

| 类别（沿用报告标签） | 指令差 |
| --- | ---: |
| 111：全部命中 | +12 |
| 110：peek 失败 | +43 |
| 011：acc 失败、peek 命中 | −236 |
| 010：acc 与 peek 都失败 | −204 |
| 100：base 失败 | +18 |

一半命中、一半 acc-string 失败的构造负载为 −112 指令／次，与线性模型一致。但所测 Bellard `prop_read` 的 3,555,200 次 attempt 中，命中 91.7%、peek 失败 8.3%、acc 失败为 0，故平均 **+14.6 指令／次**。其余所测 bench／语义套件多数没有触发 S3；不能把“不触发”说成“候选加速了它们”。

`run::run` 规范化后的指令序列 0 diff，outlined 函数仍发生 helper 内联翻转：candidate 把 `property_ic_peek_number` 剥离成调用，累加器跨调用存活，引入 spill/reload。改成早拒绝后再读 acc 的变体全命中更差（+40）。这反驳了“只要主循环不变，guard 重排就免费”。

只含命中与一种 acc 失败时，盈亏平衡约为 `12/(12+236)=4.84%`；另一失败类型约为 `12/(12+204)=5.56%`。真实多类别分布必须计算：

```text
ΔI/attempt = 12*p111 + 43*p110 - 236*p011 - 204*p010 + 18*p100
```

不能把 4.8% 当作适用于所有语料的固定门槛。#41 可能改变内联，以上成本也必须重新测，不能永远沿用。

peek 失败还包含 `Megamorphic(1024)` 倒数期的反复 burst，不只是首轮冷启动。这只构成“测量 IC 策略成本”的线索，不是删去身份／shape 校验或立即更改冷却策略的授权。

裁决：拒绝当前重排与其更差变体；只有新的真实失败分布加新的 codegen A/B 表明净收益，才重开。

## E45. 历史债务与不应复活的结论

[PR #37] 记录 BigInt32／64／256 相对其 pre-B2 基线累计退休指令 +4.07%／+3.64%／+2.30%。每片 <2% 不能消除累计回退。[E45] 要求保留累计裁决。#41 的下降与上述增长来自不同测量身份，不得直接相加后声称债务已清偿；必须同协议重建 pre-B2、B37、R0 与候选。

B1 的解码、二次分类和 C 单槽 TOS 的准入／spill 负结果保留在 [原负结果报告](../reports/s3-c-negative-result.md)。B2.0 的统一 `span(pc)` 查询曾引发 helper 内联翻转；新计划不无条件复活它。

B2.2 的旧上界实验只把 `prop_read` 从 752→632 指令／轮（约 1.19 倍工作量改善）；可上线校验保留后的 90–100 指令回收是旧估计，不是实现结果。outlined handler 25–45% cycles 自时间不意味着全部可删除。更早的 V8 对 QuickJS 17.1×／16.6× 差距见 [全量重测历史](../reports/s3-full-rerun-results.md)，不是当前 R0 成绩。

## E46. 本次补全：静态设计证据，不是新增性能测量

对 PR #6 head `6f09205c51f8b34e3c7a90ce406739fe1dd07c48` 的源代码重新核对，得到四项会直接影响实施的约束：

1. `FusionPlan::update` 通过 bit16 识别旧更新跨度，新 flags 必须避免误识别；v1 固定使用 1–13。
2. `lower_update_expression` 的 postfix 是 PostInc/PostDec + Put，prefix 是 Inc/Dec + Set；索引更新必须暂存到唯一提交点，miss 不能预先修改绑定。
3. computed assignment 包含 Insert3/PutArrayEl；写跨度要保住原栈契约、尾部 Drop 和 run 的 property_generation 更新，不能仅写 heap 后跳 PC。
4. 数组 canonical 叶仍做 release-readiness 与可变 Runtime 借用；新 Number-only 读明确使用短期共享借用，不能把通用事务包进新 API。

[当时实施规格](numeric-array-spans.md) §8 给出固定源码链接；13 种序列及栈代数来自这些规则。对 pin 上游源码另核对：project 实际使用 `u[++nextValue]` 等前缀更新，advect 的原顺序是 `d0[i0 + row1]`，不能以等价手写表达式冒充原始程序。

最初整理本文时尚无 Rust 编译或真实函数 PC。后续已用[捕获入口](probes/run_dump.py)和[test-only 探针](probes/dump_numeric_spans.rs)取得[发布覆盖总结](receipts/all-dense-6db6bfb0/README.md)，并完成[四方 V8 与 profile 总结](receipts/fourway-2026-09-25/README.md)。生成文件只保留在本机测量目录，不作为 PR 附件。

## 来源

[E41]: https://github.com/pocket-nexus/quickjs-oxide/issues/41#issuecomment-5826405666
[E42]: https://github.com/pocket-nexus/quickjs-oxide/issues/42#issuecomment-5826183030
[E43]: https://github.com/pocket-nexus/quickjs-oxide/issues/43#issuecomment-5826530731
[E44]: https://github.com/pocket-nexus/quickjs-oxide/issues/44#issuecomment-5826783292
[E45]: https://github.com/pocket-nexus/quickjs-oxide/issues/45
[PR #37]: https://github.com/pocket-nexus/quickjs-oxide/pull/37
[array_immediate_read_current]: https://github.com/pocket-nexus/quickjs-oxide/blob/f531f6052cb497ce4707f01c276e8642e5e26788/src/engine/vm/stack.rs#L1001-L1080

# PR #53：M1 加固与 M2 数值操作

本系列把原有 `sum += array[i] * scale` 扩展为同一 `ExecCode` 字流中的四种
计划操作：local 累加、`out = array[i] * scale`、已有元素的
`array[i] += delta`，以及数组元素的数值比较分支。原指令仍在字流中，
准入失败从原入口执行；运行时没有第二套解释器或公开 embedding API 变更。
这是在 PR #53 **原分支上**的增量结果，基线为该 PR 的
`10262309c580c43ba984b4f371acdc081147b4f0`，其直接父版为 PR #52
`4287e8c6019933289f3a06e703aea15bf79c9f11`。原
[M1 收据](../m1-numeric-region-2026-09-28/README.md)保持原始测量身份，
不把其中旧工具链和源码的数字当作本轮复现。

## 实现与证明范围

临时基本块图使用可复用的平坦输入存储；选中的操作携带精确消耗范围。
编码时以源指令索引找到区域，不逐个线性查找描述符；空区域列表跳过
区域专用验证。发布器验证来源、常量、lexical 初始化、逻辑栈峰值、
内部入口、分支与成功/回落 continuation。目的地准入只解析和分类一次，
在同一作用域内持有可写 Number 槽；index/scale 等别名输入先复制。
数组更新在一次可变 heap 借用中检查 genuine Array、现有 writable own
Number 元素，读取、相加、写回，并沿用属性 generation 记账。成功分支
直接跳转；失败按拒绝点计数，回到原词。

支持同一顺序块内已经初始化、未捕获的 lexical `let` 目的地和 `const`
来源。动态/eval、capture、mapped 参数和不确定的初始化仍用普通路径。
数组更新与比较选择仍识别有限的连续形态；这不是通用表达式规划。
当前 `PotentialEffects` 只描述可能调用 JS、分配和抛错，窄形态的释放与
布局安全来自受限指令和动态准入。最终只做一次 `Number::compact` 的
想法经过定向 Number 边界测试，但**未进入产品路径**：本轮没有证明其
机器码与 plain 性能收益，产品继续按 `mul` 后 `add` 的现有语义执行。

## 源码、构建和工作量

三个 plain CLI 均用干净提交、Rust **1.88.0**、同一 release 配置
（fat LTO、CGU=1）分别重新构建。主机是 AC 供电的 Apple M1，
8 logical CPUs、16 GiB、macOS 26.6.2。完整命令、编译器与工具脚本
身份见 [build JSON](data/candidate-qjs.build.json)及同目录的 `pr53-`、
`pr52-` 构建文件。

| 角色 | Git commit | plain CLI SHA-256 | 字节 |
| --- | --- | --- | ---: |
| Candidate | `b4843af813e5dcece7f11012ea7666bdf5723ff1` | `177d2be3997e9e3d98670035c26f942f4ef206421f1d6e240a7ea00407e5a4c5` | 8,073,552 |
| 当前 #53 head | `10262309c580c43ba984b4f371acdc081147b4f0` | `48f538f3b14aae32695b96c8c85f966d18ac3e2d23fd0be2560d39586f86557f` | 8,056,320 |
| #52 parent | `4287e8c6019933289f3a06e703aea15bf79c9f11` | `8acb204e0ec9e027f7ea37cfc238cb887348d9d28b6cc346cae436f4a17f5f5f` | 8,037,792 |

候选比当前 #53 大 **17,232 字节**，比 #52 大 **35,760 字节**。
测量提交 `b4843af8` 包含实现、内部测试和实施说明；本收据后写入，
不称文档提交为新引擎构建。

[七个固定工作量 JS](workloads/manifest.json)各执行 2,000,000 次，
覆盖 product、update、branch 的 hit/miss 与无候选情况。原始 M1 的
hit/miss/nonapp 工作量仍来自[原收据](../m1-numeric-region-2026-09-28/workloads/hit.js)；
本轮另存[实际 M1 manifest](data/m1-manifest-measured.json)。
`fixed.py` 逐次校验 stdout、退出状态和文件哈希，ABBA/BAAB 交错，
每引擎每 case 8 次；`/usr/bin/time -l` 保存退休指令、cycles 和最大 RSS。
以下 wall 是**整进程**中位数，包含启动、编译和退出，不是隔离的 VM
执行时间，也不是 V8 Score。比值小于 1 表示 Candidate 更快。
完整逐次样本在 [#53 对照](data/fixed-pr53-results.json)、
[#52 对照](data/fixed-pr52-results.json)、
[M1/#53](data/m1-pr53-results.json)、[M1/#52](data/m1-pr52-results.json)。

## 固定工作量结果

| Case | Candidate/#53 wall | Candidate/#52 wall | Candidate/#53 退休指令 | 解释 |
| --- | ---: | ---: | ---: | --- |
| product hit | 0.833 | 0.847 | 0.854 | 完整直接目的地操作有可测收益 |
| product miss | 1.022 | 1.009 | 1.002 | 接近持平；不能从一次系列宣称 miss 改善 |
| array update hit | 0.371 | 0.381 | 0.394 | 一次借用与写回删除了大量临时工作 |
| array update miss | 1.101 | 1.052 | 1.049 | 准入失败增加成本，后续需降低或利用 miss 事实 |
| comparison hit | 0.990 | 0.933 | 0.868 | 指令减少，wall 跨两组波动；不宣称稳定 wall 收益 |
| comparison miss | 0.981 | 0.971 | 0.987 | 近似持平 |
| nonapp | 1.076 | 1.071 | 1.023 | 无选中区域也观察到执行成本上升 |

原 M1 工作量的 Candidate/#53 wall 为 hit **1.025**、miss **1.021**、
nonapp **1.025**；退休指令比分别为 **1.032**、**1.019**、**1.024**。
Candidate/#52 的相应 wall 为 **0.755**、**1.006**、**1.067**，
退休指令为 **0.764**、**1.001**、**1.024**。这保留了 M1 对 #52 的
累积 hit 收益，同时显示本次扩展相对原 #53 有小幅增量成本。

同一 Candidate 二进制做了 [首次 A/A](data/aa-results.json) 与
[低负载重复 A/A](data/aa-repeat-results.json)。首次个别 case 的
中位 wall 差达 7.3%；重复系列各 case 为约 0.04%–2.4%。主机在
首次系列附近有其他构建与 conformance 任务，原始结果全部保留。
上述大幅 hit 差异同时出现在退休指令与 cycles 中；小幅 wall 变化
只能在这一噪声范围内解释。逐次最大 RSS 随 case 和系列变化，
未建立由单个 descriptor 引起的进程内存差值。

## 编译、元数据与覆盖

plain 无 profiling 的 compile probe 分别由三份干净源码构建。四个
[编译 corpus](compile/large-block-200.js)包含一个函数中的 200 个区域、
500 个 M1 函数、约 500 个 M2 函数及 500 个非候选函数。
100 次新进程测量只围住 Script-goal 编译，不计 Runtime/Context 构造、
文件 I/O 与销毁。[100 次结果](data/compile-repeat-results.json)中位数：

| Corpus | #53 ms | #52 ms | Candidate ms | Candidate/#53 |
| --- | ---: | ---: | ---: | ---: |
| large-block-200 | 1.075 | 0.876 | 1.053 | 0.979 |
| m1-500 | 8.296 | 7.661 | 8.506 | 1.025 |
| m2-500 | 7.665 | 7.254 | 7.979 | 1.041 |
| nonapp-500 | 6.299 | 6.244 | 6.305 | 1.001 |

受主机负载影响的[首次 40 次系列](data/compile-initial-results.json)
曾给 large-block 相反方向的结果；另一次
[100 次单 case 重复](data/compile-large-repeat-results.json)支持
Candidate 与当前 #53 在该 case 接近。不能把 flat arena 或 indexed
lookup 单独归因为编译提速；M1/M2 多函数 corpus 的额外选择工作仍有成本。

新选中操作的 profile snapshot 各有 1 个 **72 字节** region descriptor；
相同 M1 工作量的当前 #53 为 **64 字节**，Candidate 为 72 字节。
200 区域函数为 12,800→14,400 inline bytes。新 product hit 的执行词
同为 144 字节；branch 为 164→160，array update 为 148→156 字节。
这些是去重 `Rc` slice 的 inline bytes，不含头部、分配器和全部元数据。
plain 的 `execute_frame` 反汇编符号为 27,024→29,376 字节；
[两份反汇编与统计](data/codegen-summary.json)证明代码尺寸变化，
不单独证明热路径速度。

[profile 原始记录与汇总](data/profile-summary.json)显示每种 M2 hit
工作量的选中站点均 10,000/10,000 hit；hole 的 product/branch miss
和不可写 update miss 均是 10,000/10,000 指定原因的 miss。
array-update hit 中 `binary_number_in_place` 从当前 #53 的 10,000 降到 0，
`runtime_pc_publication` 两侧均为固定的 8 次，不按循环次数增长。
product/branch 在当前 #53 已分别使用 `dense_read_binary` 和
`dense_read`/`compare_branch_stack`，因此 M2 对它们是替换已有特化，
不能把全部减少量算作从纯 generic 删除。profile 是逻辑计数，
不是执行耗时或机器指令总数。

用固定的 [V8 v7 源码](https://github.com/ahaoboy/js-engine-benchmark/tree/2034d98fc8c5f8044e186267593f5d5ea5232caf)
各做一次不计时的 profile 覆盖：八个 case 均完成；仅 `crypto` 有
2 个 M2 比较站点，共 **43 attempt / 43 hit**，其余七个没有 M2 站点。
[覆盖原始结果](data/v8-profile/results.json)不构成原版 V8 Score 或
整体引擎加速结论。第三方生成 JS 仍留在外部工作目录，仓库只保存
身份、结果和 profile 记录。

## 语义验证

- Rust 1.88.0 workspace/all-targets、profiling、doc、test262-host、
  Clippy CI 变体及格式检查通过；主库 all-targets 为 **1,966/1,966**。
  差分测试以固定 QuickJS oracle 运行；13/13 JS fixtures、9 个 C
  oracle 与动态导入 trace 检查通过。
- 内部测试覆盖四种 opcode 的选中与 generic 差分、lexical 初始化/TDZ、
  alias、Number 边界、dense/materialized/frozen/hole/accessor/Proxy、
  可写性、异常身份/位置、async 恢复、实际容量准入、相邻特化、
  200 区域函数及非法发布描述符。miss 的 attempt/hit/reason 另由上述
  profile 原始记录验证。
- frozen full Test262 在最终引擎源码上匹配原向量：**80,010 pass /
  80,060 eligible / 102,037 total**，其中 3,552 fail、3,502 unsupported、
  18,475 skipped 属于既有冻结分类；[完整运行日志](data/test262-full.log.gz)
  与 `dev-support/test262/current.conf` 中的结果体哈希可核验。
  最终 focused 复验为 **6,844/6,844 pass**，
  [日志](data/test262-focused.log.gz)已保存；
  `node scripts/test262/current-test262-metrics.mjs --check-docs`、
  `cargo +1.88.0 fmt --all -- --check` 与 `git diff --check` 均通过。
  各验证项的身份和结果见[验证摘要](data/validation-summary.json)。

这些结果证明四种真实 JavaScript 操作可通过同一发布与回落契约运行，
且 array-update/product 的选中 hit 在这些固定负载上删除了可测工作。
拒绝成本、非候选成本、编译覆盖及真实工作负载命中率仍限制后续取舍；
不预先承诺更大的性能收益。

# PR #53：可直接执行的数值计划与真实工作量覆盖

本系列继续在 PR #53 分支上，以提交 `38e9eb86e2c2db8fb2e607c9f3ff611407941b4b`
作为扩展前版本，以 PR #52 `4287e8c6019933289f3a06e703aea15bf79c9f11`
作为父版。实现提交为 `6f1de9c5`。原 [M1](../m1-numeric-region-2026-09-28/README.md)
和 [M2](../m2-numeric-operations-2026-09-28/README.md) 收据及其原始数值保持原有身份。
固定工作量与编译测量目录的 `results.json`、`samples.jsonl`、`metadata.json` 保留
原始记录；逐样本 stdout/stderr 与 Darwin `time -l` 文本无损打包在同目录
`raw.tar.gz`，`raw.sha256` 给出归档校验值。JSON 内记录的临时绝对路径是
采样时路径，归档内按 `raw/` 相对文件名保存对应原件。

## 实现与证明边界

临时编译计划在发布时转换为只含执行操作数的描述符；成功、回落、分支与
fallthrough 入口均解析为 `ExecCode` 字 PC。发布验证操作数、常量、内部入口、
栈峰值与 continuation。执行路径不再把 compiler PC 转换为字 PC，也不读取
临时 `NumericOperation`。原指令仍留在同一字流，准入失败从原始首词按原顺序执行。

累加仍要求旧目的地为 Number。product assignment 允许覆盖已经初始化且不持有
可观察所有权的 direct undefined、null、Boolean 或 Number；持有 owner 的值及
未初始化 lexical binding 回到普通路径。拒绝点在内部使用紧凑类型，只有 profiling
输出将其转换为名称。可达普通 CFG 边的 lexical 初始化事实以所有前驱的交集建立；
`InitializeLocal` 建立事实，`SetLocalUninitialized` 删除事实，异常和 resume 等
不明入口不推测初始化。无 lexical local 的函数跳过该分析。

四种 opcode 不变。数组更新现在可接受一个由临时 use graph 识别的
`dt * s[i]` 作为 delta，因此固定 V8 v7 Navier–Stokes 的
`x[i] += dt * s[i]` 使用原有 array-update opcode。准入先读取 Number 输入和
两个数组的 own Number 元素，再写目标；`x === s` 也遵守读先于写。缺口、
getter、Proxy、非 writable target、可能运行转换或释放 owner 的情况均走普通词。
成功的 dense 更新在生成代码中只做一次对象身份认证；materialized 路径仍在
读取 shape 后重新获取可写对象。属性 generation 与 invalidation 行为不变。

Profiling 的 `compiler_rejections` 报告有界的编译拒绝数，`rejections` 将
可识别的降低后指令窗口及首要原因映射到已发布 generic 字 PC，并另计该入口的
实际 visits。窗口识别不是对所有可能表达式的穷举，也不是执行耗时。

## V8 v7 覆盖发现

固定 [V8 v7 源码](https://github.com/ahaoboy/js-engine-benchmark/tree/2034d98fc8c5f8044e186267593f5d5ea5232caf)
和 `scripts/benchmark/profile_v8.py` 的一次 Setup/run/TearDown driver 对八个 case
全部完成。选择前的诊断快照是实现过程中的中间构建，二进制身份及原始输出在
[发现数据](data/v8-rejection-discovery/results.json)。Navier–Stokes 的同一
generic 入口执行 **50,700 visits**，降低后的窗口是：

```text
GetArg(0), GetLocal(0), GetArrayEl3,
GetArg(2), GetArg(1), GetLocal(0), GetArrayEl,
Mul, Add, Insert3, PutArrayEl, Drop
```

它是执行过的、可由现有 update family 表达的最高流量拒绝点。发现构建把缺失的
计算 delta 标为 `unsupported_use`；新增 producer fact 让该站点发布一个操作。
发现 profile 的 `omitted` 项全部为零。后续同一 driver 的干净候选构建覆盖，
以及其他 case 的 miss 与 omission，见下方最终测量表和原始数据。本系列不是
原版 V8 Score，不从一个站点推断整体引擎加速。

## Dense 单次认证实验

在保留两次对象查找的候选与单次 dense 查找的候选间，以 Rust 1.88 plain release
构建交错运行同一批固定工作量，每 case 每二进制八次。
[A/B 原始样本](data/dense-ab/results.json)与[同二进制 A/A](data/dense-aa/results.json)
保留所有 wall、retired instructions、cycles、最大 RSS 与 stdout 验证。此实验的
两个实现快照还包含编译期 producer-fact 重构；固定工作量的执行部分相同，不能把
微小编译差全部归给 dense 变更。

| Case | dense/old wall 中位比 | dense/old retired 比 | dense A/A wall 比 |
| --- | ---: | ---: | ---: |
| update hit | 0.924 | 0.940 | 1.008 |
| array-product update hit | 0.846 | 0.950 | 1.001 |
| update miss | 0.992 | 1.000 | 1.023 |
| nonapp | 0.967 | 1.000 | 0.983 |

同一 LTO release 配置下，`Runtime::try_add_array_own_number` 的反汇编中
`validate_slot_identity` 静态调用点从 **3 个降为 2 个**；dense 分支不再执行
第二次认证。原始汇编由该实验二进制生成，未将整函数反汇编当作孤立的 VM
指令成本。命中工作量的退休指令减少；miss/nonapp 的指令数基本不变，wall
差在这组 A/A 波动附近。因而保留 dense 改动。最大 RSS 随运行波动，未据此
声称堆内存收益。

## 同一 V8 driver 的覆盖结果

最终 [profiling 原始数据](data/v8-candidate/results.json)来自实现提交 `6f1de9c5`
的独立 release 构建（SHA-256
`c90c57a5a89a9aae77e592c6e42f0a379e39f79c5e54d30668d35e8fa8ed48ac`）；
固定八个 case 全部正常结束。在 Navier–Stokes 中，上述 generic PC 的
**50,700 次执行**变为同一逻辑站点的 **50,700 次 update 准入、50,700 次
命中、0 次 miss**。该 workload 的诊断不再列出这个拒绝点。其他 case
仍按实际窗口报告拒绝与执行次数，没有把冷编译拒绝数充作热执行次数。
八个 case 的所有 `omitted` 字段均为零。后续提交 `734bc128` 仅调整
profiling 扫描器的局部偏移写法，以通过源码检查；不改变该操作的准入或执行。

## 最终干净构建对照

本节只使用实现提交、pre-extension #53 与 #52 的独立 Rust 1.88 plain
release 构建。固定工作量包含既有 product/update/branch hit、miss、nonapp，
以及本轮新增的 [array-product hit](workloads/update-product-hit.js) 与
[array-product miss](workloads/update-product-miss.js)。wall 是包含启动、编译、
执行与退出的整进程中位数；与编译 probe 和 profile 逻辑计数分开解读。

| 角色 | 来源提交 | plain CLI SHA-256 | 字节 |
| --- | --- | --- | ---: |
| Candidate | `6f1de9c5` | `b7a93458076bfbcf8fc0943957f0c9372ac47630e173c610e0f362767e570bb4` | 8,091,360 |
| pre-extension #53 | `38e9eb86e2c2db8fb2e607c9f3ff611407941b4b` | `7570a75233501629d132acabebe371db845585562391aadd37e5cabb05fe33b1` | 8,073,552 |
| #52 parent | `4287e8c6019933289f3a06e703aea15bf79c9f11` | `6c5bf467481283c5ac581408ca7899f0fdf02fdca861c4415aa084247a1e9c8c` | 8,037,792 |

Candidate CLI 比 pre-extension #53 大 **17,808 字节**，比 #52 大
**53,568 字节**。这是二进制文件大小，不等同于运行时内存；下面的 RSS
仅来自固定工作量的整进程测量。

[Candidate/pre-extension #53](data/final-vs-pr53/results.json)、
[Candidate/#52](data/final-vs-pr52/results.json)及
[Candidate A/A](data/final-aa/results.json)均使用九个相同 case、每二进制
八次、ABBA/BAAB 交错顺序、`/usr/bin/time -l` 整进程计数。表中是
Candidate/对照的中位数比值；小于 1 表示 Candidate 较少。A/A 两列是
同一个 Candidate 二进制的两种标签，不是另一版本。

| Case | wall / #53 | retired / #53 | wall / #52 | retired / #52 | A/A wall |
| --- | ---: | ---: | ---: | ---: | ---: |
| branch hit | 0.934 | 0.986 | 0.763 | 0.855 | 0.953 |
| branch miss | 0.957 | 0.999 | 0.973 | 0.991 | 1.003 |
| non-applicable | 1.016 | 1.001 | 1.189 | 1.024 | 1.048 |
| product hit | 0.982 | 1.000 | 0.742 | 0.852 | 1.023 |
| product miss | 1.006 | 1.000 | 1.052 | 1.001 | 1.096 |
| update hit | 1.024 | 0.990 | 0.393 | 0.390 | 0.968 |
| update miss | 0.975 | 1.004 | 1.036 | 1.053 | 0.961 |
| array-product update hit | **0.339** | **0.407** | **0.345** | **0.412** | 0.908 |
| array-product update miss | **1.412** | **1.181** | **1.235** | **1.189** | 1.019 |

新 hit 的工作消除超过该 case 的 A/A wall 漂移，并在退休指令上同方向；
新 miss 的额外准入成本也超过 A/A 漂移，必须保留为已测代价。其他 case 的
wall 小差异在各自的指令计数与 A/A 波动下不宜断言收益。相对 #52 的
non-applicable wall 比值为 1.189，但退休指令比值为 1.024；这同时包含
较早 M1/M2 改动，不能归因给本次操作。两组对照的最大 RSS 中位比
分别落在 0.962–1.029、0.992–1.010，A/A 为 0.995–1.007；
没有可归因的内存改善。原始样本也保留 cycles、stdout 和逐次状态。

## 编译、字流与元数据

三个独立 Rust 1.88 plain compile probe 分别连接上表三份源码，测量
`Context`/Runtime 构造与 source I/O 之外的 Script 编译时间。既有
[M2 corpus](data/compile-m2-final/results.json)、新增
[500 站点 array-product corpus](data/compile-product-update-final/results.json)
各运行 30 次；[既有 corpus A/A](data/compile-m2-aa/results.json)与
[新增 corpus A/A](data/compile-product-update-aa/results.json)使用同一 Candidate
probe。以下比值同样是 Candidate/对照的中位数；绝对时间为 Candidate 中位数。

| 编译 case | Candidate ms | / #53 | / #52 | A/A |
| --- | ---: | ---: | ---: | ---: |
| large-block-200 | 1.310 | 1.054 | 1.235 | 0.957 |
| m1-500 | 11.010 | 1.013 | 1.146 | 1.009 |
| m2-500 | 10.041 | 1.013 | 1.101 | 1.011 |
| nonapp-500 | 7.758 | 1.043 | 1.045 | 0.887 |
| product-update-500 | 11.772 | 1.120 | 1.169 | 1.036 |

新增 array-product 选择带来可见编译成本；该 case 对 #53 的 1.120
比值超过本次同二进制 1.036 的标签差，仍应在更多机器上确认。nonapp
的 A/A 本身达到 0.887，说明微小编译差不稳定。完整输入、二进制
SHA-256、每次时值和失败状态都在上述原始结果中。

Candidate 的 Navier–Stokes [profiling 快照](data/v8-candidate/raw/navier-stokes.cost.jsonl)
报告去重后的执行字 **12,132 inline bytes**、边界表 **11,152 inline bytes**、
一个已发布数值 descriptor **120 inline bytes**；均不含 `Rc` 头、分配器、
冷数据和全部其他元数据。历史 M2 descriptor 为 72 bytes；本次更宽的
执行态契约增加单站点元数据。旧版在这个 V8 站点没有发布该 descriptor，
因此不把这 120 bytes 表述成完整程序内存差。源码检查确认成功处理器不做
compiler-PC 转换或第二次 `NumericOperation` 判别；dense 的独立反汇编实验
证实少一个身份认证调用点。单靠源代码或静态调用点数不能推断整机速度。

## 验证身份与限制

实现提交 `6f1de9c5` 的 Rust 1.88 workspace/all-targets 全部通过（主库
**1,970 tests**）；后续 `734bc128` 只调整 profiling 拒绝扫描器的等价偏移
表达式。PR #53 的 [fast CI](https://github.com/pocket-nexus/quickjs-oxide/actions/runs/36377728373)
对更新后的分支通过，包含 Rust 1.88 feature/test/Clippy 矩阵、源码检查与
文档指标；同一 PR 的 focused Test262 job 通过。独立
[full Test262 workflow](https://github.com/pocket-nexus/quickjs-oxide/actions/runs/36378159777)
在此分支通过，冻结向量为 **80,010 pass / 80,060 eligible / 102,037 total**。
本地 QuickJS fixtures **13/13**、C oracles **9/9** 通过。一次本地 focused
runner 曾在编译期间检测到引擎源码发生更改而主动中止；这不是 conformance
失败，该次没有计入验证结果。测量只对应上述固定 workload 与机器，不是
V8 Score，也不预言全局性能。

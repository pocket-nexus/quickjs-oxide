# S3-C / B 第二批：C1 定向测量与 B1b 发布实验

日期：2026-09-22。起点：`3392e4e95a8c65c700e862e743123558ee290149`。
承接 [开头实施记录](s3-c-b-opening.md)、[C 计划](s3-c-plan.md) 与
[B 首批计划](s3-b-initial-plan.md)。本轮证据目录为 `target/s3-c-b-next/`。

## 1. 本轮范围与阶段状态

本轮推进 C1 的输入覆盖、同协议构建和方向性测量；同时实现 B1b 的 eager
发布实验、共享/回滚验证及内存观测。默认执行仍为 canonical。另交付
[C2 边界审计](s3-c2-boundary-audit.md)，不接入 TOS。

| 工作包 | 本轮交付 | 尚未关闭 |
| --- | --- | --- |
| C0 | C1 前与 pre-A 的可认证同协议构建；定向输入、实际命中证明、A/A | E/RSS、残余分配、A4 裁决以及完整 saved 基线 |
| C1 | 两轮独立定向 A/B，区分目标与边界/融合保护项 | 正式性能接受与完整历史矩阵 |
| C2 准备 | 构造点、facade、backing、51 个 RunExit 及隐式释放边界清单 | C1 接受、缓存实现与所有动态边界验证 |
| B1b | 内部 cfg 发布/共享/回滚、五例成本诊断、完整 Test262 | 嵌套 cold RSS 两轮超限；完整矩阵/首次执行及默认启用未接受 |
| B1c/B1d | 未启动 | 等待 C facade 冻结；不提前接热派发 |

再次检索未发现新增 E/RSS、有界残余及 A4 完整关闭 receipt。本轮不将 pre-C1
自动当作 E 后 saved，不用定向实验代替完整验收，也不将 16B 现状充作 A4
数据裁决。B 的成本对照在 C 尚未 accepted 时同样只属方向性实验。

## 2. B1b 的发布与所有权

新增内部 Rust cfg `oxide_quick_projection`；没有公开 Cargo feature 或每条
指令的运行时模式分支。普通默认构建 M0 不包含 QuickOp sidecar 字段、建表
调用或 quick 内存扫描；M1 eager 建表，仍执行现有 canonical handler。
Rust unit test 配置也启用发布路径，便于验证真实 publisher。

发布顺序固定为：

1. 清除 draft 携带的 executable/quick。
2. 完成 metadata、payload 和 generic bytecode 验证。
3. 从确切 code/locals 重建 fusion，再构建并认证 QuickProgram。
4. 成功后才 reserve arena、retain edges、publish。

`QuickProgram` 为不透明共享类型，内部区分 `CanonicalOnly` 与
`Words(Rc<Vec<QuickOp>>)`；draft 的 `None` 不等于已认证全冷程序。
同 bytecode 的 snapshot/closure 共享数组；sidecar 没有 Runtime/GC/atom
owning edge。缺失发布证书直接报错，跨 runtime 沿既有认证拒绝。
synthetic fixture 明确选择 canonical 测试模式，不能由它产生生产证书。

主 word buffer 的 `try_reserve_exact` 失败映射 `HeapError::Allocation`，
仍由原 publisher 链释放 atoms、converted constants 和已发布子函数 roots。
窄 thread-local 测试 hook 注入该可恢复失败；`Rc::new` 全局 allocator OOM
仍沿标准终止策略，没有伪称可回滚。新增测试涵盖验证先于建表、stale 重建、
失败无 arena/edge 残留、全冷不 reserve、共享销毁、BC5 两条发布链。

## 3. 观测口径

新增 `CompilePhase::QuickProjection`，与已有编译阶段共用嵌套计时。
`quick_projection_counts` 是成功构建的累计计数，包括 canonical PC、
word 长度/容量、7 类 tag 和控制区估算；不代表执行频率或当前常驻内存。
全冷函数的规范位置计入 Generic，word 数仍为零。clone 不增加 build 计数。

内存快照按 buffer identity 去重：

- `bytecode_quick_words`：len/capacity × 8。
- `bytecode_quick_controls`：每个独立 `Rc<Vec<_>>` 按 Vec header + 两个
  usize 引用计数估算，明确排除未知 allocator bookkeeping/padding。
- `bytecode_quick_canonical_only` 与 7 类 tag：只有逻辑数量，不再计 bytes。

inline 字段已由既有 arena/executable 大小统计承担，不重复加算。多 snapshot、
1→64 个 closure 的共享，以及 drop/GC 回到基线均有测试。

64 位构建中，QuickProgram 实验字段将统一 `ArenaSlot` 从 M0 的 **440B**
扩大到 M1 / unit-test 配置的 **456B**，每槽 **+16B（+3.64%）**。首次
workspace 验证中的 `arena_layout_is_compact` 实测 456B，触发原 440B
断言；证据保留在 `target/s3-c-b-next/workspace-initial.log`。现分别以编译期断言
固定 M0=440B、M1/test=456B，测试明确检查实验布局，默认布局保护没有放松。
该增量作用于所有 arena 槽，预留容量成本为 `capacity × 16B`，必须与
word backing、控制区及 executable 的增量一并计入 M1 成本。3.64% 表示
单槽跨度变化；进程 RSS 仍须独立测量。

阶段中的 maximum IR capacity 仅是最大单函数 word buffer；真实进程峰值
仍由独立 RSS 观测承担。JSON schema 保持 v1，新增字段采用可扩展 map；
CLI metadata 明示 `disabled` 或 `eager-experiment-canonical-execution`。

构建命令及新增诊断工具说明见 [benchmark README](../../scripts/benchmark/README.md)。

## 4. C1 输入冻结与覆盖

新增 `scripts/benchmark/direct_store.py` 与测试，生成 8 类项目自编输入：
local/arg consume/keep 的 scalar 路径、local consume 与 arg keep 的四类
heap owner、getter/call/throw/captured/mapped 边界，以及已有 add fusion 保护。
manifest 保存源 SHA-256、独立整数公式计算的精确 stdout、目标 opcode
和待实测的事件最低数，直接交给已有 `fixed.py`。

第一次 profiling 发现普通赋值语句实际走 SetLocal/SetArg + Drop：三个
consume case 只命中一次初始化 consume。已改为循环内 `var` initializer；
parameter case 使用同名 formal 的 `var` redeclaration。未降低覆盖门槛。
初次失败的原始 JSONL 保留在 `store-coverage-initial/`。

257 次迭代的修正版实测：

| case | 目标事件 | 最低数 | 实测 |
| --- | --- | ---: | ---: |
| local-consume-scalar | consume | 514 | 515 |
| arg-consume-scalar | consume | 514 | 515 |
| local-consume-heap | consume | 1285 | 1286 |
| local-keep-scalar | keep | 514 | 771 |
| arg-keep-scalar | keep | 514 | 771 |
| arg-keep-heap | keep | 1028 | 1542 |
| mixed-boundaries | consume / keep | 257 / 257 | 261 / 1318 |

所有 stdout 精确匹配；保护 case 不设虚假的 direct-store 最低命中数。
这些事件证明目标 handler 被执行，不等于物理机器搬运或加速证据。

仅用 before 做 100000 次迭代 pilot，随后在看 candidate 时间前冻结规模：
scalar 与 fusion protection 2000000；local heap 300000；arg heap 200000；
mixed 150000。目标是每例约 0.5–0.8 秒。最终输入在 `store-inputs/`，
协议在 `store-protocol.json`；CPU 2，7 轮 A/A，之后两次各 10 轮交错 A/B，
第二次反转首次执行顺序。前六项为目标，后两项为保护。

A/A 各例两标签的中位数比值范围为 0.9889–1.0116，通过预定 2% 噪声门槛。
计时期间不运行构建、测试、profiling、perf 或 RSS 采样。

## 5. 结果与验证

### 5.1 C1 对照身份与计算口径

已保存 `before-c1`（`bcfb4fe5` 冻结导出）与 `pre-a`（`85afd564` 干净
checkout）release build receipts。双方 rustc 1.94.1、显式
`x86_64-unknown-linux-gnu`、fat LTO / CGU=1、无 PGO/无 profiling。
C1 candidate 复用上轮经认证的冻结导出与 binary receipt，避免将 B 改动混入
C1 对照。pre-A 本节只有构建身份，没有对照耗时；保留用于历史回退追踪，
不能替代 E 后 saved，也不能替代 B 的 M0/M1 比较。

| 对照 | binary SHA-256 | source receipt |
| --- | --- | --- |
| before-C1 | `6c3b132f050d691bf3ee8af3278ead7eee042ad40823f412f5eb8cda383d7e64` | `s3-c-b-next/before-c1/x86_64-unknown-linux-gnu/release/qjs.build.json`；1232 个文件，manifest `a839358df6a07396abd5a2cb5c86b5d2e531b3d1ae25cac48b0c738d8d7a4ff6` |
| C1 candidate | `a698722f2041fb7e1006d9afae658112aaee38e56a09568f5fad515e453ec822` | `s3-c-opening/candidate-build/x86_64-unknown-linux-gnu/release/qjs.build.json`；1242 个文件，manifest `c79e4a15207a56190bc76a4b83abbf9c91ac1825e311c606e40bfe4e827a894b` |
| pre-A | `17694ed44a98b67223b3e2d74f5da6d5364181766edf82b424a99215714fa358` | `s3-c-b-next/pre-a/x86_64-unknown-linux-gnu/release/qjs.build.json`；干净 commit `85afd56493393065080680d562d4116ff9bd7974` |

表中 receipt 路径均相对 `target/`。两轮 A/B 的 `metadata.engines` 保存了
相同的 before/after binary hash 与 build receipt；A/A 的两个标签使用同一个
before binary。机器是 Ryzen 7 7840HS，实际样本命令固定 `taskset -c 2`；
metadata 的父进程 affinity 为 0–15，不应误读为样本运行于所有核。

耗时取各 case、各 engine 的 `samples[].process_wall_ns` 中位数，每轮各
10 次；ratio = `median(after) / median(before)`，小于 1 为下降。
表内毫秒仅为展示舍入，ratio 使用未舍入中位数计算。六目标 geomean 为
六个 ratio 的几何平均，两个保护项不混入目标汇总；不合并两轮样本。
所有 320 个 A/B 样本和 112 个 A/A 样本均为 `status=ok`、exit 0、未超时，
原始 stdout 精确匹配 manifest，stderr 为空；重算中位数与 `summary` 一致。

### 5.2 C1 两轮整进程耗时

| case | 第 1 轮 before / after（ms） | after/before | 第 2 轮 before / after（ms） | after/before |
| --- | ---: | ---: | ---: | ---: |
| local-consume-scalar | 509.000 / 507.963 | 0.9980（−0.20%） | 507.663 / 514.734 | 1.0139（+1.39%） |
| local-keep-scalar | 540.375 / 528.383 | 0.9778（−2.22%） | 538.302 / 531.380 | 0.9871（−1.29%） |
| arg-consume-scalar | 612.972 / 523.178 | 0.8535（−14.65%） | 613.611 / 521.756 | 0.8503（−14.97%） |
| arg-keep-scalar | 596.103 / 545.824 | 0.9157（−8.43%） | 595.208 / 547.505 | 0.9199（−8.01%） |
| local-consume-heap | 525.516 / 513.955 | 0.9780（−2.20%） | 523.870 / 511.422 | 0.9762（−2.38%） |
| arg-keep-heap | 652.897 / 667.748 | 1.0227（+2.27%） | 652.534 / 666.966 | 1.0221（+2.21%） |
| mixed-boundaries（保护） | 626.079 / 615.366 | 0.9829（−1.71%） | 623.253 / 616.499 | 0.9892（−1.08%） |
| fused-add-protection（保护） | 448.745 / 440.421 | 0.9815（−1.85%） | 447.425 / 441.855 | 0.9876（−1.24%） |
| 六目标 geomean | — | **0.9559（−4.41%）** | — | **0.9597（−4.03%）** |

两轮目标汇总均达到预注册的方向性 `≤0.98` 条件，主要收益来自参数 scalar
consume/keep；local consume scalar 没有稳定收益，local keep scalar 的幅度
接近 A/A 噪声范围。`arg-keep-heap` 两轮均回退约 2.2%，不能被目标汇总掩盖。
两个保护项没有观察到耗时回退，但此处不足以证明完整边界矩阵均无回归。

可复核原始路径（均相对 `target/s3-c-b-next/`）：

- `store-ab-1/results.json`、`store-ab-2/results.json`：`metadata`、`summary` 与逐次 `samples`；同目录 `samples.jsonl` 和 `raw/` 保留逐次命令、状态、stdout/stderr。
- `store-aa/results.json`：每标签每例 7 次，同 binary 的 ratio 范围 0.9889–1.0116；此 2% 门槛是运行资格检查，不是置信区间。
- `store-protocol.json`、`store-inputs/manifest.json`：预注册目标/保护分组、规模和精确输出。

### 5.3 C1 用户态 cycles / instructions

perf 与计时轮次分开执行。`store-perf/samples.json` 共 80 条，每例每个
engine 5 次，字段为 `events["cycles:u"]`、`events["instructions:u"]`。
下表分别取 5 次原始计数的中位数，再作 after/before；不是每次比值的中位数。
逐条核对对应 `.perf` 的 CSV event/value 字段均一致，计数运行比例均为
`100.00`，没有 `<not counted>`；80 个 stdout 均精确匹配，stderr 均为空。

| case | cycles:u 中位数 before / after | after/before | instructions:u 中位数 before / after | after/before |
| --- | ---: | ---: | ---: | ---: |
| local-consume-scalar | 2358806382 / 2372177867 | 1.0057 | 7073590988 / 6981592339 | 0.9870 |
| local-keep-scalar | 2464266461 / 2410944589 | 0.9784 | 8079068962 / 8187069002 | 1.0134 |
| arg-consume-scalar | 2816477242 / 2389485424 | 0.8484 | 7451666097 / 7307666654 | 0.9807 |
| arg-keep-scalar | 2741452282 / 2507572587 | 0.9147 | 7344243666 / 7388242437 | 1.0060 |
| local-consume-heap | 2395562143 / 2342404766 | 0.9778 | 5765001907 / 5722402665 | 0.9926 |
| arg-keep-heap | 2948655736 / 3040544930 | 1.0312 | 7473957372 / 7486753508 | 1.0017 |
| mixed-boundaries（保护） | 2852885997 / 2828213274 | 0.9914 | 7548929059 / 7558111180 | 1.0012 |
| fused-add-protection（保护） | 2063782223 / 2014776027 | 0.9763 | 6362179938 / 6434179157 | 1.0113 |
| 六目标 geomean | — | **0.9573（−4.27%）** | — | **0.9968（−0.32%）** |

参数 scalar 的 cycles 方向支持耗时结果，但 instructions 并未在每条路径下降；
不能将逻辑 slot move 计数减少直接解释为机器指令同比下降。`arg-keep-heap`
cycles 增加 3.12%，instructions 增加 0.17%；这与两轮耗时回退方向一致，
且超过协议所列 3% 需复测归因的幅度，列为下一步必须调查项。

原始路径为 `target/s3-c-b-next/store-perf/samples.json` 和同目录
`<case>-<before|after>-<0..4>.perf`、`.stdout`、`.stderr`。当前该目录未附
独立的启动 command、binary hash、CPU 与 perf version receipt；逐条原始
计数可复核，但其构建/启动身份记录不如 A/B 完整，暂按诊断证据使用。

另做独立复测，不与首次 perf 合并：只复测 `arg-keep-heap`，并保留
`arg-consume-scalar` 作为方向对照。每例每 engine 5 次，首次 after 先行，
后续交错反转，CPU 2。结果取 `events[event].count` 的各组中位数：

| case | 复测 cycles:u before / after | after/before | 复测 instructions:u before / after | after/before |
| --- | ---: | ---: | ---: | ---: |
| arg-consume-scalar | 2830147968 / 2418105997 | 0.8544（−14.56%） | 7451334190 / 7307333640 | 0.9807（−1.93%） |
| arg-keep-heap | 2979548765 / 3060365378 | 1.0271（+2.71%） | 7473624475 / 7486420121 | 1.0017（+0.17%） |

复测 receipt 为 `target/s3-c-b-next/store-perf-recheck/metadata.json` 与
`samples.json`，原始文件沿用上述 `<case>-<engine>-<repetition>` 命名。
metadata 在测量前冻结 input manifest、workload/engine/build identity、
perf `7.2.6-1`、CPU 和 runner hash；每个 sample 保存完整 command。
runner 为 `target/s3-c-b-next/recheck_perf.py`，SHA-256 为
`4100d23257d89f1e5dbbab3b9b764f22c93b8c15bfdebae01ce7d7c758e69366`。
复核 runner/manifest/workload hash 一致，binary hash 与 §5.1 相同；20 条
均 valid、exit 0、stdout 精确、stderr 空，原始 counters 一致且运行比例 100%。
参数 scalar 收益与 heap keep 回退的方向均复现；heap keep 此次幅度为 2.71%，
没有再次超过 3%，但不能据此把前次超限或回退归因视为关闭。

### 5.4 C1 结论与局限

本轮支持继续调查 C1，尚不接受 C1，也不启动 C2 实现。独立复测已为两个
case 补齐 perf 启动身份；继续归因 `arg-keep-heap`，再决定是否保留整项
事务优化或缩小适用路径；
后续变更需重做相同输入的覆盖、A/A、两轮 A/B 和物理计数对照。

已完成一轮纯静态归因准备，记录与最窄后续采样方案见
[C1 assembly notes](../../target/s3-c-b-next/c1-assembly-notes.md)。两个 binary
重新计算 SHA 与 §5.1 receipt 一致；源码核对相邻 `qjs.source.json`。
LTO 保留独立符号，已能定位下列变化；这些是机器码事实，不是运行成因裁决：

| 已证实变化 | ELF 地址 / 符号偏移 |
| --- | --- |
| 新参数 wrapper 与共享事务仍为独立函数 | candidate `RunSlots::store_parameter_from_top` 在 `0x7346c0`（178B），`store_binding_from_top_current` 在 `0x73a3f0`（804B）；Ready 调用位于 `run+0x650a` |
| `run::run` 减少 1794B，整个 `.text` 净减 944B | before `0x71f210 / 39612B` → after `0x71f2f0 / 37818B`；静态尺寸不等于 retired instructions |
| `run` 宿主栈局部预留增加 80B | 两版 `run+0xa` 的 `sub rsp` 从 `0xf48` → `0xf98`；只是布局/寄存器压力线索，不能据此认定 heap keep 回退或 RSS 增加 |
| Consume 消除旧 pop 与 caller 的 JsValue 重组 | before Ready `run+0x6939` 调 pop、`run+0x6cf5` 调 replace；after helper `+0x11c..+0x151` 直接清源/写目标/减 depth。displaced 返回仍有 spill，不能称零搬运 |
| retain/readiness/查询 helper 没有新增代码 | `copy_reference`、`slot_value_release_readiness_jsvalue`、`parameter_current`、`peek` 去除重定位地址后的反汇编逐条一致；Keep 仍复制一次，外围释放门禁保留 |

原始材料为同一 target 目录的 `c1-before-run.asm`、`c1-after-run.asm`、
`c1-before-helpers.asm`、`c1-after-helpers.asm`、`c1-assembly-symbols.json` 与
`c1-store-source.diff`。没有 DWARF 源行映射或运行 IP 样本：临时值依赖链、
code layout、Keep 跨 retain 的寄存器存活均是待采样假设，不能将其中任一项
写成已证实的加速/回退原因。下一步仅对现有 arg consume scalar / arg keep
heap 采 cycles IP，再按这些符号/偏移定位，暂不新增源码变体。

- 这是单机、单核固定输入的整进程实验，包含启动、编译与执行；不能当作纯 VM handler 耗时。机器 governor 为 `powersave`，未记录锁频证明，A/A 合格也不排除温度与频率漂移。
- perf 首次覆盖八例，独立复测只覆盖两例，每批每格均为 5 次，且只统计用户态；其余六例仍只有首次身份记录不完整的诊断证据。没有额外的硬件事件或汇编归因，不能由 cycles/instructions 单独判定收益成因。
- 六目标为定向合成输入，保护仅两类；没有用本表补写原始 58 fixed、67 compile、9 original 及 3 bigint 等完整历史矩阵结果，也没有测得 E/RSS 或残余分配下降。
- C0 的 E 后 saved 身份、E/RSS、残余分配与 A4 裁决仍未关闭。C1 正式接受、C2 前置和 B 默认启用均维持原门禁。

### 5.5 B1b 首轮资源诊断：编译均值 +0.20%，RSS 两项待复核

本轮只比较 M0 与 eager projection、canonical execution 的 M1；没有 M2，
也没有 QuickOp 执行收益。五个定向输入的 compile geomean 为 **1.002029**，
低于 B 首批计划用于完整 compile 矩阵的 `≤1.02` 数值阈值，尚不构成
compile 门禁通过；RSS 有两个单项超过
`max(M0 × 3%, 1024 KiB)`。因此本轮**不能判定 B1b 资源成本通过，也不能
默认启用**。以下保留首轮完整结果，超限项须独立复测，不与首轮混样。

**输入、构建与样本身份。** 下列路径均相对 `target/s3-c-b-next/`。
五例来自 `quick-resource-inputs-v2/manifest.json`，不是历史 67 compile
矩阵。manifest SHA-256 为
`515defc9b4a43c516ab5b53aa0abbb5e25c1100e62bd3106200ef0f939756dc2`，
generator SHA-256 为
`ec0339931c329416899bf8243244d65acbaf73cdbbd0655a06771c96fe69b9ee`。
原 v1 的 M0-only pilot 中 eval 外层 compile 为 5.969939ms；在任何 M1
资源采样前，将该例由 64 扩为 256 个 source，保持每 source 256 次加法、
4 passes，合计 1024 次 eval。v2 的 M0-only pilot 为 23.650490ms，符合
约 24ms 的尺度目标；前四例 source/stdout 字节保持不变。
`quick-pilot.json`、`quick-pilot-v2.json` 各含 15 条 Node/M0/M0-compile
结果，均有效；变更依据与原 hash 在 v2 的 `revision-receipt.json`。
v2 eval 为 1064585B，精确 stdout 为 `12678656\n`。输入选择依据是 before
尺度，不能把 v2 相对 v1 的时间变化解释成 M1 效果。

五个 binary 都来自同一 `source/` 的 1251 文件冻结导出；`source.json`
SHA-256 为 `44f1c108ac4a26092818657d98a8c6012271d0bf2bb1e4ccb8f3ac63eea883ad`。
相邻 build receipt 均记录构建前后认证，manifest 副本与冻结源逐文件 hash
复核一致。普通测量均为 release、fat LTO、CGU1、无 PGO；M1 仅增加
`RUSTFLAGS="--cfg oxide_quick_projection"`。CLI 显式 target 为
`x86_64-unknown-linux-gnu`，probe 为同架构 host；rustc 为 1.94.1。
probe 的 LTO/CGU1 环境保存在 `mode-builds.json` 与 `build_modes.py`，
其独立 build receipt 未重复记录这两个环境字段，需联合审计这三处身份。

| binary 相对路径 | SHA-256 |
| --- | --- |
| `m0/x86_64-unknown-linux-gnu/release/qjs` | `d233c3776e6602f623b3fed44afd13e872b44c2fbab9eae4eb12701775d5a0db` |
| `m1/x86_64-unknown-linux-gnu/release/qjs` | `09a99d28323826b3426ca336ad71df424f6fe0220ff42608d3dbbb834017eb83` |
| `m0-probe/target/release/oxide-compile-probe` | `46340033d549f9685cd7d2b5197d27cc4234e1bd1b5b3a742cb0321cbe9d3840` |
| `m1-probe/target/release/oxide-compile-probe` | `4ff31166a0c9d5d181aeb4a17b721aa73680158a2faa18a7429d21df6705e40a` |
| `m1-profile/x86_64-unknown-linux-gnu/release/qjs` | `5f5845af339c50212b992fe7f3f78587ba234c9457a17232417e099255bc14a8` |

build receipt 为各 binary 相邻的 `.build.json`；probe 根目录同时有
`build.json`、`source-manifest.json`。完整 receipt 内容与自身 SHA-256
已冻结进 `quick-resources-run-01/protocol.json`，其 SHA-256 为
`e84fd159ff4c59f91dc3432058ed763a0fca79dc9968e030b1461871e0d68065`。
runner `run_quick_resource.py` 的 SHA-256 为
`262fdc4dae8d343e2ad0785e23ce90e4bd2e3e7cbb462847a97f49f1f7b9f4b3`。
CPU 2、串行分 cohort，case/repetition 交错 M0/M1 先后顺序。
`samples.jsonl` 与 `results.json` 的 255 条完全一致：compile 100、cold 50、
RSS-compile 50、RSS-cold 50、独立 M1 profile 5；全部 exit 0、无 timeout、
stdout 符合各阶段契约、stderr 空。逐条命令、source/binary hash、原始输出、
RSS/profile 文件及所有 summary 中位数/比值已只读重算核对。

**普通 release 时间。** 每格先取各自样本中位数，再算 M1/M0；compile
每格 10 次、cold 每格 5 次，表中时间单位 ms。compile 来自公共 compile
API 的 `compile_ns`，不包含源文件读取、Runtime/Context 创建或执行。
eval 的 compile 只编译外层 Script、字符串常量和循环，**不包含 1024 次
eval body 的动态编译**。cold 为新进程从启动到退出的 wall time，包含
读取、编译/发布、执行和清理；它既不是独立首次执行，也不保证磁盘缓存冷。
本轮没有用两个独立进程时间相减来推算首次执行。

| case | compile M0 / M1 | M1/M0 | cold M0 / M1 | M1/M0 |
| --- | ---: | ---: | ---: | ---: |
| many_short_functions | 167.858 / 171.307 | 1.0205（+2.05%） | 205.278 / 209.977 | 1.0229（+2.29%） |
| one_large_function | 54.875 / 55.609 | 1.0134（+1.34%） | 62.684 / 63.922 | 1.0197（+1.97%） |
| moderate_nested_functions | 38.749 / 38.355 | 0.9898（−1.02%） | 60.749 / 61.898 | 1.0189（+1.89%） |
| many_closures_shared_code | 38.221 / 38.911 | 1.0181（+1.81%） | 90.114 / 90.841 | 1.0081（+0.81%） |
| repeated_eval_release | 23.617 / 22.892 | 0.9693（−3.07%） | 712.361 / 719.934 | 1.0106（+1.06%） |
| 五目标 geomean | — | **1.002029（+0.203%）** | — | **1.016030（+1.603%）** |

本轮 compile/cold 无单项增加超过 3%；五例 compile 均值低于 `1.02`，
完整 compile 验收仍未完成。
cold geomean 单独报告，不能将只适用于 M2/M0 执行保护的 `≤1.01`
错套为本轮 M1 裁决，也不能替代尚未隔离的首次执行/发布测量。

**独立 RSS。** 每格 5 次新进程、取 child peak RSS 中位数；这些进程的
wall time 不进入上表。环境缺少 `/usr/bin/time`，使用预先编译的 Linux
`posix_spawnp + wait4(child).ru_maxrss` helper，单位 KiB，排除 helper
父进程自身内存。`rss_wait4.c`、`rss_wait4`、`rss-wait4-build.json` 保存
源代码、binary 与构建命令/编译器身份；源 SHA-256 为
`6a3caad9ff1bc439b8d3601e6fb557ffeb18f184b4ab3086b595e6161deeed73`，
binary SHA-256 为
`e0e8dfd4febd5d11acf80caeee9e907b533c5066b9236c5f00feb4e71f3259aa`。
下表的 Δ 与阈值单位同为 KiB，阈值逐项取 `max(M0 × 0.03, 1024)`。

| cohort / case | M0 / M1 KiB | Δ KiB | M1/M0 | 允许增加 KiB | 首轮 |
| --- | ---: | ---: | ---: | ---: | --- |
| compile / many_short_functions | 38636 / 38084 | −552 | 0.9857 | 1159.08 | 未超限 |
| compile / one_large_function | 36900 / 37664 | +764 | 1.0207 | 1107.00 | 未超限 |
| compile / moderate_nested_functions | 27908 / 27608 | −300 | 0.9893 | 1024.00 | 未超限 |
| compile / many_closures_shared_code | 26708 / 27992 | **+1284** | **1.0481** | 1024.00 | **超限，待复核** |
| compile / repeated_eval_release | 14392 / 14088 | −304 | 0.9789 | 1024.00 | 未超限 |
| cold / many_short_functions | 38948 / 39004 | +56 | 1.0014 | 1168.44 | 未超限 |
| cold / one_large_function | 38224 / 37312 | −912 | 0.9761 | 1146.72 | 未超限 |
| cold / moderate_nested_functions | 28056 / 29364 | **+1308** | **1.0466** | 1024.00 | **超限，待复核** |
| cold / many_closures_shared_code | 27312 / 28036 | +724 | 1.0265 | 1024.00 | 未超限 |
| cold / repeated_eval_release | 15844 / 15796 | −48 | 0.9970 | 1024.00 | 未超限 |

RSS 包含源缓冲、canonical code、quick words、heap 与 allocator retention，
不是纯 sidecar bytes。两项超限不能由其余项的负差值抵消；也不能把负差值
当作 sidecar 减少内存的证明。按 B 首批 §6.3，复核后仍超预算就应减小或
收窄，保持默认关闭；本轮没有放宽预算。

**独立 profiling 的分配、共享与存活快照。** 每例只有一次 M1 profiling
运行，不能与普通 release 作速度或 RSS 比值。下表 quick 累计字段来自
`oxide-compile-vm-cost-v1.quick_projection_counts`；capacity 是累计构造
的 backing capacity，不是同时常驻或进程峰值。控制区按每独立 `Rc<Vec>`
40B 估算（64 位 Vec header 加两个 usize 引用计数），不含 allocator
bookkeeping/padding。这里各例累计 `word_capacity_bytes = words × 8`。

| case | 构造函数 / CanonicalOnly | 累计 words | 累计 backing B | 累计 control 估算 B | 最大单 buffer B |
| --- | ---: | ---: | ---: | ---: | ---: |
| many_short_functions | 4097 / 0 | 65544 | 524352 | 163880 | 327744 |
| one_large_function | 2 / 1 | 120006 | 960048 | 40 | 960048 |
| moderate_nested_functions | 3329 / 0 | 28168 | 225344 | 133160 | 69696 |
| many_closures_shared_code | 3 / 0 | 80093 | 640744 | 120 | 640128 |
| repeated_eval_release | 2049 / 0 | 1325600 | 10604800 | 81960 | 10288 |

累计 GenericCanonical 逻辑位置依次为 53255、96013、21511、64066、1061386；
这包含 CanonicalOnly 的逻辑位置，不代表动态 Generic 执行比例。单大函数
例有 1 个 CanonicalOnly、9 个逻辑位置，未分配该函数的 word backing/control。
`quick_projection` 诊断 phase 累计分别为 1.466、0.842、1.077、0.577、
10.734ms；最后一项包含全部动态 eval 发布，不是外层 compile 的分解。
这些 instrumented 单次计数仅用于归因。

存活快照来自 `oxide-memory-v1` 的 `after-jobs-before-context-drop` 阶段，
word backing/control 按 identity 去重。arena 的 count 包含 vacant 槽，
其 used/capacity 是容器存储，不能称为 live payload。下表保留实际 bytes：

| case | arena count / capacity 槽 | arena used / capacity B | 留存 word 数 / backing B | 留存 control 数 / 估算 B | executable projection 数 / B |
| --- | ---: | ---: | ---: | ---: | ---: |
| many_short_functions | 21019 / 32768 | 9584664 / 14942208 | 24576 / 196608 | 4096 / 163840 | 4096 / 1114112 |
| one_large_function | 542 / 1024 | 247152 / 466944 | 120006 / 960048 | 1 / 40 | 1 / 272 |
| moderate_nested_functions | 4907 / 8192 | 2237592 / 3735552 | 19456 / 155648 | 3328 / 133120 | 3328 / 905216 |
| many_closures_shared_code | 24556 / 32768 | 11197536 / 14942208 | 80020 / 640160 | 2 / 80 | 2 / 544 |
| repeated_eval_release | 1037 / 2048 | 472872 / 933888 | 0 / 0 | 0 / 0 | 0 / 0 |

上表全部留存 CanonicalOnly 计数为 0；单大函数例的那个 CanonicalOnly
已随外层 Script 释放。M1 的 456B arena slot 与 M0 的 440B 相比，按
上表相同 capacity 推算，预留 inline 增量依次为 524288、16384、131072、
524288、32768B；这是同容量理论差值，本轮没有 M0 profiling 快照，
不能冒充两边实测 heap 差值。executable projection 272B 是 M1 留存结构
总大小，已含 inline 字段，不能再把其中 quick inline 重复计费。

`executable.published_data_rc` 的 `capacity_growths / shared_storage_clones`
依次为 4097/4097、2/2、3329/3329、**3/8002**、2049/5121。共享例建立
8000 个 closure，却只有 3 次 quick 构造和 3 次 executable 分配；
8002 次共享观察与终点 2 个独立 backing 支持没有按 closure 复制数组。
其 `observed_capacity_bytes=2176544` 是重复观察累计量，不是常驻量；
实际 executable 累计分配为 816B。eval 的 2049 次构造对应外层 1 次加
1024 次 eval 各自的 Script/child function；终点 quick backing/control
和 executable projection 均为 0。这个单终点证明本次临时投影已释放，
不代替多规模持续增长或 GC 后完整生命周期门禁。

profile 原始记录为 `quick-resources-run-01/raw/0250-profile-*.profile.jsonl`
至 `0254-profile-*.profile.jsonl`，同样保存在对应 sample 的 `profile_reports`。
本轮没有独立首次执行、profile compile-only、M0 profile 对照或多规模线性
曲线，也未跑历史完整矩阵。首轮资源样本完整不等于 B1b 正式验收完成；
RSS 两项复核和一致性验证继续单独登记。

### 5.6 RSS 独立复核：嵌套 cold 仍超限

完整 Test262 结束后，单独复测首轮两项超限，每项每模式 5 次，将首轮
各 pair 的 M0/M1 顺序反转。沿用原输入、普通 release binary 和 wait4
helper，重新冻结 `quick-rss-recheck/protocol.json`；20 条均 exit 0、
无 timeout、输出符合契约、stderr 空。只采纳 RSS，不混入首轮或时间样本。

| cohort / case | 复测 M0 / M1 KiB | Δ KiB | M1/M0 | 允许增加 KiB | 复测结论 |
| --- | ---: | ---: | ---: | ---: | --- |
| compile / many_closures_shared_code | 27588 / 27324 | −264 | 0.9904 | 1024 | 首轮超限未复现 |
| cold / moderate_nested_functions | 28156 / 29280 | **+1124** | **1.0399** | 1024 | **再次超限** |

复核记录为 `quick-rss-recheck/results.json`、`samples.json` 和 `raw/`，
runner 为 `rss_recheck.py`。共享 compile 的正负差异提示峰值测量存在
波动，不能由一次负差值宣称节省内存。嵌套 cold 两轮分别增加 1308、
1124 KiB，均超 1MiB 门槛，故 **B1b 内存成本尚未通过，默认保持关闭**。
后续应先拆分统一 arena 每槽 +16B、每函数控制区及 allocator retention，
减小或收窄存储后再以相同输入复测；不能直接进入默认发布或热派发。

### 5.7 本轮回归验证

所有构建、测试、计时、perf、RSS 与 profiling 均由主执行者串行调度。
初次 workspace 的唯一失败是旧 arena 布局断言仍期待 440B，日志保留在
`workspace-initial.log`；已分别加入默认 M0=440B 和实验/test=456B 的
64 位编译期约束，测试明确验证实验布局，未把默认限制放宽。

| 验证 | 结果 | 本轮 receipt / log |
| --- | --- | --- |
| benchmark Python suite | 40 passed；包含修正后的 consume 输入、独立 Node 输出与 manifest 拒绝测试 | `python-final.log`、`validation-initial.json` |
| fmt / source-layout / rust-only | 全部通过 | `fmt.log`、`layout.log`、`rust-only.log` |
| `cargo test --locked --workspace` | 3476 passed，0 failed，1 个既有 ignored；其中库测试 2388 | `workspace.log` |
| profiling lib | 2578 passed，含所有权、融合与 quick 内存/计数测试 | `profiling-lib.log` |
| profiling CLI | 默认版和内部 cfg 实验版各 7 passed | `cli-profiling.log`、`m1-cli.log` |
| Rust 1.88 workspace/all-targets Clippy，`-D warnings` | 默认与内部 cfg + profiling 均通过 | `clippy.log`、`m1-clippy.log` |
| M1 current-source 完整 Test262 | 完整既有结果向量匹配 | `m1-test262/` |

Test262 的 `--check` 只认证旧基线并报告 stale，因此没有把它当新源码
通过；随后执行 `--full`，`TEST262_WORKERS=2`，
`RUSTFLAGS="--cfg oxide_quick_projection"`，未设置覆盖优先的 encoded flags。
完整向量为 `pass=79982 / eligible=80032 / total=102037`；既有 50 个
eligible 失败保持匹配，不能声称 102037 项全部通过。`current.conf` 未改变。

源码语义指纹：
`2bf9d08eae135793c3dde61ce73afc09ed62aec4f524651832735002cfd49f1f`。
认证 runner SHA-256：
`5317677e9a147758427dafb0f53b83053c7b16da57679cdb925d1fc03787c8c6`。
`m1-test262/receipt.json` 保存命令、环境、退出码与实际新产出的完整文件：

- `test262-full.tsv`：`674c3a9ba8cbe8064ae8becdbd509fcf53e7b33aa4e05b4d7dd2841231b6b9ab`。
- `test262-full.jsonl`：`56176d293da8a4a31b0352264cc189b8054c9913e64df1fc2e9e02640472ed0c`。
- `runner.json` 保留构建认证；内部 cfg 由外层 receipt 记录，不能从其空 Cargo feature 列表推断 M0。

本次完整向量属于 M1。默认配置的当前源码验证为 workspace/CLI/Clippy；
不把实验版完整向量另称一次 M0 全量运行。收尾时 `final-source-check.json`
验证 1206 个非 Markdown 文件与测量用冻结源逐字节相同；后续只追加报告
文字，代码没有在测试或采样后发生变化。上述正确性证据不能覆盖未完成的
完整性能矩阵、独立首次执行、C0 前置裁决与 B 内存成本接受。

## 6. 后续顺序

1. 结合两轮定向结果与物理成本证据，决定 C1 是否值得进入完整矩阵；不越过
   E/残余/A4 门禁宣称接受。无可靠收益时保留实验结果并停止扩大 C 改造。
2. 按边界审计表准备 C2 测试；新增 Drop 所需显式 drop、push 后 release 的
   二次 canonicalize 等全部处理后才能接缓存。
3. B1b 默认保持关闭，先处理嵌套 cold 两轮 RSS 超限：分离 arena 槽膨胀、
   每函数控制区和 allocator retention，缩减或收窄投影存储，再复测同一输入。
   完整 compile/RSS、首次执行和执行保护门禁通过后才讨论默认路径；
   B1c/B1d 仍等待 C facade 冻结。

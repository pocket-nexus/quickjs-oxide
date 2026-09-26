# VM 执行架构重写：结构、语义与性能收据

本次重写在隔离工作树从 `f879c351521e4ec162bac69acaf3b9a7609fb47e`
开始。旧版只作为工作树外的对照二进制；新产品代码没有调用旧解释循环。
用于性能测量的干净源码快照为 `e0c2037970e168c0cfd2a606b1558ab780020b41`。
该快照是临时测量对象，不是分支提交；本收据及其入口文档在冻结快照后写入。

## 结构

- 实际删除 `src/engine/code/fusion.rs`、`code/fusion/`、
  `src/engine/vm/run.rs` 和 `vm/run/` 的执行实现，并移除产品代码的
  `FusionPlan`、`RunExit` 和旧 driver 接线。相对起点，测量快照约删除
  10,567 行、增加 6,455 行；统计包含删除后建立的新执行代码。
- 编译器继续使用临时 `Instruction` IR；发布器验证和编码后，已发布
  函数只保留 `ExecCode`。执行字为 32 位，包含 opcode、短操作数和经
  验证的字宽；宽操作数使用扩展字。PC 使用执行字偏移，边界表负责
  跳转、异常和源码位置映射。
- `src/engine/vm/execute.rs` 是唯一执行循环。短时 `FrameCursor` 借用
  与 owner 转移在进入可能调用 JS、抛错或挂起的 driver 前结束。
  发布器直接选择通用和专用 opcode；动态 guard 在副作用前失败时，
  新循环继续执行同一字流中的通用操作。生成器和 async 的恢复 PC
  由新边界验证。保留的对象和转换算法是语义实现，不是旧指令派发。
- 产品源码搜索 `FusionPlan|RunExit|vm::run|mod run;|code::fusion`
  无命中；`check-source-layout.py` 验证 618 个可达 Rust 文件。

## 基线与语义门禁

冻结的 `f879c351` release 二进制 SHA-256 为
`bb931a92e0448133ee5eceba652639ed411cabd030477136701272384d7d71d5`；
新执行器 release 二进制 SHA-256 为
`f326ca73149d561b14bfabf1d8ea61850abe5f640fc17cff8859d93d2649bdf2`。
`main=2ed79f4649ca9ce438cad481ecfc89120e8d01be` 和 PR #37
`3341ac456ea2719858fd6173e8dcd9123ad9e660` 也各自从干净源码
构建；其 release SHA-256 分别为
`eade925a8fa58271433864cda892970727f6ba1dd5b90eff7cd2b604760b3e6c`
和 `f0c6628fdc9b5cda513cebc27615f000c9fb80f3bf3e92bc0c8156c6e52338ac`。
构建收据、编译命令、工具链、二进制和原始日志保存在工作树外的
`vm-rewrite-f879c351/{plain,candidate-final-plain,candidate-final-profile,main-plain,b37-plain}`。

- `cargo test --locked --workspace --all-targets`：3,037 pass，0 fail。
- `cargo test --locked -p quickjs-oxide --lib --features profiling`：
  2,102 pass，0 fail。
- focused Test262：6,844/6,844 pass。完整 Test262：80,010 pass，
  80,060 eligible；完整结果与冻结向量逐项一致。
- 旧二进制与新二进制执行同一烟测，TDZ、Proxy、数组、闭包、异常
  转换顺序、generator、async 的退出码及 stdout/stderr 完全一致。
- `cargo fmt --all -- --check`、`git diff --check`、源码布局检查通过。
  Clippy 在 `-D warnings` 下通过；明确豁免以下样式告警
  `collapsible_if`、`manual_is_multiple_of`、`needless_range_loop`、
  `cloned_ref_to_slice_refs`、`assertions_on_constants`、
  `items_after_test_module`（最后一项来自新执行文件的测试模块位置）。

## 测量方法与初步机制证据

V8 源码固定为 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。
固定迭代协议记录 A/A、交错 A/B 的整进程 wall time、退休指令、cycles
和 RSS；它包含启动、编译与回收，不是原版 V8 Score。原版 Score 单独
运行和报告。所有结果同时保存基线身份、二进制哈希、原始样本与生成
负载。Apple Silicon 上使用 `/usr/bin/time -l` 的硬件计数器；同机
其他活动可影响耗时，以 A/A 估计可分辨范围。

八项固定次数 profiling 显示：EarleyBoyer 的 `compare_branch_stack`
约 122 万次尝试中约 2 万次命中；RegExp 的 `dense_array_read`
约 18.4 万次尝试中未命中。这是新专用 opcode 的动态失败成本证据，
不是给方向设收益上界。发布字与边界表的切片载荷在八项中约为旧
`Instruction` 切片的 68%–74%；该比例不代表总 RSS。发布期编码
比旧融合计划做更多工作，编译成本仍需单独评价。

## 固定迭代性能裁决

三次直接对照均为 8 isolated 加 combined，交错 A/B 与同二进制 A/A，
各 144/144 样本正常。下表是**候选／表头所列分母**的中位数比例：
wall、指令、cycles 高于 1 表示候选更贵；isolated 八项为 wall 比例的
几何平均。这些是固定工作量整进程指标，**不是原版 V8 Score**。

| 分母 | isolated wall | combined wall | combined 指令 | combined cycles | combined A/A wall 范围 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `f879c351` 直接父版本 | 1.1542 | 1.1584 | 1.1478 | 1.1594 | 2.62% |
| `main` `2ed79f46` | 0.9423 | 0.9875 | 0.9831 | 0.9874 | 1.78% |
| PR #37 `3341ac45` | 0.9394 | 0.9869 | 0.9807 | 0.9860 | 1.81% |

相对父版本的 combined 回退大于 A/A 时间波动，且退休指令与 cycles
同向，已经建立的机制事实是执行了更多工作。相对更早的 `main` 与
PR #37，combined 指令略少；约 1.3% 的 wall 差仍落在各自 A/A 范围内，
不能宣称整套时间收益。不同分母各有自己的 pilot 工作量；不能拼接
绝对耗时、把旧分支收益计为本次重写收益，或用符号 self% 设上界。

相对直接父版本的八项结果如下，比例定义同上；A/A 是同一二进制
wall 样本的观察范围。RayTrace 的约 1.5% 时间差落在 12.0% A/A
波动内，但其退休指令仍增加。NavierStokes 等确定的工作量回退
不能归因于测量噪声。

| 子项 | wall | 指令 | cycles | A/A wall 范围 |
| --- | ---: | ---: | ---: | ---: |
| Richards | 1.2825 | 1.2405 | 1.2840 | 10.98% |
| DeltaBlue | 1.1456 | 1.1278 | 1.1465 | 1.08% |
| Crypto | 1.0789 | 1.0677 | 1.0779 | 0.19% |
| RayTrace | 1.0152 | 1.0206 | 1.0175 | 12.04% |
| EarleyBoyer | 1.1132 | 1.1000 | 1.1138 | 6.39% |
| RegExp | 1.0635 | 1.0599 | 1.0634 | 0.73% |
| Splay | 1.2782 | 1.2175 | 1.2796 | 1.01% |
| NavierStokes | 1.2932 | 1.3771 | 1.2913 | 1.56% |

原始证据目录为 `compare-f879-final`、`compare-main-final`、
`compare-b37-final`。同机有其他应用活动；时间结论仅限于上述 A/A
可分辨范围。文件大小：父版本 7,776,864 字节，候选 7,774,112 字节；
小幅的文件大小变化不能解释或抵消整进程指令回退。八项 profiling
中的发布代码切片载荷约减少 26%–32%，但三组 combined 的 RSS
中位数只变化约 −0.2% 至 −1.2%，不据此宣称总内存显著改善。
两份 release 反汇编另存为 `run-f879.asm` 与 `execute-frame-final.asm`；
主解释函数约为 22,084 与 22,244 字节。函数体积接近，不足以单独
解释父版本的整进程指令回退；生成代码与命中、失败路径仍需后续针对性
归因。profiling 的发布期 `encode` 工作也高于旧 `fusion` 阶段，
但本轮单次受干扰的阶段计时不足以给出稳定编译耗时倍率。

## 原版 V8 Score 与剩余问题

原版八项 isolated 每引擎各完成一次。表中分数越高越快；直接用
八项同名分数求几何平均，候选／父版本为 **0.8656**，候选／`main`
为 **1.0807**，候选／PR #37 为 **1.0766**。这是单次样本，
不把小幅差异说成稳定 Score 收益。原始 32 个成功样本及哈希摘要
保存在工作树外的 `v8-score-final/{samples.jsonl,isolated-summary.json}`。

| 子项 Score | 候选 | `f879c351` | `main` | PR #37 |
| --- | ---: | ---: | ---: | ---: |
| Richards | 66.2 | 84.8 | 75.8 | 76.3 |
| DeltaBlue | 89.8 | 105.0 | 90.7 | 91.0 |
| Crypto | 180.0 | 193.0 | 97.2 | 98.0 |
| RayTrace | 135.0 | 138.0 | 129.0 | 129.0 |
| EarleyBoyer | 157.0 | 177.0 | 171.0 | 172.0 |
| RegExp | 105.0 | 109.0 | 106.0 | 105.0 |
| Splay | 390.0 | 502.0 | 444.0 | 447.0 |
| NavierStokes | 453.0 | 581.0 | 326.0 | 329.0 |

最初的 `all` 进程超过 120 秒的旧 timeout，不记作引擎失败或
0 分；停止相同上限的其余进程后，combined 单独用 300 秒上限
采集，避免重复八项 isolated。四个 combined 进程均正常完成：

| 原版 combined Score | 候选 | `f879c351` | `main` | PR #37 |
| --- | ---: | ---: | ---: | ---: |
| Score（高为快） | 159.0 | 183.0 | 147.0 | 145.0 |
| 候选／该分母 | 1.000 | 0.869 | 1.082 | 1.097 |

combined 原始样本、负载身份与哈希在 `v8-score-combined-final`。
上述 Score 每引擎只有一次，不能把相对旧基线 8%–10% 的单次差异
当成可重复的目标收益，也不能与固定迭代矩阵的 wall 比例直接相除。
相对直接父版本，原版 combined 与固定矩阵均明确回退；3–4 倍目标
远未达到。

性能债包括普通解释派发、动态失败站点以及发布期编码成本；后续
修改仍须保持本次语义与结构门禁，不恢复旧执行循环。

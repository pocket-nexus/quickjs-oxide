# M1：数值数组读取与 local 累加区域

M1 在现有 `ExecCode` 解释器内把符合条件的
`sum += array[i] * scale;` 编译为一个计划操作。编译器在 lowering 后
建立临时基本块 use/effect 图，从丢弃表达式结果的语句中选择直接输入和
未捕获的普通 local 目的地。发布器在同一字流中保留原指令用于回落，
验证描述符、常量、入口、continuation、源码位置和逻辑栈要求。
成功执行时，guard 读取 Number 输入与 genuine Array 的 own Number
元素，在作用域内认证目的地，按 `element.mul(scale)`、
`old_sum.add(product)` 计算并写入一次。guard miss 从原本的第一条
local 读取开始执行普通字流。

这是一个有意狭窄的形态：dynamic/eval、mapped arguments、capture、
lexical/const 目的地及不支持的输入源不选择。仅测试构建提供作用域化
selection override，供同一解释器的 optimized/generic 差分测试使用；
公开 embedding API、通用值表示、中央 dispatch 和数组 backing 未变。

## 源码和产物身份

| 角色 | 不可变提交 | plain `qjs` SHA-256 | 文件字节数 |
| --- | --- | --- | ---: |
| Candidate | `3d98c0a8ee065e5c71607b46bd0059282c6d7e87` | `903c2ab07750474a06fec6a331f78ed39a143a44800b7dbd2d4796d8e780e084` | 7,792,288 |
| Parent | `4287e8c6019933289f3a06e703aea15bf79c9f11` | `9ad89c5eb7176ba44a5b6cf7b0650f3a98f90ea0f316bc1c068e2c0f7dfde475` | 7,774,112 |
| PR #52 | `996663f771afdabdc69d52c94bd4d2fb392e27b1` | `4d41a89b97c5f1a6dee9eb7bf3aafee2f51d409142c94c1c9524f9a3334a55f4` | 7,774,112 |

Candidate 源码树为 `7d684a46710d1865bb27076d4a81984850fa46c8`；
三者均从干净提交分别构建。Parent 与 #52 的 `src` 差异仅是七个
`README.md`，`Cargo.toml`/`Cargo.lock` 无差异，但仍独立重建对照。
本收据和入口文档在上述 Candidate 测量提交之后写入，不把文档提交
称为新的引擎验证。后续提交
`55ef26f508c06798876d6886bb5c7ac60b1ef075` 只增加 `#[cfg(test)]`
下的容量测试，plain 产品代码未变。构建命令、锁文件哈希、工具链、工作树状态和二进制
身份保存在 [`data/*-qjs-build.json`](data/candidate-qjs-build.json)
及同目录的三个 compile probe 构建文件。测量主机为 AC 供电的
Apple M1、8 logical CPUs、16 GiB、macOS 26.6.2；plain release 使用
stable `rustc 1.96.0`、fat LTO、CGU=1。语义/MSRV 检查另用 1.88.0。

## 工作量与测量方法

[`workloads/`](workloads/hit.js) 保存全部实际 JS 输入：`hit.js`
把 dense own Number 元素累加 2,000,000 次；`miss.js` 同次数读取 hole
并检查 NaN；`nonapp.js` 用 lexical `let sum`，不发布 M1。
输出依次是 `10000000`、`true`、`10000000`。三份 SHA-256 分别是
`377a595995fd6764db69e4157a74526e8ebb18fe27453bcdcfa2d5b9f4dd720f`、
`cc7450fce7ab2d6bdc41ddc72e73e4fe21287f8e91f51efaf62919bb14d5566c`、
`aea2f5ce87d42dfb991bee998c2f1223ce3f6a8cb530921821f65d0724c9fd2d`。
这些是固定工作量整进程探针，包含启动、编译与退出，不是隔离的
执行耗时或原版 V8 Score。

`scripts/benchmark/fixed.py` 用 ABBA/BAAB 交错，逐次验证状态和输出，
通过 macOS `/usr/bin/time -l` 记录整进程 wall、退休指令、cycles、
最大 RSS。Parent/Candidate 各 8 次；#52/Candidate 首轮各 8 次受其他
主机活动干扰，尤其 hit wall 出现 1,036.7 ms 的 #52 离群值，故完整
保留[首轮结果](data/fixed-pr52.json)，再独立完成各 12 次的
[重复对照](data/fixed-pr52-repeat.json)。Candidate/Candidate 的 4 次
[A/A](data/aa-candidate.json) hit wall 中位数相差 0.6%，但单次仍有明显
波动。下表使用每组成功样本各自中位数；所有正式样本状态均为 `ok`。
完整样本及硬件计数器见 [Parent 对照](data/fixed-parent.json)和
[#52 重复对照](data/fixed-pr52-repeat.json)。

| 工作量 | Parent wall ms | #52 wall ms | Candidate wall ms（Parent 对照 / #52 对照） | Candidate/Parent | Candidate/#52 |
| --- | ---: | ---: | ---: | ---: | ---: |
| hit | 244.360 | 244.953 | 133.417 / 135.152 | 0.546 | 0.552 |
| miss | 700.118 | 702.712 | 752.126 / 755.234 | 1.074 | 1.075 |
| nonapp | 256.084 | 245.722 | 252.397 / 241.754 | 0.986 | 0.984 |

hit 的 Candidate/Parent 与 Candidate/#52 退休指令比均为 **0.7353**，
cycles 比为 **0.5464 / 0.5442**。miss 的退休指令比约 **0.98**，
但 cycles 比 **1.076 / 1.073**，与约 7.5% 的 wall 回退一致：更少
指令不等于更低失败成本。nonapp 的退休指令比约 **1.000**；小幅 wall
差不构成未选择路径收益。该结论只针对这些负载和本机，不推算引擎
整体分数。miss 成本是后续 guard/fallback 工作的明确目标。

plain Candidate `qjs` 比两个基线各大 **18,176 字节（0.234%）**。
hit 的 profiling memory snapshot 中，Candidate 与 Parent 的执行词
均为 54 个 / 216 字节，边界均为 48 个 / 192 字节；Candidate 另有
1 个 region descriptor / 64 字节。该分类只统计去重 `Rc` slice 的
inline bytes，不包括 `Rc` 头、分配器或整个函数。整进程 hit 最大 RSS
中位数：Parent 5,980,160、#52 5,799,936、Candidate 分别在两轮为
6,094,848 / 6,037,504 字节。RSS 包含整个进程且受运行环境影响，
不能把差值归因于单个 descriptor。

前端编译另由 `scripts/benchmark/build_compile_probe.py` 分别从三份
干净源码构建无 profiling 的 probe。40 次交错新进程编译同一
[`compile.js`](workloads/compile.js)（500 个含 M1 形态的函数，
41,890 字节，SHA-256
`eba8bd933075bea6b6e967f77396baef6a57b5eae0a0052aacf8b805d7137617`）。
计时只包围 Script-goal 编译，不含 Runtime/Context 构造、文件 I/O 或
销毁。Parent **6.469 ms**、#52 **6.488 ms**、Candidate **6.989 ms**
中位数；Candidate 比 Parent / #52 分别多约 **8.0% / 7.7%**。
[40 次原始结果](data/compile-matrix-repeat.json)和受噪声影响、方向
相反的[12 次先导结果](data/compile-matrix.json)都保留。此 corpus 不能
代表完整前端编译分布；目前没有将编译成本归因于单个规划阶段。

## 覆盖与工作删除证据

profile 构建不用于上述时间结论。10,000 次的
[`profile-hit.js`](workloads/profile-hit.js) 在执行字 PC 33 的
`numeric_array_accumulate` 站点为 10,000/10,000 hit；
[`profile-miss.js`](workloads/profile-miss.js) 在 PC 27 为 10,000/10,000
guard miss；[`profile-nonapp.js`](workloads/profile-nonapp.js) 没有 M1
站点。原始 JSONL 分别见
[hit](data/profile-hit.jsonl)、[miss](data/profile-miss.jsonl)、
[nonapp](data/profile-nonapp.jsonl)，Parent hit 对照见
[这里](data/parent-profile-hit.jsonl)。hit 的
`binary_number_in_place` 逻辑事件 Candidate 为 10,000、Parent 为
20,000，恰好少了该乘加区域每次的一次 generic binary 事件；其他循环
运算仍计入，不能将此计数当作总 owner/allocator 成本。

Candidate release 的 [M1 closure 反汇编](data/m1-closure.asm) 中，
`peek_dense_number` 和 `admit_numeric_local` 各有一个调用点，成功
算术包含 `fmul`、`fadd` 与目标 slot 的直接 store；closure 没有
operand push/pop 或 retain/release 调用。反汇编仍有失败分支及冷
bounds-check 路径，故没有声称 wrapper 消除了所有 Rust 边界检查。
源码成功分支没有可观察释放、分配、回调或 driver 退出；profiling
hit 的 `execute.continuation` 与 Parent 均为 9 个基础逻辑事件。

## 语义与工具门禁

- Rust 1.88.0：workspace/all-targets `cargo check`，CI 配置的 clippy
  `-D warnings`，`cargo fmt --check` 均通过；workspace/all-targets
  `cargo test`、profiling、文档、test262-host、oracle 及 unsupported
  diagnostics 相关测试通过。测量提交的主 crate lib 为 1,954 pass；
  追加容量测试后完整 workspace/all-targets 重跑，主 crate lib 为
  **1,955 pass**，其余 suite 亦通过。
- 新测试在同一解释器对照 selection on/off，覆盖发布和拒绝、非法内部
  descriptor/entry/continuation、别名、Number 边界、dense/materialized/
  frozen、hole/getter/Proxy、转换顺序、TDZ/capture/eval/mapped 参数、
  例外位置和 async 恢复。低容量内部 frame 的新增测试确认 M1 peak
  容量 guard 在操作数改变前拒绝，普通第三次 push 保留既有错误行为；
  合法发布帧已预留 peak，因此普通 JS 无法触发该防御分支。
  保留的 generic 词也有结构断言。
- benchmark Python 测试 42 pass；source-layout、Rust-only、oracle
  registry、无 Test262 特判及 Test262 文档指标检查通过。
- focused Test262 **6,844/6,844 pass**；full Test262 的冻结向量为
  total 102,037、pass 80,010、fail 3,552、unsupported 3,502、
  skipped 18,475，80,010/80,060 runnable pass，与既有冻结逐项一致。
  full TSV SHA-256 为
  `4bc1cc02efd7309f65079fbdb7b793eaa449f20b75dab2198522f45133e37a00`；
  runner 的引擎语义 SHA-256 为
  `d2789cc20f44f14f22d2156c1f98597ae17e0040fe8c8937d2f17c750ef4133b`。
  `fail`/`unsupported` 是冻结分类，不是本次新增回归。

这些检查证明 M1 的窄形态通过了本次语义门禁，且 hit 的临时执行工作
确实减少；没有建立对所有程序的性能收益。下一轮可分别减少
guard miss 成本、扩展直接目的地，或改善编译阶段规划成本；每个方向
都需要新的隔离测量与语义比较。

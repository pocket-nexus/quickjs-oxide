# 三个 Profile 驱动方向：独立候选与集成验收

## 目标、范围和选择

本轮以固定工作量的完整进程执行时间为主要目标（包括启动、编译、执行和退出），
场景是 React/Solid/Vue RealWorld 及 V8 v7 八个固定工作量子项。
保持 safe Rust、JavaScript 可观察顺序、完整代际身份、所有权及释放责任；
允许小幅本机代码体积交换，不引入逐函数或逐帧固定元数据。
这不是启动、尾延迟、嵌入式峰值内存或原版 adaptive Score 的验收。

Parent 为 `6435a389cae8ddf83056ef4f5087d8bb5fbb3efe`，集成源码为
`ec291e62`。另从 `996663f771afdabdc69d52c94bd4d2fb392e27b1` 重建 #52，
只用于累计比较。所有版本本轮重建，Rust 1.88.0、普通 release、fat LTO、CGU=1；
计时二进制没有 profiling。独立字符串、数组和 native 候选分别为
`20db729c`、`b96fa119`、`f108e331`，均基于 Parent；完整补丁和构建身份见 data。

选方向使用现有 React/Crypto cycles 样本和逻辑计数，并在本轮重建 Parent 后
重收逻辑计数。采样 self% 不等于预期提速，也未用于给收益设上界。

| 方向 | 现状证据与目标机制 | 限制与方案选择 |
| --- | --- | --- |
| 字符串 scalar 内建 | React 有 61,706 次 string query，primitive 输入仍复制参数、创建 Box 状态并发出转换请求 | 现有 Complete 已支持直返；借用 primitive 输入并提取共享 indexed kernel，不添加同步 trait/dispatcher |
| 物化数组已有元素写入 | Crypto 有 414,711 次 set 描述符读取；已有 slot 被强制送到特殊数组协议再查询和定义 | 现有 DataReplace/Setter 合同足够；放行已有索引槽，缺失索引和 length 保留数组算法 |
| native 调用交接 | React 有 134,944 次 native argv transfer；分类先 retain callee，随后 pop/release 原 owner，同一事务再次扫描已验证参数 | 同一独占 FrameTransaction 允许复用事实；直接转移 callee owner，普通防御入口保留原检查 |

编译器前瞻缓存也曾作为候选比较，但当前数组写入有更直接的真实工作量证据，
本轮未修改编译器。未采用统一 continuation 框架，也没有用字符串搜索算法解释
短 scalar 调用的协议成本。

## 合同与可观察边界

- 字符串先处理 receiver，再按原顺序转换实际使用的参数；有相关 Object 输入时
  在执行转换之前进入原路径。忽略额外参数的行为、Symbol/BigInt 抛错、UTF-16 索引
  和 iterator realm 保持不变。concat 字符串 chunk 继续借用 rope，不能通过通用
  ToString 意外扁平化。新增 all-primitive 扫描的成本由尾部 Object 负载单独检查。
- 数组槽只在相同状态借用中选择；已有 data slot 检查 writable，accessor 保留 setter
  与 receiver。没有扩大到缺失索引或 length，也没有绕过冻结、只读长度、新属性、
  不同 Reflect.set receiver 及原型语义。原 owner 释放仍由原有替换协议完成。
- native 分类与消费之间没有槽写入、换帧或 JS 重入。可失败的缓冲预留都在消费前。
  原 callee owner 连续存活并成为 CallableRef；callee 取出后、receiver 取出前保留
  原 release 附带的 deferred drain。后续 publication 仍检查 domain/function/target。
  新计数只在 profiling 构建存在。

## 机制验收

本轮独立 profiling 二进制上的完整进程逻辑事件：

| 负载与事件 | Parent | 集成版 |
| --- | ---: | ---: |
| React string query dispatch | 61,706 | 52 |
| React 全部 query dispatch | 231,643 | 108,335 |
| React conversion transition | 275,556 | 28,992 |
| React native activation transported to wait | 74,223 | 12,569 |
| Crypto set local descriptor read | 414,711 | 1,947 |
| Crypto property storage set probe | 846,111 | 433,347 |

新事件在集成版记录了 React 61,654 次字符串无状态完成、134,847 次 native callee
owner 转移及验证复用；Crypto 记录了 412,764 次物化数组槽写入选择。
这些计数不是采样时间占比，也不是 libc 分配总数。字符串输出本身仍可能分配；
删除的是 argv 副本、Box 恢复状态及不必要的转换协议。数组新事件计选择的事务，
正式负载全部正常完成，不能将错误路径的该事件解释为已提交写入。

[机器码片段](data/parent-promote_selected.asm)记录旧 `promote_selected` 对
`retain_object_handle` 的调用；新分类及原 owner 转移不再建立该临时引用。
[String start 的汇编](data/combined-start.asm)、共享 kernel、直接 Complete
测试与事件计数共同证明同步输入确实使用新路径。普通对象与 Object 转换继续通过
现有协议；无需新增元数据或通用 continuation。

## 测量方法与分辨率

普通计时均为 fresh process，固定源码和校验输出，CPU 2，ABBA/BAAB 交错。
AMD Ryzen 7 7840HS，Linux 7.2.7，powersave governor；没有本任务编译、测试、
profile 或其他 benchmark 并行运行。机器仍有浏览器等后台进程，未声称整机隔离。
每次运行核对工作负载与二进制 SHA-256，并要求退出码、stdout、stderr 全部正确。

表中比值为每对 candidate/reference 耗时比的中位数，**越小越好**。
普通 A/A 4 对的中位比分别为 Crypto 0.9993、React 0.9970、数组写入 1.0032、
native numeric 1.0069、字符串索引 1.0068；单对波动更大，不能将该中位偏差当作硬阈值。
独立候选各 12 对；组合 focused 8 对、apps 16 对、V8 8 对；累计 apps 8 对、V8 4 对。

native 局部变化较小时额外作 8 对同负载 A/A（中位比 1.0146），随后作 24 对
候选确认。其确认中位比 0.9661，描述性 bootstrap 中位数区间 0.9609–0.9729，
重复出现局部收益。所有区间来自 4,000 次成对比值重采样（seed 29），只辅助判断
此次样本的分辨率，不保证其他机器、输入或部署模式的表现。

## 单项验收与采用范围

| 方向/负载 | Candidate / Parent | 判断 |
| --- | ---: | --- |
| 字符串索引调用 | 0.6028 | 明确改善 |
| primitive concat | 0.6618 | 明确改善 |
| rope concat | 0.7339 | 明确改善，保持原 rope 策略 |
| 字符串单项 / React | 0.9327 | 真实应用收益成立 |
| 字符串 Object 回退 | 1.0035 | 未分辨 |
| 尾部 Object concat | 1.0123 | 小幅回退未分辨，区间跨 1 |
| 物化数组标量写入 | 0.5059 | 明确改善 |
| 物化数组引用写入 | 0.5981 | 明确改善 |
| 数组单项 / Crypto | 0.8410 | 真实负载收益成立 |
| 继承 setter | 0.9882 | 变化很小，不作为主要收益 |
| 普通对象写入 | 0.9924 | 未分辨 |
| native Map 调用（确认轮） | 0.9661 | 局部收益成立 |
| native numeric | 0.9925 | 未分辨 |
| native Object coercion | 0.9981 | 未分辨 |
| native 单项 / React | 0.9944 | 整体收益未分辨 |

采用三个实现。字符串和数组方向满足机制、语义与实际负载时间三层验收。
native 以较小局部改动获得可复测的 Map 调用收益，保留原错误与清理协议；
不宣称它单独改善了 React 整体，也不据此扩大框架或引入更多 native 特化。
其单独源码快照与最终源码之间仅有注释和测试借用结束写法的变化；数组快照与
最终源码仅有测试格式变化，不影响 release 生产算法。

## 组合与累计结果

累计结果包括 Parent 已有的其他优化，不能全部归功于本轮三项改动。
这些都是完整固定工作量进程时间，**不是原版 V8 adaptive Score**。

| 负载 | 集成 / Parent | 集成 / #52 |
| --- | ---: | ---: |
| realworld-react | 0.9292 | 0.8782 |
| realworld-solid | 0.9385 | 0.8944 |
| realworld-vue | 0.9766 | 0.9518 |
| crypto | 0.8143 | 0.7813 |

| V8 固定子项 | 集成 / Parent | 集成 / #52 |
| --- | ---: | ---: |
| crypto | 0.8134 | 0.7779 |
| deltablue | 0.9791 | 0.9447 |
| earley-boyer | 1.0065 | 0.9776 |
| navier-stokes | 0.9937 | 0.8274 |
| raytrace | 0.9818 | 0.9394 |
| regexp | 0.9794 | 0.9424 |
| richards | 0.9455 | 0.9484 |
| splay | 0.9933 | 1.0124 |

八项耗时比的等权几何平均（集成 / Parent）：**0.9596**。

八项耗时比的等权几何平均（集成 / #52）：**0.9182**。

Parent 对照的 Earley–Boyer +0.65% 区间跨 1，Navier–Stokes 和 Splay 的小幅改善也
未分辨。#52 累计对照的 Splay +1.24% 保留为不利结果；本轮相对 Parent 的 Splay
为 0.9933、区间跨 1，未继续归因该历史累计差异。组合版其他收益不抹掉这条记录。
普通对象写入在组合版变快，但退休指令约持平，尚未对代码布局或硬件成本作因果归因。

## 其他成本与边界

stripped binary 为 8,438,592 → 8,442,688 bytes，增加 4,096 bytes（约 0.05%）；
未 strip 的测量产物为 9,409,936 → 9,414,256 bytes。Frame 保持 56 bytes，
FrameCold 272 bytes，VmAction 16 bytes。React 最大 slot/frame 容量仍为 4,144/256，
最终 heap 仍为 20,681 live、4,315 vacant；Crypto 对应容量和最终 heap 也一致。
这只是部分存储与最终状态证据，不能解释为所有内存峰值或分配次数完全不变。

独立 `perf stat` 4 轮的退休用户指令中位数比为：

| 负载 | 集成 / Parent 指令数 |
| --- | ---: |
| crypto | 0.8745 |
| materialized-array-write | 0.5265 |
| native-collections | 0.9551 |
| ordinary-property-write | 1.0008 |
| realworld-react | 0.9342 |
| string-scalar-index | 0.6608 |

指令数用于解释工作量，时间结论来自普通二进制计时。RSS 首次通过 Python wait4
读取时受到 fork 继承高水位的约 29 MiB 下限污染，已拒绝用它评价引擎峰值。
机器没有 `/usr/bin/time`，最终用 zsh builtin time 对 qjs 子进程独立采样，4 轮中位数：

| 负载 | Parent peak RSS KiB | 集成 peak RSS KiB |
| --- | ---: | ---: |
| crypto | 11288 | 11170 |
| materialized-array-write | 9264 | 9272 |
| native-collections | 9288 | 9194 |
| ordinary-property-write | 8812 | 8780 |
| realworld-react | 31082 | 30534 |
| string-scalar-index | 9040 | 8960 |

本轮不宣称 RSS 小幅差异为内存优化收益；未发现这些受测负载明显的 RSS 增长。
没有单独测编译时间、allocator 总请求、尾延迟或取消响应，因此不对它们宣称收益。
concat 预扫描与新生成代码仍是成本，异常/非参与路径的样本均保留。

## 正确性与复现

- Rust 1.88.0 `cargo test --locked --workspace --all-targets` 全部通过：2,021 库测试、
  908 oracle 测试（1 个原有 ignored）及其他 workspace 测试。
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` 通过；
  `cargo fmt --all -- --check`、source layout、Rust-only、Test262 frozen receipt 检查通过。
- 完整 Test262 102,037 变体的结果体逐项匹配冻结基线：80,010 pass / 80,060 eligible。
  首次准备器因继承的 GIT 环境拒绝运行；清理仅子进程 GIT 环境后完整重跑成功，
  未修改 admission、预期输出或基线。
- 新测试覆盖转换与抛错顺序、UTF-16、rope 不扁平化、Object 回退、只读 length、
  freeze、继承 setter、不同 receiver、大索引、原 owner 释放、native 零/额外参数、
  回调重入和 deferred drain；另有 profiling 机制单测。

[完整样本及原始 stdout/stderr](data/measurements.json)、[统计摘要](data/summary.json)、
[Profile 计数及部分内存](data/profiles.json)、[硬件计数](data/hardware.json)、
[独立 RSS](data/rss.json)、[构建回执](data/builds/combined.json)和三份独立候选补丁均已保存。
构建日志、完整原始 Profile JSONL 及原始 runner 目录留在本机 `/tmp/oxide-three-*`。
外部应用和 V8 不 vendor 入仓库；原路径、pin、输入 SHA-256、固定工作量及输出契约
保存在 manifest/样本 metadata，缺文件必须按原配方重建并核对 hash，不能换输入续用结论。

重放本仓库定向输入（替换两侧二进制路径；构建命令见 measurement.md）：

```sh
python3 scripts/benchmark/fixed.py \
  --manifest docs/performance/receipts/three-directions-2026-09-29/data/focused-manifest.json \
  --workload-dir docs/performance/receipts/three-directions-2026-09-29/workloads \
  --engine before=/absolute/parent/qjs --engine after=/absolute/candidate/qjs \
  --repeat 12 --order abba-baab --cpu 2 --output /tmp/new-three-direction-replay
```

`data/run-matrix.py`、`data/hardware.py`、`data/rss.py` 记录本次本机编排，包含本机路径；
它们不是新的通用 benchmark 入口。所有轮次保留成功和失败准备记录，未删除不利样本。

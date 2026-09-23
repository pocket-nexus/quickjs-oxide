# 阶段 C 负结果与撤回落（2026-09-23）

> 状态：已执行。阶段 C（C1–C4）按 benchmark 与 profile 的负结果整体撤销，
> 代码树回退到 B/C 起点 `bcfb4fe5`（post-A 基线，与 B/C 实施前逐字节一致）；
> B 首批因与 C 在实现上耦合而一并回退。本文记录撤销依据与“为什么 C 没有用”
> 的归因。其它 B/C 报告保留为实现与测量证据，其中源码链接不再对应当前代码树。

## 1. 决定与范围

- **撤销对象**：C1 直接存储事务、C2 标量单槽 TOS、C3 String/heap BigInt
  数值结果缓存、C4 认证 StoreDrop，以及同批落地的 B1a–B1d（QuickOp codec、
  发布投影、共享热 handler、Quick 执行入口）。
- **回退点**：`bcfb4fe5`，即 [C 计划](s3-c-plan.md) 与
  [B 首批计划](s3-b-initial-plan.md) 记载的 B/C 起点。`docs/` 保留全部
  B/C 报告，仅代码树回退。
- **无法只保留 B**：C 与 B 在提交与文件层面耦合——`3392e4e9` 同一提交引入
  C1 与 B1a；`d1b52c9a` 把 C4 StoreDrop 与 B1c/B1d 接到同一 TOS facade 与
  数值 handler；`33a02659`/`e0c506f8` 混改两边。B 单独保留需要重写 `run`
  集成并重新验证，且 B 自身也未通过默认启用门禁（见 §4），故整体回退。

## 2. Benchmark 证据（同协议全量重测第 1 轮）

数据来源：[S3 全量同协议重测结果](s3-full-rerun-results.md)（源码
`e0c506f8`，fat LTO + CGU=1、无 PGO、无 profiling、`taskset -c 2`，
A/A 同二进制聚合偏差 <0.2%）。各档相对 pre-A 的几何均值，时间类 <1 表示
比 pre-A 快，V8 Score >1 表示比 pre-A 快：

| 套件 | m0（默认） | c（C 组合） | bc（C+B） | 观察 |
| --- | ---: | ---: | ---: | --- |
| scaling 86 | 0.960 | 0.989 | 0.990 | c/bc 比 m0 慢约 3% |
| scaling-regexp | 0.948 | 0.957 | 0.937 | 单项混杂 |
| property 3 项 | 0.736 | 0.742 | 0.749 | 略差于 m0 |
| V8 isolated（Score） | 1.045 | 1.017 | 1.013 | 低于 m0 |
| V8 combined（Score） | 1.035 | 1.000 | 1.000 | 低于 m0 |
| fixed 58 | 0.912 | 0.895 | 0.890 | c/bc 略快，但幅度低于噪声 |
| original 9 | 0.977 | 0.983 | 0.981 | 略差于 m0 |
| microbench 8 项 | 0.750 | 0.688 | 0.688 | 50ms 量化，仅作参考 |

- **目标项未改善**：C3 验收点名的 bigint256 在重测中仍相对 pre-A
  墙钟 1.21×、用户态指令 1.32×（m0 档，2026-09-23 指令口径更正），与阶段
  A 遗留回退一致。
- **单项噪声限制**：单项噪声 −12%~+6%（fixed 单项 max 1.204），c/bc 与 m0
  之间的个位数时间差不能单独定论；撤销的主要依据是 §3 的机器指令与热点
  证据，时间与 Score 只是方向一致。
- **C1 独立矩阵同样不支持保留**：[实施记录](s3-bc-implementation.md) §4 的
  C1/previous 几何均值 0.996413，但 array_write（1.059）、array_update
  （1.080）、math_min（1.031）、int_to_string（1.032）四项超过单项复核线，
  A/A 线性插值 P95 在 array_read 达 12.2%。m0 相对 pre-A 的改善不能归因
  给 C1。C1 定向两轮六目标 geomean 0.956/0.960，但 arg-keep-heap 两轮均
  回退约 2.2%，未进入正式接受（[第二批记录](s3-c-b-next.md) §5.4）。

## 3. Profile 证据：为什么 C 没有用

数据来源：[实施记录](s3-bc-implementation.md) §7（冻结版本 `d1b52c9a`，
普通 release、fat LTO、CGU=1、固定 CPU 2；`perf stat` 用户态计数 +
`perf record` 调用栈；M0 为当时的 canonical，已含 C1 与共享热 body）。

### 3.1 组合整体：机器指令明确增加

八个输入的五次进程 wall 中位数与三次 `instructions:u` 中位数比值（>1 为增加）：

| 输入 | 时间 C/M0 | 时间 BC/M0 | 指令 C/M0 |
| --- | ---: | ---: | ---: |
| local-consume-scalar | 1.521 | 1.678 | 1.494 |
| arg-consume-scalar | 1.557 | 1.514 | 1.539 |
| mixed-boundaries | 1.131 | 1.218 | 1.143 |
| bigint256 | 1.211 | 1.354 | 1.226 |
| string_build1 | 1.261 | 1.481 | 1.323 |
| prop_read | 1.019 | 1.291 | 1.218 |
| c3-owned-bigint | 1.255 | 1.448 | 1.240 |
| c4-store-drop | 1.071 | 1.198 | 1.320 |

local/arg 的时间回退伴随约 +49%/+54% 的机器指令增加，不能只解释为时钟
抖动；prop_read 时间比 1.019 虽在噪声内，指令数仍 +21.8%。

### 3.2 C2 单槽 TOS：驻留窗口太短，缓存成本摊不薄

- local/arg 分别提交缓存 110,009/130,011 次、spill 50,008/60,010 次，
  其中约 **80%/83% 的 spill 由下一次 push 驱逐造成**：栈顶值大多只活
  一两条指令，单槽缓存的命中窗口本来就短，准入/恢复的固定成本无法摊薄。
- 热点集中在新增的准入与缓存操作：`tos::resident` 内联路径占整个程序
  7.30%，`is_ok_and → resident` 1.88%，owning-output 查询链约 2.93%，
  `ScalarTos::install` 9.43%，`tos_store_binding` 5.01%。
- canonical 路径每条指令先选择 cache/canonical facade 并查询 owning cache，
  共享 handler 再读取/证明同一 binding，store facade 继续认证目标——同一
  事实被重复证明，这正是机器指令上升的来源。

### 3.3 C3：命中存在，但没有时间收益

定向 BigInt 输入确有 20,000 次 `tos.owned_numeric_output`，缓存并非未启用；
但 C/BC 在完整输入上分别慢 25.5%/44.8%，且历史 bigint256/string_build1
的该事件为 0，缓存覆盖不到真正的目标负载。

### 3.4 C4：dispatch 减少 ≠ 工作减少

定向 StoreDrop 输入 local/argument 各成功 10,000 次，dispatch 从
180,028 降到 160,028，但逻辑指令仍为 220,029，组合机器指令仍增加。
prop_read 的 StoreDropLocal 成功 40,002 次、dispatch 260,304 → 220,302，
同样没有转化为加速。单点 match 已被阶段 A 的 profile 排除为 V8 残余主因，
派发本身不是主成本。

### 3.5 B/Quick：认证后解码与二次分类抵消了 8B word 的节省

- 认证后的 `decode().expect(...)` 内联链占 BC 六个输入程序采样的
  16.55%–30.26%；紧凑 8B 存储没有消除运行期解码与结果传递成本。
- 回到 canonical 的执行比例 25.0%–72.5%；prop_read 的 guard decline
  达 100,006 次（Generic 仅 242 次），C4 输入 decline 40,001 次。
- 时间上 BC/C 在八个输入为 0.973–1.267（多数 >1），BC/M0 全面慢于 M0。
- 静态成本：BC 的 `.text` +1.04%，`run` 主循环实例总量 +91%。

### 3.6 修复没有改变结论

`33a02659`/`e0c506f8` 已针对上述热点做收窄（opcode 准入、认证后 word 借用、
标量 Drop、错误载体等）。修复后的全量重测（§2）显示 c/bc 仍不优于 m0：
结论不是“修复不彻底”，而是这条路线在 A 阶段已消除大部分 owner 往返之后，
可收割的临时栈流量不足以覆盖缓存自身的固定成本。

## 4. B 一并回退的理由

- B1c/B1d 复用 C 的 TOS facade 与数值 handler，无法在撤销 C 的同时保留。
- B 自身也未通过默认启用门禁：m1（projection-only）compile 相对 m0 +1.3%
  （单项最大 +4.3%），第二批记录中嵌套 cold RSS 两轮超 1 MiB 门槛
  （+1308/+1124 KiB），scaling 相对 m0 无增量，bc 未超过 m0。
- B 的后续价值（quickening 特化）需要重新设计并独立立项，不能把本次实验档
  当作已完成基础。

## 5. 结论

阶段 C“减少临时操作数搬运”的前提没有兑现：在 A 阶段已消除大部分
owner 往返之后，剩余栈顶驻留窗口过短（80%+ 的 spill 由下一次 push 驱逐），
新增的准入证明、缓存状态查询与 canonical 退回成本高于节省，机器指令数
上升 14%–54%，时间与 V8 Score 均未超过默认 m0。因此按负结果关闭并撤销，
不再保留默认关闭的实验代码。

测量局限：本机单轮、governor=powersave、单项噪声 ±6–12%，所以结论表述为
“无可辨别净收益且指令数/热点证据指向新增成本”，而不是“已证明显著负收益”。

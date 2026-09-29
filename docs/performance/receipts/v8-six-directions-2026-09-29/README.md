# 六个 V8-v7 优先子项：Profile、并行候选与验收

## 目标与范围

主目标是在当前源码基线 `7723e825` 上降低固定工作量的完整进程时间，优先
Earley-Boyer、RayTrace、Richards、Crypto、DeltaBlue、Splay。保留上游 workload
主体、断言、JavaScript 可观察顺序、完整代际身份、所有权、safe Rust，以及冻结
Test262 admission/result body。允许小幅代码增长；通过其他两项 V8、八项合并程序
和 RealWorld 应用检查交换。新发现的重复 shape 反向边另以存储增长上界验收。

用户给出的 QuickJS Score / m0 Score（28.0×、27.0×、23.1×、18.0×、17.0×、
14.8×）用于优先级，未重新确认其历史构建身份。本报告不把固定工作量进程耗时
冒充 adaptive V8 Score，也不把本轮收益直接从这些历史倍数中相减。

基线和最终候选 `ee999771` 都用 Rust 1.88.0、普通 release、fat LTO、CGU=1
重新构建，profiling 另建。实现并行推进，正式时间/硬件/RSS 采样串行；采样期间
本任务构建、测试和其他 agent 负载停止。CPU 2，ABBA/BAAB，fresh process，逐次
校验源码/二进制 hash、退出码、完整 stdout 和空 stderr。桌面后台程序仍存在，
没有声称整机隔离。全部有效原始样本和不利结果保留。初期 /tmp 容量不足导致的构建失败未进入
计时，构建目录已迁往磁盘后重跑。

## 现状证据与机制选择

八项均重新收集逻辑 Profile；六项另收 `cycles:u` 探索样本。后者用代码与当前
基线仅差文档注释的上一轮普通二进制，在收逻辑 Profile 时进行，样本数有限，
只用于定位，不能当正式时间、收益上界或各候选收益之和。验收使用本轮普通构建。

| 子项 | 当前证据 | 本轮改变及限制 |
| --- | --- | --- |
| Earley-Boyer | 512,379 call、267,379 construct、534,555 missing committed；`sc_Pair` 频繁构造 | 追加属性保留旧槽 owners；scalar write 实际命中为零，不能声称它改善本项 |
| RayTrace | 66,600 construct、235,680 set，其中 194,985 missing committed | shape 追加和已有数值字段写入分别测；大量 miss 会承担新快路径检查成本 |
| Richards | 50,751 set，只有 9 个 set state；主要是同步写入交接与协议成本 | own scalar 字段在 execute 内提交，不归因于 continuation 分配 |
| Crypto | 12,534,392 in-place number binary；`am3` 是高频 guest 执行函数 | Int ToInt32 实验被拒绝；另试 operand push 冷错误提取，现有数组优化不重复计收益 |
| DeltaBlue | 115,452 call、22,662 set；大量字段读和引用流动 | existing scalar 写覆盖部分工作，未为所有调用增加新协议 |
| Splay | 626,943 call；Profile 指向 layout edges、retain 和 shape cache 清理 | 保留旧槽 owners，限制重复反向边；现有帧池已经大量复用，未再做 pooling |

逻辑事件和 guest PC visits 不是 CPU 时间占比。函数/PC 原始记录和 omission 字段
随 Profile 一同保存，探索采样的低样本热点也完整保留。

五项候选分别基于同一基线实现并独立构建，其中整数实验已拒绝：

1. **整数事实直接消费（拒绝）**（`03046503`）：`Number::Int(i32)` 无需转换为 f64 后
   截断/取模。现有 kernel 接口允许直接完成；Float 分支保持原算法。实际普通
   二进制机器码确认旧 Int 路径调用 trunc/fmod，新路径跳过这些调用。相关函数
   自身体积增加 327 bytes，不能把删除运算等同于所有代码都变小。
2. **已有普通标量字段本地写入**（`3cb5fd54`）：现有 execute 可本地完成，增加
   局部 helper。只准入 ordinary、own writable data、旧/新值均无引用边、receiver
   释放不会触发 cleanup 的事务。未准入时保持两个 operands；成功递增 property_generation 并
   消费原 owners。引用槽、缺失、只读、accessor、Proxy、数组等沿原合同执行。
3. **canonical shape 追加保留旧 owners**（生产补丁 `775a4f9e`，独立最终
   `c1917034`）：原完整布局替换接口接收完整 Slots，并重复 retain/release 全布局。
   新内部 append 事务表达旧槽不变，仅准备新增 slot 和 successor shape 的边，
   发布后释放旧 shape。它保留完整前缀、prototype、storage 与代际验证；准备失败
   回滚新 owners，已发布错误不撤销新槽 Atom 所有权。前缀检查仍 O(width)，没有
   宣称属性创建整体 O(1)。常见 new shape + one data edge 还复用已有的小边列表
   preflight，避免旧全布局边较多时的 HashMap 聚合。底层 `Slots::try_reserve`
   仍会重建 spilled Vec 并复制旧槽；本轮没有消除全部复制或分配。
4. **反向 shape 边唯一性**（`bf5d9b88`）：命中同一前向边时不重复 push 反向记录。
   利用已有 HashMap insert 的 previous 值，重复路径不新增线性去重扫描。重绑定
   时先摘除旧代际反向边，避免旧目标延迟 cleanup 删除新 forward edge。无新通用
   cache 框架；反向记录条目数由不同关系决定，不再随同边重复使用增长；容量保留历史高水位。

5. **operand push 冷错误提取**（`e635b49d`）：仅把两个错误构造移到 cold、
   inline(never) helper，保留检查顺序、错误文本和 pending owner 消费时机。原有
   inline 标记因此生效，实际普通机器码中独立 `operand_push_index` 消失，
   `commit_push`/`FrameSlots::push` 成功路径不再调用 helper，重复 bounds 合并为
   一次。不是删除容量/空槽校验，也未增加 VM 模式。独立 .text 增 400 bytes，
   完整 ELF 增 184 bytes；47 项已有栈测试通过。

独立静态审查未发现可确认缺陷；这不能替代动态测试或 OOM 注入。定向测试覆盖
饱和旧引用不被再次 retain、Symbol 回滚、失效对象/shape、完整前缀、相同 getter/
setter 双边、自引用、删除重增和原型缓存。重复边测试执行 10,000 次后长度仍为 1、
容量等于第一次，且不多留 shape 强引用。

## 调研与未采用路线

[QuickJS 内部实现说明](https://bellard.org/quickjs/quickjs.pdf)提供 shared shapes、
atoms 和引用计数的背景；[V8 属性实现说明](https://v8.dev/blog/fast-properties)
说明命名属性、元素和 shape transition 的分工。这些资料只提供候选机制，优先级
与本轮收益均由本仓库实际执行证据确定，不直接套用 JIT 的结论。

- 空闭包 owner 删除已作为小实验隔离，但本轮未采用；现有调用 cache、参数转移
  和帧池已经生效，shape 追加证据更直接。
- 命名读的 cache miss 值得继续区分：Richards hit/miss 104,019/97,905，DeltaBlue
  187,874/146,074，RayTrace 470,894/502,269。应区分 shape 多样性、短命 shape 代际
  和真正原型修改；普通新对象变布局不会使全局原型 epoch 失效。直接增大 PIC
  容量尚无足够证据，也会增加不参与路径的存储成本。
- Crypto 的 operand push/commit 已做最小实验；剩余索引协议可继续区分。扩大 VM 融合、
  重写 Float ToInt32 或改变数值表示需要新的区分实验，本轮没有为此增加指令模式。
- 不把“没有明显收益”自动解释成覆盖不足；各独立结果与采用理由见下文。

后续已确认的局部机会：`Slots::try_reserve` 对 spilled Vec 仍分配新 Vec 并复制
旧槽，即使可能保有可用容量。可独立测试原 Vec reserve，先保留分配失败合同与
inline→spilled 转换，再测 Splay/Earley 和内存高水位。本轮未实现，未计入收益。


## 独立候选：结果与决策

表内为配对时间比的中位数减一；负数表示耗时下降。每个候选独立基于同一源码基线，
每项 8 对、ABBA/BAAB。它们不是可相加的收益；完整分布与描述性 bootstrap 区间
在 [summary.json](data/summary.json)。A/A 每项 4 对，点估计约 -1.8%～+0.9%，
说明小幅变化需要保留不确定性。

| 子项 | Int ToInt32（拒绝） | 标量字段写 | shape 追加 | 反向边去重 | 栈冷错误 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Earley-Boyer | +1.00% | +1.06% | -2.60% | +0.55% | -0.26% |
| RayTrace | +2.06% | -0.06% | +1.15% | +0.84% | +0.24% |
| Richards | +4.08% | -3.84% | +2.77% | +1.94% | +1.44% |
| Crypto | +2.36% | +0.50% | +2.89% | +0.51% | -1.05% |
| DeltaBlue | +6.82% | -0.67% | +1.78% | +1.36% | -1.21% |
| Splay | +0.63% | -0.95% | -7.79% | -0.05% | -1.53% |
| Navier-Stokes | +4.05% | -2.28% | -2.44% | +1.37% | -0.64% |
| RegExp | -1.41% | -1.40% | -1.40% | -2.48% | +1.23% |

- **拒绝 Int ToInt32**：机制确已发生，但 Crypto、DeltaBlue、Richards 等明显变慢。
  不是“覆盖为零”；`am3` 的掩码写仍生产 Int。机器码增长/布局是可能因素，尚未
  证明因果。补丁和汇编保留，源码已撤回，不扩大到其他数值 kernel。
- **采用标量字段写、shape 追加和栈冷错误**：有具体消费者与独立收益，
  以最终组合和外部场景结果决定交换；保留各自的退化样本。
- **有条件采用反向边去重**：采用理由是重复使用同一边不再积累记录；
  六项 V8 没有分辨出普遍时间收益，不将其包装成速度优化。期望重复查找 O(1)，
  重绑定删除旧反向项仍 O(d_old)，新边 Vec 扩容最坏 O(d_new)。Vec 容量保留历史
  高水位，不宣称整个 shape 图有固定内存上限。

## 最终组合：目标六项

固定工作量包括进程启动、解析、执行、Setup/TearDown；原版 Score 保留上游
自适应执行和至少 32 次 timed run，排除 Setup/TearDown。**两个口径分开验收**。
Score 只有每引擎 2 次 ABBA，是低分辨率交叉检查，不能据此确认微小收益。

| 子项 | 基线 / 最终进程中位数 ms | 配对耗时变化 | 95% bootstrap 时间比区间 | 原版 Score 变化 |
| --- | ---: | ---: | --- | ---: |
| Earley-Boyer | 2343.3 / 2249.7 | -4.02% | 0.9537–0.9626 | +3.95% |
| RayTrace | 637.1 / 631.3 | -1.38% | 0.9801–1.0017 | +1.28% |
| Richards | 136.7 / 129.8 | -4.76% | 0.9395–0.9713 | +4.92% |
| Crypto | 729.0 / 728.7 | -0.01% | 0.9938–1.0077 | +0.30% |
| DeltaBlue | 96.4 / 94.6 | -1.75% | 0.9755–0.9965 | +1.05% |
| Splay | 2286.2 / 2091.6 | -8.63% | 0.9100–0.9213 | +10.57% |

原版 Score 原始值（按各引擎两次执行顺序）：

| 子项 | 基线 Score | 最终 Score | 两对 Score 比 |
| --- | --- | --- | --- |
| Earley-Boyer | 138.0, 141.0 | 145.0, 145.0 | 1.0507, 1.0284 |
| RayTrace | 118.0, 117.0 | 119.0, 119.0 | 1.0085, 1.0171 |
| Richards | 53.8, 54.0 | 56.6, 56.5 | 1.0520, 1.0463 |
| Crypto | 170.0, 168.0 | 170.0, 169.0 | 1.0000, 1.0060 |
| DeltaBlue | 76.2, 76.0 | 77.1, 76.7 | 1.0118, 1.0092 |
| Splay | 326.0, 327.0 | 361.0, 361.0 | 1.1074, 1.1040 |

时间比小于 1 较好；Score 比大于 1 较好。8 对样本的区间仅描述本机此次运行，
不能代替跨机器重复验证。没有重新测用户历史 m0/QuickJS 构建，因此不声称已将
28× 等差距缩减为某个新倍数。

采用判断：主要时间收益集中在 Earley-Boyer、Richards、Splay；DeltaBlue 为小幅
改善，RayTrace 的区间跨 1，Crypto 基本持平。组合与三项独立应用均改善，且代码
增长为 4 KiB，因此保留四项组合。未声称六项均获得确定提速。Navier-Stokes 点估计
回退 1.21%、区间跨 1，作为尚未排除的小幅回退保留，需在跨机器复测时继续检查。


## 集成与独立场景

| 场景 | 配对耗时变化 | 95% bootstrap 时间比区间 |
| --- | ---: | --- |
| Navier-Stokes | +1.21% | 0.9960–1.0206 |
| RegExp | -0.61% | 0.9871–1.0130 |
| 八项同进程 | -3.40% | 0.9599–0.9721 |
| realworld-react | -3.76% | 0.9549–0.9700 |
| realworld-solid | -2.16% | 0.9657–0.9846 |
| realworld-vue | -2.63% | 0.9656–0.9785 |
| ordinary-property-write | -35.58% | 0.6353–0.6588 |
| repeated-shape-edge | -4.07% | 0.9391–0.9675 |

RealWorld bundle 使用已固定的 React/Solid/Vue 输入及完成标记；不是浏览器 UI
交互测量。[bundle 来源回执](data/realworld-bundle-receipt.json)保存来源提交、工具链和
文件 hash；外部 bundle 本体不纳入此回执目录，重跑必须先取得并校验这些固定文件。
focused 只验证指定机制，不代表全应用收益。

## 机制、硬件与空间成本

逻辑 Profile 使用独立 profiling 构建，各 benchmark 固定一轮；不使用它的时间
比较普通 release。以下事件确实发生（事件次数，不是节省的 retain 次数或 CPU 占比）：

| 子项 | 本地 scalar 写 | 追加事务保留旧 owners | 避免重复反向边 |
| --- | ---: | ---: | ---: |
| Earley-Boyer | 0 | 543,766 | 4,490 |
| RayTrace | 38,280 | 76,721 | 110 |
| Richards | 29,054 | 420 | 74 |
| Crypto | 10,432 | 708 | 170 |
| DeltaBlue | 16,635 | 1,966 | 120 |
| Splay | 128 | 1,042,899 | 387,892 |

栈 helper 的机制证据来自普通 release 汇编，见 [assembly](data/assembly)；
没有把所有 in-place binary 事件算作 push 覆盖。

硬件/RSS 单独运行，每项每引擎 4 次；instructions/cycles 来自 perf stat，
RSS 是独立进程的 zsh time peak RSS。RSS 受 allocator/页粒度和地址布局影响，
不以最终 heap snapshot 冒充峰值。

| 子项 | instructions 中位数变化 | cycles 中位数变化 | peak RSS 中位数：基线 / 最终 KiB |
| --- | ---: | ---: | ---: |
| Earley-Boyer | -4.33% | -3.15% | 44822 / 44922 |
| RayTrace | -2.49% | -1.74% | 10754 / 10840 |
| Richards | -6.54% | -4.16% | 10096 / 10126 |
| Crypto | -5.77% | -0.51% | 11346 / 11476 |
| DeltaBlue | -3.50% | -0.88% | 10922 / 11080 |
| Splay | -9.14% | -8.72% | 344144 / 344418 |
| 八项同进程 | -4.50% | -3.64% | 361784 / 362484 |
| ordinary-property-write | -40.11% | -36.60% | 8826 / 8722 |
| repeated-shape-edge | -2.89% | -3.06% | 10364 / 8896 |

Crypto 指令数约减少 5.8%，完整进程时间却基本持平；不把指令数当速度替代。
六项目标 RSS 未显示普遍下降。反向边去重的资源结论限定于重复关系的存储增长，
不承诺这些应用的整体峰值降低。

最终 ELF 9,418,768 bytes（基线 9,414,256，差 +4,512）；strip 后 8,446,784 bytes（差 +4,096）。
Frame 56 / FrameCold 272 / VmAction 16 bytes 不变。未测编译时间净变化，
未对尾延迟、取消响应或嵌入式内存预算作结论。最终 heap_states 另逐项核对；
一致只说明结束时逻辑状态，不证明中途内存峰值相同。

## 正确性验收

最终源码 `ee999771` 重新执行以下门禁，详细命令/结果在
[validation.json](data/validation.json) 和 [日志](data/validation)。

- cargo fmt、源码布局和 Rust-only 检查通过。
- workspace all-targets 测试通过（核心 2,031 项、oracle 908 项；1 项原有忽略）；
  profiling lib 2,192 项通过。
- Clippy workspace all-targets all-features，`-D warnings` 通过。
- Test262 frozen baseline 检查及完整运行通过：102,037 variants 的结果体与冻结
  基线一致；80,010 pass、3,552 fail（包含 3,502 unsupported）、18,475 skipped。
  eligible/runnable 80,060，其中 50 项为基线已分类失败。不是“102,037 全通过”。
- 定向测试覆盖所有权回滚、饱和引用、Symbol/accessor/self-reference、代际重绑定、
  容量失败和 pending owners；源码中保留相应回归测试。
- [静态交互审查](data/final-review.md)未确认额外缺陷，不替代 OOM 注入或动态证明。

## 复现与证据索引

1. 基线 `7723e825`，最终代码 `ee999771`。使用干净独立 checkout，
   `RUSTUP_TOOLCHAIN=1.88.0 python3 scripts/benchmark/build.py` 分别生成普通和
   profiling 构建。完整选项、特性、rustc 命令和二进制 hash 见 [builds](data/builds)。
2. 外部 V8 source 固定 `2034d98fc8c5f8044e186267593f5d5ea5232caf`；
   [freeze-plan.json](data/freeze-plan.json)保存生成配方与上游/driver hashes。
   [v8-nine-manifest.json](data/v8-nine-manifest.json)固定输入、期望 stdout；移机时
   只改路径并重新校验 hash，不改循环、断言或 workload 主体。
3. [final-measure-commands.json](data/final-measure-commands.json)给出最终完整命令。
   固定测量使用 `fixed.py --repeat 8 --order abba-baab --cpu 2`；原版 Score 使用
   `taskset -c 2 ... run.py --suite v8-v7 --repeat 2 --order abba`。
4. [measurements.json](data/measurements.json)包含所有独立、A/A、最终及原版 Score
   样本，并嵌入 stdout/stderr；[summary.json](data/summary.json)保存配对比值/区间；
   [score-summary.json](data/score-summary.json)保留原版 Score。
5. [profiles](data/profiles)保留基线、旧四候选试验和最终源码的原始诊断；
   旧四候选 `a4ec9f89` 包含后来拒绝的数值实验，其验证单独归档，不能冒充最终源码。
   独立补丁（含拒绝项）和所有方向说明一同保存。
6. 当前证据范围为单机 x86_64 Linux 普通解释器 release；多机器、其他架构和
   编译成本需独立验证，不外推小幅变化。

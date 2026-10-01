# V8 v7：追赶 Boa 的实验记录

## 目标与边界

目标是让**原版完整 V8 v7 运行输出的八个子项和 Combined Score 均稳定超过 Boa 0.22.0**。固定工作量时间、计数和汇编用于筛选与解释，不能替代这个验收条件。当前还没有达到目标。

保持 JavaScript 效果顺序、所有权、引用计数饱和规则、异常与栈限制、GC 服务边界以及 safe Rust。禁止针对 benchmark 名称或输入识别选择实现。候选分别提交、分别计时，通过后再验证累计结果。低覆盖、未兑现收益或维护成本过高的候选可以缩小或拒绝。

简化也是候选机制：合并选槽与消费、删除中间状态、集中检查责任，使消费者直接使用当前已知事实。它需要兑现为执行工作或目标指标的改善；仅重命名、减少源码行数或增加通用抽象不构成性能验收。

本轮对照是清理后的 `3d42040738b2a40569889a3fac4744c58bf2ae0c`，不是此前测量中的旧 main。历史分数与本轮独立收益不能混合归因。

## 当前证据

外部 suite 固定在 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。Rust 1.88.0、普通 release、fat LTO、CGU=1 与独立 target 的回执标识时间用二进制；profiling 构建只提供机制计数。时间采样固定 CPU 2，停止并行构建、测试和其他采样。CPU governor 是 powersave，未锁频，保留 A/A 观察范围。

八项各完成一次原始 Benchmark 的 Setup/run/TearDown，全部通过完成标记校验。原生 profile 使用相同原版主体；不同引擎采用不同重复次数以获得样本，**这些不是跨引擎时间或 Score 比较**。

固定单次 profile 包含 Setup 与 TearDown，不能将其频率直接当作原版 Score 的计时区间权重。具体检查已发现：Splay 的 `traverse_`/`exportKeys` 分支只在 TearDown 执行；`splay_` 同时用于 Setup 的 8000 次插入和 timed run 的 80 次替换。NavierStokes 的求解器尺寸、步长与迭代次数捕获读取则发生在 timed run 的循环内。后续方向按实际 run 覆盖排序，并补充原版自适应运行的诊断 profile。

| 项目 | 已观察的成本 | 优先研究的机制 |
| --- | --- | --- |
| Richards | 属性选择、普通调用安装、owner 复制 | 属性消费、调用协议与重复认证 |
| DeltaBlue | 属性选择与 owner 复制、ready 调度 | 已持有事实的消费与交接 |
| Crypto | 12,534,392 次原地数字二元操作；数字 kernel | 保留整数表示、运算与访问路径 |
| RayTrace | 66,596 次快速 Array/Arguments 载体；66,600 次 Construct | 删除中间参数表示、普通构造前缀本地完成 |
| EarleyBoyer | 501,657 次 predicate 交接；267,379 次 Construct | 普通 instanceof、构造与比较的本地完成 |
| RegExp | 查询、恢复与 split 路径中的状态搬运 | 内建状态生命周期；作为独立回归控制 |
| Splay | 775,702 次 Environment 动作，其中 767,600 次是 Object/ArrayFrom 字面量分配；另有 161,521 次 Binding 动作 | 分配结果搬运、重复帧发布与调度；按 timedRun 覆盖判断 |
| NavierStokes | 1,355,222 次 binding 交接；数字循环 | 绑定访问和数字区域覆盖 |

### 竞争引擎的可借鉴之处

- QuickJS 2026-06-04 的 `build_arg_list` 直接建立最终 argv 并复制必要 owner；没有先建立通用 raw 参数快照的要求。
- QuickJS 的 `JS_OrdinaryIsInstanceOf` 在普通原型链上同步行走，遇到 exotic 才进入其他协议。Boa 0.22.0 的普通 `HasInstance` 同样直接调用普通算法。这支持研究本地完成；不能据此宣称我们的实现达到它们的速度。
- QuickJS 原生热点集中在解释循环；Boa 的缓存、操作分派、存储和引用管理更分散。最初 V8 JIT 的短 profile 主要覆盖启动和编译。后续五个引擎均完成原版完整自适应运行的诊断 profile：V8 JIT 约 10K 样本已覆盖 `montReduce`、`Planner.removePropagateFrom`、`Scheduler.schedule` 等 JS 执行热点；Oxide 约 80K 样本仍显示 owner 复制和状态搬运。两个报告均无 lost sample，但频率权重包含准备、warmup 和计时区间，不能当作同工作量跨引擎时间比较。jitless 独立保留。

证据在仓库外 `/home/eric/.cache/oxide-v8v7-boa-campaign/`：`baseline-logical`、`baseline-native`、`original-adaptive-native` 保存机器、源码、二进制与负载身份、原始输出和 profile。`original-adaptive-native` 的所有打印分数仅作 profile 完成校验，没有作为时间验收收据。原始大文件不入库。

## 独立候选

| 提交 | 候选 | 机制证据 | 当前判断 |
| --- | --- | --- | --- |
| `ba57e9ec` | 借用驻留属性缓存状态 | read 生成代码 2641→1962 字节；栈 600→328 字节；每个 cache 增加 8 字节 | 144 个固定样本有效，Combined 时间比约 1.0023，在 A/A 范围内；尚未采用 |
| `8c53413f` | 快速 apply 直接投影最终 argv | RayTrace 中间 raw 复制 160,717→0；54,681 次排序缓冲扩容消失；必要的 6,679 次堆 owner 复制保留 | 144 个固定样本有效：RayTrace 耗时 −0.74%、combined −0.56%，均在 A/A 范围内；不宣称提速，尚未采用 |
| `daccb30b` | 普通 instanceof 在解释循环完成 | EarleyBoyer 397,585 次本地完成；predicate 501,657→104,072；native activation 501,788→104,203 | 144 个固定样本有效：EarleyBoyer 耗时 −21.18%、combined −2.93%，但 Crypto +11.11%，超出 A/A 2.95%；拒绝当前版本，缩小实现 |
| `ce30f34a` | Int(i32) 的 ToInt32 保留表示 | 数字二元 helper 的整数转浮点指令 40→29；代码 3912→4239 字节 | 144 个固定样本有效：Crypto 耗时 −5.46%，RegExp +1.57%（A/A 1.01%），combined +0.22% 未分辨；保留实验，需复核原版控制 |
| `dc2b0c15` | 普通 Base 构造的 lazy child 安装 | 独立实现及 2248 个 profiling 库测试通过 | 审查发现 miss 可能增加临时 retain，暂停验收并修正准入 |
| `bc497542` | 缩小 instanceof，使用现有同步 Complete 入口 | 同样删除 397,585 次 native activation；解释循环归一化 7,724 条指令恢复逐项相同；2273 个库测试通过 | 144 个固定样本有效：EarleyBoyer 耗时 −18.55%、combined −2.98%，均超出本轮 A/A；Crypto −1.23%，初版约 11% 回归消失，未分辨其他控制回归；进入原版复核 |
| `ddef3000` | 已持有对象的严格相等本地完成 | EarleyBoyer strict driver 交接 129,305→30,524；2274 个库测试通过 | 时间待验收；大部分新 completion 计数来自原已本地路径，不能全算新增收益 |
| `fe7b9462` | ready 堆值取反与分支本地完成 | Splay materialize 请求 32,797→0，pure 交接 12,139→0；2250 个 profiling 库测试通过 | 时间待验收；保留 producer owner 协议 |
| `b560f6b4` | binary64 整数位替代 ToInt32 浮点取模 | 128,672 组位模式与独立算术 oracle 一致；数字 helper 3912→3156 字节；Crypto 原有操作计数保持 | 144 个固定样本有效：NavierStokes 耗时 −7.35%（A/A 1.59%）；Crypto +0.16%、combined −0.65% 均未分辨；不声称 Crypto 提速，进入原版复核 |
| `1350c5f0` | 修正 Base 构造准入与失败合同 | EarleyBoyer callback 267,268→4；RayTrace 133,195→66,598；2252 个库测试通过 | 144 个固定样本有效：RayTrace 耗时 −8.76%、EarleyBoyer −10.66%、combined −2.72%，均超出本轮 A/A 范围；未分辨控制回归，进入原版复核；每次 admitted child 仍发布一个 active-frame record |
| `2af5282a` | 已持有捕获 cell 的标量直接消费 | NavierStokes Binding 1,355,222→229,637，差额与 1,125,585 次命中一致；96.61% 命中位于 timedRun；库测试 2248/2032 均通过 | 144 个固定样本有效：NavierStokes 耗时 −25.50%、combined −2.08%；Crypto +3.14% 接近 A/A 3.02%，需原版及累计控制复核；解释器栈 +16B、整体 .text +32B |
| `977785cf` | 同一次借用内选槽与提交 | Richards 已有 own data 21,536 次、missing 141 次；EarleyBoyer 2,328/534,555 次；重复查找/第二借用消失；2268/2029 个库测试通过 | 时间待验收；共享 kernel outlining 为通用调用者增加一次 ABI 调用，循环指令形态相同 |
| `b47458a3` | 构造复用共享帧安装（父 `1350c5f0`） | 同样命中 267,264 次 Base 构造；生产代码净 −27 行；两个相关函数 text 合计 −982B；2252/2036 个库测试通过 | 相对 `1350c5f0` 增量时间待验收；有新增 helper 调用，不声称峰值栈下降 |
| `9b2944cf` | 接受的写入直接恢复同一帧 | EarleyBoyer 538,070 次直接恢复；接受路径汇编跳过通用返回 transport；2244 个库测试通过 | 时间待验收；解释循环指令形态相同、完成 helper +153B |

计数减少只证明机制发生。缓存候选的 Combined 结果未分辨，不用其他层的收益替它背书。构造候选的测试通过也不能替代饱和引用计数与失败边界的合同审查。

### 第二批独立时间筛选

八个候选各完成 144 个有效固定负载样本，保留全部原始输出。下表是**耗时变化**，不是原版 Score；观察到的 A/A 范围仅用于筛选，不能代替原版验收或统计置信区间。证据索引为外部 `wave2-gate-ledger.json`。

| 提交 | 机制 | 本轮时间证据 | 决定 |
| --- | --- | --- | --- |
| `86930db6` | Int×Int 使用 checked integer multiply，溢出及负零保留原数字语义 | NavierStokes −5.99%（A/A 2.34%）；Crypto −1.31%、combined −0.51% 均未分辨 | 进入原版复核；Crypto 3,107,216 次命中只证明机制，不声称 Crypto 提速 |
| `6f48c178` | 普通读取不准备消费者不用的 native selection | Richards −4.08%、NavierStokes −6.13%、combined −1.43%；Crypto +1.89%（A/A 1.56%） | 保留条件实验，先复核 Crypto 控制 |
| `c3bf45e2` | public field 经现有定义 kernel 本地完成 | Splay −8.96%、NavierStokes −4.16%、combined −2.29%，均超出本轮 A/A | 进入原版复核；Splay 固定负载包含大量 Setup，不能宣称同幅度原版分数收益 |
| `22b9443d` | 字面量 ready 路径直接消费现有分配结果 | DeltaBlue +4.49%（A/A 2.31%）、Crypto +5.94%（A/A 1.61%）；combined −0.36% 未分辨 | 拒绝当前版本；检查非参与者表示、调用和执行频率后再决定缩小 |
| `977785cf` | 同一次借用选槽与提交 | Richards −3.29%、NavierStokes −5.26%，均超出本轮 A/A；combined −0.62% 未分辨 | 进入原版复核；没有分辨出的其他控制回归 |
| `aa5894ae` | 容量充足时跳过既有 RawVec reserve 边界 | Richards −3.57%、NavierStokes −4.99%；Crypto +3.23%（A/A 3.01%） | 保留条件实验，先复核 Crypto 控制 |
| `5cb4e2f8` | Int 索引 dense own Object 结果本地 checked retain | Richards −3.26%、DeltaBlue −5.26%；Crypto +5.57%（A/A 2.29%）；combined −0.35% 未分辨 | 拒绝当前版本；下一实验检查与 scalar kernel 共享分类/借用能否避免新增探测，不预先归因回退 |
| `67b9501d` | apply 快速参数前缀按需建立 continuation | RayTrace −2.14% 未分辨；Crypto +5.70%（A/A 1.67%）；combined −0.26% 未分辨 | 拒绝当前版本；66,596 次 Arguments 交接消失不替时间收益背书 |

简化与其他技术候选同时推进：普通定义和帧退休研究重复查找/借用；数字与数组路径研究表示和专门化。任何新候选先确定实际热点覆盖和旧接口支持程度，再用独立提交验收。没有通过时间控制的版本不进入累计组合。

`daccb30b` 的生成代码还显示解释循环栈帧增加 96 字节，新增的预算值跨整个解码循环存活并被重载。它说明非参与者确实付出了表示与寄存器成本，不能单凭汇编把 Crypto 的全部回归归因于此。下一版先恢复原解释循环，利用现有 `InstanceStep::start` 的同步 Complete 分支，重新测量收益和控制项。

### 正在验证的简化

- 普通写入在同一次存储借用内完成选槽和提交，移除中间选择状态、第二次借用和重复查找；已有路径几乎都同步完成，不把它描述成消除了通用 continuation。
- 捕获标量绑定直接读取当前 cell，移除临时 cell owner、重复借用和 driver 交接。checked 路径原来的结果复制已是标量叶操作，不能声称减少结果 retain。
- 构造 child 安装复用既有 `push_frame`，减少重复安装代码；守卫和非参与者成本需要相对 `1350c5f0` 单独验收。
- 已接受的普通写入直接恢复同一帧，避免 `Accepted`→未消费的 `Undefined` completion 搬运；保持拒绝、异常和 callback 原合同，等待生成代码与时间证据。

已拒绝在当前证据下拆分 operand arena：`Option<FrameBinding>` 已打包为 16B，与 `JsValue` 同宽，没有证据支持体积或额外机器检查减少。捕获堆结果扩展也不能用 Splay 宽 Environment 计数支持：目前可见的相应读取主要位于 TearDown。字面量路径另行审查，保留每次分配后的 ready GC 服务；每次 timedRun 有 7,600 次 Object/ArrayFrom，而 Setup 有 760,000 次，二者不能混计收益权重。

## 验收顺序

首轮累计实验另建 clean worktree，分提交集成 Int 表示、binary64 kernel、checked Base 构造、同步 instanceof 和捕获标量读取，当前实验 head 为 `72a4de30`。Rust 1.88.0 下 profiling/test262-host 库测试 2,286 项、默认库测试 2,046 项通过。该 head 仍是累计实验，未最终采用；各层独立归因、Richards 控制、Boa 比较与资源交换仍需验收，不能用累计收益替各层独立结论背书。

### 首轮原版完整累计测量

`integrated-original` 保留普通 release 下 CPU 2 的 ABBA–BAAB 四轮/引擎、全部八次完整运行输出。八项均来自 combined 运行，主体与计时/重复策略未修改。所有样本退出成功、九个分数标签完整；没有删除低样本。

| 子项 | `3d420407` 中位数（范围） | `72a4de30` 中位数（范围） | 分数变化 |
| --- | ---: | ---: | ---: |
| Richards | 93.15（91.5–93.6） | 91.75（90.8–92.2） | −1.50% |
| DeltaBlue | 104（101–105） | 104（103–104） | 0.00% |
| Crypto | 190.5（190–191） | 198（197–198） | +3.94% |
| RayTrace | 144（143–145） | 158.5（158–160） | +10.07% |
| EarleyBoyer | 163.5（163–165） | 215（214–217） | +31.50% |
| RegExp | 89.2（88.4–89.3） | 89.65（88.9–90.4） | +0.50% |
| Splay | 430（427–431） | 445.5（426–451） | +3.60% |
| NavierStokes | 413.5（411–416） | 562（476–565） | +35.91% |
| Combined | 170（169–171） | 186.5（181–187） | +9.71% |

这里是分数变化，不是耗时变化。Richards 的下降需要继续复核；Splay/NavierStokes 第三个组合样本同时偏低，原因未确认，完整保留。该测量支持累计目标收益，不替各层单独归因。组合已通过 focused Test262 6,844/6,844 及完整 102,037 变体的冻结正文比较，完整向量仍为 80,010 通过 / 80,060 eligible。新一轮同机 Boa 比较仍待执行，尚未达到全项超过 Boa 的目标。

1. 原始主体上的逻辑 profile 验证命中、删除的工作和保留的 owner。
2. `iterate_v8.py` 对八项及 combined 固定负载执行 A/A 与交错 A/B。复用首次冻结的负载，逐样本验证身份、输出和退出状态。
3. 有可信收益的候选验证硬件计数、内存、编译与代码体积，再跑原版完整 suite；其他子项独立检查回归。
4. 分 commit 集成，重新验证语义与累计性能。最终与同机、同原版 suite 的 Boa 做新一轮平衡采样，八项与 Combined 分别判断。

本页是进行中的实验记录；没有原版 Score 的新数据时，不更新历史分数为新结论。

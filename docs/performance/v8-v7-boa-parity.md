# V8 v7：追赶 Boa 的实验记录

## 目标与边界

目标是让**原版完整 V8 v7 运行输出的八个子项和 Combined Score 均稳定超过 Boa 0.22.0**。固定工作量时间、计数和汇编用于筛选与解释，不能替代这个验收条件。当前还没有达到目标。

保持 JavaScript 效果顺序、所有权、引用计数饱和规则、异常与栈限制、GC 服务边界以及 safe Rust。禁止针对 benchmark 名称或输入识别选择实现。候选分别提交、分别计时，通过后再验证累计结果。低覆盖、未兑现收益或维护成本过高的候选可以缩小或拒绝。

简化也是候选机制：合并选槽与消费、删除中间状态、集中检查责任，使消费者直接使用当前已知事实。它需要兑现为执行工作或目标指标的改善；仅重命名、减少源码行数或增加通用抽象不构成性能验收。

本轮对照是清理后的 `3d42040738b2a40569889a3fac4744c58bf2ae0c`，不是此前测量中的旧 main。历史分数与本轮独立收益不能混合归因。

## 当前证据

外部 suite 固定在 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。Rust 1.88.0、普通 release、fat LTO、CGU=1 与独立 target 的回执标识时间用二进制；profiling 构建只提供机制计数。原版分数、首两批固定筛选及原生采样固定 CPU 2，停止并行构建、测试和其他采样。回执复核发现第三批六项固定筛选及两次 wave2 累计固定比较使用默认 affinity（0–15），此前笼统写成 CPU 2 不准确。这些原始样本完整保留、按该条件解释；后续固定筛选显式绑定 CPU 2，不跨这两种条件拼接单层结论。CPU governor 是 powersave，未锁频，保留 A/A 观察范围。

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

累计版本 `eaf23ba8` 另完成原版 combined 的原生诊断 profile，约 68K cycles 样本、零 lost sample，九个输出标签完整。self 权重中 execute 18.06%、dup_jsvalue 4.88%、memmove 3.86%、release_jsvalue 2.56%、release_heap_reference 2.16%、commit_push 1.96%、select_linked_data_into 1.81%、commit_owned 1.72%、FrameSlots::push 1.60%。这些权重包含准备、warmup、timedRun、TearDown 和退出，不能相加为某项候选的可删除成本。memmove 的可见调用链部分来自 RegExp 恢复；`commit_push` 与 `FrameSlots::push` 是不同来源，不是两层重复认证。证据为外部 `integrated-wave2-native/{metadata,summary}.json` 和完整 stacks/self 报告；打印 Score 不用于时间验收。

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

### 后续并行交付，时间待验收

| 提交 | 候选与机制 | 正确性和新增成本 |
| --- | --- | --- |
| `7a656d2c` | 普通 Define 在一个借用内完成选择、验证、提交，own lookup 4→1；主要覆盖 Splay 的 1,018,080 次，其中 timedRun 10,080 次 | profiling/host 2,274、default 2,035、目标 7 项通过；owner/storage 计数不变；.text +2,160B，定义 helper 栈 +112B，global/raw 多一次共享验证调用 |
| `d8b0e5ee` | 帧退休保持原槽位顺序和每个边的释放，延迟打开并复用 heap state 借用；Richards 19,128 条堆边用 12,583 次共享借用，省 6,545 次重复借用 | profiling/host 2,276、default 2,037、release 目标 9 项、trace 目标 1 项通过；解释循环不变；clear_frame +1,331B、栈 +128B，.text +1,328B |
| `f0cf87b2` | kept receiver Array 读取复用现有 materialized own Number kernel；只有 Crypto 新命中 21,459 次，GetElement 退出同量减少 | profiling/host 2,270、default 2,031、目标 3 项通过；Query、materialization、Object copy 不变；helper +76B、栈 +16B，.text +80B，解释循环不变 |
| `f0906245` | 现有 fused numeric comparison 直接比较两个 Int；Crypto 1,009,679 次、NavierStokes 934,807 次、Splay 525,126 次整数命中；混合/Float 使用原 binary64 比较 | 数值边界矩阵及 profiling/host 2,268 项通过；解释循环 +305B、栈不变，.text +768B；额外分支对 Float 消费者的时间影响待测 |

整数比较借鉴的是 [V8 13.6 的 RelationalComparison](https://raw.githubusercontent.com/nodejs/node/v24.21.0/deps/v8/src/codegen/code-stub-assembler.cc)：Smi 双输入直接比较，混合 HeapNumber 才进入浮点比较。我们只消费已有 Number::Int 事实，既有接口已允许，不引入 V8 的指针表示或新执行架构。kept 数组读取则补齐现有接口对存储表示的覆盖；Crypto 实际读取的是 `this.array`/`w.array`，不是普通 BigInteger 数字属性，不能将此前回退归因到未经证实的普通对象探测。

第二轮累计实验 `eaf23ba8` 在 `72a4de30` 上分提交加入 `977785cf`、`c3bf45e2`、`86930db6`。clean plain/profile 构建、八项逻辑 profile、库测试 2,297/2,056、focused Test262 6,844/6,844 已完成；完整 102,037 变体的冻结正文比较也已通过，仍为 80,010 pass / 80,060 eligible。资源交换仍待验收。这些新交付候选尚未加入组合。

`5fdc3e54` 另行验证已注册 shape successor 的直接消费：八项新增命中逐项等于旧重复注册计数；必要 shape 生命周期测试 12 项、profiling/host 2,269、default 2,030 项通过。plain 汇编保留 full generation/checked retain，registered hit 跳过 `record_transition`，解释循环不变，.text +336B、append 栈不变。追加一次 body 重复获得 RayTrace 193,723、EarleyBoyer 542,843 次追加命中；这证明 body 覆盖，不等同冷首轮精确 timed 计数。独立时间仍待验收。

此前拒绝的 dense Object 读取已缩小为 `3d420407..37c2aa10` 的独立完整补丁：同一次借用内完成 consuming 结果分类与 checked retain，删除没有覆盖的 kept Object 扩展，必要结果 owner 保留。八项 Object 命中逐项等于初版，scalar/materialized Number 计数不变，kept 新命中为零。profiling/host 2,271、default 2,032 项、clean plain/profile 和八项机制验证完成。kept helper 归一化机器指令与基线相同；consuming helper +126B、栈 +16B，整体 .text +304B，解释循环不变。最终 alias `dense-object-consume-direct`；不是只 cherry-pick 最后一提交，旧中间版本不用于验收。

### 第三批独立时间筛选

串行使用同一个冻结负载，继续只测变化的 Oxide 候选和所需 Oxide 基线，复用 Boa。外部索引为 `wave3-gate-ledger.json`。首个 `f0cf87b2` 的 144 个样本全部有效：Crypto 耗时 −0.069%（A/A 1.103%）未分辨，21,459 次 GetElement 退出消失没有兑现可分辨的目标时间收益。Richards −2.39%、NavierStokes −6.42%、combined −1.29% 分别超出本次观察 A/A，但这些项没有新增机制命中，归因尚未确定，不将它们归给 materialized Number 覆盖。保留实验，暂不加入累计组合；其他候选继续按独立顺序验收。

随后三个候选各完成 144 个有效样本：

| 提交 | 本轮固定负载时间证据 | 决定 |
| --- | --- | --- |
| `d8b0e5ee` | EarleyBoyer +0.23% 未分辨；Crypto +4.42%（A/A 2.08%）；combined +1.23% 未分辨 | 拒绝当前版本。共享借用机制成立，不能替新增代码和栈成本背书，也不能单凭它们归因回归 |
| `f0906245` | NavierStokes −6.38%（A/A 4.39%）；Crypto +5.91%（A/A 2.23%）；combined −0.92%（A/A 0.76%） | 拒绝当前版本。V8 的整数分支概念需要接受我们的表示、生成代码及完整控制验收 |
| `5fdc3e54` | 九项时间均在各自 A/A 范围内；RayTrace −0.43%、EarleyBoyer +0.67%、Splay +0.12%、combined −0.51% | 不作为性能收益采用；重复注册消失证明机制，但时间未分辨 |
| `7a656d2c` | Splay −0.48%（A/A 1.46%）未分辨；Crypto +2.56%（A/A 2.16%）、EarleyBoyer +2.26%（A/A 2.02%）；combined +0.15% 未分辨 | 拒绝当前版本；不因查找 4→1 而扩大共享定义抽象 |
| `37c2aa10` 完整补丁 | Richards −4.36%（A/A 3.08%）、DeltaBlue −2.79%（A/A 1.42%）；Crypto +0.50%（A/A 1.73%）未分辨；combined −1.28%（A/A 4.00%）未分辨 | 进入 Richards/DeltaBlue 原版复核，旧版 Crypto 回归未分辨；NavierStokes −5.67% 不能归给仅 3 次新增 Object 命中 |

此后暂停候选队列的编排父进程，当前 matrix 正常完成，插入累计独立控制，没有与编译、JS 或采样并行。`72a4de30`→`eaf23ba8` 的 `integrated-wave2-fixed` 144 个样本有效：Richards −5.16%（A/A 2.76%）、Splay −7.10%（A/A 5.33%）、RegExp −2.03%（A/A 1.48%）、combined −3.38%（A/A 1.91%）；未分辨其他子项回归。Crypto −4.34% 仍在 A/A 4.51% 内，NavierStokes −2.56% 仍在 A/A 2.80% 内。这证明三层累计交换有时间证据，不拆分归给每一层。

`3d420407`→`eaf23ba8` 的 `integrated-wave2-total-fixed` 另完成 144 个有效样本：Richards −4.79%、Crypto −5.87%、RayTrace −9.34%、EarleyBoyer −27.67%、Splay −8.64%、NavierStokes −25.80%、combined −11.86%，均超出各项本轮 A/A；DeltaBlue −0.98%、RegExp −1.22% 未分辨。没有分辨出的控制回归。两次累计固定比较均为默认 affinity，证据索引 `cumulative-fixed-ledger.json`；不能拿它们与 CPU 2 的独立候选数值相减作为层收益。

同一冻结 combined 负载的串行资源诊断（每二进制各两次）显示，`3d420407`→`eaf23ba8` RSS 中位数 346,216→347,502 KiB（+1,286 KiB，+0.37%）；cycles −11.84%、instructions −11.11%、branches −9.68%、branch misses −2.51%。四个计数器均有 100% running，无 multiplex；完整进程包含启动、Setup、fixed run、TearDown 与退出。这是资源诊断，不是峰值分布、置信区间或原版 Score。证据 `integrated-wave2-resources`。编译时间尚无同条件对照，不将不同构建日志的 elapsed 差异归因于实现。

### 第四批已验证候选

| 提交及基线 | 机制及实际覆盖 | 新增成本与状态 |
| --- | --- | --- |
| `a72f5a8f` / `eaf23ba8` | Number unary 与 immediate Not 在已验证栈顶原地完成；Crypto `am3` run 命中 810,765 次，slot move 删除数逐项等于命中 | execute +474B、栈 +16B；2301/2060 库测试通过，八项计数验证与 plain 完成验证通过；时间待验收 |
| `f67e0fde` / `eaf23ba8` | Spilled 属性槽复用 `Vec` 容量，增长不显式克隆所有槽；RayTrace 重复 body 增量免重建 3,875 次、免克隆 17,323 个槽，EarleyBoyer body 增量为零 | 整体 .text +112B，execute 不变；结束时容量多约 5–19 KiB；2301/2060 库测试及八项逻辑验证通过，时间与 RSS 待验收 |
| `fddcc56b` / `3d420407` | 两个暖缓存字段的 Object 结果在同一借用内严格比较；EarleyBoyer 136,786 次命中，每次删两次 owning promotion 与四次 operand 搬运；其余七项计数逐项不变 | 整体 .text +8,160B，execute +16B；2274/2035 库测试、trace fallback、八项逻辑验证通过；时间待验收 |
| `adb6a969` / `eaf23ba8` | 累计试验加入 `37c2aa10` 的完整 dense Object consuming 补丁；八项差额与独立机制逐项一致 | 2301/2060 库测试通过；完整 Test262 102,037 变体冻结正文一致（80,010 pass / 80,060 eligible）；增量固定与原版时间待验收 |

第四批中央计时显式 CPU 2，仅源码研究并行。新增类型或消失的 helper 不能替代时间和非参与者控制。

前三个候选分别完成 144 个有效 CPU 2 样本，索引 `wave4-gate-ledger.json`：

- `a72f5a8f`：Crypto +5.38%（A/A 2.08%）、NavierStokes +3.25%（A/A 1.40%），其他项及 combined 未分辨收益。拒绝当前版本，不继续增加原位模式。
- `f67e0fde`：RayTrace −0.59%（A/A 2.03%）、Splay +0.04%（A/A 1.22%）目标未分辨；Richards +4.06%（A/A 2.42%）。Crypto −1.97%（A/A 1.67%）不足以替目标覆盖及 Richards 回归背书，拒绝加入累计。
- `fddcc56b`：EarleyBoyer −0.23%（A/A 3.01%）、combined −0.81%（A/A 3.29%）未分辨；NavierStokes −4.15%、RegExp −2.35% 超出各自 A/A，但没有新增命中，归因未定。保留能力实验，暂不作为性能收益采用。
- `adb6a969` 相对 `eaf23ba8`：EarleyBoyer −2.25%（A/A 1.26%），但 Crypto +5.71%（A/A 1.87%）；Richards +2.96%、DeltaBlue −2.64% 各在约 9% 的 A/A 范围内，combined +0.65%（A/A 1.34%）未分辨。拒绝累计加入当前 dense Object consuming 补丁。旧基线上的局部收益没有在该累计组合通过控制；不继续跑原版或资源测量替它背书。

下一组独立源码实验继续覆盖不同机制：已注册 shape 后继的标量字段创建、warm 双字段 Number 算术直接消费、新建 factory owner 的直接交接，以及复用既有 `push_owned` 的 owning 结果搬运。新建对象交接单独保留 pending cleanup 的历史 retain→push→drop 边界，修正未提交 owner 的失败回收；不混入已拒绝的 literal-ready 调度变化。所有草稿先验证正确性和实际消费者，不能因预计免除协议就宣称性能能力完成。

### 第五批独立实现与覆盖

| 提交 / 基线 | 已验证机制和消费者 | 生成代码与资源成本 | 状态 |
| --- | --- | --- | --- |
| `cbf33ee5` / `eaf23ba8` | `commit_owned` 复用现有 `push_owned` 的成功转移/失败保留合同，移除 `Option` 搬运；失败 release 留在短 slots 借用之后 | helper 320→245B，静态指令 87→70，栈 56→48B；execute、普通 push、release 归一化指令相同，size 汇总 text −56B | 目标 owner/GC 测试、2298/2057 库测试通过；双回执及八项诊断通过，既有事件、VM 存储、内存分类/堆状态逐项相同；CPU 2 时间出现回归，拒绝当前版本 |
| `e57b1fcf` / `eaf23ba8` | 仅使用已注册且仍存活的 shape 后继及 retained parent，在一次借用内创建标量字段；RayTrace 166,779、EarleyBoyer 116,886 次完成，重复 body 增量分别 166,797 / 116,184 | execute 不变；write helper 816→877B / 栈 72→88B；new helper 1362B / 栈 152B；共享 append 由内联转为 outlined，整体 .text +1472B | 新 8 项目标测试、2305/2064 库测试、双回执、八项诊断及重复 body 通过；既有协议差额精确对应完成数，内存分类/堆状态相同；后续饱和审查发现 receiver/key 准入不等价，当前不可采用，修复另树验证 |
| `ef6721cc` / `eaf23ba8` | 仅 CreateArray/Object/Variable 的 fresh factory edge 直接发布；Splay 767,604 次，其中 Setup 760,000、每次 run 7,600；EarleyBoyer 仅 95 次。Richards、DeltaBlue、Crypto、NavierStokes 没有 timed body 覆盖；未观察 Variable 或 pending fallback 命中 | environment_step 11124→10523B / 栈 1672→1656B；新 helper 876B / 栈 120B；必要 pending 检查及 `into_handle` 再检查保留，整体 .text +272B；execute、ready、push_owned、release 归一化相同 | 8 项目标、2305/2064 库、trace 验证通过；双回执、八项诊断与 codegen 完成；目标时间未分辨，保留实验；不能拿完整 Environment 计数作覆盖 |
| `345179fb` / `eaf23ba8` | Local/Arg/This 的两个 warm Number 字段在一次借用内经原有算术 kernel 直接消费；miss 保留原左字段读取 | RayTrace 148,450/148,473 个候选命中，Crypto 1,239/1,240；execute +266B、栈 +32B，整体 .text +9,104B | 2304/2063 库及 trace 测试、双回执、八项覆盖验证通过；CPU 2 时间出现多项回归，拒绝 |

以上仍是独立候选，没有将新的机制计数记为分数收益。第五批也只测 Oxide 候选及所需 Oxide 基线，复用 Boa。

第五批每候选完成 144 个有效 CPU 2 样本（A/A 与 ABBA–BAAB，索引 `wave5-gate-ledger.json`），以下波动范围不是置信区间：

- `cbf33ee5`：Richards +5.55%（A/A 1.60%）、Crypto +7.28%（A/A 0.81%）、combined +1.86%（A/A 1.37%）。helper 缩小没有兑现时间收益，拒绝合入，不为当前版本追加原版或资源测量。
- `345179fb`：目标 RayTrace +0.24%（A/A 1.13%）未分辨；Richards +3.12%、DeltaBlue +1.51%、Crypto +6.11%、EarleyBoyer +2.09%、Splay +1.52%、NavierStokes +8.19%、combined +1.90% 均超过各自本轮 A/A。拒绝当前字段算术融合，不扩大模式覆盖。
- `ef6721cc`：Splay −0.99%（A/A 2.26%）、combined +0.59%（A/A 1.33%）；八项及 combined 均未分辨收益或回归。当前不作为性能收益采用，失败 push 的旧 owner 泄漏另作为正确性修复评估。
- `e57b1fcf`：RayTrace −4.90%（A/A 3.15%），NavierStokes +1.82%（A/A 1.50%），combined +0.08%（A/A 1.17%）。即使局部时间有信号，也不能采用：旧 missing driver 对 receiver 的 checked retain 在 MAX 溢出、MAX−1 达 immortal，旧 key retain 对非 pinned MAX 溢出；新叶路径跳过了这些可观察边界。旧短名 `x` 测试实际是 pinned None，没有覆盖 key 饱和，修复测试使用长非 pinned 字段名并强制断言 Some。独立修复候选拒绝这些状态并保留原 fallback，验证后重新计时，原测树与回执保留。

下一组同时验证不同机制：在现有 Pure 协议内直接消费 `typeof` 的 number 比较，修复已注册标量创建的饱和准入，以及根据真实汇编宽返回区研究冷错误搬运。Math 现有入口已同步完成，暂不引入 continuation 重构。

`e08a9416` 独立修复了 `e57b1fcf` 的 receiver/key 饱和准入，12 项目标、2308/2067 库测试及双回执/八项机制验证通过。新增 guard 在最终汇编保留；原覆盖及已报告部分内存/堆状态逐项相同。144 个 CPU 2 固定样本显示 RayTrace −4.92%（A/A 3.19%），但 Richards +3.26%（A/A 1.48%），Crypto +1.35%（A/A 1.20%），combined +0.61%（A/A 1.68%）未分辨。暂不合入当前版本；后续 owner 创建实验仍须相对采用的累计基线接受完整控制。

私有属性位置缓存也另做表示实验：`67211c16` 借用当前状态并原地调整多态顺序，保留全部 identity/layout/epoch 守卫和既有冷却策略。15 项定向、2298/2057 库测试通过。release 暖读取 2641→2141B / 栈 600→376B，miss 4970→3138B / 栈 680→136B；整体 size text −1956B、data +48B。解释循环仍为 37433B / 7585 静态指令 / 1176B 栈，仅一条运行时字段偏移增加 104B（13 个 Proxy trap 缓存各新增一个借用标志）。这些是代码与表示证据；八项实际事件、VM 布局/存储、call buffers/preparation、已报告的部分内存分类与堆状态均相同；这些内存分类未覆盖新增 cache 元数据，不能声称总内存相同。144 个 CPU 2 固定样本显示 Richards +5.58%（A/A 1.34%）、Crypto +3.40%（A/A 1.68%），其他项与 combined 未分辨。拒绝借用版，不继续原版/RSS 验收。热读取新增借用 flag 检查和写入，是实际新增工作，尚未将全部回归归给它。

### 第六批：契约修复与局部表示实验

`ac41fa33` 在既有 Pure 协议内直接消费 `typeof` 的 number 字符串比较，无新 opcode/metadata。2308/2066 库、10 项目标及 TRACE_ROOTS 测试通过；真实发布前缀和 EarleyBoyer 的 TypeOf PC 1 命中 36,608 次，其他七项零命中。String owner copy、strict comparison/PC publication 各少同量，slot move −146,432；slot authentication +73,216。既有 calls、frame、argv/布局不变；frame_operations::pure +1192B，整体 .text +1328B，execute/ready 归一化相同。

其 144 个 CPU 2 固定样本显示 EarleyBoyer +0.14%（A/A 2.53%）、combined +0.31%（A/A 1.15%）目标未分辨；Richards +3.61%（A/A 2.71%）、RayTrace +2.02%（A/A 1.00%）。拒绝当前版本，不扩展融合形式。Crypto 的 A/A 跨度 32.24%，保留全部样本并记为未分辨，未删掉低样本。控制项无命中，不能将它们的变化归为 typeof 算法收益或成本的独立量值。三候选固定索引为 `wave6-gate-ledger.json`。

释放接口的独立源码审查证明 `release_jsvalue` 各配置下只能返回 `Ok(())`：底层检查、释放、defer/drain 仍负责原有 invariant 诊断。`e6dbf36a` 让其返回 `()` 并迁移 227 个 Rust 文件；34 个聚合/尾表达式专门审查，所有可达 release 参数与顺序保留，仅删除 6 个无法进入的 release 错误分支中的额外清理。`root_and_release_jsvalue` 的 root 阶段仍真正可失败，保持 Result。全源码签名审计的 5,074 个函数和 132 个类型声明中，只有目标 API 的返回类型变化。2297/2056 库测试、12 项 release/trace 边界测试和 clean 双构建通过。八项 plain 运行及 profile 诊断完成；除生成文件路径归一化和计时字段外，全部存储、事件、布局与诊断逐项相同。完整 Test262 的 102037 变体冻结正文、行数、runner provenance 均匹配：80010 pass / 80060 eligible，既有 fail/unsupported/skipped 也逐项相同。release 的四个下层 checked/defer/drain 函数归一化机器码不变。时间尚待测量，不能因删掉 Result 宣称性能收益。

### 第七批：释放协议、字段 owner 和缓存表示

三个独立候选均以已采用的 `eaf23ba8` 作为完整时间基线；没有把已拒绝的父层收益记为新增层收益。

| 候选 | 机制与正确性证据 | release 代码 / 表示成本 | 状态 |
| --- | --- | --- | --- |
| `e6dbf36a` 内部 release 返回 unit | 保留真实 defer/drain、诊断和 release 顺序，移除不可达错误协议；库、trace、八项逻辑及完整 Test262 冻结向量验证完成 | execute 37433→37333B，栈 1176→1016B；release 323→288B，无返回区/Ok tag 写回；native argv cleanup 489→385B，栈 136→80B；commit_owned 320→237B，栈 56→16B | CPU 2 时间 gate 出现 Crypto 回归、combined 未分辨；拒绝当前整体版本，不据代码缩小声称净提速 |
| `8d88151e` registered heap 字段 owner 转移（含 `e08a9416`） | mutable 输入在 unpublished 失败时保留、publication 成功或错误时立即置 Undefined；8 组新增、10 组旧目标测试及 2316/2075 库测试通过；完整身份、receiver/key/value 的旧 checked retain 饱和边界保留 | Earl heap 完成 417617、Ray 26887；Earl 新增 materialize 267195，frame authentication 净减少 267308，而非全部 534503 次 Set exit；execute +239B、栈 +16B，整体 .text +6656B | clean 双回执、八项诊断与 repeat 完成；CPU 2 整体控制失败，拒绝。部分内存分类相同不等于峰值 RSS 相同 |
| `4f03cc8c` split Cell 位置缓存 | 小状态头与四个 Cell<Option<Location>>，不使用 RefCell 借用标志；保留原 guard、probe、冷却及 prefix 顺序合同；16 项目标、2299/2058 库测试通过 | 实测每 cache 240→232B、13 个 Runtime 内联缓存合计 −104B；warm read 2641→2242B，栈 600→168B；miss 4970→3288B，栈 680→184B；execute 相同，仅一条字段偏移 −104B | clean 双构建、八项诊断完成；CPU 2 combined 和控制项回归，拒绝；Option 检查及布局交换不能被代码缩小掩盖 |

`8d88151e` 的计数按 `T = scalar + heap`、`M = 新增 materialize` 分开核对：Set action 减少 T，frame authentication 减少 T−M，slot authentication 减少 2T−M。此前 Set driver 已 materialize 的职责会在后续 heap Drop/Nip 等消费者处重新出现，不能把全部 Set 退出数当成净调度收益。BigInt 在 V8 八项没有实际命中，只有针对性正确性见证。八项 storage、部分内存分类与 heap states 相同，原版时间、峰值资源仍未验收。

第七批 432 个样本全部有效，每候选 144 个，CPU 2 上 A/A 与 ABBA–BAAB；没有删除样本，索引 `wave7-gate-ledger.json`。下列是固定负载的耗时变化，A/A 跨度不是置信区间，也不是原版 Score：

- `e6dbf36a`：EarleyBoyer −2.26%（A/A 1.60%）、RegExp −3.23%（A/A 2.28%），但 Crypto +3.57%（A/A 1.29%）；combined −1.05%（A/A 1.12%）未分辨。拒绝整体迁移作为当前性能改进；完整语义通过没有替代时间控制。普通 Number kernel、dense read/index 和比较 helper 的归一化机器码相同，尚未找到可以单独归因 Crypto 回归的新增工作。
- `8d88151e`：RayTrace −6.07%（A/A 5.29%）、EarleyBoyer −4.91%（A/A 1.26%），但 Richards +7.66%（A/A 1.12%）、Crypto +2.79%（A/A 1.84%）、NavierStokes +3.73%（A/A 2.31%）；combined −0.16%（A/A 2.95%）未分辨。拒绝当前整个候选，不追加原版/RSS 测量为控制失败背书。
- `4f03cc8c`：Richards +4.40%（A/A 0.77%）、RayTrace +1.35%（A/A 1.04%）、EarleyBoyer +2.47%（A/A 1.72%）、NavierStokes +4.83%（A/A 1.39%）、combined +2.83%（A/A 1.08%）。拒绝当前缓存表示。header 实际位于 offset 0xe0，与首 location 分开，是需要区分的 locality 交换；尚不能据此归因全部回归。

后续源码研究分开验证两个数值机制：`Number::compact` 的 guarded safe cast 能否消除饱和转换的 clamp，以及已知 binary Opcode 能否直接选择现有 kernel。普通 Number 的 slot 读取/提交已内联，仍有一次 outlined `binary_number_result` 调用和第二张 20-entry 动态分派表；不是普通路径上两个 outlined calls。Crypto 12,534,392 次原地 Number 操作仅为逻辑覆盖，不是时间权重。数组前缀补齐后恢复 dense 表示的另一假设，仍须按同 ObjectId 与 Setup/run 阶段确认，694,066 次 materialized Number read 只是上限。

`daccb30b` 的生成代码还显示解释循环栈帧增加 96 字节，新增的预算值跨整个解码循环存活并被重载。它说明非参与者确实付出了表示与寄存器成本，不能单凭汇编把 Crypto 的全部回归归因于此。下一版先恢复原解释循环，利用现有 `InstanceStep::start` 的同步 Complete 分支，重新测量收益和控制项。

### 暂停队列的剩余六个候选

暂停的 orchestrator 已恢复并正常结束，六个矩阵各 144 个有效样本，全部为 CPU 2；`wave8-remainder-gate-ledger.json` 保留 hash、全部九项及取舍。它们没有和此前 default-affinity 的六个矩阵混为一批。以下同样是固定负载耗时，括号为各项 A/A 跨度；不是原版分数或置信区间。

| 候选 / 独立基线 | 主要观察 | 取舍 |
| --- | --- | --- |
| `fc7a3189` Object copy / `3d420407` | EarleyBoyer −0.68%（2.67%）、combined −0.10%（1.00%）；Crypto +3.80%（66.73%），其 A/A 一个 1.082s 样本与其他约 0.65s 样本全部保留；NavierStokes −6.18%（4.74%）没有目标覆盖归因 | 目标及 combined 收益未分辨，不采用；不删除异常样本再宣称 Crypto 结论 |
| `9b2944cf` write completion / `3d420407` | DeltaBlue +2.53%（0.80%）、Crypto +3.12%（1.88%）；目标 EarleyBoyer +1.25%（1.11%），combined +1.07%（1.36%） | 拒绝；减少 completion 搬运没有兑现目标收益 |
| `e3b0f32e` owning property value / `977d7158` | Richards +3.28%（1.07%）、DeltaBlue +2.50%（1.85%）、Crypto +3.02%（1.30%）；EarleyBoyer +0.55%（2.78%）、Splay −0.08%（1.24%）、combined +1.02%（0.90%） | 拒绝；目标未分辨，三个控制回归；实际基线为 same-borrow write selection，不与 eaf 层差混算 |
| `b47458a3` shared constructor install / `1350c5f0` | RayTrace +2.43%（2.56%）、Splay +0.52%（1.65%）、combined +0.66%（1.63%）；EarleyBoyer +2.02%（1.88%） | 不作为性能改进采用；复用函数与代码缩小没有独立时间收益 |
| `ddef3000` strict equality / `3d420407` | EarleyBoyer −0.88%（2.31%）、Splay −1.69%（1.29%），但 combined +1.76%（1.36%） | 拒绝当前组合控制；目标 EarleyBoyer 未分辨 |
| `fe7b9462` truthiness / `3d420407` | DeltaBlue −1.81%（0.88%），Crypto +4.20%（2.58%）；Splay −0.86%（1.29%）、combined −0.12%（3.42%） | 拒绝 Crypto 回归，combined 未分辨；不把多数发生于 Setup 的搬运计数作为 timed body 收益 |

这些候选均未进入采用的八层性能实现，且没有为已失败的完整控制继续追加原版或资源测量。下一窗口先验证独立数值算法与静态 kernel 的语义和 release 代码，再决定是否值得计时。Return owner 方向先有仅 profiling 的实际 published-tail/type 诊断草稿，尚未宣称 owner 转移能力；PGO 已有旧工具，但必须先补齐 versioned profile、训练、构建证据合同并显式区分 build technique，不能把旧 v1 PGO 标为 plain 绕过 gate。

### 数值算法、静态分派与真实消费者

`a64920fd` 的 guarded safe-cast compaction 完成目标 9、profiling/host 2,301、默认库 2,059 项测试和 clean plain 回执。真实算术 join 仍保留 `maxsd/minsd/cvttsd2si` 与 NaN select，预定的饱和转换删除机制未兑现，停止，不追加 profile 或计时。bit equality 确实替换了原 equality/negative-zero 检查，不能称它完全没有改变代码，也不能未经计时宣称变慢。helper 3337→3463B，整体 .text +1328B；证据在 `crypto-raytrace-next-mechanisms/guarded-number-codegen/mechanism-ledger.json`。

`eda47fab` 静态 Number opcode 专门化通过目标 2、profiling/host 2,299、默认库 2,058 项及 clean plain/profile 构建。真实 ordinary 路径的动态 `binary_number_result` 调用 1→0、第二分派消失；LLVM 将原 inline 选槽和固定 kernel outline 为 18 个专门化 helper，另保留原动态 fallback。全 candidate .text +11,392B、execute +1,550B / 栈 +16B；新增成功 `Result<bool, Error>` 返回检查，不能称删除分派没有代价。八项新增 static opcode successes 之和逐项精确等于旧 `binary_number_in_place`，原事件、storage/layout/call/dispatch/omitted 计数均相同。计数包含初始化、Setup/run/TearDown 和 driver，不是 timed 权重。固定时间启动前发现候选回执多了 `CARGO_BUILD_JOBS=2`，校验拒绝、没有计时样本；保留失败日志，用相同环境 fresh target 重建后再比较，不编辑回执或放松 gate。

另一独立候选只替换 `Number::compact`：通过 binary64 指数、fraction 和 sign 恢复精确 i32，无 FP→integer cast。K=E.wrapping_sub(1023) 在 0..30 时两次 shift 分别限于 12..42 / 22..52，其他情况只有 positive-zero/MIN 例外；拒绝返回原 Float bits。旧 ToInt32 没有修改。目标 9 项已通过，库测试、真实 release codegen 和时间尚待完成；变量 shift/依赖与全部旧消费者的成本仍需接受验收。

`82946f63` 只 profiling 的真实 Return 邻接诊断完成目标 2、profiling/host 2,299、clean profile 与八项一次逻辑运行。direct Object 标签：Richards 4,010、DeltaBlue 1,016、Crypto 18、RayTrace 25,073、EarleyBoyer 89,314、RegExp 104、Splay 160、NavierStokes 0。EarleyBoyer 的 `sc_term_12` 是 78,781 次 Object，另一 term 返回是 15,284 次 String；不能混算。它只证明已发布 GetLocal/GetArg 紧接 Return 时的当前类型，不证明 full generation/count/cleanup 准入、所有权转移或时间收益。plain 无新增调用/检查，没有 owner 优化。证据 `direct-binding-return-logical/joint-coverage-ledger.json`。

PGO 另在独立 `perf/v8v7-heldout-pgo` 工具实验中推进：保持 Rust 1.88、匹配 LLVM 20.1.5，用通用 scaling scripts 训练、原版 V8 v7 留作验收。构建模式、真实 flags、profile merge/训练输出/源码回执必须完整，显式 build-technique 比较不能伪装 plain source 比较。准备阶段只完成工具校验测试；后续实际训练和测量见下文。

### 简化方向的累计取舍

- 普通写入在同一次存储借用内完成选槽和提交，移除中间选择状态、第二次借用和重复查找；已有路径几乎都同步完成，不把它描述成消除了通用 continuation。
- 捕获标量绑定直接读取当前 cell，移除临时 cell owner、重复借用和 driver 交接。checked 路径原来的结果复制已是标量叶操作，不能声称减少结果 retain。
- 构造 child 安装复用既有 `push_frame`，减少重复安装代码；相对 `1350c5f0` 的独立时间验收未分辨目标收益，当前不采用。
- 已接受的普通写入直接恢复同一帧，避免 `Accepted`→未消费的 `Undefined` completion 搬运；保持拒绝、异常和 callback 原合同，但当前时间控制失败，拒绝该候选。

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

这里是分数变化，不是耗时变化。Richards 的下降需要继续复核；Splay/NavierStokes 第三个组合样本同时偏低，原因未确认，完整保留。该测量支持累计目标收益，不替各层单独归因。组合已通过 focused Test262 6,844/6,844 及完整 102,037 变体的冻结正文比较，完整向量仍为 80,010 通过 / 80,060 eligible。尚未达到全项超过 Boa 的目标。

### 第二轮原版完整测量，复用 Boa

按用户指示，Boa 不再追加测量，复用已完成的三个原版 combined 样本（0.22.0，二进制 SHA `ff6dbbdc…fe953`）。追加对照已经中止，部分 Boa 输出不使用。`eaf23ba8` 的首个完整样本保留，再续测三个 Oxide-only 样本；四个样本的二进制、原始主体 hash 一致，九个分数标签完整，所有样本都保留。原始收据及聚合在外部 `integrated-wave2-original-summary.json`。

| 子项 | `eaf23ba8` 中位数（四轮范围） | 既有 Boa 中位数（三轮范围） | 我们 / Boa |
| --- | ---: | ---: | ---: |
| Richards | 97.85（96.2–98.3） | 225（223–229） | 43.5% |
| DeltaBlue | 106（104–106） | 206（204–209） | 51.5% |
| Crypto | 205.5（202–206） | 242（241–242） | 84.9% |
| RayTrace | 159.5（158–160） | 398（395–399） | 40.1% |
| EarleyBoyer | 216.5（215–217） | 502（497–507） | 43.1% |
| RegExp | 90.65（87.1–91.8） | 67.6（67.2–68） | 134.1% |
| Splay | 475.5（472–480） | 833（832–883） | 57.1% |
| NavierStokes | 579.5（565–584） | 520（519–521） | 111.4% |
| Combined | 191（191–192） | 300（299–303） | 63.7% |

两个子项的所有新样本均高于既有 Boa 样本范围，其余六项及 combined 仍未超过。这是复用历史 Boa 的描述性比较，不是当前交错测量的单层归因；第二批三层相对 `72a4de30` 的累计独立控制仍须检查。后续只测改变的 Oxide 候选、需要的 Oxide 基线和累计组合。

八层累计实现已分提交落实到工作分支，首次生产 head `737410e6`。当时相对实验 `eaf23ba8` 的唯一文件差异是本报告；全部生产源码、manifest、lockfile 与测试源码逐项一致。对应的工作分支提交为 `ce30f34a`、`7f4722be`、`2d472ccd`、`e7e4e091`、`35ccc54e`、`b7f87c32`、`3c78a1f2`、`737410e6`。上述二进制与测量仍明确标识实验 `eaf23ba8`，没有伪造新 head 的构建回执。新的拒绝/未定候选没有混入此生产源码。

主线随后单独接受正确性修复 `2ef2f013`（实验 `92269743`）：CreateArray/Object/Variable factory 的 raw push 被拒绝时，在原 ObjectRef Drop/drain 后释放未提交 retained edge，并返回原 push 错误。成功路径仍为 checked retain→push→Drop；不采用已拒绝的 fresh owner 转移或新增成功 guard。5 项目标测试覆盖三 factory 的成功/拒绝、共享 borrow 下 deferred release、pending zero cleanup 和 retain 失败，全部通过。现有原版分数仍对应上述 eaf 二进制；未把该修复当作性能收益或声称已测最新主线。

1. 原始主体上的逻辑 profile 验证命中、删除的工作和保留的 owner。
2. `iterate_v8.py` 对八项及 combined 固定负载执行 A/A 与交错 A/B。复用首次冻结的负载，逐样本验证身份、输出和退出状态。
3. 有可信收益的候选验证硬件计数、内存、编译与代码体积，再跑原版完整 suite；其他子项独立检查回归。
4. 分 commit 集成，重新验证语义与累计性能。复用现有 Boa 三轮原版完整 suite 结果，八项与 Combined 分别判断，遵守用户不重复测量未变 Boa 的指示。

### 新数值机制的验收进展

静态 Opcode 候选 `eda47fab` 用环境匹配的 fresh plain 构建重试后，CPU 2 固定矩阵 144/144 样本有效。DeltaBlue −2.51%（A/A 1.69%）、RegExp −2.51%（1.91%）、NavierStokes −2.44%（1.38%）；目标 Crypto −1.48%（4.64%）、Richards +2.18%（2.66%）、combined −0.34%（1.07%）均未分辨。保留实验，不采用；原版与资源尚未追加。固定矩阵和代码体积增加不能单独证明完整净价值。

纯 bits `Number::compact` 候选 `1e081737` 已通过目标 9、profiling/host 2,301、默认库 2,059 项测试，clean plain/profile 回执与 eaf 的 source gate 配置匹配。真实五个 arithmetic compact sites 删除旧 clamp/NaN select/roundtrip；旧 ToInt32 保留。二进制 .text +1,568B，execute 栈 +32B，部分 native 消费者新增 outlined compact/Number::add 调用。八项原事件、storage、layout、指令计数不变；新增 compaction 分类只是逻辑覆盖。第二个完整 Setup/run/TearDown cycle 没有重置 NavierStokes 内部状态，也继续消耗原版确定性随机流，不能当作统一稳态频率。固定时间尚待验收，不采用。

随后 CPU 2 固定矩阵 144/144 有效：Richards +5.57%（A/A 2.26%）、Crypto +1.33%（0.73%）、RayTrace +1.97%（1.50%）；NavierStokes −2.65%（1.15%），combined +1.11%（1.90%）未分辨。拒绝当前候选，不为已失败的控制追加原版/资源或完整 Test262。`exact-i32-bits-compaction-gate-ledger.json` 保留全部样本与源码身份；删除机器工作没有替代完整净价值。

### 独立 PGO 构建技术：原版四轮与资源

`17c6c883` 只改变构建工具，运行期源码仍为 eaf 八层实现。Rust 1.88.0 / LLVM 20.1.5 下，22 个通用 scaling case × 两种规模完成 44/44 个预期输出验证，每个训练进程都产生独立非空 raw profile；V8 v7 未用于训练。训练凭据 SHA `c88451d8…fd9c9`、merged profile SHA `713c4bb0…2da`，generate/none/use 的源码、显式 target 和实际 codegen 参数逐项校验，唯一声明差异为 PGO。工具提交不是性能实现层，不把 profile-use 标成 plain。

CPU 2 上固定负载 144/144 样本有效，Richards/DeltaBlue/Crypto/RayTrace/EarleyBoyer/RegExp/Splay/NavierStokes 耗时分别下降 22.86% / 24.54% / 28.33% / 24.78% / 25.41% / 15.13% / 18.80% / 22.82%，combined −21.37%，均超过各自 A/A 筛选跨度。该矩阵使用自己的冻结负载，不与旧 source 矩阵跨减。

原版完整 combined ABBA–BAAB 四轮/引擎全部正常，九个标签均来自同一完整运行，主体 SHA 仍为 `777f2c2e…d9946`。历史 Boa 三轮复用，未重跑；所有样本保留。

| 子项 | 匹配 none 中位数（范围） | PGO 中位数（范围） | PGO 分数变化 | PGO / 历史 Boa |
| --- | ---: | ---: | ---: | ---: |
| Richards | 94.5（94.4–94.7） | 122.5（122–123） | +29.63% | 54.4% |
| DeltaBlue | 102.5（102–103） | 134.5（130–135） | +31.22% | 65.3% |
| Crypto | 204.5（204–205） | 286.5（279–292） | +40.10% | 118.4% |
| RayTrace | 157（155–158） | 208（206–210） | +32.48% | 52.3% |
| EarleyBoyer | 212.5（212–213） | 288（285–290） | +35.53% | 57.4% |
| RegExp | 88.65（87.9–89.4） | 103（102–105） | +16.19% | 152.4% |
| Splay | 475（468–479） | 639.5（626–645） | +34.63% | 76.8% |
| NavierStokes | 580（577–583） | 713（699–718） | +22.93% | 137.1% |
| Combined | 188.5（188–189） | 245（243–248） | +29.97% | 81.7% |

Crypto、RegExp、NavierStokes 的四个 PGO 样本均高于历史 Boa 全部样本。其余五项和 combined 仍低于，目标没有完成。PGO 的单项归因来自匹配 none 对照；历史 eaf Score 191 不是本次匹配对照，也不能把主线后来的 factory 正确性修复算进当前结果。完整证据为 `heldout-pgo-original-summary.json`。

固定 combined 整进程资源 ABBA 各两次：峰值 RSS 中位数 347,262→347,570 KiB（+308 KiB / +0.089%），cycles −22.17%、instructions −14.50%、branches −15.90%、branch-misses −28.46%；全部 perf 事件 running=100%，无 multiplex。它们包含 startup/Setup/run/TearDown/exit，是资源诊断，不是原版 Score 或延迟分布。

生成代码的交换也保留：.text −213,936B，.rodata +52,448B；`ready::run` 16539→177987B、栈 1320→3192B，独立 execute_frame 符号消失。编译器同时改变了多处 inline 与布局，不能把全部收益归给一处调用删除。训练、生成与再编译增加构建投入；构建日志耗时没有经过独立控制，不作编译性能结论。完整 PGO Test262 正在验收，当前尚未采用；进入主线时需在包含 factory 修复的实际新源码上重新训练与验证。

完整实验 PGO Test262 已结束，102,037 变体的 TSV/JSONL 冻结正文完全匹配，80,010/80,060 eligible 通过；实际 conformance runner flags、target、profile/source 前后 hash 均记录，runner 不是 timed CLI。针对实测 ready 栈变化，原有 mixed string search 与 typed array 回调的有限深度、溢出和恢复脚本在实际 none/use CLI、child main-thread 2MiB stack limit 下 4/4 通过。旧独立 execute 栈为 1176B，与旧 ready 同激活时的名义合计不同于 ready 单独增长，不能乘普通 JS 帧深度当作峰值。

工具工作流已分提交采用为 `e96a7f7f`。主线包含 factory 正确性修复，接下来用当前 clean source fresh generate→44 个通用训练脚本→merge→use/none，重新确认累计性能；旧实验 profile 不复用于新源码，也不把旧 Score 245 标为最新主线测量。

### 主线 fresh PGO 的累计验收

实际主线 clean `f7759050` 包含 factory 正确性修复，独立完成 generate→44 个通用训练进程→merge→use/none；没有复用 `17c6c883` 的 profile。训练回执 SHA `5f4c6606…f8da4`、merged SHA `333a004f…d15ec`、PGO CLI SHA `fe7265b9…2b8e`、匹配 none SHA `7a5e91ad…705aa` 固定该次构建。

CPU 2 fresh pilot/冻结负载下 144/144 固定样本有效。Richards/DeltaBlue/Crypto/RayTrace/EarleyBoyer/RegExp/Splay/NavierStokes 耗时分别下降 28.73% / 29.07% / 28.79% / 23.98% / 24.19% / 12.63% / 17.83% / 20.80%，combined −21.47%，均超出各自 A/A 筛选跨度。该矩阵与首个 PGO 实验的冻结次数不同，不跨矩阵相减。

四轮当前主线 PGO-only 原版完整 combined 全部正常，九个标签均来自完整运行，主体 hash `777f2c2e…d9946`、CPU 2、无并行构建/测试/采样；Boa 仍复用既有三轮，没有重跑。当前 Score 是累计证据，未重新跑 none 原版作本层 Score 归因；首个实验的匹配 none/PGO 归因独立保留。

| 子项 | 当前主线 PGO 中位数（四轮范围） | 既有 Boa 中位数 | 我们 / Boa |
| --- | ---: | ---: | ---: |
| Richards | 124（124–125） | 225 | 55.1% |
| DeltaBlue | 136.5（136–137） | 206 | 66.3% |
| Crypto | 270（264–275） | 242 | 111.6% |
| RayTrace | 207（206–208） | 398 | 52.0% |
| EarleyBoyer | 287.5（286–288） | 502 | 57.3% |
| RegExp | 103（102–103） | 67.6 | 152.4% |
| Splay | 631.5（629–632） | 833 | 75.8% |
| NavierStokes | 714（699–720） | 520 | 137.3% |
| Combined | 244（243–245） | 300 | 81.3% |

Crypto、RegExp、NavierStokes 的所有新样本均超过历史 Boa 全部样本，仍有五项和 combined 未超过。`heldout-pgo-integrated-root-original-summary.json` 保留全部原始输出身份及范围，未挑样本。旧 Score 245 的平衡实验与当前 244 的累计测量不能用于归因 factory 修复或时间阶段变化。

固定 combined 整进程资源 ABBA 各两次：峰值 RSS 中位数 347,372→347,038 KiB（−334 KiB），cycles −21.56%、instructions −14.47%、branches −15.87%、branch-misses −24.58%，四类计数 running=100%，无 multiplex。实际当前二进制 .text −214,320B、.rodata +52,440B；ready/execute 所选栈尺寸与首个实验相同。完整主线 PGO Test262 的 102,037 变体冻结正文完全一致，80,010/80,060 eligible 通过，源码和 profile 前后不变，实际 runner flags/target 逐项记录。接受该显式 PGO 构建技术；普通 Cargo release 配置未自动变成 PGO。

### 并行的下一组机制实验

- `f9befc96` 的有限标量谓词候选保留原槽位准备、checked owner、窗口/帧 generation 和清理边界，只跳过无回调 body 的 child header/Cold 使用和逐指令执行。目标/库测试、clean plain/profile 构建与八项逻辑对账完成：Richards 实际完成 10,670 次，frames_pushed、Cold reused、child Complete、direct retirement 各少同量；call_preparation 全部保持。其他七项既有事件完全相同，但真实 ready 栈 +48B、一般安装新增 outlined helper 调用，仍须完整计时控制，不采用为性能层。
- `8d36373a` 仅诊断 Crypto 的已完整化 full-identity cohort。3 个身份的后续 Number 成功读取上界 690,994/694,066（99.557%），generic indexed write attempts 上界 412,764/412,764；不是恢复命中、episode 或 Number 写成功。旧计数和 10 个完整边界观察完全相同。下一候选复用共享恢复算法扩大可选触发，新增非zero路径须独立审查 shape 缓存、近 MAX 的 checked retain、失败和发布合同。
- `8aec89a8` 的非测试、无 JS 布局诊断和 DWARF 字段测量证明 Map/Set 的 128B records 将 ObjectPayload 撑到 136B、ArenaSlot 撑到 272B。外置这两个成员预测每槽少 24B，必须测实际新布局和时间/RSS；Map/Set 新 Box 分配及 clone/GC 回滚是交换，不把 arena endpoint 容量投影当作峰值 RSS。窄候选不改普通对象、数组、函数或 arena 架构。QuickJS 2026-06-04 的 JSObject 保存独立 JSMapState 指针是实现参考，不能替 Rust 候选验收。

V8 的 [elements kinds](https://v8.dev/blog/elements-kinds) 和 [fast properties](https://v8.dev/blog/fast-properties) 说明表示专门化与连续访问的实现机会；这些是机制参考。V8 一般 holey 转换有单向限制，2025 年增加 `Array.prototype.fill` 例外，不能把它描述为任意写入后都会重建 packed 表示。我们的候选仍按当前实现的完整 own-data 证明和实际覆盖决定，不迁移 V8 的整个表示系统。

本页是进行中的实验记录；没有原版 Score 的新数据时，不更新历史分数为新结论。

### 本轮继续：拒绝结果与新的执行证据

`f9befc96` 已完成 CPU 2 的 144/144 有效固定样本：目标 Richards 耗时 +2.08%（A/A 0.84%），DeltaBlue +5.51%（1.85%），Splay +1.52%（1.19%）；combined +1.48%（1.75%）未分辨。拒绝该完整候选，不扩展同一 Prepared 表示，不追加原版、资源或完整 Test262 为失败控制背书。Crypto 的 A/A 跨度 31.01%，其中较长样本完整保留，不删样本再宣布结论。实际逐指令执行减少成立，反复识别、一般安装新增调用和 ready 栈增长属于需要区分的成本；尚未单独归因时间回归。证据为 `scalar-ordinary-leaf-gate-ledger.json` 与 `scalar-ordinary-leaf-validation/formal-rejection.json`。

当前 `f7759050` PGO 二进制的原版完整 combined 原生诊断已完成：56,591 个 cycles 样本、零 lost sample，DWARF capture 8192B。self 权重为 ready 30.34%、memmove 6.19%、FrameSlots::push 3.01%、dup_jsvalue 2.68%、RegExp standard_replace 2.29%、property_write dispatch 2.23%。PGO 改变了 inline 和符号边界，这些百分比不能跨版本相减或相加为可删除成本；采样包含启动、编译、Setup、warmup、timedRun、TearDown 和退出。打印分数只校验诊断运行，不替代已验收的四轮原版分数。证据 `heldout-pgo-integrated-root-native/` 保存原始 self/stacks、输出及身份。

新的三个机制分别处理存储表示、通用状态宽度与本地返回交接：

- 数组候选 `dc39cf80` 保持原 zero 触发和恢复算法，新增 nonzero 定义只复用已经存活、完整身份匹配且 count 有余量的缓存 shape。8 项目标及 2305 项 profiling/host 库测试通过；默认 Array/shape 目标 13/12 项通过。Crypto 新进入 1882 次、成功 5 次，1877 次在既有 insufficient-slots 检查早退；成功数不是对象数。实际 materialized Number reads 694066→3072，generic indexed write attempts 412764→0、dense scalar writes 451654→864418；materialization 66→68 的新增成本保留。其他七项既有逻辑 sections 相同。独立时间待验收，尚未采用。
- Map/Set 候选 `cbed0c0c` 只外置两个 records 成员。非测试 release 布局与 DWARF 实测 ObjectPayload 136→104B、ArenaSlot 272→240B，每槽少 32B（11.76%），替代早先的 24B 预测。目标 3、profiling/host 2300、默认 2059 项通过，八项原逻辑及 heap counts 相同。Map/Set 仍新增 128B Box 分配和 indirection；旧 memory category 未计这块 offheap storage，Splay arena capacity endpoint 少 32MiB 不是 RSS/峰值结论。真实 live-node stride 272→240，整体 .text +7648B，Map/Set constructor 栈各 +48B。时间与资源待验收。
- 独立非测试 Step 布局诊断保留真实枚举，shadow 仅将两个冷门 DefinitionInput 外置；真实 Step 184B，shadow 128B，Resume 32B、NativeStep 64B。此前实际代码中的部分 memcpy 传递 184B Step，但并非所有 memmove 都来自它。下一实验只改变这两个生产字段，包含 Box 分配、错误与 abandon 的交换，不扩展通用执行架构。诊断宽度下降尚未证明生产代码或时间收益。

本地返回的另一独立实验保留 canonical owner 复制和源绑定，仅研究在已发布的 GetLocal/GetLocalCheck 紧邻普通 Return 时减少临时 operand 搬运。GetArg、标量与需 callback/词法读取的分支保留原消费者路径；正确性和真实命中尚待验证。上述新候选均不更新当前主线的 Score 244 或三项领先的验收状态。

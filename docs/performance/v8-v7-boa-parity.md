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

私有属性位置缓存也另做表示实验：`67211c16` 借用当前状态并原地调整多态顺序，保留全部 identity/layout/epoch 守卫和既有冷却策略。15 项定向、2298/2057 库测试通过。release 暖读取 2641→2141B / 栈 600→376B，miss 4970→3138B / 栈 680→136B；整体 size text −1956B、data +48B。解释循环仍为 37433B / 7585 静态指令 / 1176B 栈，仅一条运行时字段偏移增加 104B（13 个 Proxy trap 缓存各新增一个借用标志）。这些是代码与表示证据；八项实际计数和 CPU 2 时间待验收，不声明时间或总内存收益。

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

八层累计实现现已分提交落实到工作分支，生产 head `737410e6`。相对实验 `eaf23ba8` 的唯一文件差异是本报告；全部生产源码、manifest、lockfile 与测试源码逐项一致。对应的工作分支提交为 `ce30f34a`、`7f4722be`、`2d472ccd`、`e7e4e091`、`35ccc54e`、`b7f87c32`、`3c78a1f2`、`737410e6`。上述二进制与测量仍明确标识实验 `eaf23ba8`，没有伪造新 head 的构建回执。新的拒绝/未定候选没有混入此生产源码。

1. 原始主体上的逻辑 profile 验证命中、删除的工作和保留的 owner。
2. `iterate_v8.py` 对八项及 combined 固定负载执行 A/A 与交错 A/B。复用首次冻结的负载，逐样本验证身份、输出和退出状态。
3. 有可信收益的候选验证硬件计数、内存、编译与代码体积，再跑原版完整 suite；其他子项独立检查回归。
4. 分 commit 集成，重新验证语义与累计性能。复用现有 Boa 三轮原版完整 suite 结果，八项与 Combined 分别判断，遵守用户不重复测量未变 Boa 的指示。

本页是进行中的实验记录；没有原版 Score 的新数据时，不更新历史分数为新结论。

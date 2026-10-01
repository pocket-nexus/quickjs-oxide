# V8 v7：追赶 Boa 的实验记录

## 目标与边界

目标是让**原版完整 V8 v7 运行输出的八个子项和 Combined Score 均稳定超过 Boa 0.22.0**。固定工作量时间、计数和汇编用于筛选与解释，不能替代这个验收条件。当前还没有达到目标。

保持 JavaScript 效果顺序、所有权、引用计数饱和规则、异常与栈限制、GC 服务边界以及 safe Rust。禁止针对 benchmark 名称或输入识别选择实现。候选分别提交、分别计时，通过后再验证累计结果。低覆盖、未兑现收益或维护成本过高的候选可以缩小或拒绝。

本轮对照是清理后的 `3d42040738b2a40569889a3fac4744c58bf2ae0c`，不是此前测量中的旧 main。历史分数与本轮独立收益不能混合归因。

## 当前证据

外部 suite 固定在 `2034d98fc8c5f8044e186267593f5d5ea5232caf`。Rust 1.88.0、普通 release、fat LTO、CGU=1 与独立 target 的回执标识时间用二进制；profiling 构建只提供机制计数。时间采样固定 CPU 2，停止并行构建、测试和其他采样。CPU governor 是 powersave，未锁频，保留 A/A 观察范围。

八项各完成一次原始 Benchmark 的 Setup/run/TearDown，全部通过完成标记校验。原生 profile 使用相同原版主体；不同引擎采用不同重复次数以获得样本，**这些不是跨引擎时间或 Score 比较**。

| 项目 | 已观察的成本 | 优先研究的机制 |
| --- | --- | --- |
| Richards | 属性选择、普通调用安装、owner 复制 | 属性消费、调用协议与重复认证 |
| DeltaBlue | 属性选择与 owner 复制、ready 调度 | 已持有事实的消费与交接 |
| Crypto | 12,534,392 次原地数字二元操作；数字 kernel | 保留整数表示、运算与访问路径 |
| RayTrace | 66,596 次快速 Array/Arguments 载体；66,600 次 Construct | 删除中间参数表示、普通构造前缀本地完成 |
| EarleyBoyer | 501,657 次 predicate 交接；267,379 次 Construct | 普通 instanceof、构造与比较的本地完成 |
| RegExp | 查询、恢复与 split 路径中的状态搬运 | 内建状态生命周期；作为独立回归控制 |
| Splay | 775,702 次 environment 交接；大量引用与堆写入 | 分支调度、绑定访问与写入职责 |
| NavierStokes | 1,355,222 次 binding 交接；数字循环 | 绑定访问和数字区域覆盖 |

### 竞争引擎的可借鉴之处

- QuickJS 2026-06-04 的 `build_arg_list` 直接建立最终 argv 并复制必要 owner；没有先建立通用 raw 参数快照的要求。
- QuickJS 的 `JS_OrdinaryIsInstanceOf` 在普通原型链上同步行走，遇到 exotic 才进入其他协议。Boa 0.22.0 的普通 `HasInstance` 同样直接调用普通算法。这支持研究本地完成；不能据此宣称我们的实现达到它们的速度。
- QuickJS 原生热点集中在解释循环；Boa 的缓存、操作分派、存储和引用管理更分散。V8 JIT 的短 profile 主要覆盖启动和编译，样本不足以归因稳定执行期的差距；单独保留 jitless 结果，不能混称为 V8 性能。

证据在仓库外 `/home/eric/.cache/oxide-v8v7-boa-campaign/`：`baseline-logical`、`baseline-native` 保存机器、源码、二进制与负载身份、原始输出和 profile。原始大文件不入库。

## 独立候选

| 提交 | 候选 | 机制证据 | 当前判断 |
| --- | --- | --- | --- |
| `ba57e9ec` | 借用驻留属性缓存状态 | read 生成代码 2641→1962 字节；栈 600→328 字节；每个 cache 增加 8 字节 | 144 个固定样本有效，Combined 时间比约 1.0023，在 A/A 范围内；尚未采用 |
| `8c53413f` | 快速 apply 直接投影最终 argv | RayTrace 中间 raw 复制 160,717→0；54,681 次排序缓冲扩容消失；必要的 6,679 次堆 owner 复制保留 | 144 个固定样本有效：RayTrace 耗时 −0.74%、combined −0.56%，均在 A/A 范围内；不宣称提速，尚未采用 |
| `daccb30b` | 普通 instanceof 在解释循环完成 | EarleyBoyer 397,585 次本地完成；predicate 501,657→104,072；native activation 501,788→104,203 | 144 个固定样本有效：EarleyBoyer 耗时 −21.18%、combined −2.93%，但 Crypto +11.11%，超出 A/A 2.95%；拒绝当前版本，缩小实现 |
| `ce30f34a` | Int(i32) 的 ToInt32 保留表示 | 数字二元 helper 的整数转浮点指令 40→29；代码 3912→4239 字节 | 144 个固定样本有效：Crypto 耗时 −5.46%，RegExp +1.57%（A/A 1.01%），combined +0.22% 未分辨；保留实验，需复核原版控制 |
| `dc2b0c15` | 普通 Base 构造的 lazy child 安装 | 独立实现及 2248 个 profiling 库测试通过 | 审查发现 miss 可能增加临时 retain，暂停验收并修正准入 |

计数减少只证明机制发生。缓存候选的 Combined 结果未分辨，不用其他层的收益替它背书。构造候选的测试通过也不能替代饱和引用计数与失败边界的合同审查。

`daccb30b` 的生成代码还显示解释循环栈帧增加 96 字节，新增的预算值跨整个解码循环存活并被重载。它说明非参与者确实付出了表示与寄存器成本，不能单凭汇编把 Crypto 的全部回归归因于此。下一版先恢复原解释循环，利用现有 `InstanceStep::start` 的同步 Complete 分支，重新测量收益和控制项。

## 验收顺序

1. 原始主体上的逻辑 profile 验证命中、删除的工作和保留的 owner。
2. `iterate_v8.py` 对八项及 combined 固定负载执行 A/A 与交错 A/B。复用首次冻结的负载，逐样本验证身份、输出和退出状态。
3. 有可信收益的候选验证硬件计数、内存、编译与代码体积，再跑原版完整 suite；其他子项独立检查回归。
4. 分 commit 集成，重新验证语义与累计性能。最终与同机、同原版 suite 的 Boa 做新一轮平衡采样，八项与 Combined 分别判断。

本页是进行中的实验记录；没有原版 Score 的新数据时，不更新历史分数为新结论。

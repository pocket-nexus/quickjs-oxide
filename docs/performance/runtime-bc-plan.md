# 从已采纳恢复栈继续 B：执行范围与检查点

阶段 A 已完成并验收。当前已采纳起点为 PR #89 `6db7e6a7`，
运行时代码与冻结二进制 `a9ba7b6a` 一致。旧 PR #88 的实现不继续扩展或
逐项移植；独立正确性修复和行为见证按 B0 审计单收割。

本次授权范围：**B0 → B1 → 检查点 1 → B2a–f → 检查点 2，之后停止**。
B3、B4、C 的顺序由检查点 2 重新决定。阶段 B 最终的零残留目标保留，
本次不宣称全阶段完成，也不以后续 C 抵消 B 的性能缺口。

## 一页技术合同

1. 内部存储拥有 JS 引用，不为了释放保存 Runtime。公共 root 保留公共生命周期。
   执行片段持有唯一的可变 State；内部 helper 使用该 State，短期事实从它借出。
2. 共享的是语义算法。现有已验证便宜入口与替代消费者同批交付。
   同步返回和同步抛错直接消费操作数、完成提交和清理。
   只有选定真实 JS 子调用、宿主交接或挂起后才发布持久 owner/恢复状态。
3. Outcome 是小的最终结果；Effect 包含已选定的调用和必要语义进展。
   复用现有 Completion、NativeStep/Resume 的可用部分，不让同步消费者
   先搭一套通用 operand/progress 表示，也不为所有 opcode 增加模式分支。
4. 持有 State 的局部 guard 直接使用它清理，并提供短期再借用。
   不通过 Runtime 重新借用 State；嵌套 guard 来自外层访问权。
   待移交的 owner 在任何可失败步骤前已有可遍历存储或 guard。
5. JS throw、资源失败完整清理；破坏性失败及 Rust panic poison runtime，
   停止访问和清理剩余不可信状态。作用域保证 State 先于外层 Weak 兜底释放。
6. 回调前结束布局/slot 借用，恢复消费已完成进展；持久缓存保留失效检查。
   所有修改经过同一可变访问权。发布、GC 服务位置、WeakRef kept-alive
   与失败优先级保持现有合同，不通过任意推迟释放获取收益。
7. 内部恢复/释放直接处理；外部 root 的协调队列保留。普通调用和返回
   留在解释循环内。尚未迁移的边界有编号、有计数，最后一个消费者迁走时删除。

## 分批交付

| 批次 | 范围 | 可检查的交付 |
| --- | --- | --- |
| B0 | 审计与冻结基线 | 两张正确性资产清单；编号残留与计数；本合同；数字阈值及热符号基线 |
| B1 | native 生命周期与实际 Array 消费者 | 当前 State 下的 owner/清理；同步消费者不建 Query/持久 progress；仅真实效果发布恢复记录 |
| 检查点 1 | 合同小复盘 | DeltaBlue Array 无效果 Query/记录为零；错误/poison 清理全过；R/D/NS Ir/Dw 增长 ≤0.1%；I1mr 仅作诊断 |
| B2a | 静态字段读取 | 普通完成直接提交；getter/Proxy 保留已选进展；删除对应适配器 |
| B2b | computed 读取与 key 转换 | primitive 同步完成；转换回调按顺序执行一次；heap 元素不因类型进入 Query |
| B2c | 静态字段写入 | 共享 Set 语义与直接帧消费者同批；替换 owner 直接移动 |
| B2d | computed 写入 | 数字索引不先恢复属性键；数组权限/hole/length 与转换顺序保留 |
| B2e | 追加与 define | canonical shape 发布共享；显式 atom/shape/值转移与失败回滚 |
| B2f | Reflect/Proxy Set | 实际 receiver、trap 选择和恢复各消费一次；同步非 Proxy 分支不建运输层 |
| 检查点 2 | 完整复盘并停止 | 新全八项 profile；A/起点/Boa 对照；残留编号关闭；决定后续顺序与布局/GC 实验是否集成 |

PR #89 收口为新 B 起点。B0、B1、B2a–f 分小的 stacked PR，各自带门槛，
通常不超过五个运行时提交。独立布局/GC 只允许最多两个机制实验，本次不集成。

## 测量和停止规则

复用 `iterate_v8.py` 冻结计划与 `fixed.py` 完整验证重放。
原 benchmark 主体、工作量及语义校验不改。普通 release、Rust 1.88、无 PGO。
`iterate_v8.py --repeat 2 --order abba` 仍包含 A/A，所以日常使用已有冻结
manifest 的 `fixed.py --repeat 2 --order abba`：每侧两次，不重新校准或重复 A/A。
计时不并行编译、测试或 profile；Boa 历史数据不重跑。

每个热循环可达类型/函数的运行时提交收集 R/D/NS Ir、Dw、I1mr，
不因称为冷迁移而跳过。调用归组包含 execute_frame 中被移走的成本，
不相加 inclusive counts。缓存是模拟值，不能换算成耗时。
热符号总尺寸增长超过 2%，或运输类型增长，必须检查生成代码与搬运。
全 text+rodata 相对 A 增长 ≤10%；固定工作量 RSS ≤5%；持续分配内存有界。
本轮 B 迁移的 Ir/Dw 硬门槛为增长 ≤0.1%，目标消费者还需兑现预期机制。
I1mr 是调查信号，不直接判失败。这是 B0 规则更正，适用于所有后续批次，
不是对 #91 的特例豁免。保留旧规则和原始测量的历史回执。
原生中位耗时在既有 A/A 噪声内标记未分辨，允许通过但不声称提速。
超出噪声的疑似回退仅针对相关项复核六对；区间完全落在噪声外的回退
必须修复，复核仍不能解释的超出噪声变化暂停该候选复盘。

每个实际消费者完成立即对前一已验证版本跑一个 ABBA 块；B2b、B2d、B2f
收口累计全八项及 Combined 对 A，当前起点同时作为性能下限。
既有 A/A 范围以内标未分辨。
两对同向且超过噪声先定位，临界项只复核相关项。Combined 不替单项背书。
阶段/检查点 2 正式 Score 用现有配对工具至少六对及 bootstrap 区间，
日常短测不冒充正式验收。正确性覆盖相关故障、CI fast、focused Test262，
完整阶段检查使用仓库原有向量和 oracle，不能更改判定规则。

B0 证据：[资产审计](runtime-b0-assets.md)、[全部见证索引](runtime-b0-test-assets.json)、
[残留编号](runtime-b0-residuals.json)、[计数与预算](runtime-b0-baseline.json)。

## 检查点 1 当前状态

B0 已合并。用户明确采纳 B1 运行时代码 `760443cb`（PR #91）。
全八项 Array 无回调 Query/mutation progress 计数为零；正确性和资源门槛通过。
R/D/NS Ir 分别 −0.032%/−4.084%/−0.038%，Dw 分别 −0.045%/−4.920%/−0.087%。
Richards I1mr +4.465% 保留为诊断；原生两对短测 −0.338%，在既有 2.780%
A/A 噪声内，未分辨。按照用户最新决定，不补跑六对作为 B1 采纳前置条件。
同步 Array Query/progress 全八项归零；DeltaBlue core.runtime_clone
从 544633 降到 121883。其余 native 族和全局残留仍需后续迁移。
完整数据与各提交回执见 [B1 检查点记录](runtime-b1-checkpoint.json)。

## B2 实施顺序

每项一个可独立审查与回退的 stacked PR，通常三个运行时提交。
每个提交保持可编译并通过相关测试；共享主体、实际消费者和旧通道删除
在同一个 PR 内完成。保留命名 IC、dense 读取和直接写入；不复制两套语义。

按用户最新要求，B2 的实现现在并行推进：B2a 负责读取效果和共享取消清理，
B2b 负责 computed/key，B2c–d 负责 Set，B2e–f 负责 define/Reflect。
各线使用独立 worktree 和提交，不在同一份源码上交叉修改。共享接口先对齐；
集成仍按依赖顺序组成上述小 PR，各批的机制与正确性门槛保持。
原生计时期间所有线暂停编译、测试和 profile，只有源码工作继续；
计时、冻结构建和性能归因统一串行。完成 B2 后仍在检查点 2 停止，
这次并行授权不展开 B3/B4 或集成 C 实验。

### B2a：静态字段读取（B2-R01）

1. 当前 State 完成 own/prototype/missing/getterless 和 primitive/string 同步读取；
   静态 key 使用发布代码已链接 atom，不创建 owning PropertyKey。
2. getter callee/receiver 直接进入内部调用安装；Proxy Get 保存已选 target、handler、
   trap 和进展，取消、抛错及发布失败通过 State 清理。
3. 接入 GetField/GetField2 及缓存变体；直接写结果、提交 PC、退休或保留 receiver；
   删除 selected-getter 公共 root 转换、Runtime 清理 guard 和对应旧适配器。

验收：同步记录创建为零；getter/trap 一次；最后 receiver 的子引用、别名、
部分输出失败和原始错误位置正确；warm IC 保持直接完成。重点短测 R/D/RayTrace。

### B2b：computed 读取与 key 转换（B2-K01）

1. primitive key 转换使用 State；合法 dense 数字索引继续直接读取所有值种类；
   字符串、Symbol、非索引 key 进入共享读取主体。
2. 对象 key 的 ToPrimitive 只在真实子调用时保存 owner/进展，恢复消费结果，
   不重复转换。共享转换 helper 的其他消费者继续公开列为残留。
3. 接入各 GetArrayEl 变体与转换恢复，保持 receiver/key 的保留表示；
   删除该路径 boxed operand 运输和公共 key/root 中间表示。

验收：heap dense 值不创建 Query；GetArrayEl3 直接 Int 与转换后 key 的表示保持；
-0、非整数、大索引、Symbol、nullish、转换抛错/修改原型、typed 特殊索引正确。
重点短测 D/RayTrace/Splay；检查 B2a 计数不回升。

### B2c：静态写入（B2-S01）

1. State 下完成 Set 的 target/prototype/receiver 选择；同步拒绝、数据写入、
   内部阶段推进不建 SetResume；明确完成、抛错和真实效果。
2. PutField 使用已链接 atom，允许移动时将源 owner 转交存储并直接退休旧值；
   源值仍需保留或别名时取得必要 owner，保留现有循环内入口。
3. setter 交接保存已选 callee 和实际 receiver，直接进入内部调用；
   删除静态写入中的 ObjectRef/PropertyKey 包装与 Runtime 持有者。

验收：不先搭 SetOperands；heap 替换、自赋值/别名、继承 setter、只读属性、
primitive receiver、strict/sloppy 正确；准入失败保留输入，提交后不重放。
重点短测 R/D/RayTrace，并检查写入、retain/release 和 Dw。

### B2d：computed 写入（B2-S02）

1. 合法 dense 数字索引直接使用 State 存储内核，不先生成属性键；
   immediate 与 heap 替换/追加使用同一所有权合同。
2. 复用 B2b 转换和 B2c Set；转换后只写一次，移除该路径 ConvertedWrite Runtime。
3. 同步处理数组权限、hole、length 和 prototype 条件；仅真实值转换/回调保存记录。

验收：替换/追加/hole、冻结/密封、不可写 length、索引原型 setter、非索引 key 正确；
typed 转换回调后重取 view，保留 detach/resize；缓存失效通知保持。
重点短测 NS/D/Splay；float 写入无新增 key 转换或持久记录。

### B2e：追加与 define（B2-P01）

1. 使用共享 canonical shape/slot 发布内核，明确 atom/shape/value/accessor 边转移；
   保留容量增长与直接追加入口。
2. 内部 descriptor 存字段和 JS owner，使用 State guard/执行存储清理，不存 Runtime；
   同步定义不建恢复记录。
3. descriptor getter、Proxy define、对象值 length 转换按既有顺序执行，
   恢复消费已完成字段读取与转换；删除对应旧通道。

验收：数据/accessor 转换、Absent/undefined、Symbol/key 顺序、不可扩展和权限正确；
故障覆盖 retain/扩容/shape 发布/部分 descriptor 获取；提交前失败保持状态，
数组缩短失败保留规范要求的部分删除；Object/Reflect define 返回分别正确。
重点短测 D/RayTrace/EB/Splay，并检查分配和资源。

### B2f：Reflect/Proxy Set（B2-X01）

1. 普通 Reflect Set 复用 B2c–e，保留实际 receiver，直接返回 accepted/rejected。
2. Proxy trap 查找、缺失 trap 转发、结果和 invariant 使用 State；
   必要 owner 进入执行存储，不保存 Runtime。
3. 接入 VM/Reflect/Proxy，删除 root 转换及旧 SetInputs/pending 清理通道。

验收：缺失/不可调用/revoked/nested Proxy、false/invariant/throw、不同 receiver、
自引用、trap 修改 target、放弃恢复正确；strict VM 与 Reflect 返回保持各自合同。
无实际回调的同步转发不建持久记录；行为/差分用例及 R/D/NS 检查前五批计数。

### 残留、正确性与最终停止

每批关闭对应消费者编号。共享类型仅在最后一个消费者迁走后删除；
其他族的残留继续公开，不以单路径归零冒充类型或全阶段归零。
每提交相关测试；每 PR 收口 CI fast、架构检查和 focused Test262，
覆盖缓存失效、别名、故障、throw/poison/放弃及错误位置，复用 B0 行为资产。

B2f 后重新采集全八项 profile，分别对照 A、#89 起点和 B1；正式原版 Score
全八项及独立 Combined 至少六对及 bootstrap 95% 区间，列历史 Boa 对照。
审计六个编号、Runtime owner/State 重借用/internal deferred/root 适配器和旧协议，
报告每批/累计机制、时间、资源和未分辨结果，然后停止，重新决定 B3/B4/C 顺序。

布局校准不阻塞 B2：仅在需要调查时使用同一 B0 链接输入、lld 未打乱对照及
seed 1–5 收集 R/D/NS。正式二进制当前为 GNU ld，lld 范围不能套成其数值门槛；
不选择有利布局、不加 padding。硬件计数辅助归因，计时不并行构建/profile。

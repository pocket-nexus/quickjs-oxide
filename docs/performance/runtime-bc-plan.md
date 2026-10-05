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
| 检查点 1 | 合同小复盘 | DeltaBlue Array 无效果 Query/记录为零；错误/poison 清理全过；R/D/NS tripwire 不增长。失败先修合同，禁止进入 B2 |
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
检查点 1 的 tripwire 默认要求相对 B0 三指标不增长；不临时放宽。

每个实际消费者完成立即对前一已验证版本跑一个 ABBA 块；每批累计全八项
及 Combined 对 A，当前起点同时作为性能下限。既有 A/A 范围以内标未分辨。
两对同向且超过噪声先定位，临界项只复核相关项。Combined 不替单项背书。
阶段/检查点 2 正式 Score 用现有配对工具至少六对及 bootstrap 区间，
日常短测不冒充正式验收。正确性覆盖相关故障、CI fast、focused Test262，
完整阶段检查使用仓库原有向量和 oracle，不能更改判定规则。

B0 证据：[资产审计](runtime-b0-assets.md)、[全部见证索引](runtime-b0-test-assets.json)、
[残留编号](runtime-b0-residuals.json)、[计数与预算](runtime-b0-baseline.json)。

## 检查点 1 当前状态

B0 已合并。B1 候选运行时代码为 `760443cb`（PR #91），尚未采纳。
全八项 Array 无回调 Query/mutation progress 计数为零；正确性和资源门槛通过。
但 Richards 的 I1 缺失相对 B0 增长 4.465%，同二进制复核仍为 4.465%。
既定零增长 tripwire 未通过，因此停在检查点 1，B2a–f 尚未开始。
门槛保持不变；不能将这批实现记为已验收或用其他子项的收益替代该检查。
完整数据与各提交回执见 [B1 检查点记录](runtime-b1-checkpoint.json)。

# 从已采纳恢复栈继续 B：执行范围与检查点

阶段 A 已完成并验收。当前已采纳起点为 PR #89 `6db7e6a7`，
运行时代码与冻结二进制 `a9ba7b6a` 一致。旧 PR #88 的实现不继续扩展或
逐项移植；独立正确性修复和行为见证按 B0 审计单收割。

已完成：**B0 → B1 → 检查点 1 → B2a**。B2a 之后的 B2b–f 原顺序已停止，
由下文“[B2a 之后：按成本中心推进](#b2a-之后按成本中心推进)”替代：
热路径条目同时完成迁移和删除工作，以每操作指令数验收；冷路径只迁移、不回退。
最终目标不变：无 PGO 原版 V8 v7 八项及 Combined 全部超过历史 Boa；
阶段 B 的六项零残留硬门槛保留，在第 8 项统一验收。

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
| B2b–f、检查点 2 | 已停止 | 由按成本中心推进的第 1–8 项替代；写入与 define 并入第 3、1 项，computed/Reflect/Proxy 并入第 7 项 |

PR #89 收口为新 B 起点。B0、B1、B2a 已完成；后续条目各自为小的 stacked PR，
通常不超过三个运行时提交，只改本条目的消费者。

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

## 已完成的 B2 部分

### B2a：静态字段读取（B2-R01，已采纳）

1. 当前 State 完成 own/prototype/missing/getterless 和 primitive/string 同步读取；
   静态 key 使用发布代码已链接 atom，不创建 owning PropertyKey。
2. getter callee/receiver 直接进入内部调用安装；Proxy Get 保存已选 target、handler、
   trap 和进展，取消、抛错及发布失败通过 State 清理。
3. 接入 GetField/GetField2 及缓存变体；直接写结果、提交 PC、退休或保留 receiver；
   删除 selected-getter 公共 root 转换、Runtime 清理 guard 和对应旧适配器。

验收：同步记录创建为零；getter/trap 一次；最后 receiver 的子引用、别名、
部分输出失败和原始错误位置正确；warm IC 保持直接完成。重点短测 R/D/RayTrace。


## B2a 之后：按成本中心推进

### 为什么改变顺序

- **只迁移、保留全部检查，收益约等于零。** A（调用/返回留在循环内）、#89（直接写入）、
  B1（Array 同步完成）的收益都来自删掉的工作；B2b 的迁移没有删掉工作，原生短测也无法分辨。
- **每操作指令数是主指标。** 它是确定性的，差距是几倍而不是 0.1%。
  原生短测在当前环境下分辨不了 5% 以下的变化（同一二进制 DeltaBlue 曾在两次会话间相差 46%）。
- **差距最大的是具体操作的固定成本**（B1 主线测得的净指令/次，oxide 对 QuickJS）：
  对象字面量约 9×、`new` 约 7×、写属性约 19×、读对象数组元素约 20×、函数调用约 7.7×，
  一对 dup/release 约 300 条。分配的逐行账本见
  [对象分配成本账本](runtime-allocation-ledger.md)。

### 解释循环实验（E）的结论

`perf/verified-loop-experiment` 上的 E0–E3 不合入主线，E4 不实施。保留 E0 的
N/2N/4N 斜率探针和实际分派计数作为测量工具。结论留给第 5 项：

- 数值循环每轮 12 次分派，E3 后约 73 Ir/分派（QuickJS 约 14）。
- 剩余成本主要是取指解析（字流读取、PC 推进、解码与 tag 选择约 28 Ir/分派）
  和巨型解释函数的寄存器压力（E3 栈帧 2,008 字节；数值循环 24% 的指令访问栈）。
- 在巨型函数内部做局部优化会被溢出和数据缺失抵消：E3 的指令减少伴随
  cycles 上升（DeltaBlue +8.6%、Crypto +11.5%，L1d 缺失 +13–15%）。
- 第 5 项必须先拆小热循环（热状态为局部变量，冷路径显式同步状态），
  再单独验证预解码；不在巨型函数内继续做局部实验。

### 新顺序

| 顺序 | 条目 | 主要影响 | 验收（净指令/次） |
| --- | --- | --- | --- |
| 1 | 对象分配：字面量、`new`、define | RayTrace、EarleyBoyer、Splay、DeltaBlue | 字面量 11,568 → ≤3,000；`new` 12,186 → ≤5,500（构造调用部分随第 4 项继续下降） |
| 2 | 引用计数与已认证句柄的快速路径 | 全部 | 一对 dup/release 约 300 → ≤30 |
| 3 | 属性写入与写缓存（原 B2c、B2d） | Richards、DeltaBlue、RayTrace | 写属性约 968 → ≤150 |
| 4 | 调用与返回的剩余成本 | Richards、DeltaBlue、EarleyBoyer | 函数调用约 2,100 → ≤700 |
| 检查点 | 全八项及 Combined 六对正式 Score，对照 A、#89、B2a 与历史 Boa | — | 按剩余差距决定 5、6 的先后 |
| 5 | 解释循环结构：先拆小热循环，再验证预解码（一轮，有时间上限） | 全部 | ≤40 Ir/分派，Dw 2–3/分派 |
| 6 | GC 扫描与对象记录体积 | Splay、EarleyBoyer | 由检查点 profile 决定 |
| 7 | 其余迁移：computed 读取、Reflect/Proxy Set（原 B2b、B2f）、B3、B4 | 冷路径 | Ir/Dw 不回退 |
| 8 | 删除旧协议与零残留验收（原 B5） | 架构 | 六项硬门槛归零 |

第 1 项的基线是 B2a `f2501839` 以 Rust 1.88 普通 release 构建的实测值；第 2–4 项的基线来自 B1 主线，
开始该项前在当时的已采纳版本上用同一探针重测。目标是待校准的操作预算，
不直接换算成 Score，也不保证超过 Boa。

### 第 1 项：对象分配（任务清单）

逐行账本（[runtime-allocation-ledger.md](runtime-allocation-ledger.md)）把
字面量的 12,215 Ir/迭代和 `new` 的 12,833 Ir/迭代（空循环 647）分到互斥的阶段与机制。
任务按账本中的成本块组织，每个任务对应可删除的工作：

| 任务 | 删除的工作 | 账本来源 | 预期 |
| --- | --- | --- | --- |
| 1a 追加缓存 | 每个 DefineField/PutField 追加站点缓存（父 shape → 后继 shape、slot 下标、原型链有效性）。命中时不做 `canonical_successor` 的嵌套哈希、`record_transition` 重写、临时 shape 引用的 retain/release 与 `apply_cleanup`、O(n) 前缀比较、缺失属性的原型链遍历 | W 全部（4,741）；D 中 slot、shape、边与 atom、引用计数四类（约 3,960） | 每个追加 ≤150 |
| 1b 字面量字段留在循环内 | 普通数据字段定义不再经过 `cold::dispatch`、`start_public_field`、公共 `Runtime::try_define_owned_property`、`PropertyKey`/`ObjectRef`/描述符包装，不触发帧 materialize；使用 1a 的追加内核 | D 的包装、准入与驱动（约 2,830）和 R（1,474） | 字面量减少约 4,300 |
| 1c 分配路径 | 空对象 shape 按 realm/原型缓存；发布时不重复完整布局校验；分配期间不建临时 owner 再释放；减少 `ObjectData` 搬运 | A（约 1,300） | ≤400 |
| 1d 释放路径 | 释放对象时不建边快照 `Vec` 与 atom `Vec`，不重复校验身份三次；保留零引用队列语义 | F（913） | ≤250 |
| 1e `this.x = …` 的值传递 | 追加字段时移动源 owner，不做 insert_copy/rotate 与额外 dup/retain | `new` 的 S（1,172） | ≤200 |

构造调用本身（C，1,979）归第 4 项；解释循环自身（I）归第 5 项。

提交顺序：1a（共享追加内核与站点缓存，先接 `this.x = …`）→ 1b（字面量消费者）
→ 1c、1d → 1e。每个任务一个 PR，不超过三个运行时提交。1b 同时关闭字面量定义路径
上的公共 root 中间转换与 Runtime 包装残留，对应 B0 残留清单中 B2e 的编号。

验收：

- 账本工具（`docs/performance/probes/allocation/ledger.py`）重跑，相关阶段达到预算，
  其他阶段不增加；斜率 N→2N 与 2N→4N 一致。
- R/D/NS 及 RayTrace、EarleyBoyer、Splay 的 Ir、Dw 同时下降。
- 第 1 项收口时跑全八项及 Combined 的一个 ABBA 块，对照 B2a。
- 正确性：shape 缓存失效（原型变化、不可扩展、冻结、字典模式、accessor 原型属性）、
  setter 与 Proxy 原型、getter 中修改 shape、追加失败回滚、GC 与 owner 计数。

### 执行规则

- 每个热路径 PR 同时交付迁移和删除的工作；冷路径 PR 只迁移，要求 Ir/Dw 不回退。
- 每个提交：相关正确性测试，加相关探针与 R/D/NS 的 Ir、Dw。
- 只在条目收口、检查点，或 R/D/NS 的 Ir 变化超过 5% 时跑原生 ABBA。
- 完整回执和阶段文档只在条目收口时出一次。
- 技术合同、B0 残留清单、poison 合同不变；每个条目关闭对应残留编号。

### 从 B2b（#94）收割的内容

保留 State 下原始值转属性键的内核（`property_key_atom_from_primitive`、
`primitive_to_js_string`），单独提交；保留能通过 JS 可观察行为验证的边界测试。
放弃其 computed 消费者、`FrameRare` 新字段、共享转换运输改造和越界的
Number/typed-element 迁移。第 7 项重做 computed 读取时：等待状态先统一为一个枚举，
对象 key 的 ToPrimitive 复用现有转换等待，每批只迁移自己的消费者。

### 残留、正确性与最终停止

每个条目关闭对应消费者编号。共享类型仅在最后一个消费者迁走后删除；
其他族的残留继续公开，不以单路径归零冒充类型或全阶段归零。
每提交相关测试；每 PR 收口 CI fast、架构检查和 focused Test262，
覆盖缓存失效、别名、故障、throw/poison/放弃及错误位置，复用 B0 行为资产。

第 8 项验收六项硬门槛：内部 Runtime 强引用、State 重借用、内部 deferred、
普通调用返回外退、公共 root 中间转换，以及迁移适配器和旧实现全部为零。
原版八项及 Combined 未全部超过 Boa 时，报告真实差距，不宣布目标完成。

布局校准不作为门槛：仅在需要调查时使用同一链接输入、lld 未打乱对照及
seed 1–5 收集 R/D/NS。不选择有利布局、不加 padding。硬件计数辅助归因，
计时不并行构建或 profile。

# #52 起点的执行优化路线

状态：M1 在 `3d98c0a8ee065e5c71607b46bd0059282c6d7e87`
实现；PR #53 随后扩展了 M2 的直接目的地、数组更新和比较分支，
并在 `6f1de9c5` 发布执行字 continuation、CFG lexical 初始化事实及
V8 v7 数组乘积更新来源。cell/shape 类型化 arena 已独立实现；
其余工作流与 M3 仍是提案。PR #53 的直接基线是 PR #52
`4287e8c6019933289f3a06e703aea15bf79c9f11`；更早的
`996663f771afdabdc69d52c94bd4d2fb392e27b1` 是原设计起点。
M1 的历史验证和测量见[收据](receipts/m1-numeric-region-2026-09-28/README.md)。当前实现见
[架构](../architecture.md)，设计约束见[原则](principles.md)，测量方法见
[测量协议](measurement.md)。本页落实
[Rust Design Patterns Examples 会话](https://chatgpt.com/c/6ab8fc22-1d2c-83ee-b108-5d89db1efba2)
最后一轮以 #52 为新起点的建议；示例类型属于内部设计，不是公开 API。

## 1. 方向与工作流

在最早有效的阶段确定事实，把结果编码进执行表示，让消费者在事实有效期内
直接使用它。优化单位是有意义的操作或有界区域。第一项交付把编译、发布、
借用和提交连通；后续工作按依赖扩展，不要求先建完整优化框架。

| 工作流 | 交付目标 | 顺序与依赖 |
| --- | --- | --- |
| 执行规划 | binding、effect、ownership 与局部 dataflow 决定显式操作数、目的地和执行形态 | M1/M2 已覆盖四种有限形态；数组乘积来源共享 producer fact，继续减少精确形态匹配 |
| 作用域执行 | 准入返回可消费的目的地访问；数组事实在相应借用内有效 | local 提交与现有 own Number 元素更新已交付；继续复用准入契约 |
| continuation 与调用 | 携带已知 fault/fallthrough 位置，按发布的布局安装帧，按被替换 owner 选择释放边界 | 四种数值操作已使用执行字 continuation 与 scalar 替换准入；通用调用布局仍独立推进 |
| 自适应操作 | 等布局 opcode family、直接 cache-site ID、有界重试、向回落传递已有 miss 事实 | 先确定操作的所有权与 continuation 契约，再接入生产适应机制 |
| 存储 | 数值 backing 与 cell/shape 类型化 arena，保留完整身份及回收边 | cell/shape arena 已实现；数组 backing 仍复用 M1/M2 操作契约推进 |
| 嵌入执行 | 有界 safepoint、activation scratch、只读 program image、明确语义的 native kernel | 扩大区域前定义中断/计费契约；冻结映像明确 linking 与自适应状态边界 |

先保留中央 dispatch、通用 `JsValue`、显式帧栈和现有数组存储。
内部 IR、frame layout、operand protocol 与存储接口可以跨模块调整。
收益来自删除临时 owner、重复地址解析、堆认证、帧发布及通用值搬运。

## 2. 三十项模式的去向

模式编号沿用会话。它们是组合设计及可比较的备选项，不是三十项强制功能。

| # | 模式 | 本路线中的用途或前提 |
| --- | --- | --- |
| 1 | 提前解析与特化 | M1 在编码前确定 binding 与操作数数据流 |
| 2 | 验证后发布 | 明确每项静态保证的生产者、消费者及失效边界 |
| 3 | 编译器可见的边界证明 | 已检查的小窗口与安全 disjoint borrow；用机器码确认检查是否消失 |
| 4 | 只表示合法计划 | 分开操作形态及 admitted destination，避免 flag/Option 组合提交器 |
| 5 | 紧凑码与冷元数据 | 单一执行字流，诊断映射与适应状态按访问需求分离 |
| 6 | 寄存器/累加器/栈缓存 | 先在现有流内使用显式数据流，消除临时 push/pop |
| 7 | 中央派发 | 初始执行后端；减少必要工作与 driver 退出 |
| 8 | 预解码 handler | 操作边界稳定后比较；间接函数调用不等于 direct threading 或保证尾调用 |
| 9 | Superinstruction | 从数据流、效果与用途选择区域，实际删除中间值处理 |
| 10 | 一次准入与提交 | M1 的 frame destination 与数组读取契约 |
| 11 | 替换式 quickening | 后续 family 必须保持布局、操作数、逻辑入口/出口和回落契约 |
| 12 | Shape/inline cache | 缓存当前 receiver 的属性位置；读写各自证明必要条件 |
| 13 | 有界多态与重试 | 仅参与适应的站点承担计数；限制链长、状态空间和重复失败成本 |
| 14 | 同质数值存储 | M1 后增加 backing；显式表达 holes、descriptor 和表示转换 |
| 15 | 紧凑值 | 先减少数值区域中的 `JsValue`；NaN boxing 留作独立表示实验 |
| 16 | 类型化 arena/冷热分离 | cell/shape 已移入独立紧凑存储，保留完整身份与回收；其他冷热拆分未实施 |
| 17 | 借用和移动 | ownership-aware liveness；只为真实新增 owner retain，保持可观察释放 |
| 18 | 连续帧与参数窗口 | 用发布的布局和实参用途削减安装、padding 与分类工作 |
| 19 | Open/closed upvalue | 先用精确 capture map；另证 sibling sharing、unwind、eval、arguments 与暂停生命周期 |
| 20 | 窄 VM 能力 | 数值/属性操作只取得所需访问，限制可调用 JS 或修改布局的能力 |
| 21 | Scratch 复用 | activation/mark 管理逻辑所有权，允许宿主重入下的嵌套使用 |
| 22 | Safepoint 与计费 | 覆盖控制流循环及长 native 工作；逻辑工作量独立于物理 opcode 数 |
| 23 | 紧凑错误与冷诊断 | 区分无副作用 miss、内部错误、guest throw 与 suspension |
| 24 | Frozen image | 静态字、常量描述与布局独立于 instance 的 atom、root、cache 身份 |
| 25 | 编译期诊断策略 | plain 构建没有诊断 hooks；限制策略组合带来的代码实例数 |
| 26 | Native 批处理 | 明确语义的 typed buffer/UI kernel，保留 alias、回调边界与轮询 |
| 27 | Guest 正确算术 | 复用 Number；溢出提升、负零、NaN、运算顺序保持 |
| 28 | 验证时机与惰性翻译 | 已知程序优先构建期发布；惰性翻译须证明启动/内存收益并限制副本 |
| 29 | 单一指令规格 | 编码、验证、effects、ownership、诊断与转换规则共享事实来源 |
| 30 | 参考执行与固定工作量 | 同一解释器中的 generic 对照、语义差分、plain 性能与独立资源账 |

函数指针派发、累加器缓存、内联策略与 PGO 在操作成本结构稳定后比较。
当前 release 已有 fat LTO/CGU=1，不能把“启用 LTO”列为新增优化。
NaN boxing 不得缩短 generation；固定 256 槽帧不是嵌入式默认布局。
Frozen/adaptive 策略在函数或 activation 边界选择，不给每条指令增加模式判断。

## 3. M1：一个完整的数值区域（已实现）

本里程碑交付以下真实 JavaScript 语句从编译到提交的完整路径；本节保留
设计及验收契约，实际边界以[架构](../architecture.md)和[收据](receipts/m1-numeric-region-2026-09-28/README.md)为准：

```javascript
sum += array[i] * scale;
```

### 编译与发布

在 lowering 已有 binding/capture 信息和控制流边界基础上，为基本块建立临时
数据流表示，随后完成 operation selection、layout/relocation 和 `ExecCode`
发布。规划 IR 只在编译期间存在；不成为第二个运行时解释器。

规划以临时值身份和 use 关系表示：读旧 sum、取 array[i]、乘 scale、相加、
写回 sum。当前 `PotentialEffects` 记录可能调用 JS、分配和抛错；M1 的
可观察释放与布局安全来自受限指令形态和动态准入，而不是通用的 ownership
或 layout refinement 证明。effects 用于选择和区域形成，不转化成主循环
逐条执行的一组 Boolean 检查。扩大覆盖时应复用事实及验证契约，不能让
每个新操作族重新构造同一份证明。

M1 支持表达式结果被丢弃的语句；目的地为可写、未捕获的普通 local。
array 来自直接 local/argument；index、scale 来自直接 local/argument 或
数值常量。拒绝 capture、eval/dynamic environment、mapped arguments、const
目的地及无法证明初始化/稳定存储的 binding。拒绝路径继续原有语义。
capture 与 lexical/TDZ 事实传到选择阶段；存在 dynamic binding 时按
函数级保守拒绝，不能以局部 guard 假定其存储稳定。

发布契约包含显式操作数来源/目的地、generic 入口、成功 continuation、
source/fault 映射与逻辑 stack peak/delta。编码沿用单一字流，generic 回落
也在该流内；成功跳过其实现词，失败进入其普通语义，不能重复尝试本区域。
layout 完成后验证范围、所有进入区域的控制/异常/恢复边、资源上界和出口。
不允许从中间进入已消除临时状态的成功路径；不能确定安全的区域不特化。
同一指令规格提供选择、编码、验证和执行所需事实，handler 不重读 generic
序列来推导算术或目的地。操作数布局由编码层表达，不能塞入未检查的通用 flag。

### 准入、计算与提交

内部增加 admitted numeric destination：持有已解析的可写 Number 槽及原值，
提交消费该访问。它不是只带 slot index 的批准凭证，也不暴露任意 Runtime
回调。先取得当前输入值，再安全借用目的地；sum 与 i/scale 同槽时读取旧值。
array 的 owner 在 frame 中保活，读取句柄不制造新的拥有式 operand。

动态准入检查 sum、i、scale 以及元素的 Number 类型；index 仅接受
`0..=u32::MAX-1` 内的精确整数 Number（`-0` 按索引 0），不作截断或取模。
字符串 key、越界/非整数/非有限 Number 和需要转换的值走 generic。
数组必须是 genuine Array 的 own Number data element；dense 与 materialized
均可，冻结 data property 可读。hole、原型可见元素、accessor、Proxy 与其他
对象继续普通 `[[Get]]`。

一次 heap 读取借用内认证对象与存储并复制 Number。借用结束后不保存 raw
slot 位置或堆引用；Copy Number 与仍在作用域内的 frame destination 足以计算
并提交。本次没有数组写入，不需要再获取 heap 借用。
按 `element.mul(scale)` 后 `old_sum.add(product)` 执行，使用现有 Number
语义，不使用 `mul_add`、wrapping 替代或浮点重结合。

成功路径只写 sum 一次，不 push/pop 中间结果、不 retain/release、不分配、
不调用 JS、不物化观察帧；提交不重新解析或检查已准入的目的地。
所有可失败准入必须发生在写入前，miss 保持 slots/depth/heap/owners/PC 状态。
需要实际抛错的路径通过 generic 执行保持位置与顺序，不能把执行后的错误
改成 miss 再重放。帧安装/发布证明承接逻辑容量要求；容量不满足时保留原错误
行为，不能只把检查改为 `debug_assert!`。

### 对照与后续

加入仅测试使用的 selection 开关，让同一源程序在同一个解释器内走普通编码，
与优化编码比较最终值、异常身份/位置、效果日志和资源清理。plain 默认执行
不携带该开关；内部 planner/admission 类型不改变公开 embedding API。
测试检查发布的操作，不能只从源码形状或结果相同推断已命中。

M2 已在同一来源、binding、准入及发布契约上扩展数组读的直接目的地、
已有 writable own Number 的元素更新和直接比较分支。更新把读、计算、写
放在一次可变 heap 借用内；比较的命中路径直接选择 continuation。M2 支持
同一基本块内已初始化、未捕获的 lexical `let` 目的地和 `const` 来源，
其他不确定 binding 回落普通指令。每种操作有精确消耗区间、独立 opcode、
描述符形态及对应的原词回落入口。当前更新和比较选择仍使用有限的连续形态
识别，共用静态来源与运行时准入；不能称为任意表达式规划。新测量见
[M2 收据](receipts/m2-numeric-operations-2026-09-28/README.md)。
读资格与写资格分开，数组 alias 的动态事实仅在借用期有效。M3 再接适应
family 和数值 backing。
cell/shape arena 已独立于完整优化器完成；continuation 的已知位置传递仍可另行推进。
扩大区域及长 kernel 前先确定精确逻辑耗尽还是块级计费、轮询上界及 root 发布点。

## 4. 验证与完成条件

| 范围 | 必须覆盖 |
| --- | --- |
| 编译/发布 | dataflow/use 选择、未初始化/捕获/动态 binding 拒绝、宽操作数、非法中间入口、逻辑容量与出口映射 |
| Number | Int 溢出提升、Float、负零、NaN、无穷、乘加顺序与结果位语义 |
| 数组 | dense/materialized/frozen 自有值；hole、原型 getter、accessor、Proxy、非 Number、非法 index 的回落 |
| 可观察顺序 | getter/转换只执行一次，旧 sum 在 RHS 副作用前读取，TDZ/const/eval/arguments 行为、throw 身份及位置 |
| 生命周期 | sum/input alias、基线容量错误、guard miss 无修改、异常展开及 generator/async 区域外恢复 |
| 执行成本 | 成功无中间 operand owner、无重复目的地解析、一次数组认证/读取、无新增 driver 退出；检查实际机器码 |

按[测量协议](measurement.md)用同工具链重建当前 #53 head、#52 parent 和 Candidate，分别
记录编译/分配、固定 guest 工作量执行、冷/热/失败覆盖、字流及元数据体积、
frame/heap/峰值内存。profiling 解释机制，plain 产物证明性能；现有整进程
runner 含编译与退出，不能声称是隔离后的执行耗时。

运行受影响测试、当前 CI feature/MSRV 矩阵及集成源码的 focused/full Test262；
保持冻结结果正文，不为性能改动放宽语义向量。实际命令、源码和结果见
[M1 收据](receipts/m1-numeric-region-2026-09-28/README.md)。
完成条件是语句贯穿这套设计、目标工作确实消失，并有可归属的测量；仅增加
opcode、proof wrapper、宏或测试通过均不足以宣称完成。没有预先承诺的提速倍数。

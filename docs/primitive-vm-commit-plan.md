# 栈 VM：一个 PR 内的 10 个 commit

状态：2026-09-12，S01–S03 阶段验收通过，S04 实施中；整体计划尚未完成。一个 PR 按 **S01–S10 共 10 个提交**交付架构、代码结构、完整语义迁移和 #16 的五项验收；以下编号表示计划中的提交，不表示已有实现。

目标见[架构计划](primitive-vm-plan.md)，目录与算法见[实施设计](primitive-vm-implementation-plan.md)，能力和结构验收见[迁移清单](primitive-vm-migration.md)。

## 当前实施记录

- **S01 阶段验收通过。** compiler 共享模型归 model/{ir,bindings,scope}；parser 的 context/builder 区分临时解析状态与完成产物，消费式 finish 移动原有存储。语法 helper 按领域归属，生产依赖显式导入。relocation/flow/optimize 复用原有绑定解析、栈验证与窄优化，保留 QuickJS 错误顺序和 source projection。
- S01 profiling 诊断覆盖 parse/resolution/lowering inclusive wall time、最终草稿指令与 inline bytes、部分 owned-Vec 容量，以及 legacy 分派/PC 发布/操作数深度。容量采样不是分配峰值；未实现的 frame/slot 成本不报为零，正式构建关闭诊断。
- **S01 最终证据（对应 S01 源码）。** 302 项 compiler 测试、2 项诊断测试、5 项 CLI profiling 测试通过；默认 CLI cargo check 通过。完整 QuickJS oracle 907 项及另行执行的 65K 实参压力用例通过，合计 908 项；701 个完整 binary-object 反例全部拒绝，源码布局和 diff 检查通过。
- **S02 阶段验收通过。** 发布输入归 code/function/publication，源代码请求和编译错误边界归 api/compile；VerifiedFunction 保留原草稿所有权。纯验证及测试归 code/verify；Atom 链接、展平、私有绑定发布和 heap 事务留在发布侧，生产验证不导入 compiler。
- S02 验证主流程按 roles、parameters、bindings、private_elements、closures、flow、eval、operands、children 组织。命名工作项和命名分析结果复用原有数组、声明索引、闭包位置与迭代队列；检查顺序不变。76 项既有测试按 modules/private/parameters/eval/bindings 分组，未增加镜像测试或改变预期。
- S02 上轮证据：436 项 code 测试、253 项 Runtime 测试通过；eval/children 拆分后完整 oracle 908 项通过。38 项定向发布反例拒绝。迁移前启动的完整反例验证因源码路径改变主动终止，不计为 S02 完整验收。
- S02 本轮 FrameLayout：只读视图从同一个 rooted executable 借用参数、局部、闭包定义和 metadata，构帧/恢复形状检查及 operand 容量已接入。RootedVmActivation 删除重复的 bytecode/code/metadata 字段，运行时从 host 的 executable 取用；动态恢复检查保留。14 项 published execution 测试与边界 scan-only 通过，源码布局为 484 个文件；完整 oracle 907 项及另行执行的 65K 实参压力用例通过，合计 908 项；40 项定向发布反例全部拒绝。
- S02 指令契约已接入：code/instruction.rs 对 198 个变体穷尽描述栈数量与类型化状态、控制流、操作数角色以及保守的 JS 异常/回调/分配效果。编译器块边界和现有窄优化、普通栈与参数验证、module initializer flow、静态名称链接、eval 环境选择和分支目标消费同一描述。动态 region/private/resume 检查保留。
- profiling 可显式捕获最终码反汇编，逐 PC 输出同一契约；默认关闭，只保留文本。诊断测试 3 项、CLI profiling 测试 5 项、最终 compiler 302 项、code 436 项和 Runtime 253 项通过；完整 QuickJS oracle 907 项及另行执行的 65K 实参压力用例通过，合计 908 项。源码布局为 485 个文件。上轮定向反例运行因描述结构更新主动终止，不计通过；本轮完整边界结果见下述最终验收。
- **S02 最终验收：**修正后的完整 boundary 套件退出码 0，709 个反例全部拒绝；此前四个简化 fixture 配置错误已改为完整源码 fixture，并先通过四项定向验证。逐项复查确认原草稿所有权、验证顺序、共享 executable 布局、动态检查及合法范围保留。
- S03 当前改动：纯 Number 实现与原有 13 项测试按 operations/integer/format/float16 归属，公共入口不变；既有帧绑定读写、捕获、关闭及闭包视图规则归 vm/bindings，现有 host 与 heap cell 验证直接消费，挂起编码和恢复验证保留。Number 13 项和 host_bridge 12 项测试通过，完整 oracle 907 项与单独执行的 65K 实参压力用例通过，合计 908 项；源码布局为 490 个文件。FrameStore/SlotStore/RunningExecution 与新主循环的初始实现见下述迁移配置；S03 尚未验收。
- S03 `stack-vm` 非默认配置已接入 ordinary bytecode 调用入口：FrameStore 持有单一 executable 和冷状态，SlotStore 分开原始实参、可写形参、局部与操作数窗口；运行登记只保留身份。主 match 完成字面量、普通槽操作、Number 算术与静态分支，未覆盖的操作在消费输入前通过显式旧路径桥交接。正常返回先安装 pending 结果，再清理窗口。
- S03 初步证据：3 项执行测试通过，其中独立测量的循环和 Number 边界为零旧 VM 分派、零交接；转换交接保持原操作数和一次回调。4 项窗口测试、2 项登记生命周期测试、迁移配置下 5 项 CLI 诊断测试通过。共享 Number 表示/更新已接入新旧执行路径和 PrimitiveValue 构造；新配置完整 oracle 907 项及单独执行的 65K 实参压力用例通过，合计 908 项；默认配置 37 项 Number 相关测试与迁移配置 3 项诊断测试通过。两个新增普通调用出口反例均拒绝。
- S03 引用事务已接入普通槽覆盖、Drop 和 Nip：修改计数前检查 runtime 域、可变借用、deferred references、zero queue 状态/容量及共享 primitive 存储。需要回收的操作保留原值交接；Symbol copy 使用可失败 retain；交接目标容器在移动任何源 owner 前完成可失败预留。7 项引用生命周期/溢出测试、4 项执行测试、4 项窗口测试、2 项登记测试通过；对象参数覆盖的独立调用为零旧 VM 分派、零交接，返回后没有额外参数 root。本次源代码下新配置完整 oracle 907 项与单独执行的 65K 实参用例通过，合计 908 项；源码布局 496 个文件。
- S03 普通值栈已补齐 Insert2/3/4、Dup1/Dup3、Perm3/4/5、Rot4Left：重排在预留窗口内移动 owner，单次插入先检查容量再 retain；Dup3 的 retain 错误是终止错误，已提交前缀留在逻辑窗口中由驱动器清理，不在热循环回滚释放。String/BigInt 常量直接共享已有 primitive 存储；动态加法仍一次性交接。7 项槽存储测试、6 项执行测试通过，含失败前缀清理、返回后字面量存活与 String/BigInt 慢路；更新后的新配置常规 oracle 907 项与单独执行的 65K 实参用例通过，合计 908 项。
- S03 初步存储成本已接到真实操作并输出到同一 CostSnapshot/CLI：槽与帧 Vec 增长、逐存储区峰值、初始化、逻辑转移、帧清理、值复制和受限 Object/Symbol 热引用计数；未覆盖的完整调用分配、primitive Rc 与回收级联明确排除。容量复用测试区分累计初始化与峰值，并验证空 arena 的析构不重复计数。迁移配置 VM 组 126 项通过、2 项既有小栈失败；新旧两种 CLI 配置各 5 项、引用事务 7 项、诊断作用域 2 项通过，真实 CLI JSON 解析通过。此前启动的 711 个边界反例全部拒绝；计数改动后的源码边界扫描与 496 文件布局检查通过。
- S03 资源审查已记录在迁移账本：原始实参快照先可失败预留，帧限额与身份失败的 2 项测试通过。最终同源检查中新旧配置各 908 项 oracle、Number 13 项、引用事务 7 项通过；VM 组 128 项通过、2 项既有小栈失败，包含的 S03 帧/槽/执行/登记 18 项全通过。最终同源边界反例 711 项全部拒绝，退出码 0；S03 阶段验收通过。现有桥仍递归等待未迁移的 JS 调用；S04–S07 必须按计划替换，不能把配置启用或混合路径 oracle 视为完整新核心覆盖。
- S04 绑定到普通字节码的调用已归一化后进入同一 driver；5 项 driver 测试通过。`this` 读取与原语装箱已接入，缓存对象身份在交接后保留；native/Proxy/挂起目标保留原始调用交接，constructor 与统一恢复尚未完成。
- S04 一元 `+` 的 ToPrimitive 已通过 operation 身份把 getter/valueOf 回复路由到父帧，嵌套转换走同一显式执行。特殊属性与非普通字节码调用只执行当前步骤的临时交接；其他转换、constructor、完整展开和绑定仍未完成。
- **S03 阶段验收通过，S04 实施中，S05–S10 尚未开始。** 非默认原语栈核心、帧/槽所有权与初步成本已达到本阶段要求；完整调用/回调、挂起和入口迁移尚未完成。正式十个提交在 PR 整理时归并。
- S04 已抽出共享构帧准备，并接入普通字节码 Call/Method/TailCall 的显式 driver：子帧共用 FrameStore/SlotStore，结果先持根再清帧，子帧 guard 在恢复父帧前结束。命名自递归 256 层在 2 MiB 栈通过且无 legacy 分派；执行核心 9 项、限额/外域参数 2 项及常规 oracle 907 项通过。公开无限递归得到可捕获的 `InternalError:stack overflow`。原有 native_stack 仍为 4 通过、2 失败（全局函数链与 TypedArray 路径仍经桥）；构造调用、转换状态、统一展开和完整绑定仍待实现，S04 尚未验收。
- S04 普通 lexical 初始化、无回收槽重置、checked 读写及非捕获 CloseLocal 已接入；共享 TDZ 诊断保留变量名可见性和错误 realm。循环块复用及下一轮 TDZ 为零旧分派、零交接。driver 18 项、run 9 项、新旧配置各 907 项常规 oracle、非 profiling 构建和边界扫描通过。捕获生命周期、完整 arguments/eval 与统一展开仍待完成，S04 未验收。
- S04 闭包单元普通/checked 读写已接入 driver，共享 TDZ、cell 只读及名称诊断规则。var/let 活单元、const 读取和逃逸 TDZ 测试为零旧分派；driver 19 项、run 9 项、新旧配置各 907 项常规 oracle 与构建/边界检查通过。闭包创建、每迭代 cell、捕获关闭和统一展开仍待完成。
- S04 catch/finally 首段控制已接入：帧持有 catch 区域，Throw/子帧抛错在弹帧前恢复 handler，Gosub/Ret 保持原返回 PC 协议，交接保留 regions。driver 20 项、run 9 项、常规 oracle 907 项及构建/边界检查通过。IteratorClose、捕获关闭和统一展开余项仍未完成，S04 未验收。
- S04 FClosure、captured CloseLocal 和 SetName 已接入，共享新 cell canonical metadata 与既有 cell view 校验。闭包跨父帧/块退出、参数捕获及循环独立 cell 用例为零旧分派。driver 21 项、run 9 项、新旧配置各 907 项常规 oracle、构建和边界检查通过；captured 局部读写/重置与统一展开余项仍待迁移。
- S04 captured 局部/参数读写、普通 lexical 初始化与 TDZ 重置已接入；重置与旧 host 共享初次 cell/异常复用规则。父槽更新及异常跨作用域 cell 身份用例为零旧分派。driver 22 项、run 9 项、新旧配置各 907 项常规 oracle 和构建/边界检查通过；arguments、eval/with/private、IteratorClose 等余项继续待办。
- S04 mapped/unmapped arguments 与 rest 创建已接入独立步骤，保留实际 arity、参数 alias 和额外实参独立 cell。修正了分配处理展开在 driver 中导致的 Proxy 有限栈回归，未更改预算或预期。driver 23 项、run 9 项、常规 oracle 907 项及单独 65K 压力用例、构建和边界检查通过；S04 其余语义仍待完成。
- S04 标识符默认参数的严格比较缺省判断已接通，Number 保持热路径，其余类型复用 strict_equal 的独立步骤。初始化顺序、TDZ、null/false 及初始化器 cell/函数体副本分离测试为零旧分派。driver 24 项、run 9 项、常规 oracle 907 项及构建/边界检查通过；解构参数和 S04 其余协议仍待迁移。
- S04 readonly/redeclaration 抛错接入当前帧展开，保留静态 Atom 消息及 RHS 优先级；闭包创建临时状态移出常驻 driver，Proxy 有限栈回归通过且预算不变。driver 25 项、run 9 项、常规 oracle 907 项及构建/边界检查通过；eval/with/private、IteratorClose 等仍待完成。
- S04 private 字段/方法/访问器初始化已接入独立步骤，新旧路径共享身份、种类、命名与 HomeObject 规则。private 相关 70 项、driver 26 项、run 9 项、新旧配置各 907 项常规 oracle 和构建/边界检查通过；完整 class/private 访问与 S04 其他协议仍未迁移完。
- S04 private 字段 get/get2/put/define/in 接入独立 step，共享私有名称解析并保留接收者和错误顺序。真实字段读写、成员检查及字段函数 this 用例为零旧分派；driver 27 项、private 72 项、run 9 项、新旧配置各 907 项常规 oracle 和构建/边界检查通过。private 方法/访问器及 S04 其他协议仍待完成。
- S04 私有方法读取、品牌检查、in 和只读写入已接入，身份解析与检查顺序由新旧路径共享。driver 28 项、private 73 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；私有访问器 JS 调用与 S04 其他协议仍待完成。
- S04 普通私有 getter/setter 接入显式子帧，ReturnValue 区分压入/丢弃结果，setter 抛错仍展开。访问器 in/只读方向也接入；getter 单次调用、接收者和 setter 返回栈形状用例为零旧分派。driver 30 项、private 75 项、run 9 项、常规 oracle 907 项及构建/边界检查通过；S04 仍未验收。
- S04 实例字段初始化器接入显式子帧，共享原有身份/realm/原型校验和品牌安装，品牌安装后不交接或重放。基类、派生类和初始化抛错用例为零旧分派；driver 31 项、class 77 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。静态初始化和 S04 其他协议仍待完成。
- S04 静态元素初始化器/static block 共用显式子帧入口，原有单次状态提交与 HomeObject/品牌安装由新旧路径共享。发布入口检查点验证三层帧、抛错和重复启动拒绝；driver 32 项、class 77 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。完整 class 创建、eval/with、IteratorClose 等仍待完成，S04 未验收。
- S04 接通实例初始化器安装和非对象父类的 DefineClass 分支，复用原有无 JS 内核。完整函数的类创建/初始化抛错、extends null 和非法 primitive 父类用例零交接；driver 33 项、class 79 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过。对象父类 getter 恢复及其他 S04 余项仍未完成。
- S04 对象父类 prototype 的普通读取/getter 接入 PendingClass 和唯一 operation 回复，共享原类发布内核，getter 之后不重读 parent。单次调用、抛错、非法 prototype 用例零交接；driver 34 项、class 80 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。exotic/非普通 getter、eval/with、IteratorClose 等余项仍待完成。
- S04 固定名称方法/getter/setter 安装复用原命名、HomeObject 与 descriptor 内核，完整类方法/继承/访问器合并用例零交接。driver 35 项、class 81 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；计算名称、字段、eval/with、IteratorClose 等余项仍待完成。
- S04 固定名称公共字段定义接入共享 DefineProperty 冷步骤，复用 CreateDataProperty 规则并绕过继承 setter。字段顺序、函数 this、抛错等用例零交接；driver 36 项、class 81 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过。计算名称和其他 S04 余项仍待完成。
- S04 computed 字段/方法定义共享 canonical key 解析，ToPropKey 接入 String-hint 转换恢复。单次转换、抛错优先级、数字和 Symbol 用例通过；driver 37 项、class 82 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。SetNameComputed、eval/with、IteratorClose 等 S04 余项仍待完成。
- S04 SetNameComputed 与静态命名进入共享冷步骤，保留 Symbol 描述命名和已有名称规则。driver 38 项、class 82 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；eval/with、IteratorClose、host 重入等 S04 余项仍待完成。
- S04 接通 with 的 ToObject/对象绑定初始化及 eval 变量环境创建，共享旧绑定规则。with 入口与 nullish 错误零交接，环境创建有真实 PC 检查点；driver 40 项、with 过滤测试 129 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。动态查找、eval 执行、IteratorClose 和 host 重入等仍待完成。
- S04 共享 eval/with 隐藏对象身份解析，接通普通动态数据查找、unscopables 和引用读取，保留 with 方法 this。driver 41 项、with 过滤测试 130 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；getter/Proxy、写入、eval 执行和其他 S04 余项仍待完成。
- S04 动态读取/引用读取的普通 getter 接入显式子帧，验证单次调用、抛错、捕获环境和返回函数 this。driver 42 项、with 过滤测试 131 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；unscopables getter、写入、eval 和其他 S04 余项仍待完成。
- S04 HasBinding 的 unscopables/排除项 getter 使用独立两阶段恢复，保留检查顺序与唯一回复，并接通普通对象分配。driver 43 项、with 过滤测试 131 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；动态写入、eval、IteratorClose 等 S04 余项仍待完成。
- S04 普通动态/eval 变量删除接入环境冷步骤，保留自有属性、不可配置结果和 unscopables 规则。driver 44 项、with 过滤测试 131 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；写入、eval 执行、IteratorClose 等余项仍待完成。
- S04 Has/Get/DefineEvalVariable 接入环境步骤，保留 eval 隐藏对象身份、普通 getter 子帧和完整数据描述符定义规则。已发布 eval 体的声明、继承 getter、抛错、不可配置拒绝及忽略 unscopables 用例零交接；driver 45 项、eval 过滤测试 137 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过。完整 eval 调用入口、动态写入、IteratorClose 等 S04 余项仍待完成。
- S04 PutDynamicBinding/PutEvalVariable/PutRefValue 的普通目标进入环境写入步骤，setter 使用显式子帧并丢弃返回值，引用写入保留 VarRef cell。driver 47 项、eval 137 项、with 131 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；完整 eval 入口、IteratorClose、普通 PutField 和特殊对象协议等余项仍待完成。
- S04 引用读取补齐普通自有 VarRef 与未解析引用错误，复用原属性读取内核并保留环境/值栈形状。driver 48 项、eval 137 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；GlobalReference、完整 eval 入口、IteratorClose 等余项仍待完成。
- S04 GlobalReference 接入共享身份/当前 lexical 解析与无 getter 的全局存在检查，保留发布后新增 lexical 优先及 RHS 前 TDZ/const 错误。driver 50 项、eval 138 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；专用全局 payload 写入、完整 eval 入口、IteratorClose 等余项仍待完成。
- S04 全局对象引用读写及字段读取复用专用描述符/存储内核，普通访问器走显式子帧；上轮全局属性赋值缺口已补齐。driver 51 项、eval 138 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；auto-init/特殊原型、完整 eval 入口、IteratorClose 等余项仍待完成。
- S04 direct eval 准备与执行分离：准备结果持有 Completion 或已实例化 closure/this，保留先编译后捕获的顺序。eval 138 项、driver 51 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；owned frame 的 eval 校验、捕获和子帧入口尚待接通，S04 未验收。
- S04 eval 帧绑定校验、按作用域顺序捕获和准备/捕获结果类型移入共享 eval_bindings，旧 host 适配自身槽位。eval 138 项、driver 51 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；owned Eval 指令与子帧入口尚待接通，S04 未验收。
- S04 普通 Eval 指令接入准备/捕获与显式子帧，父帧保留全部实参至返回并统一清理；GetVar/GetVarUndef 补齐其前置读取。driver 53 项、eval 140 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；ApplyEval、IteratorClose 等余项仍待完成，S04 未验收。
- S04 为 ApplyEval 整理无 getter 的稠密 Array 实参预检，复用既有快存储并保留 65534 上限；通用实参入口已使用该步骤。Reflect 8 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过；ApplyEval 快照保活与 owned 调用入口尚待接通。
- S04 稠密 Array 的 ApplyEval 接入 owned 原始 eval/普通及 bound callee，父帧快照保活并在返回后释放。driver 54 项、eval 141 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；通用实参读取、展开构造、IteratorClose 等余项仍待完成。
- S04 ArrayFrom 接入 owned 冷步骤，复用原数组创建内核并保留元素顺序与 callee realm，跨 realm/继承 setter 用例零交接。driver 55 项、eval 141 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；Append/迭代读取和 IteratorClose 等余项仍待完成。
- S04 DefineArrayEl 的普通对象/Array 数字索引写入接入现有 owned 冷步骤入口，复用完整自有数据描述符定义并保留数组/索引。真实发布指令检查点验证继承 setter 不执行、孔位/长度与后续返回零交接；不把该检查点计作完整展开覆盖。driver 56 项、eval 141 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过；Append/IteratorClose 等仍待完成。
- S04 Append 接入显式迭代状态与普通/bound 回调子帧，保留两次 Symbol.iterator 读取、cached next、快数组快照、u32 索引和异常关闭原异常优先级；父帧保活输入及恢复代次。完整数组/String 展开与 eval(...数组)、回调顺序和写入失败关闭通过；2048 元素在 2 帧上限下零旧分派完成。driver 61 项、eval 142 项、run 9 项、新旧配置各 907 项常规 oracle 及构建/边界检查通过。原生/特殊读取仍有单步骤同步回退；for-of 与通用 IteratorClose 等 S04 余项尚未完成。
- S04 for-of 与同步 IteratorClose 接入 owned 迭代记录和异常区域；Append 状态扩为共享 iterator_driver，按操作区分一次/两次 iterator 查询及 next 失败的关闭策略。普通/保留返回值的关闭、嵌套异常关闭、解构和每迭代捕获用例零旧分派；driver 65 项、eval 142 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过。Apply/构造展开入口、host delimiter 与完整阶段验收等仍待完成，尚未进入 S05。
- S04 Apply/ApplySuper 接入普通/bound 调用及既有构造 continuation，稠密实参快照保留 receiver、new.target 和错误顺序，包括 nullish 列表按普通调用执行的原行为。完整调用/构造/super 展开、递归展开与 prototype getter 跨越实参载体修改通过；driver 69 项、eval 142 项、run 9 项、新核心常规 oracle 907 项及构建/边界检查通过。通用实参读取仍在无副作用预检后交接；全局写绑定、host delimiter 与 S04 完整验收等仍待完成。
- S04 工作区检查点（2026-09-13）：全局 PutVar/PutVarInit/DeleteVar 接入共享绑定规则；未迁移 callee 使用单次同步调用边界，Array 命名属性缺失可沿普通原型读取。帧退出和冷绑定操作已从常驻 driver 拆出。当前 driver 76 项、run 9 项、eval 142 项及格式/边界扫描/源码布局（521 文件）通过。当前 native_stack 为 3 通过、3 失败：TypedArray 字符串转换、有限嵌套排序、递归调用/构造的旧溢出预期；未修改预算、原测试或 skip。最新冷操作拆分后尚未重跑完整 oracle。此提交仅保存实施中的工作区，S04 未验收，S05–S10 尚未开始；正式十个提交仍需后续整理。
- **小栈基线仍须解决。** 默认和 profiling debug 构建的 32 层 bytecode 调用、TypedArray 字符串转换两项失败，在隔离导出的改动前提交 5fe11ea 同样复现（native_stack 测试 4 passed、2 failed）。未提高预算、修改预期或增加 skip；S03–S10 必须满足调用与小栈硬门槛。

## 1. 提交顺序与关口

| 提交 | 完整交付单元 |
| --- | --- |
| S01 | 编译器结构、线性栈 IR 流程与基础诊断 |
| S02 | 指令契约、验证流程、发布与 executable 布局 |
| S03 | 帧/槽所有权、纯 Number 算法与栈执行核心 |
| S04 | 显式调用、转换恢复、异常与完整绑定 |
| S05 | 属性及同步内置 JS 回调迁移 |
| S06 | generator、async/Promise 与 async generator |
| S07 | 模块、宿主、API 与二进制入口集成 |
| S08 | 局部指令融合与 PC 观察边界 |
| S09 | 调用存储优化与同架构布局实验 |
| S10 | 完整验收、默认切换、旧路径清理与结果文档 |

- **A：S01–S03。** 编译/发布边界、帧/槽所有权和原语栈核心可运行、可测量。
- **B：S04–S07。** 普通调用、内部回调、挂起与全部既有入口完成新核心覆盖。
- **C：S08–S10。** 在完整语义上优化，完成 #16 验收并切换、删除旧路径。

每个提交中的实现、对应测试、源码契约及相关架构说明和结构检查一起完成。语义检查随提交执行，不推迟到 S10；诊断样本不充当正式性能成绩。临时桥明确记录覆盖；经过旧 VM 的样本不能宣称新核心已覆盖，禁止按测试或 benchmark 名选择路径。

## 2. 编译与执行基础

### S01 — `refactor(compiler): organize stack compilation and diagnostics`

把既定栈架构、基础诊断与编译器结构整理合在一个提交：

- 明确本 PR 的五项问题、状态所有者和模块边界，采用线性栈 IR、有限局部优化与现有 RC/循环回收。
- 将 compiler 的共享模型按 ir/bindings/scope 归属；parser 的 context/builder 区分临时解析状态和后续产物。复用已有 resolution/lowering，保持完整语法、hoist/capture/eval 与声明顺序。
- 整理 resolved IR → 栈码草稿、块边界、正常/异常/恢复栈状态及片段重定位。消费已有存储；可选分析超预算时不使用未收敛事实。
- 建立最终码、编译阶段和执行成本的诊断入口；先接现有基线，新 frame/slot 路径的计数在 S03/S04 接入同一口径。复用 benchmark harness，正式热路径关闭诊断。

**验收：**编译器/语法/绑定 oracle、TDZ/private Reference、try/finally、恢复与源码位置用例；错误顺序保持。显式导入、源码契约和架构说明同步，不为文件移动增加镜像测试。不恢复 Tachyon 工具或历史结果文件。

### S02 — `refactor(code): unify stack contracts verification and publication`

形成一套可由 compiler、VM 和外部格式共同消费的代码契约：

- 统一指令栈效果、潜在效果和操作数描述。保留不同验证层分别证明的事实，不能删掉动态恢复检查。
- 把发布验证大函数按参数/绑定、控制流、私有状态、模块与函数树拆分；长 tuple 改成命名工作项，保持检查顺序，共享必要上下文，避免重复扫描整树。
- 分开纯验证与 Atom/heap 发布事务，保留已有 VerifiedFunction。发布输入归 code，编译请求编排归 api，生产验证不再依赖 compiler::EvalCompileContext。
- 定义 FrameLayout、code/sites/handlers 与 Runtime bindings 的边界；每帧通过一个 executable owner 取得 metadata，复用已有共享数组。

**验收：**真实发布、外部格式、畸形 code、mutation 反例与失败回滚；合法参数/局部/跳转范围不缩小。测试按语义组织，物理归属及源码检查同步迁移，不以只换哈希代替验证。

### S03 — `perf(vm): build the owned stack execution core`

把新存储和首个实际消费者一起交付：

- 建立 RunningExecution、FrameStore、SlotStore、typed binding、冷状态、运行登记 guard 和资源限额。按 max_stack 预留窗口、复用容量，定义 move/copy/clear；区分原始实参与可写形参。
- 从 host_bridge 提取已有绑定共享读写/捕获/关闭规则，建立运行 owning 值与长期挂起 raw 边的交接契约。测试构造、调用和恢复保留各自验证。
- 复用现有 pow/ToInt32，按 operations/integer/format/float16 整理纯 Number 算法；对象转换与栈操作各有所有者。
- 一个主 match 执行字面量、栈操作、普通局部、确切 Number 和静态分支。冷 payload 由 pending 拥有，RunExit 保持小型；经证明无分配/回收/回调的槽引用事务可留在循环内。

**验收：**区间互斥、扩容后无旧引用、逻辑 pop 的引用处理、返回最后引用、Runtime owning cycle 防护；zero queue 容量预检和事务不重复提交。验证 NaN、±0、溢出、位移、乘除余幂及 BigInt/String 慢路，记录 #5 路径和初步成本。调用/回调的未迁移范围明确归 S04–S07。

## 3. 调用与完整语义迁移

### S04 — `refactor(vm): drive calls conversions and unwinding with explicit frames`

在同一 driver 中完成普通调用与一条完整回调/恢复流程：

- 调用请求拥有 callee、receiver、实参、realm 和正常/异常恢复点；ordinary/bound/constructor 调用通过显式帧 push/pop，完整处理 this、new.target 与 derived return。
- ToPrimitive 状态和 step 归 value/conversion；建立 parent operation、单次回复和恢复阶段。一条 getter/valueOf 回调完整通过新 driver，其余领域调用点归 S05。
- unwind 统一安排 catch/finally、break/continue、IteratorClose 和 return/throw，保留各自错误优先级；关闭 capture 后才清帧，pending 和结果始终有明确 owner。
- 接通 lexical/const/TDZ、closure、每迭代 cell、默认参数、mapped/unmapped arguments、direct eval/with、private 与 readonly view。建立真实 host 重入的 delimiter 骨架，回调前结束内部借用。

**验收：**有限递归、小栈、可捕获无限递归、缺少/额外实参、跨 realm、嵌套 finally 和清理抛错；getter 次数与转换顺序。验证 `x+(x=2)`、回调改绑定后抛错，以及 `function f(a){'use strict';a=2;return arguments[0]}` 保留原实参。统计调用分配、初始化和引用记账；保留真实 native 栈保护。

### S05 — `refactor(runtime): resume synchronous JavaScript callbacks through the driver`

按领域完成同步回调迁移，保留逐调用点清单：

- **object/Proxy：**Get/Set、getter/setter、Object/Reflect、trap 与 receiver；复用现有属性存储内核。
- **Array/iterator：**callback、sort、species、迭代与 close；状态保存阶段、下标和必要值，回调后按语义重读可变化内容。
- **String/RegExp/buffer：**replacement 和 TypedArray/buffer 参数转换；回调后重新取得 view/buffer 凭证。
- **其余同步内置：**逐项核对 Function、scalar、Math、collections、Date、JSON、Error 和 globals。无回调 helper 保持普通函数。

**验收：**trap invariant、holes/原型访问器、修改 length、排序/迭代副作用、IteratorClose、Unicode/零长度匹配、resize/detach、共享内存、BigInt 与 GC；保留 PR19 的 Array/TypedArray 回退。全部内部同步回调由 driver 推进，不递归等待 JS，也不假装成外部 host。默认预算原始 Earley-Boyer 进行新核心覆盖筛查；异步/模块/API 余项明确归 S06/S07。

### S06 — `refactor(vm): unify suspension across generators and async execution`

共用 frame/stack/control 与 freeze/thaw，分清各语言状态机：

- **generator：**初始状态、next/throw/return、yield/yield*、finally 与 reentry。
- **async/Promise：**同步前缀、await、thenable assimilation、job 恢复；保持已有微任务顺序与 draining 政策。
- **async generator/iteration：**请求队列、各 Promise capability、异步迭代/close，以及 finally 内 await。

挂起时源 owner 保活到目标 raw 边全部发布；恢复时 heap owner 保活到运行 roots 和状态验证全部完成。关闭、失败和被放弃状态有唯一释放责任，不永久注册 owning wrapper 或全局 root。

**验收：**强制 GC、最后引用、半转换失败、单次 completion、重复恢复拒绝、交错请求、yield*、私有/捕获状态与关闭回收。内部 poll 不新增调度时机；不引入 #20 的 Fiber 调度或新的可挂起 host ABI。

### S07 — `refactor(api): integrate modules host and binary entries with the stack driver`

完成全部既有生产入口的可测新核心路径：

- 模块 link/evaluate、live imports、循环模块、TLA、dynamic import 与 loader 失败/重入；保留模块自身状态机和特殊 linking 验证。
- Context、native/web adapters、CLI 和已支持的二进制入口接入统一 driver；外来 code 翻译后进入统一验证。
- 真实同步 host 边界保留 ABI，以 delimiter 和 native 栈预算处理 JS→native→JS；正常、异常及 unwinding 退出均解除运行登记。
- 审核 S05/S06 留出的全部调用点，收口编译请求与 code 发布的责任；测试和文件归属检查跟随真实入口。

**验收：**模块声明/初始化顺序、TLA 错误和 realm、binary fixtures/round trip/畸形输入、wrong-runtime 拒绝、平台与宿主重入。测试配置能让全部入口使用新核心；默认切换和旧桥删除在 S10。

## 4. 优化与最终交付

### S08 — `perf(vm): fuse stack operations and publish precise observation state`

在完整绑定、异常和恢复流程之上合并执行开销优化：

- 有限 UpdateLocal、CompareBranch 和保持读取顺序的局部融合；discard/prefix/postfix 共用算法，复用 S01/S02 的效果、栈、重定位和源码契约。
- 热循环维护局部 pc/sp，在准确观察点直接向已知 FrameId 发布 fault/resume 和语义阶段 site；覆盖 debug/hook、GC/release、interrupt/fuel、host 与挂起。
- 融合和 PC 发布分别诊断、分别 A/B，再进入同一提交。原逻辑操作的 fuel 权重与调试契约保持，不能盲目每 N 条才更新 PC。

**验收：**`x += g()` 不推迟旧 x 读取；postfix 返回转换后的 Numeric；const 更新在转换后 PutValue 报错；NaN 下不混淆否定 `<` 与 `>=`。最终码、动态分派、fault/resume/backtrace 和恢复状态正确，取得 #9/#10 独立证据；PC 成本不显著时如实记录。

### S09 — `perf(vm): reduce call storage costs and tune stack layout`

优化完整语义下的调用和存储成本：

- 改进 frame 热冷布局、容量复用、metadata owner 与独占 outgoing 参数区；区分必要复制、冗余保活和初始化，保持 arguments/eval/capture 的正确关系。
- 在同一栈 VM 内逐项评估规范内存栈与栈顶缓存、typed enum 与紧凑码，以及必要的热冷拆分。一次隔离一个变量，记录去留依据。
- 缓存值 owns/moves、分支入口以及异常/观察/挂起物化均有明确契约；无可信收益或维护成本过高的实验删除，不留下闲置选项。

**验收：**#7 的分配、retain/release、初始化、峰值/活跃槽与吞吐；暂停区段不可复用，不靠保留死值制造低成本。比较 code bytes、.text、实际 native frame、编译时间和 native/WASM 成本；实验不重新打开 ISA/SSA/GC 选型。

### S10 — `refactor(vm): finish validation and retire the previous execution path`

按顺序完成同一提交的最终交付：

1. 新核心配置运行相关 QuickJS oracle、完整回归/Test262、native/Web/WASM、原始 V8 与固定 50+8，核对编译/内存/调用/暂停成本及全部能力清单。
2. 验收通过后切换全部默认入口，删除旧 VmHost、重复 activation/帧投影、旧驱动与迁移桥，清理通配导出和无用实验。
3. 在切换和删除后的最终源码上重新完成相应检查及正式发布验收，记录源码/构建身份。固定工作量交错至少 5 轮，敏感项 10 轮；诊断与正式计时分开。
4. 更新架构说明、源码契约、实际 commit 导览和 #1/#5/#7/#9/#10 的真实结果。完成新增 Number、转换顺序、异常/恢复、binding/eval 四种维护演练。

**验收：**默认预算原始 Earley-Boyer 独立及组合、小栈、有限/无限递归和重入通过；新核心完全覆盖且旧路径退出。不增加 skip，不按测试名 fallback，不改冻结预期掩盖回归，不用旧 receipt 替代本轮结果。#16 的其余优化与 #20 的 Fiber 调度不随此 PR 自动完成。

## 5. 审查规则

正式 PR 以这 **10 个完整提交单元**组织。文档、诊断、测试和局部结构调整并入其所属单元；开发过程中的临时提交在整理 PR 时归并，避免把每个 helper 或修正重新扩成独立的计划 commit。阶段验收可以发生在一个提交内部，验收顺序与证据仍需记录。

代码结构任务按[实施设计第 15 节](primitive-vm-implementation-plan.md#15-代码结构的独立改进清单)验收；接口、类型、算法和反例在同一提交可审查。较大提交按本文件的领域条目组织审查说明，保留逐调用点账本。文件移动与语义改变在 diff/说明中清楚标识，不引入只有占位抽象、没有消费者的提交。

每次增加恢复路径同时完成 roots、异常和释放责任；构建、测试、正式计时与诊断分阶段串行。架构、结构和 #16 问题验收同时满足后才完成本 PR。

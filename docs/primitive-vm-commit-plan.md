# 栈 VM：一个 PR 内的 10 个 commit

状态：2026-09-14，S01–S07 阶段验收通过，S08–S10 尚未开始；整体计划尚未完成。一个 PR 按 **S01–S10 共 10 个提交**交付架构、代码结构、完整语义迁移和 #16 的五项验收；以下编号表示计划中的提交，不表示已有实现。

S07 commit 后的完整 benchmark/profile 已完成：固定 58 项耗时均回退，为 PR19 的 1.17–5.28 倍；67 个新核心成本样本零旧分派、零桥接。[结果与源码归因](reports/primitive-vm-s07-performance.md)已用于重写第 4 节：S08 先压低执行与状态推进成本并完成融合/PC 优化，S09 收口调用存储、编译和布局，修复剩余回退。此次仅更新计划，不表示 S08/S09 已实施。

目标见[架构计划](primitive-vm-plan.md)，目录与算法见[实施设计](primitive-vm-implementation-plan.md)，能力和结构验收见[迁移清单](primitive-vm-migration.md)。

用户于本轮要求：从当前 S05 剩余实现继续，按 S05 → S06 → S07 顺序，
每阶段仅一次正式验收与一次 commit，不创建中途 checkpoint commit。
开发中的定向构建/排错不作为阶段验收；S07 完成后再运行新旧 VM 完整
benchmark/profile，并在 PR #21 comment 汇报。此要求优先于下文开发临时提交规则。

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
- S04 小栈修正：根帧交接通过 RunningExit 返回到原 bytecode 入口后才执行，先退出 owned 准备/driver 的全部 Rust 帧；普通 Call/CallMethod 与旧扩展调用分派分开，准备步骤提前返回。原有 32 层调用及 TypedArray 有限字符串转换、排序用例现已通过，原生栈预算未变。溢出用例改用真正无限递归，保持原错误/恢复断言；owned 有限递归加强为 1000 层、1001 个显式帧且零旧分派。新旧 native_stack 各 6 项、新旧常规 oracle 各 907 项、owned VM 209 项和 modules 134 项通过。完整边界反例 714 项全部拒绝；S04 的 host delimiter、调用成本和阶段审计继续待办。
- **历史小栈基线已修正。** 默认和 profiling debug 构建的 32 层 bytecode 调用、TypedArray 字符串转换两项失败，曾在隔离导出的改动前提交 5fe11ea 同样复现（native_stack 测试 4 passed、2 failed）。上述 S04 修正后，原有限用例通过，预算保持不变；这段保留为历史基线记录。


- S04 host delimiter 骨架已接到真实同步 module host callback，登记只保存 runtime/执行/父帧身份；返回检查父登记恢复，错误和 Rust unwind 自动解除登记。实际 loader 在 owned 父帧存活时重入 JS、更新捕获绑定并执行 GC，正常返回、loader 拒绝及 Rust panic 均验证恢复。普通函数读取和写入模块 import view 的独立测量保持 readonly 规则，零旧分派、零交接；模块实际入口迁移仍属 S07。
- S04 调用准备诊断已接入同一 CostSnapshot/CLI：参数和局部 Vec 容量分配、初始化、实参/heap-root 复制、owned 冷帧及捕获复用表分配；传入实参 buffer 只统计观察容量，不伪称其分配次数。完整调用分配和全部 retain/release 仍属 S09；统计范围见 profiling.md。缺参/多参/对象实参/主体抛错的诊断测试与两种 CLI 配置均通过。
- S04 完整库门禁发现并修正了一元 `+` 恢复使用通用 ToNumber 的偏差：新旧执行路径现在共享原语 OP_plus 处理，保持 BigInt 特定消息和 Float 原表示。二进制既有断言未改，回调 Float/-0/NaN 位模式与 BigInt 抛错测试零旧分派通过。修正后新配置库测试 2092 项、默认配置 1981 项通过；最终阶段结果见下条。

- **S04 阶段验收通过（2026-09-13）。** 一元 `+` 修正后的同源门禁：owned 库 2092 项、默认库 1981 项；两种配置的常规 oracle 各 907 项加单独 65K 压力 1 项，合计各 908 项；CLI profiling 各 5 项通过。完整 boundary 714 个反例全部拒绝，退出码 0；非 profiling stack-vm 构建、格式、diff 和源码布局（521 文件）通过。普通调用/构造、完整绑定、同步展开、一条完整转换回调、host delimiter 骨架和初步调用成本按本阶段范围验收；逐项证据与后续边界见迁移账本。S05–S10 尚未开始，整体目标未完成；正式十个提交仍在最终 PR 整理时归并。

- S05 首批属性读取迁移：object 共享准备阶段覆盖 non-Proxy 描述符/原型查找，保留 Array 孔位、TypedArray 整数索引终止、Arguments/String/namespace/autoinit 的原存储规则。GetField/GetField2 与原语动态键的 GetArrayEl/2/3 由 property_driver 消费已选 getter，原对象键转换/Proxy/Set 等继续待办。相关 getter、receiver、旧键保留、nullish 顺序和请求放弃测试通过。
- S05 临时同步调用请求先拥有 callee/实参并返回到外层 driver，常驻分派帧退出后才调用未迁移 Runtime 入口；父执行和 operation 代次保留，回收或错误不重放调用。此桥仍可能同步等待内部 JS，不能计作 S05 callback continuation 完成。新增 owned_sync_call_bridges 单独揭示这些调用；原 native 小栈用例保持预算并通过。完整 owned 库 2097 项、默认库 1981 项、新旧常规 oracle 各 907 项（各 1 项手动压力用例本批未重跑）、新旧 CLI profiling 各 5 项、非 profiling 构建、边界扫描/定向 mutation 与源码布局 522 文件通过。第一轮 [121 个直接调用点账本](primitive-vm-sync-callbacks.md) 已建立，间接回调图仍须补全；S05 未验收。

- S05 后续属性迁移：对象键 string-hint 转换、原语 base、bound 字节码 getter 已接入。Proxy Get 的 handler 读取、trap、转发与普通 target invariant 由 object 阶段状态和 owned driver 共用；嵌套 Proxy handler 与字节码回调不再通过原 Get 同步桥。新库 2108 项、旧库 1983 项及新旧 Proxy/Reflect oracle 各 16 项通过；native/Proxy callable、Proxy descriptor target、Set 和其他同步内置仍待迁移。S05 尚未验收；按用户最新指示先完成 S05，再 S06、S07，最后提交完整新旧 VM benchmark/profile 对比到 PR #21 comment。

- S05 查询与调用继续推进：Proxy target GetOwnProperty、descriptor Has/Get、嵌套 Has/IsExtensible 和 ToPrimitive 的 Proxy Get 接入共享 continuation；Proxy apply 读取、可调用标记顺序、转发和 trap 也已接入普通调用/getter/转换/查询路径。最终 owned 库 2117 项、默认库 1986 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、新旧 CLI profiling 各 5 项通过。Set、其他 traps、Object/Reflect native 入口及其余同步内置仍待收口；S05 仍未验收，未提前进入 S06/S07。

- S05 写入继续推进：普通 Set、Proxy Set、receiver GetOwnProperty/DefineProperty 与 strict/sloppy 完成改为共享阶段，PutField/PutArrayEl 的 setter 和对象键转换接入 owned driver。新库 2122 项、旧库 1988 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、CLI profiling 各 5 项及扫描/契约/布局通过。Array length/TypedArray 对象参数转换、其他 traps 与 native 内置仍待继续；S05 未验收，S06/S07 仍按顺序延后。

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

- S05 特殊写入转换继续推进：Array length 两次 ToNumber、后置 writable 检查，TypedArray 整数索引 Set/Define 的 Number/BigInt 转换与 buffer 凭证重取已接入 owned 阶段；Drop/Nip 最后临时引用交给 driver 冷路径。新库 2127 项、旧库 1990 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、CLI profiling 各 5 项、扫描/契约/布局通过。super 属性、其他 traps 与同步内置继续待办；S05 未验收，S06/S07 仍按顺序延后。

- S05 super 属性继续推进：HomeObject、冻结 base、读取/调用/写入及对象键转换接入 owned 驱动，保留 pinned getter receiver 差异与读写顺序，并验证 base 独立 GC 保活。super 15 项、owned 库 2129 项、owned 常规 oracle 907 项（1 项手动压力未运行）、CLI profiling 5 项及构建/扫描/契约/布局通过。本批仅修改 stack-vm；S05 其余 Proxy/native 同步回调仍待完成，未进入 S06/S07。

- S05 属性谓词继续推进：in/delete 接入 owned 转换/查询，Proxy deleteProperty/preventExtensions 共享阶段并保留不同的 target 不变量；Symbol 键跨等待保活。修正删除后 reference/with 用 Get(undefined) 误判 TypedArray 属性存在的问题，原 oracle 预期不变。新库 2133 项、旧库 1991 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、CLI profiling 各 5 项及构建/扫描/契约/布局通过。剩余 Proxy/native 同步路径继续待办，S05 未验收，未进入 S06/S07。

- S05 Proxy 原型操作继续推进：getPrototypeOf/setPrototypeOf 共享有序阶段，owned 查询覆盖嵌套 target 的可扩展性与原型身份检查，保持各阶段抛出值身份，并验证等待/放弃的 GC 所有权。新库 2136 项、旧库 1992 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、CLI profiling 各 5 项及构建/扫描/契约/布局通过。独立查询已验证，Object/Reflect 生产 native 入口、其余 traps 和同步内置仍待迁移；S05 未验收，未进入 S06/S07。

- S05 native 调用所有权准备：从原调用器抽出参数/帧所有者，保留完整 argv、错误的定义 realm 和 native 栈记录，验证放弃/异常展开及 metadata/runtime 拒绝。新库 2140 项、旧库 1996 项、新旧常规 oracle 各 907 项（各 1 项手动压力未运行）、CLI profiling 各 5 项及构建/扫描/契约/布局通过。当前仍由原同步调用器消费，下一步接 native 内置 continuation；S05 未验收，S06/S07 仍延后。

## 2. 编译与执行基础

S05 本轮统一收口（阶段验收通过）：同步 native 全领域共享 Step/Resume，
VM 查询只调度有类型的请求；对象/Proxy、iterator/collection、String/RegExp、
buffer/TypedArray/Atomics、Function/scalar/Date/Math/Error/JSON/weak 与间接转换已接入。
最后引用释放、literal/method 定义、非法调用/派生构造器、子帧安装事务和放弃执行的
释放顺序一并收口。现有默认预算不变，统一阶段门禁已完成；同一次验收中
修复失败并完成复核：非法 Construct 的 TypeError 已改由共享异常出口返回；有限递归
资源探测区分默认 VM 的物理栈和 owned VM 的逻辑帧，原错误与恢复断言保留；
async 预检探测改用直接索引收集 Promise，避免 Array.push 先耗尽预算。
具体入口审计见同步回调账本。S06/S07 未开始，不创建中途 commit。

本次门禁的语义与结构部分已完成：新库 2197 项、默认库 2032 项；新旧 oracle
各 911 项及单独执行的 65K 实参压力用例各 1 项；CLI profiling 各 5 项；
非 profiling 构建、4 项属性契约测试、723 个完整边界反例、631 文件源码布局与
格式/diff 检查通过。CLI 原有 `instanceof Array` 探测脚本保持不变，计数预期更新为
owned 配置零旧分派、零整帧交接和一次 S07 print 桥；默认配置保持旧分派预期。
原始 Earley-Boyer 独立执行与调用栈归因均完成，三个残余桥逐个确认是
`QjsConsoleLog`。原始组合脚本的八项 suite 与总分在完整归因运行中全部输出；
零旧分派、零整帧交接，十个同步桥事件逐个确认属于同一 S07 输出入口。
组合诊断首次触及外部 600 秒超时的失败记录保留；完整归因使用 1800 秒外部
watchdog，610.16 秒退出 0，VM 默认预算与原始脚本均不变。诊断成绩不作性能结论。

统一验收证据保存在 `target/primitive-vm-s05-acceptance/`，含所有失败/复核日志、
905 个 Rust/Cargo 输入的最终哈希及 `coverage/coverage-verdict.json`。覆盖二进制
SHA-256 为 `e458c028531a958baf724eac7dc570f0861de62173d1b5b81ba873c797d80b8a`。
S05 阶段验收通过；以本阶段计划消息提交一次后，才恢复 S06 的独立 stash。

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

语言状态机实现已收口，S06 统一阶段验收通过。挂起记录保存独立原始 argv
及对应 raw/Atom 边；共同 freeze/thaw 与 owned frame 处理五类挂起、恢复输入
和异常展开。generator 创建、resume、async body/await/settle、async generator
队列/await/completed return、Async-from-Sync、Promise 全部 selector 和作业均
通过有类型的请求/回复推进；getter、转换中的非 Normal 字节码也接入同一路径。
root 请求持有真实 continuation，不创建占位字节码帧。语言状态机保留原有
同步前缀、队列重入与微任务政策；模块和宿主入口按计划留在 S07。
开发定向 oracle、逐作业 GC/零桥接测试、原始实参与放弃状态检查已完成；
这些不单独计作阶段验收；半转换失败、wrong-runtime、最后引用等已在该次
完整新旧配置验收中一并复核。默认旧 VM 保留到 S10，freeze/thaw 表示适配
本身不执行旧分派。S07 尚未迁移的 opcode 仍可进入已计数的过渡桥；该桥
必须同时返回完成或挂起，避免 async 中 await import 被错误当作同步终点。
既有 256 KiB 模块图测试暴露查询 dispatcher 的 debug 原生帧过大；
仅按请求领域拆分原有分派和调用准备，保持状态转换、测试栈及预算不变。
旧 generator 栈测试假定 1000 层有限委托必然溢出；改用 Infinity 验证同一
溢出错误与恢复断言，并把 owned 的零桥接有限委托/finally 用例加强到 1000 层。

本阶段唯一统一验收结果：owned 库 2208 项、默认库 2035 项；新旧 oracle
各 911 项及单独 65K 实参压力各 1 项；CLI profiling 各 5 项通过。
非 profiling 构建、4 项属性契约测试、725 个完整边界反例、653 文件布局、
格式与 diff 检查通过。全部失败和修复日志保留在
`target/primitive-vm-s06-acceptance/`；最终 1024 个源码/构建输入哈希与
`stage-verdict.json` 记录同一次阶段验收，不创建 checkpoint commit。

共用 frame/stack/control 与 freeze/thaw，分清各语言状态机：

- **generator：**初始状态、next/throw/return、yield/yield*、finally 与 reentry。
- **async/Promise：**同步前缀、await、thenable assimilation、job 恢复；保持已有微任务顺序与 draining 政策。
- **async generator/iteration：**请求队列、各 Promise capability、异步迭代/close，以及 finally 内 await。

挂起时源 owner 保活到目标 raw 边全部发布；恢复时 heap owner 保活到运行 roots 和状态验证全部完成。关闭、失败和被放弃状态有唯一释放责任，不永久注册 owning wrapper 或全局 root。

**验收：**强制 GC、最后引用、半转换失败、单次 completion、重复恢复拒绝、交错请求、yield*、私有/捕获状态与关闭回收。内部 poll 不新增调度时机；不引入 #20 的 Fiber 调度或新的可挂起 host ABI。

### S07 — `refactor(api): integrate modules host and binary entries with the stack driver`

当前实现（S07 统一验收通过）：模块 link/evaluate 保留原 DFS/SCC 状态机，
分别由 `modules/link.rs`、`evaluation.rs` 拥有等待状态；body、异步完成回调和
Import 参数算法归同领域共享 Step/Resume。dynamic import load job 依次进入
link、evaluate、Promise attach 和 settle 根操作，保留每次操作之间的 RuntimeError
转换及原有 FIFO 政策。编译请求仍归 API，所有输入仍通过 code 验证/发布。

Context call/construct 和属性 API 使用带原始域验证的 owned 根请求；own descriptor
通过有类型的根结果返回。binary 翻译后的 callable 使用相同入口。Test262 evalScript、
Agent 转换及 qjs 输出已经登记；native/web/Test262 构建支持显式 `stack-vm`。
真实 loader/rejection tracker 边界使用 delimiter 和原 native 栈预算；活跃 Runtime
内没有真实宿主边界的嵌套根执行被拒绝。HostServices 的时钟/时区仍遵循既有
禁止同 Runtime 重入的同步值服务契约，未修改其 infallible ABI。

开发回归记录：首次 owned 库完整检查 2203 通过、7 失败。失败暴露了
ActiveFrameProbe 测试入口误登记为无回调操作，以及 checkpoint 测试在手动暂停
执行仍登记时调用 Context 的旧做法。probe 改用共享 InvokeStep；测试先准备输入、
或结束暂停执行再读错误，原错误和 active frame/backtrace 预期不变。放弃测试改在
实际 JS 子帧尚未执行时停住，继续检查 native owner 与 Runtime 释放。binary 测试
在原 25642 字节完全不变的文件末尾新增 GC 后执行及零桥接断言。

S07 首次正式 workspace 检查中，owned 库 2211 项、CLI 32 项通过，oracle
905 通过、6 失败（另 1 项手动压力尚未运行）。六项失败共同暴露模板对象
`PushConst` 仍走旧整帧交接；补入共享 value-constant 检查/持根冷操作，默认 host
消费同一规则。原 oracle 输入/预期不变，定向 11 项 QuickJS 对照通过，并增加
模板对象跨回调/GC 的身份及零桥接测试。失败日志保留，仍是同一次正式验收。

首次完整 owned Test262 执行 102037 variants：79966 pass，比冻结基线少 16。
新增失败均为 Promise all/allSettled/any/race 的 null/undefined iterable：
ReadValue 的准备错误越过了选定 continuation。修复复用原 value-property 读取
的 nullish TypeError → Throw 转换，让 Promise 算法按原阶段拒绝 capability。
未修改用例、admission、配置或预期；原失败向量与 runner 凭证完整保留，继续同一次验收。
原 16 项定向复验全部通过；再次完整执行恢复 79982 pass，TSV/JSONL 分类投影
通过，但冻结 receipt 哈希因首行 current-source 指纹不同而拒绝。逐字节验证确认
只还原首行指纹即可得到两个原冻结 SHA-256。门禁先认证当前 runner/报告来源，
再仅在临时副本中还原首行来源字段，严格比较全部结果字节；原报告不修改。
五项定向门禁检查覆盖有效来源、结果字节篡改、其他 metadata 篡改、错误来源与
重复来源，均按预期接受/拒绝。门禁修改后重新生成完整新旧 receipt。


WASM 首次运行要求有限 1000 层 yield* 溢出，新 VM 返回成功而失败。
默认配置的原输入/预期保持不变；owned 配置明确验证 1000 层返回 42，再用
Infinity 委托验证相同的可捕获 InternalError 和后续执行，未改变生产代码或预算。
两种配置重新构建并通过全部 15 个 playground 示例、metadata 和 Node/WASM 检查。

**S07 统一阶段验收通过（2026-09-14）。** 最终 owned/default workspace 全部通过：
库测试分别 2212/2035，CLI 各 32，常规 oracle 各 911，另行执行的 65K 实参压力
各 1，CLI profiling 各 5，Test262 runner 单元测试各 122。两种配置完整 Test262
均为 102037 variants、80032 eligible、79982 pass；全部结果字节与冻结基线一致，
只在认证后对临时副本还原首行源码指纹。完整 boundary 726 个反例全部拒绝；
属性契约 4 项、非 profiling 构建、662 文件布局、格式和 diff 通过。14 项门禁、
全部失败与修复、原始报告和最终 1163 个源码/构建/fixture 输入哈希保存在
`target/primitive-vm-s07-acceptance/`，最终判定为 `stage-verdict.json`。
本阶段只创建一次 commit；此后已在该干净 commit 上执行完整 benchmark/profile，
结果及原始样本说明写入 PR #21 comment，见[回退分析](reports/primitive-vm-s07-performance.md)。S08/S09 优化、S10 默认切换和旧路径删除未实施。

完成全部既有生产入口的可测新核心路径：

- 模块 link/evaluate、live imports、循环模块、TLA、dynamic import 与 loader 失败/重入；保留模块自身状态机和特殊 linking 验证。
- Context、native/web adapters、CLI 和已支持的二进制入口接入统一 driver；外来 code 翻译后进入统一验证。
- 真实同步 host 边界保留 ABI，以 delimiter 和 native 栈预算处理 JS→native→JS；正常、异常及 unwinding 退出均解除运行登记。
- 审核 S05/S06 留出的全部调用点，收口编译请求与 code 发布的责任；测试和文件归属检查跟随真实入口。

**验收：**模块声明/初始化顺序、TLA 错误和 realm、binary fixtures/round trip/畸形输入、wrong-runtime 拒绝、平台与宿主重入。测试配置能让全部入口使用新核心；默认切换和旧桥删除在 S10。

## 4. 优化与最终交付

### 两阶段共同基线与验收口径

本节供实现者按依赖顺序执行；S08/S09 仍各为一个完整提交单元，下面的工作编号表示内部步骤与隔离实验，不增加正式 commit。原定 #5/#7/#9/#10 优化与迁移回退一起验收，不能只交付融合微基准，也不能把回退留到 S10 切换默认入口后再处理。

保留三个比较层次：PR19 是整个 PR 的性能参照；冻结的 S07 `23f5dcfe` 是本次修复基线；S08 最终源码是 S09 的增量基线。原始 S07 样本和失败记录保持不变。每个候选从固定父版本隔离一个变量，再测合并后的交互；记录源码/补丁、编译器、features/flags、二进制及 workload 哈希。同源 default/stack-vm 可作为归因对照，不能取代 PR19 的端到端比较。

| 证据 | 共同要求 |
| --- | --- |
| 正式吞吐 | 普通 release、profiling 关闭；固定 50+8 全部用例交错 10 轮，原始 V8 八项和 combined 至少 5 轮。阶段内定向 A/B 不能替代完整矩阵 |
| 比值 | 固定和 compile 统一报新/基线耗时；原始分数报新/基线 score。某项任一轮失败即不提供该项有效比值，不从子集拼 combined |
| 独立编译 | 相同公开 compile API 和 67 个冻结源码，10 轮；源码读取/Context 创建在计时外，不执行 JS。不得用完整进程减去另一批编译时间估算执行时间 |
| 诊断 | 每个冻结 workload 验证 owned 指令为正、三种旧分派/桥计数为零。动态分派、逻辑搬运、分配、RC、PC 发布分开定义；尚未覆盖的值报 unavailable |
| CPU 与内存 | 单独构建/串行采集。对短热点另加标明用途的等工作量放大探针，获得足够采样后再归因，不改冻结输入。普通构建 RSS 与诊断 RSS 分开；self 占比不充当绝对耗时或收益 |
| 误差与身份 | 保留所有轮次、失败、min/max 和交错顺序；噪声敏感项预先固定配对区间估计方法并复测。若改绑核/频率政策，两边一起重测，不能混入既有样本 |

阶段报告逐项列出“目标热点 → 机制改动 → 计数/机器码变化 → 正式 A/B → 剩余回退与归属”。中位差超过 5% 是必须复核的工作阈值，**不是允许永久回退的额度**；小于该值但持续、可复现的下降仍要解释。S09 的退出目标是双方有效固定用例恢复到 PR19 水平或更好，并保留 #5/#7/#9/#10 的独立优化证据；这是验收目标，不是速度承诺。确认存在的剩余回退不得用平均数、栈深收益或“已完成架构迁移”抵销。

### S08 — `perf(vm): streamline owned execution and fuse stack operations`

**阶段目标：**先修复所有普通操作都在支付的执行/状态推进开销，再在相同所有权和观察契约上完成原定局部融合与 PC 优化。S07 的空循环约 2.03 倍回退与 TypedArray 写入约 5.28 倍回退属于两种不同路径，必须分别取得机制证据。

#### S08.1 — 建立可归因的执行诊断

- 在现有诊断内补充 run 出口原因、无 JS 回调即完成/实际发起回调的次数、各领域状态转移和 parent continuation 压入/弹出、Frame PC 写入与 Runtime 观察发布次数；普通计时构建不携带诊断开销。
- 记录 `Step/Resume/Next`、转换任务及其最大变体的实际布局，检查按值参数/返回的 memcpy 调用点、函数 `.text` 与 prologue/实际 native 栈消耗。已有 472/560 B 复制和数 KiB 栈预留是调查起点，不能当作完整类型大小或动态复制总量。
- 区分 immediate copy、String/BigInt Rc、Object/Symbol fallible retain、槽认证、逻辑 slot move 和机器码 payload 搬运。旧版没有的新计数不得补零；模型估算字节必须标明估算依据。
- 冻结热点组：循环/Number；属性和 TypedArray 写；iterator/for-of；String/BigInt/Math 的无回调转换；普通/closure/global 调用。同时保留 getter/Proxy、异常、GC 与挂起用例作为语义对照。

#### S08.2 — 压缩状态传递，让无回调步骤直接完成

- 查询驱动只传递小型动作/结果；跨回调 payload 由明确 owner 保存，避免完整聚合枚举在领域函数、`Next::Continue` 与中央循环间反复移动。领域内部步骤优先在同一领域推进，跨领域请求保留类型约束；不是把所有算法重新塞入巨型 match。
- 对 Primitive/Number/Element 已确定可完成的输入、已完成 NumericStep、两侧均为 primitive 的加法，直接消费共享领域结果。String/BigInt 可能分配/报错，仍在发布观察状态后的冷步骤执行；只省调度往返，不另写一份转换或数值规则。
- 普通 data property、稠密 Array、TypedArray 的安全读取/写入及 iterator 已完成结果，先尝试共用存储/领域内核；只在确实需要下一语义阶段、getter/trap/species/用户转换时安装等待状态。普通属性的 flags/prototype/receiver 检查与 PR19 Array/TypedArray 回退保留。
- 审核 TypedArray key/value 转换、resize/detach 和重新取得 view 的顺序；转换可以执行 JS 时必须保存阶段，回调后重取凭证。不能按“数值索引”跳过 canonical key、原型或失败规则。
- 不以每步 `Box::new` 替代 memcpy，不给普通快路增加每操作堆分配；只有需持久等待的冷状态才取得长期 owner。落地一个共同协议后，由 S09 继续处理其容量复用，不再更换推进模型。
- 立即完成仍保留 Return/Throw/引擎错误的区别、已选 continuation 的错误接收点，以及 native activation、错误 realm 和资源记账。用 S07 的 Promise nullish iterable 回归检验准备错误不会越过父状态；等待/放弃时 roots 与 native guard 仍按原顺序释放。

**定向证据：**typed_array_write、prop_write、array_write/update、array_for_of、math_min、bigint64_arith、string_build 系列；记录每操作的通用分派/parent 状态次数、实际复制调用点和时间。零回调路径应少建状态；真实回调仍由显式 driver 推进、单次回复，不递归等待 JS、不走旧桥。

#### S08.3 — 认证运行窗口，减少普通槽操作的重复工作

- 在进入 run 时认证 FrameId、窗口、已验证布局和容量，建立只在本次连续执行内有效的窗口视图。简单局部和操作数使用已认证索引；在切帧、扩容、释放 drain、分配/GC、回调或挂起前结束借用，恢复后重新取得视图。继续禁止 unsafe。
- immediate 的 copy 和 Number 运算保持短小，Object/Symbol 可失败 retain 使用独立 helper；已证明无分配/回收/回调的引用事务可继续留在热路，复杂释放才退出到冷边界。String/BigInt 共享存储和最后引用释放规则不变，不将 value copy 一概视为可删除的 RC。
- 两个已认证的 Number 操作数就地消费/替换，省掉重复 peek/pop/push 认证与包装；按实际活跃区更新 sp，死槽立即失去 owner。先证明操作不会分配/回收/回调，才允许在连续运行区内完成。
- 本步先使用规范内存栈，保留绑定的 Direct/Captured/Uninitialized 区分；不同时引入 S09 的栈顶缓存或紧凑编码。原始实参、checked/readonly 绑定仍走各自规则。

**定向证据：**empty_loop/down_loop、int/float_arith、局部读取、Crypto/Navier-Stokes；展示槽认证和 helper 调用减少、逻辑 copy/move 的变化，以及正式吞吐。跨窗口/过期身份、容量失败、最后引用与中途 retain 失败必须仍可检出且只提交一次。

#### S08.4 — 精确观察状态与有限融合分别 A/B

- 在 S08.3 的借用边界上用局部 pc/sp；正常、错误和冷出口统一物化到已知 FrameId。分别计量 Frame fault/resume 写入与 Runtime 活跃帧发布，避免把 S07 已经按 run 出口发布的行为误算成逐指令 Runtime 更新。
- 列全 throw/backtrace、debug/hook、GC/分配/release drain、interrupt/fuel、JS/host 调用、yield/await 与恢复的观察点；pending 保存准确 read/convert/write site。异步 CPU 采样不宣称任意时刻精确 JS PC。不得每 N 条盲目发布。
- 在 S01/S02 的效果、栈、块入口、重定位和源码契约上加入有限 UpdateLocal（discard/prefix/postfix）与 CompareBranch；只有读取时刻已证明相同才加入少量 AddLocal 模式。不可进入融合中段，不跨 callback/handler/resume 边界偷移求值。
- 先测窗口优化后不融合的内核，再独立测 PC、UpdateLocal、CompareBranch，最后合并验证交互；只用于实验的开关和失败候选不留在正式代码。记录最终发布码、动态频率、逻辑操作 fuel 权重及编译新增成本。

**语义验收：**`x += g()`、`x+(x=2)` 保留旧值读取；postfix 返回转换后的旧 Numeric；const 更新先转换再 PutValue 报错；对象转换修改绑定后正常/抛错均正确；NaN 下否定 `<` 不替换为 `>=`。同时覆盖 ±0/BigInt/TDZ/captured、finally、恢复 PC 和单步源码位置。调试契约无法保持的模式在该模式下禁用融合。PC 收益不显著时保留真实结论。

**S08 退出条件：**上述两类执行热点均有独立 A/B 与机制收敛证据；相关语义、完整 owned/default 回归/oracle/Test262、boundary、native/Web/WASM 与默认预算 Earley-Boyer 通过，完整性能矩阵和零桥计数齐全。非目标路径出现新回退先修复。本阶段不宣称全部恢复到 PR19；剩余项逐项列明 S07/PR19 差距和 S09 的存储/编译/布局责任。尚未解决的 S08 状态或槽热路问题留在本阶段，不能笼统转交“布局调优”。

### S09 — `perf(vm): reuse call storage and close performance regressions`

**阶段目标：**在 S08 的推进协议和规范栈内核上消除逐调用成本，修复编译与其余可复现回退；布局实验由剩余热点决定。调用存储的主要证据是 func_call 在最大深度 3 时仍产生约 160 万次参数缓冲和 FrameCold 分配，而非“arena 没有复用”。

#### S09.1 — 调用参数直接进入窗口，保留必要快照

- 先补齐每个调用种类的成本账：callee/realm/executable 获取、argv/request、parameters/locals、capture flags、FrameCold、native invocation、bound/apply 临时容器、返回/清理。分别报告容量增长次数、分配字节、初始化、owner move/copy/retain/release；区分累计与活跃/峰值。
- 预留与构帧验证完成后直接初始化 SlotStore 的参数/局部区，消除 parameters/locals Vec 的中转。对 caller 已求值且独占的 outgoing 尾区采用拥有式区段转移，记录 caller 恢复形状；无法转移时使用同一布局算法的必要复制形式。
- 单独保存实际 arity。只有 code/binding 信息证明没有 arguments/direct eval/其他原始实参观察者时才省掉原始 argv；其余保留独立来源或有证据的写时分离。strict/non-simple arguments 不随形参赋值改变，mapped arguments 按重复/缺失参数与 cell 规则维持别名。
- bound/method/constructor、rest/spread、默认参数和 65K 实参沿同一所有权事务。所有可失败预留在源 owner 移出之前完成；调用建立失败、参数初始化抛错和派生 return 都由同一清理规则处理。

**定向证据：**func_call、func_closure_call、global_func_call、arguments 两类、固定 Richards/DeltaBlue/Earley-Boyer。另用固定调用数/深度/arity 探针区分深度增长与平稳重复调用；预热达到容量后，普通无 capture 调用不应逐次再分配参数/局部中转缓冲。

#### S09.2 — 帧、continuation 与 metadata 容量复用

- 普通帧保持短头部；FrameCold、捕获 flags、Query parents/native scopes 和真实等待 payload 使用 execution 所有的可复用容量。复用空存储，不复用仍活跃、暂停或被 capture 引用的区段；不为压低分配保留已死的 Value/ObjectRef。
- 一个持根 executable 提供不可变 code/layout/metadata，减少每次 snapshot 的多个 Rc 投影与重复 callee root；长期原始边、运行 root 与 realm 验证仍有清楚边界，不能让借用跨 Runtime 可变操作。
- 返回值/抛出值先取得结果 owner，关闭 capture、处理 finally/IteratorClose、结束 guard 后才清帧。尾调用只在返回/构造/观察契约许可时复用；普通 call 的收益不依赖先实现通用尾调用复用。
- freeze/thaw 与 generator/async/模块挂起使用同一最终布局；source owner 保活至目标全部发布。半转换失败、放弃执行、wrong-runtime、重复恢复、回调重入各有唯一释放责任，不能引入 Runtime owning cycle。

**定向证据：**稳定深度下 frame/冷状态容量达到平台，平稳调用的临时分配明显减少；必要参数复制单列。补充带对象参数/返回、深递归、交错挂起和放弃的固定探针，报告普通 RSS、活跃/预留槽、冷状态容量、GC/release 与 freeze/thaw 时间及暂停分布。原始 combined 自适应累计值只用于形态分析，不用旧版失败 RSS 做比值。

#### S09.3 — 编译回退与代码布局收口

- 对同一公开 compile API 补齐 parse/resolution/lowering、块/融合/relocation、verify、publish 的分阶段归因；inclusive 与 exclusive 分清，不把嵌套时间相加。先用长源码或重复编译探针定位，正式结论仍用原 67 项。
- 检查共享指令契约的取用、重复控制流/栈事实遍历、临时容量和发布投影；只合并已证实重复的工作。保持验证顺序、完整输入反例、合法范围及 source projection；S08 新融合带来的时间/内存成本一起计入。
- 将 `.text` 分为 run/driver、领域协议、compiler/code 与过渡路径，结合 cycles/instructions、branch/cache 事件及机器码判断取舍。不要从 +46.25% 体积直接推出缓存瓶颈，也不要提前删除 S10 的旧路径来掩盖成本。
- 冷 payload 外置与拆分以机器码和 native 栈结果为准，不靠全局 `inline(always)` 扩张热代码。普通 release、FP 诊断、debug 小栈、native/WASM 分开记录；诊断构建的栈溢出不能覆盖普通构建的成功记录。

#### S09.4 — 有条件地评估栈顶缓存与紧凑编码

先重新 profile S09.1–S09.3。只有规范栈流量仍主导才比较 0/1/2 个栈顶缓存；只有 code/decode/指令缓存证据支持才比较 typed enum 与紧凑码。两项是原定评估任务，是否保留实现由结果决定；有充分否定证据时不必制作完整第二套执行器。

一次只隔离一个变量：规定缓存 owns/moves、分支汇合和正常/异常/GC/host/挂起的物化；编码继续消费同一指令契约与外来 binary 验证。比较正式吞吐、code bytes、`.text`、编译时间/内存、实际 native 栈和 WASM 体积/耗时。无可信收益或维护成本过高就删除候选，不留下闲置 feature/双协议；不重新打开 ISA、SSA、GC 或 JIT 选型。

**S09 退出条件：**按共同口径同时给出 S09/S08、S09/S07、S09/PR19；完整固定 50+8、原始八项/combined、67 项 compile、调用/内存/暂停证据齐全。原始旧 Earley-Boyer/combined 无有效基线，要求新核心默认预算持续成功，并与 S07 的有效分数比较。确认存在的吞吐/编译回退逐项修复或保持阶段未完成，不能靠可选缓存/编码实验的预期收益结案。#7 的分配/初始化/引用成本、#9 的最终码/分派和 #10 的独立 PC 结论同时交付；完整语义、零桥覆盖、有限/无限递归、小栈/宿主重入、native/Web/WASM 门禁通过后才进入 S10。

### S10 — `refactor(vm): finish validation and retire the previous execution path`

前置条件：S08/S09 的联合优化与回退修复达到上述退出条件。S10 不接收未归因回退作为默认切换后的待办。

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

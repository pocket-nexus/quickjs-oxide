# 编译器

compiler 负责把完整 JavaScript 源码转换为尚未发布的函数和模块草稿，
包括词法与语法分析、声明和作用域处理、名字解析、闭包捕获及栈指令生成。
修改语法、绑定规则或编译期改写时，从这个模块开始。

入口只编排请求；options 管配置，parser 管语法/诊断与构造状态，model 管
阶段间共享产物。FunctionBuilder 的消费式 finish 检查临时控制状态后交出 IR；
resolution 解析名字，lowering 生成栈码，relocation 管目标重定位，flow 复用
必需验证，optimize 管有限局部改写与源码位置投影。

当前管线使用线性 FunctionIr/IrOp，解析和绑定解析完成后生成栈式代码。
声明顺序、稳定绑定身份、异常区域和源码位置贯穿这些阶段；名字索引是
查找工具，不能代替遮蔽、重复声明或 eval 的语义判断。

compiler 使用 source 的精确源码表示和 code 的代码契约。它不选择宿主
provider，也不拥有正在执行的帧；运行时链接、roots 和发布事务属于 code
及 heap。对象和 Symbol 的运行时身份不成为普通编译常量。

[当前架构](../../../docs/architecture.md)
记录当前阶段边界；完整前端及其语义仍由本模块负责。

优化必须保留精确源码字节、UTF-16 字符串、token span、ASI 与
`line_terminator_before`、词法 goal/context、转义标识符和 private name 身份。
名字驻留不能代替作用域/遮蔽判断。前瞻缓存必须保持各探针的吞错/传播错误
策略，不得把 speculative 失败变成提交错误；缓存身份包含 source、offset、
goal 与 context，不能跨词法上下文复用。重扫保留已观测的换行与 token 起点，
模板续段不能按除法 goal 重扫；前瞻不提前修改已提交状态。

模板的 cooked/raw、tagged/untagged 无效转义行为与首次错误位置必须保持。
转义标识符按解码名判断；private name 的身份及包含 `#` 的长度限制保持。
惰性文本处理不能推迟扫描期必须报告的畸形字节或长度错误。诊断的阶段、
文案、位置、首错及求值顺序依照现有契约和 oracle 保持，解析资源限制与
栈耗尽行为也需对拍。已删除计划中的 arena、parser 形态、旧 BC5 编码顺序
及收益阈值是旧候选范围，不是这些语义要求。

当前 capture 分析主要用于局部生命周期处理；将精确 storage/初始化事实传递
到执行操作选择，以及编译期 dataflow/effect/ownership 规划，是后续设计。

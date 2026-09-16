# 栈虚拟机

vm 执行已发布代码，协调函数调用、绑定访问、异常展开、回溯以及 generator
和 async 的挂起、恢复。当前仅有显式栈执行核心，默认构建直接使用它。

`SlotStore` 持有参数、局部与操作数的窗口，`FrameEntry` / `Frame` 保存代码所有者、
PC 和恢复状态。普通 JS 调用与内部回调由 driver 推进显式帧；只有真正的宿主调用
建立宿主边界。旧 `RuntimeVmHost`、`VmActivation` 和递归解释器已退役。
生成器 freeze/thaw 直接编码与恢复新帧，不经过旧宿主适配层。

转换算法属于 value，属性语义属于 object，内置方法属于 builtins。
VM 执行这些算法请求的 JS 调用，并传播返回值、异常及挂起结果。
长期挂起数据的引用边由 heap 保存；发布代码的静态保证与恢复状态的动态验证
共同约束重新进入执行的边界。

[迁移记录](../../../docs/primitive-vm-migration.md)描述实现和测试迁移；
[逐阶段计划](../../../docs/primitive-vm-commit-plan.md)记录后续优化及验收状态。

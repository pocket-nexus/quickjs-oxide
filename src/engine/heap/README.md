# 堆存储与回收

heap 拥有运行时记录、代际身份、引用边、roots、引用计数和循环回收。
对象、代码、realm、模块、Promise 与挂起状态的原始载荷存放在这里；
这些载荷对应的 JavaScript 算法仍由各语义模块负责。

记录描述保存什么，存储操作负责怎样变更记录及其引用边。分配、发布、
替换和释放需要保持一致的所有权关系，避免把同一条边重复保留或提前
释放。延迟清理和循环回收沿用这套记录模型处理弱引用与级联销毁。

Runtime 的共享堆与 Atom 状态也由本模块管理。高层属性访问、数值转换、
模块求值和内置算法通过各自的 Runtime 方法使用存储，不应集中到堆
实现中。修改存储时，要区分物理槽位置、稳定身份和可观察顺序。

VM 重组继续使用现有 RC 与循环回收。活动 owning 值与长期挂起的原始
引用边之间如何交接，见[当前架构](../../../docs/architecture.md)。

对象、Context 与 FunctionBytecode 留在共享 arena；捕获变量 cell 和 Shape
分别有独立的代际 arena，String/BigInt 使用 leaf arena。不同 arena 的数字
index 可以相同，跨 arena 的引用与回收须使用完整 typed identity。cell 和
shape 的槽不携带对象专用的 weak-link 状态；循环回收仍跟踪它们的边，并由
对象、Context 和 FunctionBytecode 锚点触发不可达循环的清理。设计及测量
见[类型化 arena](../../../docs/performance/typed-arenas.md)。
显式 GC 成功并完成延迟 root 释放后，空 zero queue 最多保留 4,096 个 ID 槽；
普通引用释放仍保留队列容量供复用。容量与回收行为由
`src/engine/heap/runtime/tests/gc.rs` 的测试覆盖。

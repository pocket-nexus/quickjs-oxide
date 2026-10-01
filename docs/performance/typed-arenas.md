# 类型化堆存储

当前堆把 Object、Context 和 FunctionBytecode 存于共享 arena，把捕获变量 cell 与 Shape 分别存于独立的类型化 arena，把 String 与 BigInt 存于共享的 leaf arena。`VarRefId`、`ShapeId` 等 ID 携带完整代际身份。主要实现位于 `src/engine/heap/arena.rs`、`auxiliary_arena.rs`、`identity.rs`、`binding_storage.rs`、`object_storage.rs` 和 `gc.rs`。

## 捕获变量与形状

捕获变量 cell 是活动帧与闭包共享的可变绑定。普通替换取得新边的 owner 并释放旧边；拥有式替换转移传入 owner，并交出被替换 owner。形状存储维护共享布局的 copy-on-write、唯一 owner 的原位更新、布局 revision、字典序、原型边、属性键 atom 边和弱 interner 清理。

每个类型化 arena 管理槽、空闲索引、代际、引用计数和状态。分配与发布建立记录身份；释放进入零计数队列；回收沿完整 `RawId` 跨 arena 遍历引用边。循环回收使用各 arena 的 trial/reachability 空间，并保留 Object、Context 与 FunctionBytecode 的 finalization 锚点。

## 容量与诊断

显式 `Runtime::run_gc` 完成并清空延迟 root 后，空零计数队列最多保留 4,096 个 ID 槽。普通引用释放复用队列 backing。profiling 构建报告各 arena 的槽容量、空闲数、队列与回收 scratch；[诊断说明](../profiling.md)定义这些字段的统计边界。

所有权和借用在 VM、堆与持有状态之间按[执行不变量](principles.md)交接。相关测试位于 `src/engine/heap/tests.rs`、`src/engine/heap/runtime/tests/gc.rs` 及 heap 的 storage/collection 测试模块。

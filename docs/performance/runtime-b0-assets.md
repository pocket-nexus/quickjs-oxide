# B0：旧 B 正确性资产审计

起点为已采纳 PR #89 `6db7e6a7`。旧 B 的每项运行时提交和新增见证索引
保存在 [完整清单](runtime-b0-test-assets.json)，原代码通过 immutable commit
和文件定位保留；不把旧迁移的类型、运输层或性能结论一并采纳。

## 清单一：独立修复是否适用

| 旧提交 | 当前起点核对 | B0 处理 |
| --- | --- | --- |
| `5215326d` / `4ecedac9` / `b037075a` | 对象、Error 和布局发布的 quarantine 合同已在 #89 采纳，有现存故障见证 | 保留现实现和测试，不重复引入 |
| `375d9462` / `f23e9d68` | native 输入在 callable 提升与帧观察期间的 guard 已在 #89 采纳 | 保留 |
| `2cb1eb5e` | Date 的 owned 入口泄漏 receiver/newTarget；移植见证前 5/5 失败 | `3173c128`，借用分派通过现有 invocation 退休入口；5/5 通过 |
| `25c152d6` | ArrayBuffer、SharedArrayBuffer、DataView 同类遗漏；移植见证前 7/7 失败 | `d56be191`，统一借用入口与 owned 退休；7/7 通过 |
| `e49eba17` | callee 认证在登记输入前失败；retain/借用拒绝遗漏清理，破坏性清理未停止后缀；3/3 失败 | `010b9fa3`，先登记输入，使用当前可用 State 直接退休，借用忙则保留现有协调队列；3/3 通过 |
| `82e107ff` 的满窗口错误发布 | 旧新建 raw computed 发布需要先消费原操作数。#89 当前 `throw_error` 返回独立 Completion，不往满窗口 push 错误 | 不移植旧发布协议；新增满窗口 nullish 读取见证，含保留 receiver 的入口，原 TypeError 和转换顺序通过 |
| `673e91ca` | 修复 `RawNativeQuery` 的 resident parent 池，该类型及生命周期未被 #89 采纳 | 不适用；不引入旧 resident Query。现有空 Query 池继续由现存存储测试覆盖 |
| 其余结构提交 | 与已弃用 State/native/read/Set 运输接口绑定，非独立基线 bug 修复 | 不 cherry-pick；行为与故障义务按残留消费者编号在迁移批次落实 |

这些是正确性修复；没有给 Date/buffer 的冷路径声明 benchmark 加速。
清理保留原输入/结果别名与错误优先级，poison 后不访问剩余状态。

## 清单二：行为见证保留情况

515 个新增 test 名称逐项索引；88 个在 #89 原路径有同名见证。
同名是定位证据，不代替等价性审查。B0 另移植 15 项独立故障见证、
26 项不依赖旧内部 API 的 JS 行为见证：

| 当前文件 | 项数 | 覆盖 |
| --- | ---: | --- |
| `builtins/date/owned_invocation_tests.rs` | 5 | receiver、新目标、原型抛错、别名、handler error 与 poison |
| `builtins/array_buffer/owned_invocation_tests.rs` | 7 | buffer/DataView owned 生命周期、独立返回 owner、错误清理 |
| `builtins/dispatch/input_admission_tests.rs` | 3 | checked callee retain 拒绝、借用冲突、破坏性后缀隔离 |
| `builtins/array/mutation/behavior_tests.rs` | 11 | push/pop/shift/unshift，各值种类；hole、继承 getter、Proxy 顺序、部分写入、mapped arguments、嵌套回调和抛错 |
| `api/runtime/behavior_asset_tests.rs` | 15 | define/length/typed resize、共享 buffer、getter this、bound 回调、Date hint、computed Proxy、delete、追加与不同 receiver |

另新增一个当前 API 的满窗口错误见证与一个消费者计数见证。
最终 profiling 核心测试 **2518/2518** 通过。

其余 386 项仍作为旧实现合同的源码索引保留：不能把它们算作当前通过的
测试，也不把旧 `start_in_state`/`RawQuery` 适配器为移植测试重新引入。
涉及 native 生命周期的义务归 B1，read/key/Set/define 归 B2a–f，剩余内建
和模块/挂起归后续 B3/B4 的复盘清单。当前 JS 行为 witness 已单独保存，
未来每个消费者还需要当前 API 的 retain、容量、部分发布和 poison 见证。

## 原始证据

目录：`/home/eric/.cache/oxide-runtime-core-20261003/`。

- 修复前：`b0-original-owned-invocations.log`、`b0-original-input-admission.log`。
- 定向修复：`b0-date-retirement-tests.log`、`b0-binary-buffer-retirement-tests.log`、`b0-input-retirement-tests.log`。
- 容量见证：`b0-capacity-witness.log`。
- 最终核心测试：`b0-final-assets-core.log`。

性能和诊断回执见 [B0 基线](runtime-b0-baseline.json)。

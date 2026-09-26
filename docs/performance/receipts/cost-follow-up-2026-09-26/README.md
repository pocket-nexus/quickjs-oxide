# 继续削减恢复、调用与静态解码成本

本轮承接[上一轮收据](../follow-through-2026-09-26/README.md)中的构造开销、零参数调用小幅回退与热路径静态解码问题。维护者要求继续实现并减少 benchmark；本轮仅执行三个已有固定工作量探针各四个进程，没有运行原版 V8 Score 矩阵或再次运行完整 Crypto 诊断。

## 实现与证明边界

- **数组恢复**（`d884da59`）：删除堆层 `selected` 位图及其分配、清零，把索引映射与命名属性顺序验证合成一次逻辑顺序扫描。唯一 atom、逐索引映射相等和精确命名项数共同证明无重复、无遗漏。Runtime 直接分类 immediate index，将命名 entries 的预留从所有槽数降到 `slots - length`；多出的命名项证明存在孔，在 push 前拒绝，避免隐式扩容。字典插入顺序、提交前预留、owner 移动与提交后清理协议保持原边界。
- **普通调用**（`131c0262`）：非 method 路径复用入口已读取的 callee，省去重复 `peek`。Method 的 receiver、参数、callee 校验顺序保持；ordinary witness 最后一次 callee 验证仍在，其删除需要更强的接口证明。此次改动不能自动解释上一轮 +0.37% 的全部来源。
- **发布增减方向**：计划字节 3/4 表达后缀／前缀递增，空闲 14/15 表达对应递减。发布器核验精确 opcode、同槽写回和控制流；执行器从 kind 读取方向，避免再次读取 `code[2]` 选择增减。`PublishedDenseEntry` 的大小、计划元素宽度与 handler 层级未增加。种类分支仍承担方向选择，动态 Number、binding、数组、容量和提交检查仍执行；四种方向保留原 PC 回落。没有把整条跨度称为零成本。
- **诊断质量**：profiling 构建把 `array_materialized` 细分为当前 own slot 的默认数值、非默认描述符、非数值、访问器、特殊槽、缺失索引，以及覆盖范围／布局不可读。查询不触发 getter、原型遍历、atom 创建或 retain/release；它描述失败时的局部状态，不证明整个数组为何未恢复。跨版本比较须聚合 `array_materialized.*`，详见[诊断口径](../../../profiling.md)。

## 普通 release 的机器码证据

- **调用**：`ordinary::enter_selected` 内联在 `ready::enter_call` 中。旧版非 method 普通分支在 `0x100419c58–0x100419c78` 重复地址／边界／tag 检查；新版 `0x100419a94` 跳过第二次读取并复用第一次结果。已选 native 分支的第二次 `FrameTransaction::peek` 也只在 method 路径保留。见[调用机器码收据](codegen-call.json)。
- **更新方向**：旧版浮点／整数更新路径分别在 `0x100421b6c`、`0x100421d3c` 读取 `code[2]` opcode；新版更新路径不再读取，改为比较已发布 kind。入口 ABI、`0x160` 栈帧和独立 dense helper 数量保持。`try_numeric_span` 到下一符号的布局范围从 3,376 B 增为 3,564 B（增加 188 B），这不是精确函数大小；kind 比较和条件选择仍有成本。见[方向机器码收据](codegen-dense.json)。

以上只证明特定工作从生成代码中消失，不把静态代码差异当作净指令／耗时收益。

## 验证与限制

源码 `c7fb5b69d88bb5e6170fab3b8af5a3062fd4cb72` 通过 Rust 1.88 的 profiling 库 **2,152 / 2,152** 项测试、profiling lib/tests Clippy `-D warnings`、626 个源文件的结构检查。随后仅格式化两处新测试数组，格式检查通过；未重跑全量 Test262。前一提交 `c6836110` 的 fast／test262-focused CI 成功不能当作本轮改动的覆盖。[验证记录](validation-summary.json)保留命令、日志哈希与测试编译／格式修正的先前失败记录。

普通 release 二进制使用 Rust 1.96.0、fat LTO、单 codegen unit，由干净 `c7fb5b69` 构建；基线为上轮修复 helper 后冻结的 `44f6d340`。沿用上一轮 manifest 中的三个探针，每项 ABBA、每版本两样本，共 **12 个有效进程**。记录包含启动、JS 编译、执行、退出的整个进程成本。

| 固定负载 | 退休指令中位数变化 | cycles 中位数变化（受干扰） | wall 中位数变化（受干扰） |
| --- | ---: | ---: | ---: |
| 倒序构造数组 | −0.588% | −0.704% | −8.378% |
| 四参数调用 | −0.178% | −0.831% | −1.462% |
| 零参数调用 | −0.275% | −1.967% | −26.484% |

零参数调用的 baseline wall 为 296.49 / 178.51 ms，清楚显示同机干扰。此次没有新增 A/A，三个指令变化都很小，**不作稳定收益或时间准入结论**。也不与上一轮不同批次的百分比连乘来宣布累计回退已经消失。RSS 变化分别为 −0.289 / +0.023 / −0.109 MiB；全部原值、二进制／工具链身份和核验规则见[短探针摘要](fixed-summary.json)。没有单独测增减方向的性能收益或 method 调用开销。

原始产物位于 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up`。正式 Score、整体时间准入、剩余 Crypto materialized 失败的真实细分分布，以及此前独立入口的 no-plan 回退仍未裁决；没有据此宣布整体 V8 加速或 3–4 倍目标实现。

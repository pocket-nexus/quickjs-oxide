# 继续实施：数组恢复、静态事实传递与普通调用

维护者澄清：跳过过多 benchmark，继续推进实现。本轮没有恢复原版 V8 Score 矩阵；验证限定为库测试、6 项短探针共 24 个进程、一次 Crypto 固定逻辑诊断，以及修复明确回退后的 4 个定向进程。

## 实现

| 提交 | 改动 | 已删除的工作／修复的问题 |
| --- | --- | --- |
| `7c0e1787` | 完整倒序填充后恢复 Array dense 存储 | 新建索引 0 后一次验证完整索引集合与默认 C/W/E data 描述符，把已有 owner 从普通槽移动回 dense；后续读取直接使用已有 dense 入口。 |
| `0e700fa4` | 将首操作数从 opcode 派发传给 Dense handler | 私有 `PublishedDenseEntry` 绑定发布的跨度种类和已经解码的 Local／Argument 槽，首槽不再重新分类 canonical opcode；动态 binding、类型、索引和数组状态检查保留。 |
| `fa964532` | 普通调用复用一次性操作数校验 | 参数校验同时计算是否存在非标量参数；安装帧消费不可复制的证明，删除第二遍槽扫描。Native／General 回落、原参数 owner 保留和失败回滚保持原协议。 |
| `44f6d340` | 修复首操作数改动引入的额外调用层 | 非首操作数的 `peek_number` 恢复单层执行。反汇编确认额外 `bl peek_proven_number` 消失，成功路径尾跳到已有 dense 读取函数。 |

数组审查同时修复了一个正确性问题：dictionary shape 删除后，物理槽顺序可能不同于命名属性的插入顺序。稀疏数组截断和 dense 恢复现在均按 `ordered_indices()` 重建命名属性，避免改变 `Reflect.ownKeys` 的字符串／Symbol 顺序。新增回归测试覆盖该情形。

### 事实的作用域与 owner

- **数组恢复**：只在成功新增索引 0 的定义边界尝试，扫描最多 8,192 个物理槽。这是可调整的成本策略；仍有孔、非默认 indexed descriptor、超出上限或索引 0 未最后补齐的数组继续使用普通存储。读取路径没有增加恢复探测。
- **准备与提交**：同一 runtime state 借用内核验索引、描述符和新 shape；准备阶段显式内存预留失败返回不恢复。发布前完成预留和新 shape retain，然后移动 Object／String／BigInt／Symbol owner，释放旧 shape，失效原型布局缓存。没有 getter、JS 重入或元素 retain/release 循环；提交后的内部清理错误仍传播。
- **Dense 首操作数**：证明只属于当前函数、当前 opcode 派发，不能跨帧或回调复用。原 canonical PC、动态失败和回落顺序保持不变。其余操作数的静态解码仍未全部消除。
- **普通调用**：证明只沿 ordinary selection → authenticate → install 的私有链消费，中间不写调用方槽、不重入 JS。window identity／depth 在提交前再次检查；这两个数值本身不能证明任意中间写入之后的内容不变。参数实际值的动态分类仍执行一次。

## 短探针：保留回退，再修正具体原因

普通 release，Rust 1.96.0，Apple M1。基线是已冻结的 `8c38bc32`；初始候选为 `828bc677`（前三项实现加探针）。该基线与上一轮最终代码之间的 VM 差异是 profiling 诊断与测试，本轮仍按各自真实提交和二进制哈希标识，不宣称二进制完全相同。

两次重复／版本、逐项 ABBA，共 24 个有效样本。统计整个进程（启动、JS 编译、执行、退出）的退休指令。未新增 A/A，不把小幅变化判为稳定收益；wall 和 cycles 均是受同机干扰的观测。

| 固定负载 | `828bc677` 相对基线的指令中位数变化 |
| --- | ---: |
| 倒序填充后反复读取 | −80.41% |
| 反复倒序构造数组 | **+1.43%** |
| 四个标量参数调用 | −0.28% |
| 既有 dense 数组读取 | **+1.23%** |
| 零参数调用 | **+0.37%** |
| 整个函数无融合计划 | −0.005% |

数组恢复有构造成本；小幅调用变化不足以给调用架构整体下结论。[全部原始值和构建身份](focused-summary.json)保留在本轮收据中；[探针 manifest](focused-manifest.json)的期望值由数学定义给出，新探针另经 Node 单次校验。

对已有数组读取的回退进行了生成代码定位：`828bc677` 的非首 `peek_number` 多调用了一层 `peek_proven_number`。同形态 64 轮逻辑探针确认该位置是 `dense_acc_index_set_drop`，64 次尝试全部命中。首槽证明本身也有 ABI 打包／解包成本，不能把源码删除 `match` 当成净收益。

`44f6d340` 删除额外嵌套调用后，仅重测该项 4 个 ABBA 样本，退休指令中位数变为相对基线 **−1.4291%**：基线 `[1493223952, 1486978778]`，修订 `[1472088828, 1465523620]`。wall 为 +3.23%，且该短批与源码结构检查重叠，不能据此作时间准入。其他五项未在该修订上重跑，仍归属初始候选。见[定向复核](array-read-fixed-summary.json)与[机器码身份](codegen-summary.json)。

机器码另确认 ordinary install 的第二遍槽位扫描已消失，改为读取已验证参数分类位；通用 `push_frame_storage` 没有改动。这说明具体删除了什么工作，不说明它占真实负载多少时间。

## Crypto 的真实覆盖

旧诊断 `b4a5f446` 与新候选 `828bc677` 使用完全相同的生成负载 SHA `b10ac2a2a6ae96c9b09e513d02da81858377035a21f0559ea6480dc2eefc8bb3`，原版两个 Crypto benchmark 各执行一次，均完成输出校验。70 个已执行函数的静态 manifest 与候选站点计数一致。

| 逻辑事件 | 旧诊断 | 新候选 |
| --- | ---: | ---: |
| Dense 尝试 | 2,421,003 | 2,420,979 |
| Dense 命中 | 929,995（38.41%） | 1,709,482（70.61%） |
| `array_materialized` 失败 | 1,490,963 | 711,446 |
| 数组恢复 | 0 | 60 |
| VM dense scalar write | 3,551 | 430,245 |

`am3` PC 51 的 dense 读取从 0 命中变为 388,695 / 744,364；`bnpSquareTo` PC 24 的 dense 写入从 0 变为 19,552 / 39,672。157 个规范化调用站点记录完全一致，共 85,340 次已观察调用。融合 omission 全为零；独立的 `native.prepare` omission 仍是 1,073，不能忽略。

旧诊断是 Rust 1.96 profiling release，新诊断复用 Rust 1.88 dev/debug 构建，二者只比较逻辑事件。新二进制未嵌入提交，`828bc677` 来自外部构建命令记录；不冒充自认证 build receipt。诊断耗时和 `owned_instructions` 均不作为机器性能指标。详细身份、遗漏和站点变化见 [crypto-summary.json](crypto-summary.json)。

## 正确性与限制

- 初始三项实现：Rust 1.88 profiling 库 **2,148 / 2,148 通过**，包括新增引用／Symbol／自循环 GC、字典键顺序、描述符、原型 setter、不可扩展、只读 length、Dense 命中／回落、调用错误顺序和 owner 回滚测试。
- 同一实现：profiling lib/tests Clippy `-D warnings`、格式和源码结构检查通过；普通 Rust 1.96 release 构建成功。
- 随后的两行 helper 修正：普通 release 构建、机器码核对与上述 4 个语义有效的定向样本完成；未重跑完整库测试或 Test262。此前的全量 Test262 不覆盖本轮新实现。
- 记录见 [validation-summary.json](validation-summary.json)。所有原始日志、普通二进制、反汇编和诊断保存在 `/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-follow-through`。

本轮证明了数组恢复的真实覆盖和若干具体工作量变化，没有证明整体 V8 分数或 3–4 倍目标。剩余 `array_materialized` 失败、零参数调用成本、上一轮独立入口 no-plan 回退以及其他热路径重复验证继续作为后续输入；当前 8,192 槽策略和接口均可由新证据调整。

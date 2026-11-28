# 十一个独立模块的验证结果

生产基线为 `be767daec3b0632482a96347b058c354ae8b5672`，目标机为 Linux x86-64。下表按原清单的行为目标验收。**11 项中，`value` 的模块内表示行为和 `heap::hash` 的 Hasher 行为有闭环证据，均使用了 Verus 以外的补证。其余 9 项还有开放目标。** 对所有操作码的目录证明已完成，解释器、验证器和反汇编器的全部语义一致性仍单列。这里的“任意”以各合约的前置条件和工具的 Rust 标准库、分配模型为限。[原始验收目标](core-modules.zh-CN.md)

| 模块 | 已经检查的生产行为 | 完整目标的结论 |
| --- | --- | --- |
| `value` | Verus 按独立标签规则证明任意 64 位输入的 `kind`、标志、特殊值、构造器与读取器，以及全部 Int32 和四类堆 ID 的往返。Kani 对任意 64 位输入检查 `number → bits → as_f64`。只有落入装箱区的 NaN 改写为规范 NaN；其他位型逐位保留，包括两个零。任意原始值的数值投影也与独立 binary64 编码相符。 | **模块内表示目标闭合，联合验证。** 调用方的 GC 根纪律和 JS 求值另有目标。Verus 单独翻译浮点入口时止于 `is_nan`、`to_bits` 和 `from_bits`。[Verus 规格](value_proofs.rs) · [浮点与装箱客户端](value_machine.rs) |
| `bytecode::opcode` | 对任意枚举值和 205 行独立冻结的目录，Verus 检查编码、名称、格式、终止标志、表长及按编码反查。编译期穷尽匹配会捕获新增但未审定的变体。VM 的 256 项比较掩码表另有证明。 | **目录闭合，消费者待证。** 目录固定的是本基线的元数据。验证器对末条指令的要求、VM 的实际 PC 转移、反汇编的操作数解释仍需分别与规格对应。例如 `Gosub` 会保存返回 PC 并跳转，而目录不允许它作为最后一条指令。[目录定理](opcode_catalogue.rs) · [验证器](../../core/src/bytecode/validate.rs) · [解释器](../../core/src/vm/interp.rs) |
| `bytecode::builder` | 合法格式和可表示 PC 下，已覆盖的发射器各追加恰好一条指定字段的指令并返回旧 PC；`handler` 追加异常项；`patch_target` 与 `retarget` 只改指定字段。启用机器字合同后还覆盖两种拆分 16 位操作数的发射器。 | **部分。** 要闭合到 `finish`，还需证明提示与回边改写、字段/全局/构造缓存编号及饱和保留项、布局与异常表进入发布的 `CodeUnit`。[状态与发射规格](builder_proofs.rs) · [生产实现](../../core/src/bytecode/builder.rs) |
| `heap::arena` | Verus 检查修改记录附属字节记账时的精确差值及其他状态保持。Kani 在一条含两次分配、标记、显式释放和清扫的轨迹上量化记录值及是否清标记，检查实际回调、存活和释放数。 | **部分。** 任意操作序列的空位、隔离周期、回收次序与收缩仍需统一不变式。容量与实际预留、`Room`、位图更新及回调合同尚未接通。[单槽生命周期](arena_machine.rs) · [生产 arena](../../core/src/heap/arena.rs) |
| `heap::hash` | Verus 对任意初态和任意长度的混合写入序列证明最终 `finish`。每次切片写入独立分块，末块右补零、按小端解释，逐块左旋 5 位、异或并模乘。Kani 检查默认初态为零。Rust 1.100.0 针对本机生成的 LLVM IR 中，其余 10 个 libcore 转发入口与指定的已证写入序列两两指向同一函数定义，包括字符串结束字节和长度前缀。 | **本机 Hasher 行为闭合，联合验证。** 编译器的符号同一性检查补足继承方法。哈希表的碰撞处理属于 hashbrown。[序列规格与可执行客户端](hash_proofs.rs) · [转发检查](hash_dispatch.py) |
| `heap::string` | Verus 将两种平坦表示的长度、非空性和索引读取对应到 UTF‑16 序列。对满足迭代器合同的有限输入，生产 `content_hash` 等于逐码元递推。Kani 对所有 0–10 码元序列检查生产索引内核的十进制语义，并证明长度大于 10 时不读取迭代器。 | **部分。** `from_units` 的全长文本保持后置条件尚未证成；要接通收集到 `Box<[u8]>` 和双字节分支。rope 首次展开与 `OnceCell` 缓存还需 Heap 合同。[平坦表示及哈希](string_proofs.rs) · [十进制索引](string_machine.rs) |
| `heap::values` | 给定有效范围，写入后按相对/绝对地址读回相同值，其他槽位和空闲管理字段保持；还检查越界读空值和切片视图。 | **部分。** `contains` 表示地址界内，活跃所有权仍需由分配器建立。尚缺 alloc/grow/free、拆分、位图合并到 finish_sweep 的不重叠、前缀保持和清空复用证明。[读写客户端](slots_proofs.rs) · [分配与清扫](../../core/src/heap/values.rs) |
| `vm::bigint` | `Big::new` 对任意 limb 向量保持数学整数，删去高位零，避免负零。机器字合同目标把生产 `add_mag` 的进位和 `sub_mag` 的借位按基数 2⁶⁴ 逐字合成；减法要求被减数幅值及 limb 数均不小于减数。 | **部分。** 有符号入口的分派与比较、乘除余、移位、补码运算、幂、浮点舍入及资源错误尚未成为一组闭合合同。[整数解释](bigint_proofs.rs) · [逐字循环](word-arithmetic.patch) · [机器整数检查](words_machine.rs) |
| `builtins::base64` | 对 2–4 个有效 sextet 和任意已有输出，Verus 证明 `decode_chunk` 追加的字节；严格模式拒绝非零填充位时，已有输出保持。 | **部分。** 仍缺生产编码到解码的任意长度往返、任意 UTF‑16/容量/末块模式的 `bytes/read/error` 和 hex 的非 ASCII 行为。单块编解码 Kani 实验在本次 115 秒限时内未得到结果。[位拼接及填充规格](base64_proofs.rs) · [端到端单块实验](base64_machine.rs) |
| `builtins::date::format` | Verus 对八种方法一起检查调用方本地/UTC 字段策略、格式族和日期/时间选择。Kani 从真实格式化入口检查全部方法的无效日期返回，含 ISO 异常分支。 | **部分。** 合法字段到完整文本仍缺字段转换、十进制补零、扩展年份、时区和固定 locale 输出证明。整模块诊断包括字符串容量和格式化写入。[选择证明](date_format_proofs.rs) · [无效日期客户端](date_format_machine.rs) |
| `builtins::date::parse` | Verus 检查任意单码元的字节映射，以及带结尾 NUL、游标有效时空格与分隔符的最长匹配和停止位置。 | **部分。** 127 码元截断与终止符的窗口实验在本次 120 秒限时内未得到结果。完整 ISO 优先/旧式回退、字段、时区、本地标志与失败结果仍需对照固定 QuickJS 语法。[窗口义务](date_parse_machine.rs) · [生产扫描器](../../core/src/builtins/date/parse.rs) |

## 待修复行为：显式释放后复用槽位会继承标记

在空 arena 分配 A，标记 A，显式释放 A，再把 B 分配到同一编号。`free` 只清除存活位，复用槽位的 `alloc` 会保留 A 的标记。默认和全部特性下的原生执行和 Kani 检查均得到下表。[可执行轨迹](arena_machine.rs) · [Arena 实现](../../core/src/heap/arena.rs)

| 复用后、清扫前 | B 的标记 | 清扫释放数 | 回调 |
| --- | --- | ---: | --- |
| 直接清扫 | 已标记 | 0 | 无 |
| 调用 `clear_marks` 后清扫 | 未标记 | 1 | 恰好一次，参数为 B 的编号和内容 |

这个行为影响独立 Arena 的调用方。直接清扫会保留从未标记的 B；此时若先调用 `mark(B)`，按其实现也会得到“先前已标记”的 `false`。当前完整 GC 会在遍历根之前统一清空标记，因此两轮收集之间复用槽位时，旧标记会在下一轮被清除。本次轨迹只在独立 Arena 上重现。[完整 GC 入口](../../core/src/heap/gc.rs#L396) · [标记入口](../../core/src/heap/gc.rs#L476)

建议后续在 `Arena::free` 清除同槽的标记位，并把上面的当前行为检查改成回归证明。两条验收路径都应从同一复用状态出发，中间不调用 `clear_marks()`。

1. 不标记 B 就清扫，恰好释放一条记录，回调恰好一次且收到 B 的编号和内容。
2. 第一次标记 B 返回 `true`，再次标记返回 `false`，随后清扫保留 B，不调用回调。

## 复跑

在仓库根目录运行下列命令。`--independent-bodies` 每次只把一个项目的未标注代码也纳入 Verus；只要有一个模块失败，最终退出码就为 1，并保留各项日志。该检查能暴露翻译和证明缺口，端到端验收以表中行为规格为准。

```sh
python3 dev-support/verus/verify.py --verus /path/to/verus
python3 dev-support/verus/verify.py --verus /path/to/verus --all-features
python3 dev-support/verus/verify.py --verus /path/to/verus --all-features --independent-bodies /tmp/oxide-verus-modules
python3 dev-support/verus/machine.py
python3 dev-support/verus/verify.py --verus /path/to/verus --word-arithmetic
```

Verus 使用 release `0.2026.11.22.1ce6a59`、Rust `1.100.0`。Kani 使用 `0.68.0`、CBMC `6.11.0` 及其配套 nightly。各工具都读取同一工作树的生产函数体。机器字目标在 Verus 中明确导入五个 libcore 合同，先用 Kani 对全部参数位型检查这些合同，再运行带合同的 Verus 证明；这里还依赖两个工具链中这些标准整数操作的语义相同。独立运行默认 Verus 目标不会导入这五个合同。[机器字合同](word_proofs.rs) · [验证入口](verify.py)

| 执行范围 | 结果 |
| --- | --- |
| 基础 Verus 合同，默认及全部特性 | 两次均为 461 verified，0 errors |
| 先检查五个机器字合同，再运行 Verus，默认及全部特性 | 两次均为 476 verified，0 errors |
| 默认 Kani 补证集 | 11 个证明客户端全部通过；其中 `Arena` 生命周期另在全部特性下通过 |
| Rust 1.100.0 在 x86-64 上的 Hasher 转发 | 每种特性配置均有 10 对同一 LLVM 函数定义 |

Kani 默认运行已取得结论的证明客户端。两个有界实验可分别用 `machine.py --harness encoded_quantum` 和 `machine.py --harness scanner_input_window` 重新运行。

默认及全部特性分别按模块纳入所有函数体时，`heap::hash` 和 `bytecode::opcode` 退出 0，其余九项退出 1；目录检查成功仍留下跨消费者目标。全 crate 的两个配置均在函数指针与动态回调类型翻译处给出 19 条诊断。作为负对照，临时把哈希旋转量从 5 改成 4、把 Nop 格式从 None 改成 A，各自使 Verus 证明失败；把字符串适配器参考结束字节从 255 改成 0，LLVM 定义同一性检查失败。临时改变规范 NaN 的最低位，Kani 也在生产构造器的位值断言上报错。[Verus 对规格与工具范围的说明](https://verus-lang.github.io/verus/guide/overview.html)

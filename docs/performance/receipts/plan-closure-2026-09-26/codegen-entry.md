# Entry choice 撤回候选的限定机器码核查

本收据只比较 plain release 二进制中 `run::run` 的 `GetLocal` 无计划路径。它是静态反汇编核查，不是 PC 采样或逐指令执行轨迹。

## 身份与工具

| 版本 | 源码提交 | plain `qjs` SHA-256 | build receipt SHA-256 |
|---|---|---|---|
| 保留 single-choice 的 Current | `c7fb5b69d88bb5e6170fab3b8af5a3062fd4cb72` | `b241d0252cfe19d54b18843ae106f91c4cb66f2d7f462f120ce21e2d643f3573` | `469a6befb0fe6344739d60f5a8bff697d82b190bc81ee4a8a2e849dcadd64fb3` |
| 窄撤回候选 Entry Revert | `ddf12f8f6b597222e24b0bddda6d981a31e91399` | `d53aaefb2664436c6c0e6c6c4de48140608876c6f149853bbf2362db9a20c50c` | `e106c5084973d3510e3ae899a95404cfe34a81acbeff1b05d3730132066ad761` |

Current 二进制：`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up/build-plain/release/qjs`；build receipt 为该路径加 `.build.json`。撤回候选二进制：`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/build-entry-revert/release/qjs`；build receipt 同理。两份 receipt 均记录 `mode=plain`、无 feature、构建退出码 0、Rust 1.96.0、Rust 内置 LLVM 22.1.2、`aarch64-apple-darwin`、fat LTO、单 codegen unit。

反汇编工具是 `/opt/homebrew/opt/llvm/bin/llvm-objdump` 与同目录 `llvm-nm`，`llvm-objdump --version` 报告 **Homebrew LLVM 23.1.1**，与 Rust 内置 LLVM 版本不同。

核对的固定输入是 `docs/performance/probes/fixed/fusion_no_plan.js`，SHA-256 `b4eeccb8624c086b664f7cb26741e82a946ca155fb2e738fbba46e372f172b7c`。它使用 `while(n)` 和 `sum=plain`；发布器定向测试已证实此修订输入的 `FusionPlan` 为 `None`。早期同名的 `while(n>0)` 输入不能视作无计划样本。

## 可复现的只读命令

```sh
shasum -a 256 /Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-cost-follow-up/build-plain/release/qjs /Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-plan-closure/build-entry-revert/release/qjs
/opt/homebrew/opt/llvm/bin/llvm-objdump --version
/opt/homebrew/opt/llvm/bin/llvm-nm -n BINARY | rg 'run3run17h|RunSlots5local17h|RunSlots4push17h'
/opt/homebrew/opt/llvm/bin/llvm-objdump --disassemble-symbols='__ZN13quickjs_oxide6engine2vm3run3run17h7ab689084215b178E' BINARY
```

将 `BINARY` 分别替换为上列两个绝对路径。实际核查将最后一条输出分别暂存为 `/tmp/oxide-c7-run.disasm` 与 `/tmp/oxide-ddf-run.disasm`，并按下列地址选取片段。`llvm-nm` 对这些 Mach-O 符号报告的 size 为 0；本收据不以此推断函数长度或动态指令数。

## 观察到的路径

两个 `run::run` 符号均从 `0x100414750` 开始。`GetLocal` 的计划准入块均从 `0x100414aa0` 开始；指令形状相同，仅寄存器与跳转目标不同：

```asm
Current  100414aa0: ldp  x10, x11, [x8, #0x10]
         100414aa4: cmp  x10, #0x0
         100414aa8: ccmp x19, x11, #0x2, ne
         100414aac: b.hs 0x100417700

Revert   100414aa0: ldp  x9, x10, [x8, #0x10]
         100414aa4: cmp  x9, #0x0
         100414aa8: ccmp x19, x10, #0x2, ne
         100414aac: b.hs 0x100416084
```

对 `FusionPlan=None`，零计划指针使 `ccmp` 采用立即数标志并令 `b.hs` 跳向 canonical 入口；该路径不读取逐 PC flag，也不执行 five-selector 选择或 fusion helper。两个目标均直接准备参数并调用 `RunSlots::local`：

```asm
Current  100417700: ldp  x0, x1, [x21, #0x28]
         100417704: ldp  x2, x3, [x28, #0x168]
         100417708: ldrh w4, [x26, #0x2]
         10041770c: bl   0x10041cfe0  ; RunSlots::local

Revert   100416084: ldp  x0, x1, [x21, #0x28]
         100416088: ldp  x2, x3, [x28, #0x168]
         10041608c: ldrh w4, [x26, #0x2]
         100416090: bl   0x10041d028  ; RunSlots::local
```

`RunSlots::local` 和随后读取值类型的 canonical 首段指令形状相同。对本例 `plain=41` 的整数复制，primitive 分支在 Current `0x1004178a4`、Revert `0x1004173c8` 起，同样依次执行 `mov #0xa`、`strb`、`cmp #7`、`b.hi`、`lsl`、`tst #0x9c`、`b.eq` 后进入值搬运；后续两侧均调用 `RunSlots::push`。普通派发尾部也保持同样的 `ldr/add/ldr/ldr/cmp/b.lo` 形状，分别位于 Current `0x1004186a4–0x1004186b8` 与 Revert `0x1004186cc–0x1004186e0`。

## 结论边界

这些片段排除了“无计划 `GetLocal` 在 Current 准入块上多执行了逐 PC flag 或 selector 检查”这一解释。它们**没有定位整个 `fusion_no_plan` 循环的退休指令差**：没有 PC 执行轨迹，其他 opcode、寄存器分配、栈搬运和二进制整体代码生成变化尚未逐路径核对。既有短对照中撤回候选相对 Current 的 `fusion_no_plan` 退休指令约低 0.1644%，这是整进程测量，不能从上面四条准入指令单独归因。也不能用“布局噪声”解释已测得的退休指令差。

## 有候选的 `object_move` 循环：compare 与 update

这部分仍只读上表两份 plain 二进制，检查 `run::run` 中已发布 flag 的入口。另有一份**不同二进制的逻辑 profile**：`/Users/eric/Documents/Benchmarks/quickjs-oxide/2026-09-26-task1-3/object-logical-profile/cost.jsonl`，输入 SHA-256 同为 `48359d0b…`。它记录 `work` 函数 PC 10 的 compare 候选 1,000,001 次命中、PC 23 的 update 候选 1,000,000 次命中、PC 20 的静态非候选读 1,000,000 次；四类 omission 都为 0。此记录只证明这份输入确实反复经过 compare/update 候选，不是 c7/ddf 的 PC 计时或分支计数。

两份 plain 二进制在 `0x100414aa0–b4` 均执行一次 fusion 表的范围检查和 flag 字节读取。**selector 没有新增间接跳转**：inspect 到的 flag 分类及 compare/update 入口均由直接条件分支连接。`run::run` 的字节码主派发仍另有 `br x11` 跳表，不能把它算成 LocalFusionChoice 的派发。

| 候选入口 | c7 single-choice | ddf independent selectors | 静态差异 |
| --- | --- | --- | --- |
| compare（flag `0x21`） | `0x100415304` 比较 `0x21`，直跳 `0x100416df0` | `0x1004152dc` 比较 `0x21`，直跳 `0x100415350` | c7 在统一分类后直接选 compare；ddf 先经过 local-add/update 的独立探测。 |
| compare 内部 | `0x100416df4–e2c` 做长度及后续 opcode 检查 | `0x100415354–3b0` 重新加载 code 长度/字节并检查后续 opcode | 具体生成代码及寄存器生存期不同；两侧均有直接分支与动态检查。不能把源码中一个 `match` 等同于机器上的单个分支。 |
| update（flag bit `0x10`） | `0x1004152f8` `tbnz w10,#4` 跳到 `0x10041531c`，随后 `tst w10,#8` | `0x10041523c` `tbz w22,#4` 决定是否跳过，随后 `0x100415240` `tst w22,#8` | c7 先有 `cbz w10` 的 canonical 短路；ddf 先查 local-add，再进入 update 探测。两侧 update 数值读写体均为展开的直接分支代码。 |

这确认了**条件分支拓扑、代码位置和寄存器分配确有变化**，也确认 single-choice 并未在 compare/update 入口引入额外的 `br` 间接派发。抽取的入口未见“先写出 flag 到栈、再载入作选择”的成对 spill/reload；它们分别以 `w10`、`w22` 持有 flag。完整 `run` 有其他栈槽搬运，这里不把它们归为 selector spill。我们没有 PC 采样、分支预测或缓存计数，无法由这些反汇编片段判定 +cycles 的微架构原因。尤其 ddf 的部分静态检查看起来更多，但其已测 cycles 较低，这本身反对仅按源码检查数或退休指令数预测耗时。

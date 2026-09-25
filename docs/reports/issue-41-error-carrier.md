# Issue 41 验证：成功路径 ABI 成本审计与紧凑错误载体小实验

> 2026-09-25 R0 复验：现行候选为 `04bb1a74`（仅新增测试的后继为
> `32f61560`），父版本为 `2ed79f46`。与下文 `fd9b4eac` 历史实验
> 分开看。Rust 1.94.1 普通 release 使用 fat LTO、CGU=1；13 个原始固定
> 负载在两轮 A/A、ABBA/BAAB 中全部减少退休指令，但 `prop_write` 的
> cycles 在两轮分别增加 11.04%／10.95%，`string_build1` 增加
> 4.42%／3.94%。错误分配探针每 100,000 次构造由 100,000 次／1.3 MB
> 增至 200,000 次／9.3 MB；保留模式的 RSS HWM 中位数由 13,312 增至
> 15,652 KiB。原始数据和构建回执在本仓的
> [门禁收据](../performance/receipts/gates-2026-09-25/README.md)。因此下文历史“正结果”
> **不是当前基线上的性能结论**。回退已记录，数组实施不因此停止。

本次恢复版的 Rust 1.88.0 `--workspace --all-targets` 测试通过；
R0 与候选 release 二进制对 14 个错误场景、未捕获异常和语法错误的
stdout/stderr/退出码逐字节一致。全量 Test262 在清除调用方 `GIT_*`
环境后运行，冻结结果正文匹配：102,037 变体中 80,010 pass、
3,552 既有 fail、3,502 unsupported、18,475 skipped；
80,010／80,060 eligible。新增测试还编译验证 `with_span` 保持
`const fn` 公共契约。全量 V8 性能重测仍待独占测量窗口。

> 恢复说明：本报告原属 `0cd4acee`，以下数值是 `fd9b4eac` 对该提交的历史实验，不能作为从 R0 `2ed79f46` 恢复后的正式性能验收。当前代码已恢复原候选并补公共错误语义测试；空载 cycles、错误分配/RSS 已按 [统一测量协议](../performance/measurement.md) 补测，完整 V8 仍待串行完成。

代码审查发现一项与 `prop_write` 回退有关、但尚未证明因果的生成代码变化：
R0 中 `RunSlots::property_ic_write_scalar` 保留约 2.5 KiB 的独立符号，
候选的 fat-LTO 产物中该符号消失；`run::run` 从 `0x9202` 增至
`0xb310` 字节，栈帧从 `0xea8` 减至 `0x888`。成功的 `PutField` 路径
不会构造公共 `Error`，因此新增的错误 Box 分配不能直接解释这一正常
循环的 cycles 回退。独立分支 `perf/issue-41-prop-write-outline` 的
`0ac97a83` 只让该属性写入 wrapper 保留为调用边界，用于检验内联变化；
未经 A/B 前不把代码尺寸或栈帧变化当作速度结论。

恢复版在独立 target 上通过：`error.rs` 4 项、ordinary 布局 3 项、
runtime exceptions 6 项、CLI evaluation 9 项，以及 Rust 1.88.0 的
`clippy -p quickjs-oxide --lib -- -D warnings`、格式检查。分配探针已独立
`cargo check` 并以每模式 3 次的非性能 smoke 验证输出；这些结果不替代
完整 workspace/Test262 门禁。

> 原实验状态（`fd9b4eac` 基线）：**当时的小实验正结果，不能作为当前接纳建议**。机器码确实变短（`.text`
> −5.84%、`run` 栈帧 −41.8%、宽返回边界成批内联），固定负载 instructions
> 全面下降；错误语义与全量门禁通过。
>
> 基线：`fd9b4eac`（main）。构建协议：
> `cargo build --release -p quickjs-oxide-cli --locked`（fat LTO + CGU=1，
> 无 PGO）。测量：`taskset -c 2` + `perf stat -e instructions,cycles`
> （perf 因 `perf_event_paranoid=2` 自动折算为 `instructions:u`）。
>
> 关系：原实验对应历史 [s3-b-plan.md](https://github.com/pocket-nexus/quickjs-oxide/blob/f531f6052cb497ce4707f01c276e8642e5e26788/docs/reports/s3-b-plan.md) §7 决策 3 的“B2.5 独立 spike”；当前收尾以 [新实施设计](../performance/implementation.md#a-error) 为准。

## 0. TL;DR

1. `Error` 80B → 8B（`Error(Box<ErrorData>)`）。`Result<(), Error>`
   80B → 8B；`Result<bool, Error>` / `Result<&T, Error>` /
   `Result<JsValue, Error>` 80B → 16B。8B/16B 均走寄存器返回，不再是
   隐藏内存返回槽。
2. 机器码确实变短：release `.text` 8,258,940 → 7,776,372 B（−482,568,
   **−5.84%**）；全部保留符号尺寸和 −6.59%；`run::run` 栈帧
   0xea8（3,752B）→ 0x888（2,184B，**−41.8%**）；80B 结果判别写入站点
   637 → 182（**−71.4%**）。
3. 固定负载 instructions（retired，确定性指标）13 项全部下降：
   B2.1 已特化的 `empty_loop`/`int_local`/`prop_read` 只降
   0.22%–0.57%，仍在走可失败 helper 的通用路径降 3%–9%
   （`bigint32` −9.14%、`call0` −6.16%、`array_read` −4.59%、
   `array_write` −4.61%、`prop_write` −4.53%、`bigint64` −4.37%、
   `bigint256` −4.31%、`string_build1` −3.02%）。
4. 错误语义不变：14 个错误场景与 CLI 诊断、退出码逐字节一致；既有
   容量/TDZ/异常传播/资源限制测试全绿。
5. 全量门禁通过（Test262 full 见 §7）。

**原实验裁决**：假设“正常路径机器码确实变短”成立，按 issue 第 3 条满足扩大
迁移的前置条件。但收益分布说明扩大对象应是**仍保留宽 `Result` 边界的
通用路径**（数组/属性/调用/字符串），而不是已经被 B2.1 覆盖的热循环。
本 issue 范围到此为止，不把扩大迁移并入本次改动。

## 1. 实施计划（确认版）

1. 在 `fd9b4eac` 上构建 release `qjs` 作为基线，记录尺寸与保留符号。
2. 把 `Error` 的 payload 冷置为 `Box<ErrorData>`；公共方法、`Display`、
   `Debug` 输出格式、`Clone/Eq/PartialEq` 语义不变。
3. 反汇编对照：返回方式、栈帧、判别写入站点；固定负载 A/B。
4. 错误语义核对 + `fmt/clippy/test/Test262/source-layout` 全量门禁。
5. 正/负结果写入本报告；正结果才讨论扩大迁移。

## 2. 尺寸事实（`size_of`，临时探针后固化为断言测试）

| 类型 | 修改前 | 修改后 |
| --- | ---: | ---: |
| `Error` | 80 | **8** |
| `Result<(), Error>` | 80 | **8** |
| `Result<bool, Error>` | 80 | 16 |
| `Result<&u8, Error>` | 80 | 16 |
| `Result<Option<u16>, Error>` | 80 | 16 |
| `Result<JsValue, Error>` | 80 | 16 |

改动：`Error(kind, message, native_message, span)` 的四个字段移入
`ErrorData`，`Error` 只持有 `Box<ErrorData>`。`Result<(), Error>` 借助
Box 的空指针 niche 编码 Ok/Err（Ok = 空指针），保持 8B；
`Result<bool, Error>` 等 16B 结果按 SysV 用 `RAX:RDX` 返回。
`Debug` 改为手写以保持原有 `Error { kind: .., message: .., .. }` 输出。

## 3. 保留调用边界审计（任务 1 结果）

方法：对 release `qjs` 做
`nm --print-size --size-sort --demangle` 与
`objdump -d --demangle --section=.text`，只比较两边都保留或消失的符号。

| 符号 | 基线字节 | 修改后 | 说明 |
| --- | ---: | ---: | --- |
| `SlotStore::push_current` | 0xf8 | 消失（内联） | 返回 `Result<(), Error>`，基线写 80B 隐藏槽 |
| `SlotStore::local_current` | 0x11c | 消失（内联） | 返回 `Result<&FrameBinding, Error>` |
| `SlotStore::parameter_current` | 0x121 | 消失（内联） | 同上 |
| `SlotStore::replace_local_current` | 0x128 | 消失（内联） | 同上 |
| `SlotStore::pop_current` | 0x1a4 | 消失（内联） | 同上 |
| `RunSlots::array_immediate_read` | 0x986 | 消失（内联） | issue 点名 API，`Result<bool, Error>` |
| `RunSlots::property_ic_write_scalar` | 0x9b9 | 消失（内联） | issue 点名 API，`Result<bool, Error>` |
| `RunSlots::ordinary_field_immediate_read` | 0x20a | 0xee | −52% |
| `RunSlots::insert_copy` | 0x538 | 0x337 | −38.9% |
| `FrameTransaction::peek` | 0x94 | 0x55 | −41.5% |
| `RunSlots::peek` | 0x9e | 0x62 | −37.4% |
| `SlotStore::push` | 0x193 | 0x22e | 吸收 `push_current`/`check_current` |
| `SlotStore::local` | 0x1bc | 0x22c | 吸收 `local_current` |
| `copy_reference` | 0x488 | 0x60f | `inline(never)` 冷路径；指令 271→351 |
| `run::fusion::update_local` | 0x2e9 | 0x3e3 | 融合 handler，吸收不可失败快路 |
| `run::fusion::numeric_local_add` | 0x20b | 0x20b | 已不可失败，不变 |
| `run::fusion::local_compare_branch` | 0x323 | 0x323 | 已不可失败，不变 |
| `run::fusion::numeric_local_field_add` | 0x369 | 0x369 | 已不可失败，不变 |

审计结论与 issue 预设的差异：

- issue 点名的 `RunSlots::binary_number`、`update_number_local_current`
  在基线的 fat-LTO 产物中**已经没有独立符号**（已被内联进调用者），
  所以“每次 `?` 都是内存返回”对它们并不成立；真正保留的宽边界是
  `push_current`/`local_current`/`parameter_current`/
  `replace_local_current`/`pop_current` 与
  `array_immediate_read`/`property_ic_write_scalar` 等。
- 这些保留边界在装箱后全部变成可内联或被吸收，`run::run` 因而变大
  （37,378 → 45,840 B，指令 8,633 → 10,887），但全符号总量仍下降
  6.59%——即“把调用边界的返回槽搬进热函数”换来跨函数的净缩短。
- B2.1 的 S1/S2/S3/S4 融合 handler 本来就返回 `Option`，不受本次改动
  影响，这解释了热循环收益很小（§5.4）。

## 4. 反汇编对照

### 4.1 Ok 路径返回方式

基线 `SlotStore::push_current`（`%rdi` 为隐藏返回槽指针）：

```asm
; 成功路径
movups (%r8),%xmm0
movups %xmm0,(%rsi)
inc    %rax
mov    %rax,0x40(%rcx)
movq   $0x2,(%rbx)      ; 向 80B 返回槽写 Ok 判别
pop    %rbx
ret
; 失败路径在 0x28..0x48 偏移构造完整 80B Error
```

修改后（`push_current` 内联进 `SlotStore::push`）：

```asm
; 成功路径
movups (%rdx),%xmm0
movups %xmm0,(%rcx)
inc    %r8
mov    %r8,0x40(%rsi)
xor    %eax,%eax        ; Ok = 空指针，寄存器返回
add    $0x50,%rsp
pop    %rbx
ret
```

`SlotStore::local` 修改后的成功路径同样是寄存器返回
（`Result<&FrameBinding, Error>` 16B，`RAX:RDX`）：

```asm
add    %rax,%rdx
xor    %eax,%eax
add    $0x50,%rsp
pop    %rbx
ret
```

### 4.2 栈帧与判别写入

| 指标 | 基线 | 修改后 | 变化 |
| --- | ---: | ---: | ---: |
| `run::run` 栈帧 `sub $imm,%rsp` | 0xea8（3,752B） | 0x888（2,184B） | −41.8% |
| 全 `.text` 内 `movb $0x5,0x48(...)`（80B 结果判别写入） | 637 | 182 | −71.4% |
| `.text` 字节 | 8,258,940 | 7,776,372 | −5.84% |
| 可执行段合计（`size` dec） | 8,497,192 | 8,017,952 | −5.64% |
| `nm` 全符号尺寸和 | 6,904,068 | 6,448,865 | −6.59% |

修改后的 182 处 `$0x5,0x48` 主要是 `ErrorData` 装箱冷路径本身
（栈上构造后拷入 80B 堆块），不再是调用边界的隐藏返回槽。

## 5. 固定负载 A/B

### 5.1 协议

- 同一构建协议、同一机器、`taskset -c 2`；每项 5 样本取中位；
  A/B 交替顺序（第 i 样本 A→B，i+1 样本 B→A）。
- instructions 为主信号（同一二进制下 A/A 差值为 **0.00%**，确定性）；
  cycles 为辅。

### 5.2 结果（instructions 为 `perf` 的 `instructions:u`）

| 负载 | 基线 instructions | 修改后 | Δ | cycles Δ |
| --- | ---: | ---: | ---: | ---: |
| `empty_loop` | 3,512,580,925 | 3,492,556,695 | −0.57% | −1.88% |
| `int_local` | 4,562,600,700 | 4,552,576,128 | −0.22% | −1.01% |
| `prop_read` | 7,522,665,564 | 7,482,639,641 | −0.53% | −0.84% |
| `array_read` | 23,302,668,513 | 22,232,641,175 | −4.59% | −8.76% |
| `array_write` | 27,122,648,946 | 25,872,622,031 | −4.61% | −9.91% |
| `prop_write` | 21,652,656,964 | 20,672,631,764 | −4.53% | +13.03%※ |
| `call0` | 39,472,709,813 | 37,042,683,114 | −6.16% | −12.00% |
| `bigint32` | 1,751,668,332 | 1,591,642,055 | −9.14% | −15.64% |
| `bigint64` | 1,578,712,704 | 1,509,686,287 | −4.37% | −10.69% |
| `bigint256` | 640,527,650 | 612,900,946 | −4.31% | −12.84% |
| `string_build1` | 464,520,839 | 450,484,177 | −3.02% | +1.62%※ |
| `throw_catch` | 47,430,482,737 | 46,861,456,306 | −1.20% | −6.86% |
| `tdz_catch` | 81,725,981,344 | 81,078,950,951 | −0.79% | −0.68% |

※ 见 5.3：本机同时有其他验证会话（issue 43/44 的 benchmark）在跑，
cycles 不可分辨。

### 5.3 噪声与可分辨性

- **instructions 完全确定性**：同一二进制副本 A/A 的全部 13 项差值为
  0.00%，因此上表 instructions 的点估计可信。
- **cycles 不可分辨**：A/A（同二进制）在同批负载上波动
  −3.40% ~ +10.51%（`bigint32` +10.51%、`prop_write` +6.01%、
  `throw_catch` −3.40%）。`prop_write` 的 +13.03% 只比 A/A 噪声高约
  一倍，且与 −4.53% 的指令下降矛盾，不能判定为真实回退；需要在空载
  机器上复测（未决项，见 §8）。
- 机器上另有会话在 CPU 3 跑 issue 44 的 A/B、在 `/tmp/opencode/verify43`
  跑 issue 43 的 v8-v7 基准，属本次测量环境事实。

### 5.4 解读

- 收益分布与审计一致：B2.1 已用融合 span 删除宽边界的三个热循环只有
  0.2–0.6%；仍走 `Result<bool, Error>` 等边界（数组立即数读、属性写回、
  调用、BigInt/字符串拼接、通用 `local/push`）的负载拿到 3–9%。
- 错误路径（`throw_catch`/`tdz_catch`）指令数也略降：`Error` 在
  `Result` 中搬运从 80B 变 8B，抵消了构造时多一次 `Box` 分配的开销；
  但这不代表错误路径的**分配次数/RSS** 不变多（见 §8）。

## 6. 错误语义验证

14 个场景逐字节一致（`qjs-baseline` vs `qjs-boxed`）：

- TDZ `ReferenceError: b is not initialized`；
- `TypeError`（读 null 属性、严格模式写原始值、只读 const 赋值）；
- `RangeError`（`new Array(-1)`）、`URIError`（坏转义）、
  `EvalError`（Proxy trap）、`SyntaxError`（eval 与 `JSON.parse`）；
- 资源限制：递归栈溢出 `InternalError: stack overflow`；
- getter 抛异常、抛出普通对象（`undefined` name/message）；
- `instanceof`、嵌套 catch 的 name 传播。

CLI 端：语法错误与未捕获运行时错误的 stdout/stderr/退出码逐字节一致
（`SyntaxError: unexpected token in expression: ';'` 带 span；
`ReferenceError: 'nope' is not defined` 带调用栈）。

回归断言：`src/engine/api/error.rs::tests::error_channel_stays_one_register`
固化 `Error` 与 `Result<(), Error>` 为一字、其余宽返回 ≤ 两字；
`driver/ordinary.rs::layout_tests` 的断言由“两者等宽”改为
“Error 一字、普通返回 ≤2 字、统一入口 ≤3 字”（旧断言在 24B/16B 下
必然失败，属表示变化而非语义变化）。

## 7. 全量门禁

| 门禁 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| clippy（rustup `1.88.0`，CI 的全部 4 组命令） | 通过 |
| `cargo +1.88.0 test --locked --workspace --all-targets` | 通过（14 个目标，3,009 测试） |
| profiling 测试（cli `--test profiling`、lib `profiling_`） | 通过 |
| `cargo +1.88.0 test --locked --workspace --doc` | 通过 |
| `--workspace --features test262-host --lib --bins` | 通过（2,048 测试） |
| `--features test262-host --test unsupported_diagnostics` | 通过 |
| `--features test262-host --test oracle test262_` | 通过 |
| `check-source-layout.py` | 通过（622 文件） |
| CI 快检脚本（benchmark unittest、anti-cheat、artifact inventory、oracle registry、host boundary、rust-only） | 通过 |
| `test-test262.sh --check` | 通过；按预期报告源码指纹漂移 baseline=`2d299f99…` → current=`1bf1d6f8…` |
| `test-test262.sh --runner-provenance` | 通过（新指纹 `1bf1d6f8…`） |
| `test-test262.sh --full` | **见下** |

本机默认工具链 `1.94.1` 的 clippy 会在无关文件报
`manual_is_multiple_of`（54 处，均不在本次改动文件）；改用 CI 固定的
`1.88.0` 后全部通过。`--focused` 按设计拒绝在未 promote 的源码上回放
（需先更新 `current.conf` 的里程碑指纹），`--full` 覆盖同一结果向量。

**Test262 full 结果**：`TEST262_WORKERS=8 … --full` 输出
`r3fj complete Test262 vector matches: 80010 pass of 80060 eligible
(102037 total) variants`，源码身份归一化后与冻结 receipt 的 TSV/JSONL
逐字节一致 ⇒ 零行为回归。（运行需清空调用方 `GIT_*` 环境，
`prepare-test262.sh` 会拒绝不安全 Git 环境。）

## 8. 原实验裁决与当前复验

**原实验在 `fd9b4eac` 上判为正结果；恢复版 `04bb1a74` 在 R0 上的复验已发现 cycles、错误分配与 RSS 回退。** 当前集成分支按用户要求仍包含该实现，以便完成组合版归因；不得把原实验裁决当作当前接纳结论。

- 满足 issue 第 3 条“第 2 步证明正常路径机器码确实变短”的前置条件：
  详见 §3/§4/§5。
- 但本 issue 不扩大迁移。下一步若要扩大，应按 §3 的“仍保留的宽边界”
  清单定向处理（数组/属性/调用/字符串路径），而不是重做热循环。

限制与未决项：

1. **冷路径多一次分配已证实**：`Error` 构造现在 = `Box<ErrorData>` 分配 +
   `String` 分配（原来只有 `String`）。当前复验每 100,000 次构造／传播为
   200,000 次分配、9.3 MB，对照为 100,000 次、1.3 MB；保留模式
   RSS HWM 中位数增加 2,340 KiB。现有探针未单独计量 `Error::clone`。
2. **R0 上的 cycles 回退已复测**：两轮 A/A、ABBA/BAAB 配对中，
   `prop_write` 候选增加约 11% cycles；组合版仍须重新测量，不能沿用
   该独立候选的速度结论。
3. **Test262 当前契约**：原报告使用旧指纹门禁；PR #48 后按结果正文比较。若全量向量与既有 receipt 一致，无需仅因性能候选改变源码指纹而晋升语义基线。
4. **实施与裁决分离**：原门禁会拒绝此独立候选；本轮用户要求完成
   既定实现并记录未达标结果，故代码仍在集成分支。正式组合版结果
   完成之前不声称性能接纳。

## 9. 复现

恢复版的错误通道分配诊断探针位于
[`scripts/benchmark/probes/error_alloc_probe.rs`](../../scripts/benchmark/probes/error_alloc_probe.rs)。
使用同一份探针源码分别链接干净 R0 与候选 checkout；`success` 验证
正常完成不预建 `Error`，`construct` 测构造，`propagate` 测转换成
`RuntimeError`，`retain` 保留错误以检查峰值 live bytes 和 Linux VmHWM。
每个模式冻结相同迭代数，保存原始输出与构建回执。带计数器的
`diagnostic_ns` 只用于错误路径归因，不作为正式 Score。命令示例：

```sh
PROBE="$CAND_WT/scripts/benchmark/probes/error_alloc_probe.rs"
RUSTUP_TOOLCHAIN=1.94.1 python3 "$CAND_WT/scripts/benchmark/build_compile_probe.py" \
  --repo "$BASE_WT" --probe "$PROBE" --name oxide-error-alloc-probe \
  --output "$OUT/error-base"
RUSTUP_TOOLCHAIN=1.94.1 python3 "$CAND_WT/scripts/benchmark/build_compile_probe.py" \
  --repo "$CAND_WT" --probe "$PROBE" --name oxide-error-alloc-probe \
  --output "$OUT/error-candidate"
for mode in success construct propagate retain; do
  "$OUT/error-base/target/release/oxide-error-alloc-probe" "$mode" 100000 \
    > "$OUT/error-base-$mode.txt"
  "$OUT/error-candidate/target/release/oxide-error-alloc-probe" "$mode" 100000 \
    > "$OUT/error-candidate-$mode.txt"
done
```

上述分配/RSS 诊断与下面旧版命令都须在无并发测量的主机上运行；
正式 cycles 和 V8 的完整配对、样本顺序、构建 receipt 以
[统一测量协议](../performance/measurement.md) 为准。

```sh
# 基线/候选构建（fat LTO + CGU=1）
cargo build --release -p quickjs-oxide-cli --locked
cp target/release/qjs /tmp/qjs-candidate

# 符号与反汇编
nm --print-size --size-sort --demangle target/release/qjs > nm.txt
objdump -d --demangle --section=.text target/release/qjs > disasm.txt

# 固定负载 A/B（负载几何见 §5.2 表内名称）
taskset -c 2 perf stat -x, -e instructions,cycles /tmp/qjs-baseline int_local.js
taskset -c 2 perf stat -x, -e instructions,cycles /tmp/qjs-candidate int_local.js

# 错误语义对照
diff <(qjs-baseline semantics.js) <(qjs-candidate semantics.js)

# 门禁
cargo +1.88.0 clippy --locked --workspace --lib --bins -- -D warnings
cargo +1.88.0 test --locked --workspace --all-targets
python3 scripts/checks/check-source-layout.py
env -i PATH="$PATH" HOME="$HOME" TEST262_WORKERS=8 \
  ./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --full
```

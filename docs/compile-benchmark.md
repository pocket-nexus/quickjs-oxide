# 编译器前端（lexer/parser）基线与剖析

本文件是前端基准与剖析的方案与结果记录。**本分支只关心 compiler 本身的性能**：
范围限于 **A 跨引擎 compile 基线** 与 **B Oxide 前端剖析**。生成代码质量
（静态体积、执行指令数）与运行期性能不在本分支，见 §7.5 的后续设计；按收益
排序的优化候选记录在文末，不在本轮实施。

现有工具链（[profiling](profiling.md)、[benchmark README](../scripts/benchmark/README.md)）
面向运行期；`replay.py --mode compile` 只做 Oxide 新旧版本回归。本方案补齐
**跨引擎、同一语料、同一口径** 的前端对比，并给出 Oxide 内部的阶段与函数级拆分。

## 1. 口径定义

统一测量边界：**读取源码之后、不执行 JavaScript、不销毁 Runtime/Context** 的
一次性编译。所有探针输出单行 `compile_ns:<整数>`（parse 档为 `parse_ns:<整数>`），
单位纳秒，直接对应现有 `replay.py` 的 Oxide 契约，保证旧工具可继续消费。

| 引擎 | 探针 | 口径 | 说明 |
| --- | --- | --- | --- |
| Oxide | `apps/cli/examples/compile_probe.rs` | 完整前端 parse→publish | 与 `replay.py --mode compile` 同一边界；默认输出保持不变 |
| QuickJS | `build_compile_probes.py` 内嵌的 C 探针 | `JS_Eval(..., JS_EVAL_FLAG_COMPILE_ONLY)` | parse + 字节码生成，eager |
| Boa | `scripts/benchmark/probes/boa_compile_probe.rs` | `Script::parse` parse-only（含 scope 分析）；compile 档再加 `Script::codeblock` | `ByteCompiler::new` 是 crate 私有，但 `boa_engine::script::Script` 提供公开的 parse/codeblock 边界 |
| V8 | `scripts/benchmark/probes/node_compile_probe.mjs` | 进程内 `new vm.Script`（配 `--no-lazy`）；parse 档用 `node --parse-only` | 不依赖 d8；`--parse-only` 是同一 V8 flag |

**已知偏差**（必须写入结果 metadata，不允许省略）：

1. **eager vs lazy**：QuickJS 单遍编译器无惰性编译；Oxide 全量 lower；V8 默认惰性，
   用 `--no-lazy` 对齐。Boa 的 parse 档只产出 AST，不进入字节码口径。
2. **AST vs 字节码**：Boa parse 档只产出 AST + scope 分析；QuickJS/Oxide/V8 是字节码口径。
3. **Oxide 包含 publish**：这是 `Context::compile_*` 的真实边界，不做删减；
   阶段拆分由 §4 的 profiling 提供。
4. **Unicode 版本**：Oxide 使用 checksum-pinned QuickJS Unicode 17 表；Boa/V8 各自实现。
5. **测量方式**：四引擎均为探针进程内计时（`compile_ns`/`parse_ns`）；进程墙钟另存
   于 `samples.jsonl`，不用于比值。
6. 本矩阵是**方向性对比**，不是上游分数，也不与运行期 benchmark 混算。

## 2. 语料策略（离线优先）

第三方 benchmark 源码与生成 bundle 一律不入库，只保留生成器、清单与哈希：

- **自生成语法压力集**（`scripts/benchmark/compile_workloads.py`）：
  `syntax-mixed`（class/generator/async/解构/模板/正则/可选链/私有字段/标签语句）、
  `functions`（大量中小函数，标识符密集）、`expressions`（表达式与调用密集），
  每类生成 64KB/512KB/4MB 档，Script goal。
- **真实大文件**：复用 primitive-vm S07 receipt 的 67 个 V8 bundle（运行时套件拼接
  结果，实际 25KB–459KB，作为真实语法分布补充），只复制到 `target/` 并记录哈希，
  源码与 bundle 均不入库。
- 语料文件、哈希、生成器身份全部写入结果目录 metadata；结果目录必须不存在，
  避免覆盖旧证据。

test262 语料拼接与 Module goal 留作后续，需要联网与各引擎 loader 口径对齐后再做。

## 3. 探针与 runner

- `scripts/benchmark/build_compile_probes.py`：在 `target/` 下生成并构建
  QuickJS C 探针（源码内嵌在 builder 中写出，`cc` + oracle `libquickjs.a`，
  避免在产品路径提交 `.c` 文件）与 Boa 探针（独立 crate，`cargo build
  --offline`，不进入 workspace），并为 Node 写版本/路径 receipt。
  Oxide 探针复用 `build_compile_probe.py`，支持 `--profiling` 诊断构建。
- `scripts/benchmark/compile_matrix.py`：
  - `--engine name=path` 通过探针 `--version` 自动识别类型；
  - `--metric compile|parse`、`--repeat N`、`--timeout`、`--output 新目录`；
  - 轮转引擎顺序；每个 case 要求所有引擎所有重复都成功，否则整组失格；
  - 保留原始 stdout/stderr 与失败样本；产出 `metadata.json`、`samples.jsonl`、
    `results.json`、`report.md`（沿用 `run.py` 的 digest/机器信息辅助函数）；
  - 报告 ns/次、MB/s 与相对参考引擎比值；不合成总分。
- 单元测试 `scripts/benchmark/test_compile_matrix.py`：探针识别、V8 档位 flag、
  输出契约（含 stderr 非空、多余行、非零退出）、语料 manifest 校验、失格传播。

## 4. 剖析方法

1. **阶段占比**：`build_compile_probe.py --profiling` 构建的探针在 stderr 输出
   parse/resolution/lowering/blocks/fusion/relocation/publish 的
   inclusive/exclusive 纳秒与 attempts（现有 `CostProfile`），在 512KB 生成语料上采集。
2. **函数级拆分**：release + `debug=1` 构建普通探针，`perf record -g` 后按符号归并
   `lexer.rs`（`scan_identifier`/`skip_trivia`/`scan_number`/…）、parser、resolution、
   lowering、publish 与分配（malloc）占比。用于回答“lexer 还是 parser 更贵”。
3. 若 perf 归因不足，再评估在 `profiling` 构建中加入 lexer 级计数器
   （token 数、identifier 分配次数、seek/re-scan 次数），仅诊断、不参与正式计时。

## 5. 基线结果

测量环境：Linux x86-64、AMD CPU、Node 24.21.0（V8）、pinned QuickJS
2026-06-04（gcc `-O2`，无 LTO）、Boa 0.22.0、Oxide release（opt-level 3，
无 LTO）、关闭 profiling。每个 case 每引擎独立进程重复 3–5 次，全部成功才计入比值；
比值是 **参考引擎中位 ns / 该引擎中位 ns**，>1 表示比参考快。

### 5.1 真实 bundle（primitive-vm S07 的 67 个 bundle，实际 25KB–459KB）

| 引擎 | 中位吞吐 | 吞吐范围 | 相对 Oxide 加速比（中位，范围） |
| --- | ---: | ---: | --- |
| Oxide | 4.79 MB/s | 4.13–10.98 | — |
| QuickJS | 28.19 MB/s | 22.68–56.45 | 5.77×（3.78–6.77） |
| Boa | 5.12 MB/s | 4.25–10.27 | 1.07×（0.64–1.51） |
| V8（Node） | 34.23 MB/s | 31.19–70.57 | 7.08×（5.75–10.50） |

结论：在真实语法分布上，Oxide 与 Boa 基本同档（中位 1.07×），比 QuickJS 慢约
5.8×，比 V8 慢约 7.1×。Boa 有约一半 case 慢于 Oxide（最低 0.64×）。

### 5.2 生成语法压力集（64KB/512KB/4MB）

| 引擎 | 中位吞吐 | 相对 Oxide 加速比（中位） |
| --- | ---: | --- |
| Oxide | 3.18 MB/s | — |
| QuickJS | 11.34 MB/s | 3.57×（0.17–6.52） |
| Boa | 5.66 MB/s | 1.70×（0.72–2.19） |
| V8（Node） | 22.74 MB/s | 6.52×（5.03–7.42） |

**QuickJS 4MB `functions` 异常（重要发现）**：QuickJS 在该 case 只有 0.66 MB/s，
比 Oxide 慢 3.8×；扩展曲线为 512KB 117ms → 1MB 440ms → 2MB 1.68s → 4MB 6.48s，
约每翻倍 4×，即 O(n²)。perf 显示 2MB 时 **90.8% 的 CPU 在
`get_line_col_cached`**（QuickJS 自己的注释也承认它慢）。这是 QuickJS 在“单文件
海量小函数”下的已知弱点，真实 bundle 不受影响；生成语料每个 4MB 文件包含数万
个函数声明，放大了该路径。因此 QuickJS 的生成语料中位比值不宜外推。

### 5.3 parse-only（仅 Boa 与 V8 有公开 parse-only 入口）

| 引擎 | 中位吞吐 | 范围 |
| --- | ---: | ---: |
| Boa（parse + scope 分析） | 7.69 MB/s | 6.10–9.08 |
| V8（`--parse-only`） | 34.40 MB/s | 21.12–44.50 |

V8 parse-only 比 Boa 快 2.9–5.0×。Oxide 与 QuickJS 没有公开的 parse-only API，
不参与该档；其 parse 占比见 §6。

## 6. Oxide 前端剖析

### 6.1 阶段占比（profiling 探针，512KB 语料，纳秒为单次 inclusive）

| 阶段 | functions-512KB | syntax-mixed-512KB | expressions-512KB |
| --- | ---: | ---: | ---: |
| parse（含 lexer） | 71.6ms（40%） | 83.1ms（41%） | 67.8ms（55%） |
| resolution | 24.7ms（14%） | 21.5ms（11%） | 13.9ms（11%） |
| lowering | 28.3ms（16%） | 36.0ms（18%） | 16.2ms（13%） |
| verify | 20.7ms（12%） | 25.5ms（12%） | 10.1ms（8%） |
| publish | 33.0ms（18%） | 37.0ms（18%） | 14.6ms（12%） |
| fusion + relocation | 1.2ms | 1.6ms | 0.9ms |

阶段为 inclusive、不可相加；百分比是各阶段占“全部 inclusive 之和”的比例，仅用于
排序。诊断构建相对普通构建的总开销约 5%（170ms → 179ms）。**parse 是最大单阶段
（40–55%）**；verify+publish 合计 20–30%，这是 QuickJS/V8 口径里不存在的独立验证
与发布事务成本，也是差距的一部分。

### 6.2 函数级拆分（release + debug info，perf，512KB × 15 次）

> 本节为 P0 基线；P1b 后 4MB 复测见 §9.7（结构未变：libc 27.4%、
> code/verify/publish 20.1%、lexer 7.0%、parser 6.8%）。

flat profile（self time 百分比）：

| 桶 | functions | syntax-mixed | expressions |
| --- | ---: | ---: | ---: |
| libc（malloc/free/memcmp 等） | 27.6 | 27.2 | 26.4 |
| lexer | 7.8 | 8.5 | 13.8 |
| code/publish/link | 10.1 | 10.1 | 9.8 |
| verify | 8.6 | 9.6 | 8.5 |
| Rust std（Vec/HashMap 容器等） | 8.4 | 7.7 | 6.8 |
| parser | 4.1 | 5.4 | 10.6 |
| resolution | 4.3 | 3.3 | 3.3 |
| value/string | 4.8 | 3.5 | 1.8 |
| scope_validation | 2.9 | 2.8 | 2.4 |
| lowering | 2.6 | 2.6 | 2.4 |
| 其他（hashbrown insert/rehash、atom 表、num-bigint 等） | 3.0 | 3.3 | 3.4 |

关键点：

1. **libc 分配/拷贝占约 27%**。整个 flat profile 里最大的单一符号依次是
   `code::bytecode::verify_parts_with_visits`（6.4%）、
   `code::bytecode::enqueue_fallthrough`（3.1%，verify 的可达性 worklist）、
   `lexer::Lexer::next_token_with_goal`（3.0%）、
   `scope_validation::validate_scope_graph`（2.9%）。
2. **libc 调用方归因**（callchain 近似）：`scan_punctuator`→`starts_with`→slice
   比较（memcmp）2.3%，`Utf16Units::new`/`JsString::from_validated_utf16` 字符串
   分配约 0.9%，`ensure_token_with_goal`、`ensure_annex_b_binding`、
   `parse_assignment` 等各有 0.2–0.5%。大量分配路径因 callchain 截断未能归因，
   但方向明确：**lexer 的字符串/UTF-16 转换与 map 插入是主要分配来源**。
3. **lexer 直接 CPU 8–14%，parser 4–11%**；expression 密集语料 lexer 占比最高。
4. `functions` 语料的 resolution 阶段占比（14%，§6.1）高于 expressions（11%），
   且其 perf self time（4.3%）在各语料中最高，说明绑定/作用域解析随声明数量增长。

### 6.3 优化候选（按证据排序，本轮不改）

1. **削减分配**（收益最大）：identifier `String` 每 token 一次分配
   （`lexer.rs:935`）；`Utf16Units`/`JsString::from_validated_utf16` 的 UTF-16
   转换；scope/binding map 的 `hashbrown::insert`/`reserve_rehash`（约 4.3%）。
2. **lexer 快路径**：`peek_char`/`peek_nth_char` 的逐字符 UTF-8 解码
   （`lexer.rs:678`）；`skip_trivia`/`scan_punctuator` 的多次 `starts_with`
   slice 比较（可改首字节分支）。
3. **verify/publish**：verify 单符号 6.4% + 可达性 worklist 3.1%；两阶段合计
   占编译时间 20–30%，可评估增量验证与更紧凑的指令容器。
4. **resolution/scope_validation**：合计 7–8%，大声明文件更明显。
5. **parser 前瞻/回溯**：`for_iteration_kind_ahead` 克隆 lexer 重扫
   （`parser/tokens.rs:69`）等，需针对 `for` 密集语料单独测量后再动。

## 7. 结果总结与问题分析

### 7.1 具体结果

真实 bundle 代表性 case（MB/s 与相对 Oxide 的加速比）：

| case | 字节 | Oxide | QuickJS | Boa | V8 | QuickJS× | Boa× | V8× |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 002-prop_read | 34097 | 4.75 | 25.03 | 5.10 | 34.23 | 5.27 | 1.07 | 7.21 |
| 050-v8-richards | 25280 | 9.31 | 48.47 | 10.24 | 53.50 | 5.21 | 1.10 | 5.75 |
| 052-v8-crypto | 57547 | 5.41 | 29.82 | 5.55 | 35.07 | 5.51 | 1.03 | 6.48 |
| 054-v8-earley-boyer | 204709 | 4.96 | 31.68 | 7.50 | 52.06 | 6.39 | 1.51 | 10.50 |
| 056-v8-splay | 20250 | 9.23 | 34.91 | 10.06 | 53.28 | 3.78 | 1.09 | 5.77 |
| 066-all | 459054 | 5.98 | 37.84 | 6.74 | 49.84 | 6.32 | 1.13 | 8.33 |

极值（67 case 全成功才计入）：QuickJS 最好 `036-array_for_of` 6.77×、最差
`056-v8-splay` 3.78×；Boa 最好 `054-v8-earley-boyer` 1.51×、最差
`063-regexp` 0.64×；V8 最好 `054-v8-earley-boyer` 10.50×、最差
`050-v8-richards` 5.75×。**Boa 在约一半 case 慢于 Oxide，是唯一同档对手**。

生成语料代表性 case（MB/s）：

| case | Oxide | QuickJS | Boa | V8 |
| --- | ---: | ---: | ---: | ---: |
| expressions-524288 | 4.57 | 29.75 | 5.66 | 33.87 |
| syntax-mixed-524288 | 2.75 | 2.85 | 4.69 | 17.32 |
| functions-524288 | 3.06 | 4.47 | 6.70 | 22.74 |
| functions-4194304 | 2.49 | 0.66 | 4.69 | 16.68 |

生成语料上 QuickJS 在 512KB 档只与 Oxide 相当（syntax-mixed 2.85 vs 2.75），
远差于其在真实 bundle 上的 5.8×，说明这些语料对 QuickJS 存在特异性（见 §7.3
表末行）；**真实 bundle 的中位比值才是主结果**。

### 7.2 成本结构：为什么 Oxide 慢

1. **parse 是最大单阶段（40–55%）**，其中 lexer 直接 CPU 8–14%、parser 4–11%
   （§6.1、§6.2）。expression 密集语料 parse 占比最高（55%），说明词法/语法
   扫描与表达式递归下降是首要成本。
2. **verify+publish 合计 20–30%**，是 QuickJS/V8 口径里不存在的独立只读验证与
   事务化发布。仅 `verify_parts_with_visits` 单符号就占 6.4%，
   `enqueue_fallthrough` 的 VecDeque 占 3.1%。这是差距中结构性的、可归因于
   Oxide 自身设计的一部分，而不是“实现慢”。
3. **分配与内存拷贝约 27%（libc）**，最大的可归因来源是 lexer 的
   `starts_with`→slice 比较（memcmp）、`Utf16Units::new`/`JsString` 的 UTF-16
   转换，以及 scope/binding map 的 `hashbrown::insert`/`reserve_rehash`
   （约 4.3%）。identifier 每 token 一次 `String` 分配
   （`lexer.rs:935`）属于同一类。
4. **resolution+scope_validation 合计 7–8%**，且 `functions` 语料下 resolution
   占比（14%）明显高于表达式语料（11%），随声明数量增长。
5. 结论排序：Oxide 与 Boa 同档说明“Rust 实现 + 完整前端”本身不是瓶颈；
   与 QuickJS/V8 的 5–7× 差距主要由 ①分配/拷贝、②lexer 扫描、③verify/publish
   三段构成，而非某个未知热点。

### 7.3 问题清单（按证据强度）

| 级别 | 问题 | 证据 | 验证/收口方式 |
| --- | --- | --- | --- |
| P0 | 分配总量过高：identifier `String`、UTF-16 转换、map 插入 | libc 27%；`Utf16Units::new`/`JsString::from_validated_utf16`；hashbrown 4.3% | 优化后重跑矩阵 + §9 分配计数探针 |
| P0 | lexer 字符扫描：`peek_char` 逐字符 UTF-8 解码、`starts_with` memcmp | lexer 直接 8–14%；callchain 归因 `scan_punctuator` 2.3% | 加 lexer 诊断计数或 ASCII 快路径后 A/B |
| P0 | verify+publish 占 20–30% | §6.1、单符号 6.4% + 3.1% | 评估增量验证、延迟 publish、更紧凑指令容器 |
| P1 | resolution/scope_validation 7–8%，随声明数增长 | `functions` resolution 14% | 声明密集语料上单独测量 |
| P1 | parser 回溯重扫未单独量化 | `for_iteration_kind_ahead`、regexp/div `seek` 代码路径 | 需要 `for`/regexp 密集语料 |
| P2 | perf 分配归因不完整 | libc 叶子中 19.1% 未归因（callchain 截断） | 需要分配计数或更完整的调用链 |
| P2 | 生成语料对 QuickJS 特异性 | QuickJS 512KB 档仅 1.0–1.5× | 以真实 bundle 为主结果；语料保留为诊断 |

**语料特异性与 QuickJS 超线性（上表末行）**：4MB `functions` 上 QuickJS 0.66 MB/s，512KB→4MB
约每翻倍 4×（117ms→440ms→1.68s→6.48s），perf 显示 2MB 时 90.8% CPU 在
`get_line_col_cached`。这是 QuickJS 在“单文件海量小函数”下的实现弱点，不是
Oxide 的优势；真实 bundle 上 QuickJS 仍快 5.8×。该发现只用于说明：**任何单一
语料的比值都可能被对手的实现特性放大，主结论必须落在真实语法分布上**。

### 7.4 方法学限制（结论只在此范围内成立）

- 口径不同：Oxide/QuickJS/Boa 为进程内计时，V8 parse 档为进程墙钟（含 Node
  启动，用大文件摊薄）；V8 compile 档为进程内计时。
- 编译策略不同：QuickJS/Oxide eager，V8 用 `--no-lazy` 对齐，Boa parse 档
  无字节码；Boa 的 parse 含 scope 分析。
- Oxide 口径包含 verify/publish（QuickJS/V8 没有对应阶段），这是真实 API 边界，
  未做删减，比较时必须计入。
- Unicode 版本不同（Oxide 固定 QuickJS Unicode 17 表）；优化级别不同
  （QuickJS `-O2` 无 LTO，Oxide opt-level 3 无 LTO）。
- 单机、无 PGO、无置信区间；samples.jsonl 保留全部原始样本，报告只用中位值。
- 本矩阵是方向性对比，不是上游分数，也不与运行期 benchmark 混算。

### 7.5 本分支之外：生成代码质量

本分支不采集生成代码质量。后续设计（不在本轮）分三层：

1. **引擎内 codegen A/B（最能隔离质量）**：固定 VM，对比两个 Oxide 编译器版本
   的 `owned_instructions`（执行指令数）、`code_instructions`、
   `code_inline_bytes`、`maximum_verified_stack`、fusion 命中率与执行墙钟。
2. **跨引擎静态体积代理**：QuickJS 用 `JS_WriteObject(JS_WRITE_OBJ_BYTECODE)`、
   V8 用 `node --print-bytecode` 的 `Bytecode length`；Boa 无公开长度 API。
   只比每源字节的体积，ISA 不同不可比语义效率。
3. **端到端执行速度**：现有 v8-v7/microbench/fixed/scaling 反映 codegen+VM
   联合质量，不能单独归因 codegen。

## 8. 复现命令

```sh
python3 scripts/benchmark/compile_workloads.py --output target/compile-corpus
python3 scripts/benchmark/build_compile_probes.py \
  --repo . --output target/compile-probes \
  --quickjs-source target/oracle/quickjs-2026-06-04
python3 scripts/benchmark/compile_matrix.py --metric compile \
  --corpus target/compile-corpus --repeat 5 \
  --engine oxide=target/compile-probes/oxide/target/release/oxide-compile-probe \
  --engine quickjs=target/compile-probes/quickjs/quickjs-compile-probe \
  --engine boa=target/compile-probes/boa/target/release/boa-compile-probe \
  --engine node="$(command -v node)" \
  --output target/compile-matrix
python3 scripts/benchmark/compile_matrix.py --metric parse \
  --corpus target/compile-corpus --repeat 5 \
  --engine boa=target/compile-probes/boa/target/release/boa-compile-probe \
  --engine node="$(command -v node)" --output target/parse-matrix
```

剖析：

```sh
python3 scripts/benchmark/build_compile_probe.py --repo . \
  --output target/oxide-profile-probe --profiling
target/oxide-profile-probe/target/release/oxide-compile-probe FILE  # 阶段行到 stderr
CARGO_PROFILE_RELEASE_DEBUG=1 python3 scripts/benchmark/build_compile_probe.py \
  --repo . --output target/oxide-debug-probe
perf record -F 999 -g --call-graph dwarf -- target/oxide-debug-probe/target/release/oxide-compile-probe FILE
perf report --stdio --no-children -g none
```

正式计时只使用关闭 profiling 的普通构建；诊断构建与 perf 结果不得与正式吞吐混算。

## 9. P0 分配与计数基线（前端重构起点）

本节是 lexer/parser 重构（[lexer-parser-refactor.md](lexer-parser-refactor.md)、
issue #32）的起点基线：分配计数、perf 硬件计数与进程 RSS。度量边界与 §1 相同
（源码读取、Runtime/Context 构造与销毁都在计时窗口外），区别只是探针故意插桩。

工具：`scripts/benchmark/probes/compile_alloc_probe.rs`，全局计数分配器统计
alloc/realloc/dealloc 调用数、请求字节与未释放字节高水位。它由
`build_compile_probe.py --probe/--name` 生成独立 crate 构建（同一套依赖钉版与
回执）；`--version` 不匹配矩阵 magic，`compile_matrix.py` 会拒绝消费，因此不会
混入正式吞吐。探针必须放在 `scripts/benchmark/probes/`：workspace 的
`unsafe_code = "forbid"` 使 `apps/cli/examples/` 无法承载 `GlobalAlloc` 实现。

### 9.1 分配基线（4MB 档，3 次运行中位数；计数逐次完全一致）

| 语料 | alloc 次数 | alloc 字节 | realloc 次数 | dealloc 次数 | peak live | 进程 max RSS | 窗口内 compile |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 7,709,081 | 991.5 MB | 938,073 | 6,257,722 | 470.3 MB | 448.9 MB | 1730.8 ms |
| expressions | 4,058,383 | 857.4 MB | 385,665 | 3,317,557 | 529.7 MB | 355.6 MB | 1098.3 ms |
| syntax-mixed | 8,442,295 | 1,441.7 MB | 785,789 | 6,990,836 | 691.3 MB | 506.4 MB | 1831.0 ms |

按源码 KB 归一（64KB/512KB/4MB 三档在 ±1% 内一致，计数随体积严格线性）：

| 语料 | alloc/KB | 分配字节/KB | realloc/KB | peak live/KB |
| --- | ---: | ---: | ---: | ---: |
| functions | 1,882 | 242,065 | 229 | 114,818 |
| expressions | 991 | 209,323 | 94 | 129,316 |
| syntax-mixed | 2,061 | 351,974 | 192 | 168,763 |

要点：

1. `functions` 每源 KB 分配 1,882 次、累计分配 242 倍源字节，peak live 约
   115 倍源字节、RSS 约 110 MB/源MB（§7 的早期单次测量为 100.3）。这与
   §7.2 的 libc 27% 相互印证：**分配次数与分配量是前端第一成本**。
2. realloc 同样密集（functions 229 次/KB），来自 Vec/HashMap 扩容路径；
   P1/P2 的验收指标应使用“alloc+realloc 总次数”斜率，而不是只看 alloc。
3. 计数严格线性，说明按 KB 归一可信，优化收益可直接用斜率下降衡量。
4. `窗口内 compile` 是分配探针自己的 `compile_ns` 中位数；由于插桩有开销、
   且与 perf `task-clock` 扣除 tiny 档的口径不同，两者不应混算。

### 9.2 perf 计数基线（4MB 档，3 次运行中位数，已扣 64KB tiny 档）

正式探针为 `target/compile-probes/oxide/target/release/oxide-compile-probe`
（`build_compile_probes.py` 在 00bb387f 构建；此后产品源码未变）。
`perf stat -x, -u -e instructions,cycles,branches,branch-misses,
cache-references,cache-misses,task-clock`，对 4MB 与 64KB 各跑 3 次取中位数、
逐事件相减后除以 4096 KB：

| 语料 | instr/KB | cycles/KB | branches/KB | branch-miss/KB | cache-ref/KB | cache-miss/KB | task-clock/KB | IPC |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 2,897,169 | 1,816,427 | 666,879 | 2,467 | 123,624 | 17,566 | 0.435 ms | 1.60 |
| expressions | 1,855,004 | 985,952 | 411,505 | 1,760 | 50,845 | 2,277 | 0.251 ms | 1.89 |
| syntax-mixed | 2,708,420 | 1,836,293 | 587,696 | 2,726 | 83,364 | 6,867 | 0.462 ms | 1.48 |

与 §7.2 的早期单次测量一致（functions 2.90M instr/KB、17.1k miss/KB、1789ms）；
本表为扣 tiny 后的 3 次中位数，作为 P1–P3 的固定对照。

### 9.3 与既有结论的对应

- §6.2 的 libc 27%（malloc/free/memcmp）对应 §9.1 的 1,882 alloc/KB 与
  242 KB/KB；
- §6.3 候选 1（削减分配）的验收量就是 §9.1 的 alloc+realloc 斜率；
- §6.3 候选 2（lexer ASCII 快路径）主要压低 instr/KB 与 cache-miss/KB，
  对分配斜率影响小，可据此区分两类改动的收益归属。

### 9.4 复现命令

```sh
# 构建分配探针（独立 crate，复用矩阵构建器的依赖钉版与回执）
python3 scripts/benchmark/build_compile_probe.py --repo . \
  --output target/p0-alloc-probe \
  --probe scripts/benchmark/probes/compile_alloc_probe.rs \
  --name oxide-compile-alloc-probe
target/p0-alloc-probe/target/release/oxide-compile-alloc-probe FILE

# perf 计数（-u 只统计用户态；tiny 档做逐事件扣除）
for i in 1 2 3; do
  perf stat -x, -u -e instructions,cycles,branches,branch-misses,cache-references,cache-misses,task-clock \
    -- target/compile-probes/oxide/target/release/oxide-compile-probe FILE 2>> out.csv >/dev/null
done

# 进程 max RSS（系统无 /usr/bin/time 时）
python3 -c "import resource,subprocess,sys; subprocess.run(sys.argv[1:],check=True,stdout=subprocess.DEVNULL); \
  print(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss, 'KB')" PROBE FILE
```

### 9.5 P1a checkpoint（HEAD `a5b651be`，2026-09-24）

P1a（`970a11be`→`ba593f9e`→`f29dd568`→`a5b651be`：Token Copy、惰性 cooked、
清 token clone、数字去 `replace`）相对 §9.1/§9.2 基线的首个 checkpoint。
度量口径与 §9.1/§9.2 相同；本机 perf 7.2.6 已无 `-u`，改用等价
`--all-user`。探针构建目录：`target/p1a-alloc-probe`（分配/RSS）、
`target/p1a-compile-probe`（perf），均用
`build_compile_probe.py --repo .` 从当前 HEAD 构建。

分配（4MB 档，计数逐次完全一致）：

| 语料 | alloc 次数 | vs 基线 | alloc+realloc 次数 | vs 基线 | 分配字节 | vs 基线 | peak live | vs 基线 | max RSS | vs 基线 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 6,709,972 | −13.0% | 7,451,101 | −13.8% | 913.7 MB | −7.9% | 408.2 MB | −13.2% | 387,316 KB | −15.7% |
| expressions | 3,265,546 | −19.5% | 3,480,553 | −21.7% | 733.4 MB | −14.5% | 409.2 MB | −22.7% | 286,996 KB | −21.2% |
| syntax-mixed | 7,243,460 | −14.2% | 7,860,641 | −14.8% | 1,310.8 MB | −9.1% | 570.5 MB | −17.5% | 434,452 KB | −16.2% |

perf（4MB 扣 64KB tiny 档，3 次中位数）：

| 语料 | instr/KB | vs 基线 | cycles/KB | vs 基线 | cache-ref/KB | vs 基线 | cache-miss/KB | vs 基线 | task-clock/KB | vs 基线 | IPC |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 2,792,483 | −3.6% | 1,677,396 | −7.7% | 116,291 | −5.9% | 16,029 | −8.7% | 0.411 ms | −5.6% | 1.66 |
| expressions | 1,763,907 | −4.9% | 883,483 | −10.4% | 44,733 | −12.0% | 1,896 | −16.7% | 0.228 ms | −9.3% | 2.00 |
| syntax-mixed | 2,598,929 | −4.0% | 1,665,298 | −9.3% | 75,641 | −9.3% | 5,662 | −17.5% | 0.426 ms | −7.7% | 1.56 |

结论与校准：

1. P1a 全部指标方向正确，但低于计划 §5 的方向性目标（−60% 分配、−15%
   instr/时间、−30% miss）。原因是 4MB 语料的分配/指令大头在 parse 之后的
   IR/常量/绑定/字节码路径，lexer 每 token 的分配消除只覆盖一部分；
   `functions` 每源 KB 仍有约 1.3k alloc+realloc 次。
2. 因此后续阶段按实测校准：P1b/P2/P3 的分配目标用“相对上一 checkpoint 再降”
   口径执行（计划已写 P1b ≥70% 相对 P1a），绝对百分比在每阶段 checkpoint
   后更新本节；instr/miss/时间目标保持“相对 P0 基线”方向。
3. 复现：`build_compile_probe.py` 生成上述两个探针目录后，按 §9.4 命令跑
   4MB + 64KB（perf 将 `-u` 换为 `--all-user`），alloc/RSS 直接跑探针。

### 9.6 P1b checkpoint（HEAD `bd3eb461`，2026-09-24）

P1b（`343acdfd` NameTable/NameId 贯穿 parser/IR/resolution/lowering +
`2a606c5a` clippy 清理 + `bd3eb461` per-NameTable `JsString` 缓存）相对
§9.5 P1a 的 checkpoint。度量口径与 §9.4 相同，但本次测量期间本机存在其他
rustc 负载（load ≈ 4/16 核），perf 改用 P1a/P1b 探针**交错各 7 次取最小值**
的稳健口径（与 §9.5 的 3 次中位数不完全可比，故同时给出相对同条件重测 P1a
的差值）。探针构建目录：`target/p1b-alloc-probe`（分配/RSS）、
`target/p1b-compile-probe`（perf），均用 `build_compile_probe.py --repo .`
从当前 HEAD 构建。

分配（4MB 档，计数逐次完全一致）：

| 语料 | alloc 次数 | vs P1a | alloc+realloc 次数 | vs P1a | 分配字节 | vs P1a | peak live | vs P1a | max RSS | vs P1a |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 5,574,819 | −16.9% | 6,275,173 | −15.8% | 793.4 MB | −13.2% | 326.4 MB | −20.0% | 308.4 MB | −18.5% |
| expressions | 2,745,312 | −15.9% | 2,949,318 | −15.3% | 650.0 MB | −11.4% | 353.7 MB | −13.6% | 245.6 MB | −12.4% |
| syntax-mixed | 6,548,536 | −9.6% | 7,163,968 | −8.9% | 1,179.8 MB | −10.0% | 484.1 MB | −15.1% | 363.6 MB | −14.3% |

perf（4MB 扣 64KB tiny 档，P1a/P1b 交错 7 次最小值；同条件重测 P1a 基线）：

| 语料 | instr/KB | vs P1a | cycles/KB | vs P1a | cache-ref/KB | vs P1a | cache-miss/KB | vs P1a | task-clock/KB | vs P1a | IPC |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 2,618,044 | −6.2% | 1,393,757 | −16.9% | 84,509 | −27.3% | 5,055 | −68.5% | 0.343 ms | −16.5% | 1.88 |
| expressions | 1,743,691 | −1.1% | 809,866 | −8.3% | 40,480 | −9.5% | 1,436 | −24.3% | 0.208 ms | −8.8% | 2.15 |
| syntax-mixed | 2,576,779 | −0.9% | 1,533,229 | −7.9% | 69,942 | −7.5% | 4,608 | −18.6% | 0.394 ms | −7.5% | 1.68 |

相对 P0 基线（§9.1/§9.2）：alloc 次数 functions −27.7%、expressions −32.4%、
syntax-mixed −22.4%；instr/KB −9.6%/−6.0%/−4.9%；cache-miss/KB
−71.2%/−36.9%/−32.9%；task-clock −21.1%/−17.1%/−14.7%。

结论与校准：

1. P1b 全部指标方向正确，cache-miss 收益最大（functions 相对 P1a −68.5%、
   相对 P0 −71.2%），说明名字驻留消除了重复字符串分配造成的 cache 污染；
   instr 收益最小（−0.9%~−6.2%），因为驻留只替换字符串构造，不改变
   IR/字节码路径的指令数。
2. 未达计划 §5 的 P1b 方向目标（分配再降 ≥70%、instr ≥15%、miss ≥40%、
   时间 ≥15%）：仅 functions 的 miss/时间达标，分配差距最大
   （−9.6%~−16.9%）。原因与 P1a checkpoint 的校准一致：4MB 语料分配大头在
   IR/常量/绑定/字节码路径，名字字符串只占其中约 1/6；`bd3eb461` 的
   per-NameTable `JsString` 缓存再贡献约 1–4 个百分点（`343acdfd` 后
   functions alloc 5,758,344 → 5,574,819）。
3. 后续按 §9.5 校准规则执行：P2/P3 分配目标继续用“相对上一 checkpoint 再
   降”口径，绝对百分比以本表为准；P2（前瞻备忘/提交缓冲）不直接消除分配，
   若 P2 后分配仍是瓶颈，需重估是否追加 IR/常量侧削减。
4. 复现：`build_compile_probe.py` 生成上述两个探针目录后，按 §9.4 命令跑
   4MB + 64KB（perf 将 `-u` 换为 `--all-user`）；高负载环境下 perf 用
   P1a/P1b 交错 min-of-7，alloc/RSS 直接跑探针。

### 9.7 P1b 外部对照与结构复测（HEAD `bd3eb461`，2026-09-24）

P1b checkpoint 后补测两块：与 Boa 的 **compile-only** 对照（`compile_matrix.py
--metric compile`，不含 runtime/Context 构建与源码 I/O）与当前前端结构复测
（profiling 探针 + perf flat profile）。测量期间本机 load 4–13/16，矩阵按引擎
轮转交错、每 case 每引擎 5 次取中位数，比值可用、绝对值不跨 campaign 比。
探针：oxide=`target/p1b-compile-probe/target/release/oxide-compile-probe`
（P1b 源码构建）、boa=`target/compile-probes/boa/target/release/boa-compile-probe`
（Boa 0.22.0，与 §5 同一探针）。

#### 9.7.1 生成语料 compile-only（4MB/512KB/64KB）

| case | oxide MB/s | Boa MB/s | Boa/oxide |
| --- | ---: | ---: | ---: |
| expressions-4194304 | 5.135 | 4.089 | 0.796 |
| expressions-524288 | 5.412 | 5.227 | 0.966 |
| expressions-65536 | 5.567 | 6.597 | 1.185 |
| functions-4194304 | 3.126 | 4.717 | 1.509 |
| functions-524288 | 3.408 | 5.494 | 1.612 |
| functions-65536 | 4.094 | 6.719 | 1.641 |
| syntax-mixed-4194304 | 2.657 | 1.643 | 0.618 |
| syntax-mixed-524288 | 2.965 | 3.983 | 1.343 |
| syntax-mixed-65536 | 3.259 | 5.594 | 1.717 |
| **中位** | **3.408** | **5.227** | **1.343**（0.618–1.717） |

#### 9.7.2 真实 bundle compile-only（67 case，25KB–459KB）

| 引擎 | 中位吞吐 | 范围 | Boa/oxide 中位比值（范围） |
| --- | ---: | ---: | ---: |
| Oxide (P1b) | 5.495 MB/s | 4.837–12.184 | — |
| Boa | 5.001 MB/s | 4.581–10.622 | 0.9225（0.554–1.271） |

对照 §5.1 基线（oxide 4.79 / Boa 5.12 / Boa 快 1.07×）：P1a+P1b 后
**真实 bundle 上 Oxide 已反超 Boa 约 10%**；生成语料中位差距从 1.70× 收窄到
1.34×，但 `functions` 档仍落后 1.5–1.6×，是压力语料上的主要缺口。
`expressions` 已持平或略快（4MB 档 Oxide 快 1.26×）；`syntax-mixed-4194304`
Oxide 快 1.62×（Boa 在该 case 超线性退化到 1.64 MB/s，与 QuickJS 的
`get_line_col_cached` 退化性质不同但同样不宜外推）。

#### 9.7.3 4MB 阶段占比（profiling 探针，inclusive，百分比=占各阶段之和）

| 阶段 | functions | expressions | syntax-mixed | P0 §6.1 512KB（functions） |
| --- | ---: | ---: | ---: | ---: |
| parse（含 lexer） | 34.0% | 50.1% | 34.9% | 40% |
| resolution | 14.6% | 8.1% | 7.7% | 14% |
| lowering | 16.0% | 15.7% | 17.9% | 16% |
| verify | 13.4% | 9.6% | 15.6% | 12% |
| publish | 21.2% | 15.6% | 23.0% | 18% |
| fusion + relocation | 0.7% | 1.0% | 0.9% | — |

P1a 把 functions 的 parse 占比从 40% 压到 34%，但 **verify+publish 仍占
34–39%**、lowering 16–18%——这三块是 P1–P3 的显式非目标，也是总收益的
结构上限。

#### 9.7.4 flat profile（perf，自时间，4MB×10 次）

| 桶 | functions | expressions | P0 §6.2 functions |
| --- | ---: | ---: | ---: |
| libc（malloc/free/memcmp 等） | 27.4 | 26.5 | 27.6 |
| code/verify/publish | 20.1 | 20.9 | 18.7（code 10.1 + verify 8.6） |
| Rust std/容器 | 9.1 | 4.8 | 8.4 |
| lexer | 7.0 | 13.1 | 7.8 |
| parser | 6.8 | 10.4 | 4.1 |
| resolution | 6.3 | 3.8 | 4.3 |
| value/string | 5.0 | 2.3 | 4.8 |
| scope_validation | 3.2 | 3.5 | 2.9 |
| source/coordinates | 2.1 | 3.4 | —（未单列） |
| lowering | 1.6 | 2.0 | 2.6 |
| other（未归类符号） | 9.1 | 7.8 | 3.0 |

单符号热点（functions）：`verify_parts_with_visits` 6.0%、`validate_scope_graph`
3.2%、`enqueue_fallthrough` 3.1%、`Lexer::next_token_with_goal` 2.9%、
`ensure_closure_variable`（线性扫描）2.5%、`QuickJsSourceCursor::locate` 1.9%、
`Instruction::operand_contract` 1.7%、`ensure_annex_b_binding` 1.4%、
`JsString::content_hash` 1.1%、`Utf16Units` drop 1.2%。

结论：P1 只动了 lexer/token（自时间 ~7%）与名字流（分配约 1/6），
**libc 27%、verify+publish 20%、IR/常量/绑定/字节码与坐标计算等大头未动**，
所以总收益有限（相对 P0：时间 −21.1%、instr −9.6%、alloc −27.7%、
cache-miss −71.2%）。名字驻留的真实收益在局部性（cache-miss）而非指令数；
`JsString::content_hash`/`ensure_closure_variable`/`QuickJsSourceCursor::locate`
是 P1b 后新可见的候选项（见 `docs/lexer-parser-refactor.md` §3 P4）。

#### 9.7.5 复现命令

```sh
python3 scripts/benchmark/compile_matrix.py --corpus target/compile-corpus \
  --metric compile \
  --engine oxide=target/p1b-compile-probe/target/release/oxide-compile-probe \
  --engine boa=target/compile-probes/boa/target/release/boa-compile-probe \
  --repeat 5 --output target/p1b-matrix-generated
# 真实 bundle：--corpus target/compile-bundles --output target/p1b-matrix-bundles

# 阶段占比（profiling 构建）
python3 scripts/benchmark/build_compile_probe.py --repo . --profiling \
  --output target/p1b-profile-probe
target/p1b-profile-probe/target/release/oxide-compile-probe FILE  # 阶段 JSON 到 stderr

# flat profile
perf record -F 999 --call-graph dwarf -o target/perf-p1b-functions.data -- bash -c \
  'for i in $(seq 10); do target/p1b-compile-probe/target/release/oxide-compile-probe \
     target/compile-corpus/functions-4194304.js >/dev/null; done'
perf report -i target/perf-p1b-functions.data --stdio --no-children
```

### 9.8 P2 checkpoint（P2a `7f8fc101` + P2b commit-path reuse，2026-09-24）

P2a（`7f8fc101`：`LookaheadCache` 备忘 17 处 clone-lexer 探针）与 P2b
（commit-path reuse：提交扫描直接消费探针已备忘的 token；原计划的 `TokenBuffer`
全量改造按实测取消，见 `docs/lexer-parser-refactor.md` §3 P2b）相对 §9.6 P1b 的
checkpoint。度量口径与 §9.6 相同（P1b/P2a/P2b 三探针交错各 7 次取最小值，
4MB 扣 64KB tiny 档；task-clock 粒度 10ms，约 ±0.7% 噪声）。探针目录：
`target/p1b-*`、`target/p2a-*`、`target/p2b-final-*`。

命中数据（4MB 档，profiling 探针 stderr；提交复用=提交路径消费的探针备忘项；
`max_entries` 是缓存活跃条目峰值，上限 8192）：

| 语料 | 探针命中 | 探针未命中 | 命中率 | 提交复用（P2b） | max_entries |
| --- | ---: | ---: | ---: | ---: | ---: |
| functions | 292,314 | 533,644 | 35.4% | 421,476 | 24 |
| expressions | 245,017 | 649,709 | 27.4% | 388,173 | 31 |
| syntax-mixed | 325,916 | 767,837 | 29.8% | 555,162 | 25 |

perf（4MB 扣 64KB tiny 档，三探针交错 7 次最小值）：

| 语料 | 指标 | P1b | P2a（vs P1b） | P2b（vs P1b） |
| --- | --- | ---: | ---: | ---: |
| functions | instr/KB | 2,621,754 | 2,597,486（−0.9%） | 2,488,994（−5.1%） |
| | cycles/KB | 1,394,373 | 1,348,550（−3.3%） | 1,324,844（−5.0%） |
| | cache-ref/KB | 84,328 | 86,184（+2.2%） | 85,441（+1.3%） |
| | cache-miss/KB | 5,107 | 5,112（+0.1%） | 5,117（+0.2%） |
| | task-clock/KB | 0.335 ms | 0.328 ms（−2.1%） | 0.321 ms（−4.0%） |
| | IPC | 1.88 | 1.93 | 1.88 |
| expressions | instr/KB | 1,751,097 | 1,748,105（−0.2%） | 1,657,049（−5.4%） |
| | cycles/KB | 821,956 | 796,958（−3.0%） | 771,238（−6.2%） |
| | cache-ref/KB | 40,764 | 42,478（+4.2%） | 42,149（+3.4%） |
| | cache-miss/KB | 1,482 | 1,476（−0.4%） | 1,454（−1.9%） |
| | task-clock/KB | 0.209 ms | 0.203 ms（−2.9%） | 0.197 ms（−5.9%） |
| | IPC | 2.13 | 2.19 | 2.15 |
| syntax-mixed | instr/KB | 2,585,892 | 2,582,731（−0.1%） | 2,453,895（−5.1%） |
| | cycles/KB | 1,517,328 | 1,497,006（−1.3%） | 1,420,726（−6.4%） |
| | cache-ref/KB | 70,395 | 72,558（+3.1%） | 71,212（+1.2%） |
| | cache-miss/KB | 4,691 | 4,687（−0.1%） | 4,631（−1.3%） |
| | task-clock/KB | 0.383 ms | 0.381 ms（−0.5%） | 0.361 ms（−5.8%） |
| | IPC | 1.70 | 1.73 | 1.73 |

分配（4MB 档，计数逐次完全一致；P2 相对 P1b 的差就是缓存 `Vec` 本身）：

| 语料 | P1b alloc | P2b alloc | 差值 | P1b 分配字节 | P2b 分配字节 | peak live 差值 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 5,574,821 | 5,574,827 | +6 | 793.4 MB | 793.4 MB | +16,384 B |
| expressions | 2,745,314 | 2,745,320 | +6 | 650.0 MB | 650.0 MB | +16,384 B |
| syntax-mixed | 6,548,538 | 6,548,544 | +6 | 1,180.0 MB | 1,180.0 MB | +16,384 B |

相对 P0 基线（§9.1/§9.2，P2b）：alloc 次数 functions −27.7%、
expressions −32.4%、syntax-mixed −22.4%（与 P1b 持平）；instr/KB
−14.1%/−10.7%/−9.4%；cache-miss/KB −70.9%/−36.1%/−32.6%；task-clock
−26.2%/−21.5%/−21.9%。

#### 9.8.1 test262 `--full`（语义中立验证）

P2 全量报告 `target/test262-full.tsv`（engine hash `6e6e2003…`，2026-09-24）
与 P1b 报告（`5dbb43ca…`）除首行 hash 外**逐字节一致**：body sha256 均为
`971cc666…`，summary 行一致（pass=80010、fail-parse=7、fail-runtime=43、
unsupported-negative-provenance=2534 等，102,037 variants）。runner 的
line-count/checksum/diagnostic-contract/metadata/classified-vector 检查全部
通过；已知 +28 runnable 漂移（80060 vs 里程碑 80032）为 P1a/P1b 既有问题，
与本次改动无关。

#### 9.8.2 真实 bundle 矩阵与病态用例

`scripts/benchmark/compile_matrix.py --corpus target/compile-bundles --metric
compile --repeat 5`，P1b/P2b 探针同场交错（每 case 5 次取中位）：

| 引擎 | 中位吞吐 | 范围 | 每 case 比值（P1b/P2b） |
| --- | ---: | ---: | ---: |
| P1b | 5.461 MB/s | 4.838–11.706 | — |
| P2b | 5.953 MB/s | 5.026–12.396 | 中位 1.080（几何均值 1.070，52/67 case 更快） |

真实 bundle（67 个多小文件，约 34KB）上 P2 收益（中位吞吐 +9.0%）大于 4MB
生成语料（task-clock −4.0%~−5.9%）：小文件上探针/提交重扫在总时间中占比更高。

病态用例 `test/staging/sm/String/string-upper-lower-mapping.js`（3.2 MB 数组
字面量）用非 profiling 探针复测：P1b 360–378ms（3 次），P2b 355–361ms；修复
后与 P1b 持平（缓存缺陷中间态为 272s）。

结论与校准：

1. **P2a 单独收益很小**（instr −0.1%~−0.9%、cycles −1.3%~−3.3%、task-clock 在
   10ms 粒度噪声内）：探针重扫在总成本中占比小（命中率 27–35%，每 4MB 省
   25–33 万次扫描），计划 §3 P2a 预期的“分配次数下降”被实测否决——token 扫描
   本身几乎不分配，P2a 分配中性（缓存自身 +4 次/+4 KB）。
2. **提交路径复用是 P2 的主要收益来源**：再省 39–56 万次重扫（=探针已扫、提交
   重扫的全部浪费），相对 P1b 贡献 instr −4.2%~−5.2%、cycles −1.8%~−5.1%、
   task-clock −2.1%~−5.3%（与 P2a 合计见表），分配中性、cache-miss 基本持平。
   它与探针共用同一纯记忆化缓存，`tokens`/`cursor`/`relex*`/`set_future*` 语义
   不变（见 §C.1 不变量），因此不改变提交 token 边界与 PR #30 的栈守卫采样时机。
3. **缓存结构修复（本 checkpoint 发现）**：探针 `array_assignment_pattern_ahead`
   会把整个 `[...]` 扫到匹配的 `]`，在巨型数组字面量上会备忘数十万 token；
   原 `invalidate_before` 用 `Vec::drain(..k)` 从头部逐条删除，每次 advance
   搬移整个活跃区，退化为 O(n²)：test262
   `test/staging/sm/String/string-upper-lower-mapping.js`（3.2 MB 数组字面量，
   约 39 万 token）从 P1b 的 376ms 恶化到 272s（>700×）。修复=活跃条目上限
   8192 条 + `base` 偏移摊销压缩（`base >= 64` 且不小于活跃数时一次性 drain），
   失效摊销 O(1)；该文件回到 355–361ms（非 profiling 探针，与 P1b 持平），
   三个基准语料的命中/条目峰值不变（24/31/25 条），语义 gate 全绿。
4. **原 P2b `TokenBuffer` 全量改造取消**：commit-path reuse 已把可归因的重扫浪费
   全部吃掉；剩余空间（探针未命中之间的重叠、`parenthesized_parameter_tokens`
   的 Vec 复制）远小于其 15 条不变量 + 逐产生式迁移的回归风险。
   `parenthesized_parameter_tokens` 去复制降级为 P3 触发项（触发=分配探针可
   归因 >1%）。因此 P2 未达 §5 原“P2a+P2b”方向目标（instr −10%~−20%、miss
   −20%~−35%、分配 −5%~−10%），按 §5 校准规则改以本表为 P3 的起点。

复现命令：按 §9.4 构建三组探针（`--output target/{p1b,p2a,p2b-final}-compile-probe`
与 `...-alloc-probe`，分配探针加 `--probe scripts/benchmark/probes/compile_alloc_probe.rs
--name oxide-compile-alloc-probe`；profiling 命中计数加 `--profiling`），
perf 用三探针交错 min-of-7，alloc/RSS 直接跑探针。test262 全量用
`target/run-test262-full.sh`（清空 `GIT_*` 环境变量），bundle 矩阵用
`scripts/benchmark/compile_matrix.py` 并传两个 `--engine`。

### 9.9 P3 lexer 字节快路（2026-09-24）

改动（`src/engine/compiler/lexer.rs`）：词法扫描全面改走可移植的 ASCII 字节快
路径，非 ASCII 与转义仍走原逐标量慢路径（语义不变）：

- `peek_char`/`bump_char`：ASCII 直接读字节，跳过 UTF-8 解码与
  `quickjs_column_delta`（原实现每字符做 slice 取字节 + filter 计数）；
  例外是 `INVALID_BYTE_CARRIER`（0x7F）——它在 carrier 里是 ASCII，但可能代表
  原始 continuation 字节（列增量为 0），仍走慢路径（回归测试
  `raw_script_error_columns_scan_authored_comment_bytes` 覆盖）。
- `skip_trivia`/`skip_to_line_end`：按首字节分派，批量跳过空白与行注释体，
  用字节比较替代 `starts_with`（消除 `scan_punctuator`→`starts_with` 的
  memcmp 归因）。
- `scan_identifier_with_value`：用字节表批量消费 ASCII 标识符 run；注入
  string limit 的报错位置按“第一个超限字符”精确对齐逐字符路径。
- `scan_punctuator`：首字节 `match` 分派替代 55 项线性 `starts_with` 表，
  `?.` 的非数字前瞻用字节判断。

perf（4MB 扣 64KB tiny 档，P1b/P2b/P3 三探针交错 min-of-7）：

| 语料 | 指标 | P2b | P3（vs P2b） | P3（vs P1b） |
| --- | --- | ---: | ---: | ---: |
| functions | instr/KB | 2,489,067 | 2,225,524（−10.6%） | −15.1% |
| | cycles/KB | 1,329,333 | 1,249,775（−6.0%） | −10.5% |
| | task-clock/KB | 0.328 ms | 0.311 ms（−5.1%） | −9.7% |
| | cache-miss/KB | 5,068 | 5,102（+0.7%） | +3.2% |
| expressions | instr/KB | 1,657,093 | 1,333,937（−19.5%） | −23.8% |
| | cycles/KB | 769,071 | 656,311（−14.7%） | −19.2% |
| | task-clock/KB | 0.199 ms | 0.175 ms（−12.0%） | −16.5% |
| | cache-miss/KB | 1,454 | 1,456（+0.2%） | +0.8% |
| syntax-mixed | instr/KB | 2,454,067 | 2,144,125（−12.6%） | −17.1% |
| | cycles/KB | 1,428,572 | 1,308,482（−8.4%） | −12.6% |
| | task-clock/KB | 0.378 ms | 0.351 ms（−7.2%） | −10.8% |
| | cache-miss/KB | 4,665 | 4,642（−0.5%） | −0.0% |

探针内计时（`compile_ns`，5 次 min）：functions 1.231s→1.130s（−8.2%）、
expressions 0.770s→0.678s（−12.0%）；64KB 档 instr/KB −15.2%/−20.2%/−15.2%
（同一方向，排除大文件效应）。相对 P0 累计（P2b×P3 相乘）：task-clock
−29.9%/−30.9%/−27.5%、instr −23.2%/−28.1%/−20.8%、cache-miss
−70.7%/−36.0%/−32.9%、分配不变（−27.7%/−32.4%/−22.4%）。

真实 bundle（67 case，P2b/P3 交错 5 次）：中位吞吐 5.874→6.547 MB/s
（**+11.5%**），每 case 比值中位 1.122（几何均值 1.121，64/67 更快）；相对 P0
基线 4.79 MB/s 累计 **+36.7%**。

分配（4MB）：三个语料的 alloc/realloc/dealloc/bytes/peak 与 P2b **逐位相同**
（快路径零分配）。

语义：全量 test262 报告与 P1b 逐字节一致（body sha `971cc666…`，102,037
variants）；oracle 912、fixtures 13/13、unsupported_diagnostics 6、lib 2270
全绿。

flat profile（expressions 4MB，self time）：`next_token_with_goal` 4.08%→3.94%
（占比因总量下降 −19.5% 而几乎持平，绝对成本约 −22%）、`bump_char`
1.72%→<0.5%、`scan_punctuator` 1.49%→<0.5%、`scan_identifier_with_value`
1.19%→0.89%；libc 桶仍是最大项（malloc/free 与 memcpy），归因不变。

结论与校准：

1. **P3 首项（lexer 字节快路）已超 §5 目标**：time −5.1%~−12.0%（目标
   −3%~−6%）、instr −10.6%~−19.5%（目标 −5%~−10%）；miss ≈0（目标
   −5%~−10%）、alloc 0（目标 0~−5%）——与 P2 一致，miss/alloc 大头不在
   lexer，转 P4 触发项。
2. 收益主要来自三点：消除逐字符 `quickjs_column_delta`/UTF-8 解码、消除
   `skip_trivia`/`scan_punctuator` 的 `starts_with` 调用与 memcmp、标识符批量
   扫描（expressions 语料标识符/标点最密，收益最大 −19.5%）。
3. 剩余 P3 项按触发规则重判：数字字面量快路（flat profile 未见
   `scan_number` ≥0.5%，触发不成立，跳过）；`SourceText` 共享（需分配归因，
   分配总量未变且深拷贝未见归因，暂缓）；SmallVec/ThinVec 按 §2.4 决策规则
   在 P4 分配归因后决定。

复现命令：同 §9.8；lexer 探针为 `target/p3-lexer-{compile,alloc}-probe`，
perf 目录 `target/p3-perf/`，矩阵 `target/p3-matrix-bundles/`。

### 9.10 P4-6 预实验：closure 描述符索引（已回滚，2026-09-24）

`docs/lexer-parser-refactor.md` §3 P4 第 6 条的触发证据与可行性预实验。按 §6
“P4 候选不进入本分支”，实验代码已回滚，本记录仅保留证据。

触发证据（临时诊断计数，P3 树 + profiling 探针；`ensure_closure_variable` 每次
调用扫描的候选总数）：

| 语料（4MB） | 查找次数 | 扫描候选总数 | 平均 | 最长 Vec |
| --- | ---: | ---: | ---: | ---: |
| functions | 6,798 | 69,315,807 | 10,196 | 13,595 |
| expressions | 27,530 | 63,314 | 2.3 | 5 |
| syntax-mixed | 5,524 | 2,761 | 0.5 | 1 |

`functions` 档是唯一的 O(N²) 形态：某个函数（按条目数应为 root）积累约 1.36
万个 closure 条目，而 `Global` 查找按名字线性扫过整个向量（生成语料每个块都
声明并引用新名字，属压力档构造，非真实负载形态）。`ensure_captured_closure_variable`
（captured 路径）在三语料均为 ≤0.5 步/次，无问题。

预实验实现：`FunctionIr` 增加 `global_closure_index: HashMap<GlobalClosureKey, u16>`
（`Global`/`GlobalDeclaration` 按名字分命名空间索引，`push_closure_variable`
写入、首见索引优先），`ensure_closure_variable` 对这两种 source 走索引，
其余 source 保持线性扫描；`limits.rs` 的直写向量测试改为走
`push_closure_variable`。

perf（4MB 扣 64KB tiny 档，P3/P4 两探针交错 min-of-7）：

| 语料 | 指标 | P3 | P4（vs P3） |
| --- | --- | ---: | ---: |
| functions | instr/KB | 2,225,543 | 2,086,331（−6.26%） |
| | cycles/KB | 1,231,437 | 1,216,343（−1.23%） |
| | branches/KB | 491,572 | 440,167（−10.46%） |
| | cache-ref/KB | 84,685 | 75,754（−10.55%） |
| | task-clock/KB | 0.311 ms | 0.309 ms（−0.63%） |
| expressions | instr/KB | 1,333,898 | 1,342,261（+0.63%） |
| | cycles/KB | 653,256 | 660,278（+1.07%） |
| | task-clock/KB | 0.175 ms | 0.177 ms（+1.32%） |
| syntax-mixed | instr/KB | 2,144,142 | 2,163,651（+0.91%） |
| | cycles/KB | 1,333,167 | 1,332,879（−0.02%） |
| | task-clock/KB | 0.351 ms | 0.350 ms（−0.28%） |

64KB 档：functions instr +0.46%、cycles +1.74%；expressions +0.48%/−0.17%；
syntax-mixed +0.78%/+2.62%。探针内 `compile_ns`（5 次 min）：functions
1.171s→1.151s（−1.8%）。即 69.3M 步扫描被消除，但指令节省多为廉价的向量化
比较，cycles/task-clock 几乎不动，而小函数侧被 HashMap 常数开销抵消。

真实 bundle（67 case，P1b/P3/P4 交错 5 次）：每 case 速度比值 p3/p4 中位
**1.007**（38/66 case P4 更快），逐 case 求和耗时 −1.50%，而 459KB 的
`066-all` 中位 51.79ms→52.42ms（**+1.22%**）——整体在噪声内，无真实负载收益。

分配（4MB）：`FunctionIr` 增大导致 arena 扩容，realloc_bytes
functions/syntax +67.6MB、expressions +33.8MB；另有 map 表 alloc_bytes
+4.49/+1.57/+3.15MB、alloc 次数 +13/+2/+1；peak live +3.1/+1.6/+3.1MB。
与 P4 的分配削减目标方向相反。

结论：触发证据成立但**收益不成立**——真实 bundle 中性、分配反向、函数侧常数
开销。判定为按 §6 不进入本分支；如后端计划重启该候选，应采用“按向量长度阈值
惰性建索引 + 索引放侧表（避免 `FunctionIr` 增大）”的形态，并先取得真实 bundle
的明确收益再落地。

复现：探针 `target/p4-global-index-{compile,alloc}-probe`（`build_compile_probe.py`
+ 临时诊断计数，计数源码未保留），perf 目录 `target/p4-perf/`（脚本
`target/p4-perf.sh`、汇总 `target/p4-summarize.py`），矩阵
`target/p4-matrix-bundles/`。

### 9.11 P4-2 预研：分配归因（P3 树 `23b918e7`，2026-09-24）

`docs/lexer-parser-refactor.md` §3 P4 第 2 条（IR/常量/绑定/字节码侧分配削减）
的触发证据与归因方法。按 §6 不改产品代码：临时插桩与采样工具全部在
`target/p4-attr/`，产品树已回滚。

方法（三路互证）：

1. **阶段精确计数**：临时给 `profiling` 加 `AllocationObserver`
   （`fn() -> (u64, u64)`，探针注册）并让 `PhaseTimer` 在边界快照，计数分配器
   探针 `target/p4-alloc-phase-probe` 输出每阶段 alloc 次数/字节（临时补丁
   `target/p4-attr/temp-instrumentation.patch`，已回滚）。分配计数逐次完全一致。
2. **站点采样**：`LD_PRELOAD` 分配拦截器 `target/p4-attr/malloc_trace.c`
   每 32 次分配记录一次 backtrace，离线用 `addr2line -f -C -i` 符号化到
   file:line（`target/p4-attr/symbolize.py`；带 debug info 的探针
   `target/p4-debug-probe`）。**调用次数**为均匀采样（±1%）；
   **字节列受重尾影响（9.11.4），本文不单独引用**。
3. **大块精确**：同一拦截器 `TRACE_EVERY=1 TRACE_MIN=8192`，≥8KB 分配全量
   记录（无采样偏差）。

#### 9.11.1 阶段分布（4MB 档，精确）

分配次数（括号为占该语料）：

| 阶段 | functions | expressions | syntax-mixed |
| --- | ---: | ---: | ---: |
| parse | 958,636（18.6%） | 1,161,827（43.1%） | 1,469,479（23.4%） |
| resolution | 690,041（13.4%） | 267,089（9.9%） | 560,726（8.9%） |
| lowering | 1,128,485（21.9%） | 459,772（17.0%） | 1,400,356（22.3%） |
| verify | 1,063,930（20.7%） | 300,141（11.1%） | 1,284,398（20.4%） |
| publish | 1,301,897（25.3%） | 509,399（18.9%） | 1,577,208（25.1%） |
| 合计 | 5,143,001 | 2,698,240 | 6,292,179 |

分配字节（alloc，不含 realloc）：

| 阶段 | functions | expressions | syntax-mixed |
| --- | ---: | ---: | ---: |
| parse | 59.1MB（14.3%） | 51.8MB（20.7%） | 136.0MB（19.4%） |
| resolution | 32.5MB（7.9%） | 11.8MB（4.7%） | 25.5MB（3.6%） |
| lowering | 145.7MB（35.2%） | 91.5MB（36.6%） | 221.2MB（31.5%） |
| verify | 57.3MB（13.9%） | 28.8MB（11.5%） | 125.2MB（17.8%） |
| publish | 118.9MB（28.7%） | 66.1MB（26.4%） | 194.3MB（27.7%） |
| 合计 | 413.5MB | 250.0MB | 702.3MB |

阶段时间占比（同探针 inclusive，与 §9.9 结构一致）：functions
27.8/16.1/17.6/15.0/23.6，expressions 40.8/9.7/18.8/11.7/19.0，
syntax-mixed 28.1/8.5/20.2/17.2/26.0（parse/resolution/lowering/verify/
publish）。

#### 9.11.2 规模与斜率（4MB）

| 语料 | lowered 函数 | 指令 | alloc 次数 | alloc/函数 | alloc/指令 | realloc 次数 | realloc 字节 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| functions | 37,390 | 1,233,839 | 5,143,001 | 137.6 | 4.2 | 700,361 | 849.8MB |
| expressions | 22,025 | 988,329 | 2,698,240 | 122.5 | 2.7 | 204,013 | 878.7MB |
| syntax-mixed | 38,669 | 1,436,242 | 6,292,179 | 162.7 | 4.4 | 615,439 | 1,090.9MB |

64KB 档斜率与 4MB 一致（functions 137.6 → 139.6、expressions 122.5 → 124.1、
syntax-mixed 162.7 → 164.6 次/函数），说明分配以函数/条目为单位的常数项为主、
无显著固定开销。realloc 字节与 alloc 同量级，是“容量增长”的第二成本；其中
大块占绝对多数（9.11.4）。

#### 9.11.3 站点 Top（functions-4MB，采样×32，±1%；仅调用次数）

| 次数 alloc+realloc | 站点（file:line） | 说明 |
| ---: | --- | --- |
| 360,320 | `scope_validation::validate_scope_graph`（scope_validation.rs:827/1129/1590-1593/1913/1995-1997） | 每函数 5–7 个 `vec![false/0; len]` 校验缓冲 |
| 345,408 + 243,360 + 204,896 | `JsString::from_validated_utf16`（primitive.rs:997 collect、999 Rc）/`try_from_utf16_with_limit`（950） | 每个字符串 2–3 次分配 |
| 190,496 + 83,232 | `verify::private_elements::setter_storage_base`（private_elements.rs:41） | 每私有 setter 2 个 collect + `to_vec` |
| 133,600（全 realloc） | parser `ops.push`（builder.rs:287） | 每函数 IR ops 扩容（大块见表 9.11.4） |
| 124,384 / 96,416 / 107,424 / 99,808 | verify：`worklist`（bytecode.rs:959）/`next_*` 克隆（989-991）/`VerificationState::clone`（1520）/`CompactVisits::depths`（1548） | 每函数/每次验证状态分配 |
| 114,752 + 54,912 + 60,672 + 38,720 | parse 文本拷贝：expressions.rs:124（NamedEvaluation 标识符 `to_owned`）、expressions.rs:1096（字段名）、tokens.rs:457（label + lexer clone）、arrow.rs:333（for-head 分隔符 `vec!`） | P1b 后残留的 owned 文本 |
| 89,216（expressions 280,608） | `parse_digits`（literals.rs:461） | 非十进制字面量走 `BigUint::parse_bytes` |
| 104,352 + 72,448 + 63,136 | `bytecode_validation`：explicit parameter layout、derived constructor（193 `with_capacity(4)`、228 `HashSet` collect） | 每构造函数校验 |
| 91,328 | `gc::function_bytecode_edges`（gc.rs:1833） | 每函数 GC 边表 |
| 71,104 | `lowering::build_unlinked_debug`（lowering.rs） | 每函数 debug 切片 |
| 70,688 | `Heap::allocate_function_bytecode`（allocation.rs:657） | 每函数 HashMap + 注册 |
| 111,904 | `lower_ops`（lowering.rs:1101 offsets、1171/1172 code/pc_sites） | 每函数 3 个 Vec |
| 116,032 | `FunctionIr::add_binding`（function.rs:571 bindings、576 map） | 每条绑定 2 次 |
| 53,536 | `FunctionIr::append_constant`（function.rs:544） | 每常量 |
| 187,136 | publish `Vec→Box` 转换（runtime.rs:234/242/244/248） | 每函数 ~7 个 `into_boxed_slice`/`into` |
| 50,784（+31MB） | `resolution::resolve_identifiers` unresolved 列表扩容（resolution.rs:150） | 预分配 |
| 37,344 | `FlattenFrame::new`（runtime.rs:457） | 每函数 constants Vec |

跨语料差异：expressions 的 parse 占比最高（43.1%），`parse_digits`（28.1 万）
与 `regexp_case_change_mask`（2,753 次 ≥8KB）更突出；syntax-mixed 的 verify
状态克隆与 `CompactVisits` 更重。

#### 9.11.4 大块分配（≥8KB，精确）

| 语料 | ≥8KB 次数 | alloc 字节（占自身） | realloc 字节（占自身） |
| --- | ---: | ---: | ---: |
| functions | 393（0.008%） | 67.5MB（16.1%） | 572.5MB（66.0%） |
| expressions | 3,066（0.11%） | 116.8MB（45.7%） | 812.5MB（91.9%） |
| syntax-mixed | 409（0.007%） | 306.3MB（43.3%） | 929.3MB（83.9%） |

大块站点（按字节）：`parser/tokens.rs:615`（提交 token 缓冲扩容；functions
234.9MB、expressions/syntax 各 469.7MB，峰值容量 117–224MB）、
`function.rs:442`（FunctionBuilder 列表，58–100MB）、`Heap::reserve`
（arena.rs:85，57.7MB）、`flatten_unlinked_tree`（bytecode_publish.rs:159，
43.0MB）、`CompactVisits` exceptional 状态（bytecode.rs:1597，syntax-mixed
167MB）、`verify/children.rs:439`（18.9MB）、`class/fields.rs:106`（14.8MB）、
`gc::finish_node`（14.7MB）、`regexp_case_change_mask`（expressions 2,753 次
×8KB）。token 缓冲与 FunctionBuilder 列表属 P4-1/表增长范畴，与 P4-2 的
“海量小分配”分开处理。

#### 9.11.5 复现

```sh
# 阶段计数（临时插桩）：target/p4-attr/temp-instrumentation.patch
python3 scripts/benchmark/build_compile_probe.py --repo . --profiling \
  --output target/p4-alloc-phase-probe --probe target/p4-alloc-phase-probe.rs \
  --name oxide-compile-alloc-phase-probe
target/p4-alloc-phase-probe/target/release/oxide-compile-alloc-phase-probe FILE

# 站点采样（debug info 探针 + 拦截器）
CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_PROFILE_RELEASE_STRIP=none \
  python3 scripts/benchmark/build_compile_probe.py --repo . \
  --output target/p4-debug-probe --name oxide-compile-debug-probe
gcc -shared -fPIC -O2 -o target/p4-attr/malloc_trace.so target/p4-attr/malloc_trace.c -ldl
TRACE_OUT=target/p4-attr/samples.bin LD_PRELOAD=target/p4-attr/malloc_trace.so \
  target/p4-debug-probe/target/release/oxide-compile-debug-probe FILE
python3 target/p4-attr/symbolize.py target/p4-attr/samples.bin \
  target/p4-debug-probe/target/release/oxide-compile-debug-probe --top 35

# 大块精确：TRACE_EVERY=1 TRACE_MIN=8192（同一拦截器）
```

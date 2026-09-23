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
3. **Oxide 包含 verify/publish**：这是 `Context::compile_*` 的真实边界，不做删减；
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
   parse/resolution/lowering/blocks/fusion/relocation/verify/publish 的
   inclusive/exclusive 纳秒与 attempts（现有 `CostProfile`），在 512KB 生成语料上采集。
2. **函数级拆分**：release + `debug=1` 构建普通探针，`perf record -g` 后按符号归并
   `lexer.rs`（`scan_identifier`/`skip_trivia`/`scan_number`/…）、parser、resolution、
   lowering、验证与分配（malloc）占比。用于回答“lexer 还是 parser 更贵”。
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
   `code::bytecode::enqueue_fallthrough`（3.1%，publish 的 VecDeque）、
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
3. **verify/publish**：verify 单符号 6.4% + publish VecDeque 3.1%；两阶段合计
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
| P0 | 分配总量过高：identifier `String`、UTF-16 转换、map 插入 | libc 27%；`Utf16Units::new`/`JsString::from_validated_utf16`；hashbrown 4.3% | 优化后重跑矩阵 + 分配计数（`profiling` 现有计数器） |
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

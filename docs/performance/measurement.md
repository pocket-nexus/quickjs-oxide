# 测量、累计门禁与回退协议

> 本协议保留当次准入阈值和可复现的测量方法。13 种跨度与 #41 已实现；一次完整[四方 V8 配对及定向 profile](receipts/fourway-2026-09-25/README.md)已完成。当时按维护者要求继续完成既定代码，并记录未达门槛的结果；这不是未来候选的实施义务。对新候选，覆盖、生成代码、A/A 和真实负载结果可以推翻预设形态、接口或继续投入的决定；未达门槛须归因，并可收缩、修改或撤销候选。正式 benchmark／profiling 仍须与构建和其他测试串行隔离。当次 PR 只提交结果总结；原始样本和构建回执保留在本机测量目录，不作为该次仓库附件。
> 与 [benchmark 工具说明](../../scripts/benchmark/README.md)、[Test262 契约](../test262.md) 配套。

<a id="baselines"></a>
## 1. 基线与比较身份

| 名称 | 源码身份 | 用途 |
| --- | --- | --- |
| B37 | `3341ac456ea2719858fd6173e8dcd9123ad9e660` | 维护者 3–4 倍总分目标的固定分母；PR #37 head，不冒充实现提交 |
| R0 | `f531f6052cb497ce4707f01c276e8642e5e26788` | 本轮新增优化的起点；文档 PR 不改变引擎源码 |
| H0 | `3b759647d32ad9b16317d4760deb646eeedefec3` | pre-B2 历史回退债务分母；#42 使用的 `8a4b89d4` 据报告与其树相同 |
| Parent | 本切片实际父提交 | 逐片增量 |
| R1 | #41 接纳后的确切提交；未接纳则为 R0 | B 线工作基线，不能抹去 B37／R0／H0 的比较 |
| Candidate | 本切片干净提交 | 受测产物；不能只报分支名 |

所有分母用该系列选定的同一工具链／target／flags 重建；绝不混用旧二进制。本轮新性能系列可使用 Rust 1.96.0，另以 1.88.0 执行最低版本／CI 兼容验证；版本不是永久限制，以实际回执为准。#42 的 1.88.0 性能数字与 #43 的 1.94.1 数字保留在各自历史系列，不直接相除。换机器、工具链、target 或有效编译配置时整套分母重建，另起 series id。

全量成绩始终同时报告 Candidate/B37、Candidate/R0、Candidate/Parent。H0 只用于旧债务台账，不能替代更难的 B37 目标分母。历史 pre-A 的问题继续通过历史报告追踪，不得宣称本次文档整理已经清偿。

## 2. 构建、机器与上游 pin

使用普通 release、fat LTO、CGU=1，无 PGO、无 profiling。同一系列的双方在同一机器、相同配置下串行构建和测量；绑核不能代替整机隔离。记录 CPU／系统版本、可用硬件计数器、电源／频率策略、亲和性、内存状态、负载和是否存在其他会话；不支持的字段标未知。`run.py` 自动收集所在平台可取得的快照（macOS 包括 CPU 型号、物理内存、`pmset`、`vm_stat` 和 load average），微码、共享缓存干扰等仍需按主机能力补记。干扰或 A/A 波动足以覆盖候选时间差、交错结果无法区分时，cycles／wall 标“未裁决”，不作通过结论。已知同机干扰必须标注；它不自动抹掉在交错复测中持续分离的回退信号，也不能提供缓存或分支预测的因果解释。

本轮外部 v8-v7 pin 为 `ahaoboy/js-engine-benchmark@2034d98fc8c5f8044e186267593f5d5ea5232caf`（#43 所用 checkout 的完整身份）；`run.py` 默认核对完整 SHA 和 tracked source 洁净度。未来系列可明确指定另一完整 pin（`--v8-source-commit`），并重建全部 bundle／分母，不把不同语料混算。每个生成 bundle 单独记录 SHA-256；pin 相同不代表旧的 `dist/` 一定由它生成，必须重新生成并保存生成器身份。QuickJS 对照仍用项目 pinned 2026-06-04 oracle，记录 C 编译器与 flags；跨引擎是诊断参考，不代替同源码 A/B。

在每个干净 worktree 独立构建。可从当前工具 checkout 构建历史源码 worktree，源码与工具身份分开记录：

```sh
# SOURCE_TREE 是干净的受测源码根目录；OUT 必须是本次新目录且与其他版本分开。
OUT=$(mktemp -d /tmp/oxide-perf.XXXXXX)
RUSTUP_TOOLCHAIN=1.96.0 python3 scripts/benchmark/build.py \
  --repo "$SOURCE_TREE" --plain-only --jobs 2 --plain-target "$OUT/plain"
# 正式计时只用 $OUT/plain/release/qjs；qjs.build.json 随二进制保留。
```

`build.py` 回执记录源码 commit/tree、工具身份与脚本快照、二进制 hash、编译器、release profile 声明、环境覆盖、Cargo 配置文件 hash、完整构建命令及 stdout/stderr 日志；构建期间源码或工具脚本变化会拒绝回执。Cargo `--verbose` 实际执行的 qjs 与依赖 rustc 参数另作摘要。缓存命中的 crate 没有新 rustc 行，不能把声明值冒充实测有效参数；正式重建应使用新 target 目录并保存完整日志。`run.py` 在每次采样前复核工作负载与二进制 hash，并保存样本、顺序与机器快照；主机干扰状态和不支持的硬件指标仍需人工注明。不同 worktree 的 OUT 必须不同。

本项目源码和计划目录内不 vendor 外部 benchmark。以下四个维护者重建探针保留为固定工作量配方；本次实施的[历史门禁总结](receipts/gates-2026-09-25/README.md)记录结果，原始负载与样本留在本机，不能混同外部 V8 正式 Score。

```sh
mkdir -p "$OUT/micro"
cat > "$OUT/micro/empty_loop.js" <<'JS'
function empty_loop(n) { var j; for (j = 0; j < n; j++) { ; } return n; }
print(empty_loop(10000000));
JS
cat > "$OUT/micro/int_local.js" <<'JS'
function int_local(n) { var j = 0, s = 0; for (; j < n; j++) { s = s + 1; } return s; }
print(int_local(10000000));
JS
cat > "$OUT/micro/prop_read.js" <<'JS'
function prop_read(n) { var j = 0, s = 0, o = {a: 1}; for (; j < n; j++) { s += o.a; } return s; }
print(prop_read(10000000));
JS
cat > "$OUT/micro/array_read.js" <<'JS'
function array_read(n) { var j = 0, s = 0, a = [1, 2, 3, 4]; for (; j < n; j++) { s += a[j & 3]; } return s; }
print(array_read(10000000));
JS
sha256sum "$OUT"/micro/*.js > "$OUT/micro.sha256"
```

期望输出依次为 `10000000`、`10000000`、`10000000`、`25000000`，均带 LF。每次都验证 stdout、stderr 和退出码；新配方的完整 hash 为自身身份，不冒充 issue 评论中的截断 hash。String／BigInt／调用等诊断也必须把源码和几何冻结下来，不引用他人机器的 `target/...` 当输入。

## 3. 三方对照与两类工作量

对新增热路径入口的候选，优先保留 Base、Entry-only、Full 三种构建或等价的可解释对照。Entry-only 保留新入口／候选发现但不执行专用提交，用来观察准入与 codegen 税；Full 真正删除 canonical 工作。若方案是替换现有路径而非新增入口，应说明怎样测静态不适用和自然未命中成本，不必为满足这三个名称而增加产品代码。对照不是纯净的因果分解：编译器可能因永不命中而删代码或改变内联，因此须核对相关版本的反汇编。必要时增加 Full 二进制上的自然命中／自然未命中配对负载，不能凭几个总数就断言某一机制成本。

固定诊断使用完全一致的工作量。先测同二进制 A/A，再用交错 ABBA／BAAB 顺序测 A/B；`run.py` 和 `fixed.py` 的 `--order abba-baab` 可复现完整交替块（`--repeat` 为 4 的倍数），默认顺序保留旧行为。保留原始样本和顺序，不只存中位数。能取得退休指令计数时，确定性差异也要调查，不把其变化与 wall 调度噪声混为一谈。旧协议的“至少两轮、每轮每侧至少五个独立进程”适用于要求正式准入裁决的完整复核，**不是每次实现迭代的启动条件**；短轮若 A/A 分辨率不足，时间变化保持未裁决。

```sh
# Linux perf 的单次示例；CPU 必须选主机允许的核，其他平台按可用工具记录等价事件。
CPU=2
perf stat -x, -o "$OUT/array.perf.csv" \
  -e instructions:u,cycles:u,branches:u,branch-misses:u -- \
  taskset -c "$CPU" "$OUT/plain/release/qjs" "$OUT/micro/array_read.js" \
  > "$OUT/array.stdout" 2> "$OUT/array.stderr"
```

macOS 可在固定矩阵运行中用 `fixed.py --darwin-counters`，让每个进程经 `/usr/bin/time -l -o` 执行；原始计数单独存档，不混入程序 stderr。解析退休指令、elapsed cycles、最大常驻集与 peak memory footprint，四项缺一即标为无效计数。它们是**整个进程**的事件和内存指标，包括启动／编译／退出，不冒充仅用户态事件或 JS 单操作成本；两侧须用完全相同的 wrapper。示例：

```sh
python3 scripts/benchmark/fixed.py --manifest "$FIXED_MANIFEST" \
  --workload-dir "$WORKLOADS" --engine base="$BASE_BIN" --engine candidate="$NEW_BIN" \
  --repeat 8 --order abba-baab --darwin-counters --output "$OUT/darwin-fixed"
```

退休指令只数真正退休的指令，不包含错误预测后丢弃的执行。`cache-references` 的具体事件依 CPU，不能翻译成全部内存访问或几百倍命中率差。cycles、绝对 branch-misses／操作、CPU 支持的前端／错误推测／后端指标分开报告；多路复用或不支持的计数器明确标注。不能单凭 miss rate 上升判断退化。没有可用计数器时，同机 A/A 可分辨的 wall 与正式 V8 Score 仍可裁决运行时间方向；退休指令、cycles 和缓存／预测归因保持“未测”，不得声称指令门禁已通过或旧指令债务已清偿。A/A 无法分辨的小幅时间变化同样保持“未裁决”。

正式 v8-v7 保持原本 benchmark body、warmup、计时、断言和 Score 计算，不为固定工作量修改后还称其原版分数。它按时间窗口增加迭代，变快可以导致整个进程退休指令更多；因此正式运行不能直接比较总 instructions。

开发迭代可用 [`iterate_v8.py`](../../scripts/benchmark/README.md#bounded-fixed-iteration-v8-comparison) 对 pinned 原始 body 做八项 isolated 与一项 combined 固定工作量对照。每个进程只加载一次上游 `base.js`，保留其中确定性的 `Math.random` 初始化；该 pin 没有独立 `ResetRNG` 方法。每个原始 Benchmark 执行一次 `Setup`、默认零次固定 warmup、冻结的 `run` 次数、一次 `TearDown`，原有内部校验照常执行。先用 baseline pilot 按子项校准，再冻结源码、driver、生成 JS hash 和运行次数；后续比较以 `--plan` 重放完全相同的工作量，不随候选快慢重新校准。A/A 和 A/B 依冻结的完整 ABBA-BAAB 块运行，预算不足可降为每侧两次的 ABBA。**单次调用**的 pilot、冻结与两类对照共用默认 600 秒采样截止；进程清理和最后写盘可能略超该时间。超时或未完成仍保留原始样本，不能输出总体指标。它报告的是固定迭代整进程时间及可用硬件计数器，不是原版自适应 V8 Score；并不保证任意机器在十分钟内完成。已知同机干扰要在收据中标注，脚本的机器快照不能证明环境空载；落在本轮 A/A 波动内的小变化标“未裁决”，不据此强制接受或撤销候选。

仅在准备发布原版 V8 Score 或声称达到总分目标时，另行规划 [`run.py`](../../scripts/benchmark/README.md#external-v8-v7-suite) 的完整 isolated 与 combined 配对复核及资源上限。该工具默认 isolated 覆盖全部八项；失败、超时、漏分、被 error callback 吞掉的失败均使该组无效。不能删去慢项、以短轮固定工作量比冒充 Score，或让两个完整套件并发运行。最终复核的重复次数与超时须在采样前固定并随原始结果保存，不要求每个研发切片重复小时级长跑。

## 4. 成本与覆盖报告

普通 release 的平台硬件计数器／反汇编负责机器成本；profiling 构建负责逻辑事件，二者不能混成正式得分。报告实际符号归组规则、self／inclusive 口径、内联和采样 skid 的限制。符号 self% 只计算落在该符号中的样本，可能遗漏内联、调用前准备及跨边界代价，不得作为语义机制的总成本或潜在收益上界。父子调用时间不可重复加总，auth cache hit 不可代替 callsite 单态率。

至少有以下字段：

| 层次 | 字段 |
| --- | --- |
| 构建 | 完整源码／树／patch／二进制／编译器／flags／profile／外部语料 hash |
| 固定工作量 | 完整输出校验、操作数、wall、样本／离散度；可用时另记 instructions/op、cycles/op 与分支事件，缺失不得填 0 |
| 候选覆盖 | canonical PC、attempts、命中、覆盖逻辑指令、每类 miss、非候选成本、cold/warm/megamorphic 分层 |
| codegen | `run` 与所有受影响 helper 的调用清单、返回方式、栈帧、spill/reload、符号尺寸、规范化反汇编 |
| 生命周期 | retain/release、值搬运、分配次数／字节、峰值 RSS、候选元数据、首次执行与编译时间 |
| 正式成绩 | 八项 Score、isolated 几何平均、combined 原始 Score、B37/R0/Parent 三个分母 |

新增片没有触发的负载也必须检查。若 `run` 未变但 outlined helper 翻转内联，按 #44 仍视为真实生成代码变化。若退休指令明显改变，不应仅用“地址布局不同”解释；只有工作量接近时，才进一步检查前端供给、分支预测、缓存与频率因素。

## 5. 准入、累计债务与 kill criteria

本节记录当次系列预先拟定的准入阈值，不把历史波动伪装成已取得的统计保证。新系列可以在测量前公开调整阈值与矩阵，记录理由和身份；不能看过候选结果后回改门槛来放行回退。阈值只裁决候选是否默认开启或继续投入，不冻结内部 API 或规定必须完成某种实现。

**正确性硬门：** 每项定向差分与完整一致性门禁都要通过；只允许记录并核对基线本来存在的失败，不允许由性能 PR 修改允许向量来放行新增失败。

**性能硬门：** 每片及阶段累计均比较相同固定矩阵。BigInt32/64/256、已特化数值／属性循环、数组读写、call0、字符串簇、类型／TDZ异常负载，已测得的确定性退休指令增长 >2% 必须阻断默认开启并归因／回退；不能称“每片 <2%”就忽略累计 >2%。没有计数器则该项未测，不能写成通过。时间类在 A/A 可分辨后出现 >2% 稳定退化也阻断；A/A 与候选差值同量级时保持未裁决，不能按中位数方向伪装通过。

正式原版 V8 准入同时检查各子项和两种总分；任一子项稳定下降 >2% 必须处理，不用总分掩盖。短轮固定迭代的几何平均与 combined 时间比只用于定位和筛选，不是这两种 Score。P2 的 −30% 微指令／+3% 对应真实子分数是继续扩大投入的历史门槛，不是接受其他项回退的交换条件。

RSS 的复核线为 `max(3%, 1 MiB)`，编译／首次执行复核线为 3%；超过后必须证明来源并回收，未裁决不默认开启。零候选函数不应承担按 PC 的新增执行数组。#41 的额外错误分配需单独披露成本，即使未触及 RSS 线也不能省略。

相对 H0 的旧 BigInt 债务独立保存同协议重测表。#41 下降不能跨系列减去 #37 增长；新工作阶段“不新增回退”也不代表旧债务清偿。总分 3–4 倍的最终声明要求 B37 分母、八项完整、isolated 与 combined 复核；仍未清偿的旧债务必须随结果披露，不能移动分母隐藏。

两轮有明确归因的候选调整仍达不到自身门槛：撤销该候选及仅为它新增的状态／入口，保留测试、配方和负结果报告。新形态、错误载体、数组写、调用缓存分别提交，便于逐项撤回。不合入长期默认关闭、但仍增加维护和布局成本的失败框架。

## 5.1 当次 B0 发布后 canonical 捕获协议

执行 [run_dump.py](probes/run_dump.py)，参数与完整路径见 [跨度规格](numeric-array-spans.md) §2。它只在新建 detached worktree 添加 ignored test 模块，不修改原工作区或产品 API；外部 checkout 必须是本协议的完整 pin，两份源文件 blob 必须未改。输出目录必须新建且在两个仓库之外。

回执须包含解析后的完整 engine SHA、benchmark SHA、Rust/Cargo 版本、probe/runner/source SHA-256、诊断 patch、精确命令、退出码及原始 cargo.log。每个函数的发布后全部 OP 行保留 PC、stack contract 与控制流目标；am3/project/lin_solve/advect 必须各出现一次。零匹配测试、缺文件、编译失败或目标不全一律失败，不将 “0 tests passed” 当作完成。

捕获后用当次生产 `FusionPlan` matcher 输出真实 site manifest，对照 flags 1–13，包含所有 slot/constant 编号和每站点拒绝原因。固定模板定义了该候选的匹配规则，不是对未知真实 PC 的预先断言，也不限制后续方案重新选取形态。动态覆盖和正式 Score 仍在独立运行中测量，不能使用这个 compile-only 诊断产物报性能。

后续实施已用 Rust 1.94.1 完成四函数 capture；[完整 manifest 与回执](receipts/all-dense-6db6bfb0/README.md)记录 25 个已发布站点。本段以上保留探针本身的执行契约。

## 6. 一致性与文档 PR 的验证边界

引擎实施 PR 的基本命令如下；clippy 的全部 feature／package 变体以当前 CI 工作流为准，不用单条命令冒充覆盖全部矩阵。

```sh
cargo +1.88.0 fmt --all -- --check
cargo +1.88.0 check --locked --workspace --all-targets
cargo +1.88.0 clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
python3 scripts/checks/check-source-layout.py
./scripts/test262/test-test262.sh --check
./scripts/test262/test-test262.sh --focused
TEST262_WORKERS=2 ./scripts/test262/test-test262.sh --full
```

PR #48 后，focused／full 对比身份行之后的完整结果正文，不能手工修改 admission、诊断契约或 frozen outcome 来接受性能退化。#41 的语义无变化候选不需要仅因 source SHA 改变就晋升结果基线。

文档本身仍须核对数据来源、算术、链接和收据身份。当前引擎实施必须分别报告已执行的 Cargo／Test262／性能命令；#41 独立候选的通过记录不能代替组合版的 CI 或正式成绩。

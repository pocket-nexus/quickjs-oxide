# 测量与性能取舍协议

本协议以 PR #52 为新系列起点，设计与取舍依照[优化原则](principles.md)。
测量用于检验工作是否消失、成本如何变化，不把旧候选的固定门槛移植成新架构
的自动淘汰规则。与 [benchmark 工具说明](../../scripts/benchmark/README.md)、
[Test262 契约](../test262.md)配套。

<a id="baselines"></a>
## 1. 基线与比较身份

| 名称 | 源码身份 | 用途 |
| --- | --- | --- |
| #52 | `996663f771afdabdc69d52c94bd4d2fb392e27b1` | 单流执行架构新起点，衡量累计变化 |
| Parent | 本候选实际父提交的完整 SHA | 逐片增量 |
| Candidate | 本次受测干净提交的完整 SHA | 候选产物；另记 tree、工具与二进制身份 |

报告 Candidate/#52 和 Candidate/Parent；两者相同时说明即可。所有分母
使用该系列同一工具链、target、features 和有效 flags 重建，不混用旧二进制。
换机器或构建配置则重建双方并另起 series id。性能工具链与 MSRV 验证分别
记录，当前 MSRV 为 Rust 1.88；下面命令中的 1.96.0 是可替换的系列示例。

B37 (`3341ac456ea2719858fd6173e8dcd9123ad9e660`)、R0
(`f531f6052cb497ce4707f01c276e8642e5e26788`)、H0
(`3b759647d32ad9b16317d4760deb646eeedefec3`) 和 R1 是
[历史系列](README.md#历史收据索引)的对照身份。旧 3–4 倍 B37 目标不再是
本路线的启动/验收要求；若另行报告该历史目标，仍必须重建其原分母。
#52 重写收据的临时受测快照身份保持原样，本次文档更新不产生新测量。

## 2. 构建、机器与上游 pin

使用普通 release、fat LTO、CGU=1，无 PGO、无 profiling。同一系列的双方在同一机器、相同配置下串行构建和测量；绑核不能代替整机隔离。记录 CPU／系统版本、可用硬件计数器、电源／频率策略、亲和性、内存状态、负载和是否存在其他会话；不支持的字段标未知。`run.py` 自动收集所在平台可取得的快照（macOS 包括 CPU 型号、物理内存、`pmset`、`vm_stat` 和 load average），微码、共享缓存干扰等仍需按主机能力补记。干扰或 A/A 波动足以覆盖候选时间差、交错结果无法区分时，cycles／wall 标“未裁决”，不作通过结论。已知同机干扰必须标注；它不自动抹掉在交错复测中持续分离的回退信号，也不能提供缓存或分支预测的因果解释。

固定工作量与 V8 示例使用的外部 pin 为 `ahaoboy/js-engine-benchmark@2034d98fc8c5f8044e186267593f5d5ea5232caf`（#43 所用 checkout 的完整身份）；`run.py` 默认核对完整 SHA 和 tracked source 洁净度。未来系列可明确指定另一完整 pin（`--v8-source-commit`），并重建全部 bundle／分母，不把不同语料混算。每个生成 bundle 单独记录 SHA-256；pin 相同不代表旧的 `dist/` 一定由它生成，必须重新生成并保存生成器身份。QuickJS 对照仍用项目 pinned 2026-06-04 oracle，记录 C 编译器与 flags；跨引擎是诊断参考，不代替同源码 A/B。

在每个干净 worktree 独立构建。可从当前工具 checkout 构建历史源码 worktree，源码与工具身份分开记录：

```sh
# SOURCE_TREE 是干净的受测源码根目录；OUT 必须是本次新目录且与其他版本分开。
OUT=$(mktemp -d /tmp/oxide-perf.XXXXXX)
RUSTUP_TOOLCHAIN=1.96.0 python3 scripts/benchmark/build.py \
  --repo "$SOURCE_TREE" --plain-only --jobs 2 --plain-target "$OUT/plain"
# 正式计时只用 $OUT/plain/release/qjs；qjs.build.json 随二进制保留。
```

`build.py` 回执记录源码 commit/tree、工具身份与脚本快照、二进制 hash、编译器、release profile 声明、环境覆盖、Cargo 配置文件 hash、完整构建命令及 stdout/stderr 日志；构建期间源码或工具脚本变化会拒绝回执。Cargo `--verbose` 实际执行的 qjs 与依赖 rustc 参数另作摘要。缓存命中的 crate 没有新 rustc 行，不能把声明值冒充实测有效参数；正式重建应使用新 target 目录并保存完整日志。`run.py` 在每次采样前复核工作负载与二进制 hash，并保存样本、顺序与机器快照；主机干扰状态和不支持的硬件指标仍需人工注明。不同 worktree 的 OUT 必须不同。

本项目源码和计划目录内不 vendor 外部 benchmark。以下四个维护者重建探针保留为固定工作量配方；[历史门禁总结](receipts/gates-2026-09-25/README.md)记录结果，原始负载与样本留在本机，不能混同外部 V8 正式 Score。

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

固定诊断使用完全一致的工作量。先测同二进制 A/A，再用交错 ABBA／BAAB 顺序测 A/B；`run.py` 和 `fixed.py` 的 `--order abba-baab` 可复现完整交替块（`--repeat` 为 4 的倍数），默认顺序保留旧行为。保留原始样本和顺序，不只存中位数。能取得退休指令计数时，确定性差异也要调查，不把其变化与 wall 调度噪声混为一谈。重复次数根据待回答问题与 A/A 分辨率预先确定，并记入收据。短轮用于迭代；无法分辨的时间变化保持未裁决，不强制套用旧实验的轮数或样本数。

```sh
# Linux perf 的单次示例；CPU 必须选主机允许的核，其他平台按可用工具记录等价事件。
PERF_CPU=2
perf stat -x, -o "$OUT/array.perf.csv" \
  -e instructions:u,cycles:u,branches:u,branch-misses:u -- \
  taskset -c "$PERF_CPU" "$OUT/plain/release/qjs" "$OUT/micro/array_read.js" \
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

完整取舍报告应覆盖以下字段；探索阶段按本轮问题采集所需子集，未测项标“未测”，不把完整报告当作每次实现迭代的启动条件：

| 层次 | 字段 |
| --- | --- |
| 构建 | 完整源码／树／patch／二进制／编译器／flags／profile／外部语料 hash |
| 固定工作量 | 完整输出校验、操作数、wall、样本／离散度；可用时另记 instructions/op、cycles/op 与分支事件，缺失不得填 0 |
| 候选覆盖 | 函数身份与执行字 PC、attempts、命中、已覆盖的 miss 分类和静态不适用成本；后续适应机制另分 cold/warm/polymorphic/changing-type；未有 emitter 的项标未测 |
| codegen | `execute_frame` 与所有受影响 helper 的调用清单、返回方式、栈帧、spill/reload、符号尺寸、规范化反汇编 |
| 生命周期 | retain/release、值搬运、分配次数／字节、峰值 RSS、候选元数据、首次执行与编译时间 |
| 正式成绩 | 八项 Score、isolated 几何平均、combined 原始 Score、#52/Parent 对照 |

新增片没有触发的负载也必须检查。即使主循环未变，outlined helper 的内联变化仍是实际生成代码变化。若退休指令明显改变，不应仅用“地址布局不同”解释；只有工作量接近时，才进一步检查前端供给、分支预测、缓存与频率因素。

## 5. 决策、取舍与累计债务

**正确性硬边界：** 定向差分和相关一致性验证必须通过；只允许记录并核对基线本来存在的失败，不通过修改允许向量放行新增语义错误。性能设计可以跨越现有内部接口与文件边界，语义证明也须覆盖新的边界。

**探索：** 每片记录目标路径的改善、其他路径的回退、固定工作量、代码体积与可用的时间证据。可分辨的回退需要调查原因，不能因单项超过固定百分比就自动撤销方向。即使当前候选只有局部收益，也可继续完成预计能改变整体成本结构的架构部分。阶段性回退记入债务表，不冒充已经取得的净收益；A/A 无法分辨的时间变化记为未裁决。

**集成：** 在相同矩阵上看逐片与累计差异，并给重要负载、资源成本和启动／编译代价分别列账。某项改善与另一项小幅退化可以形成合理取舍；应检查真实覆盖、净得分、使用场景、回退机制及进一步修正空间。不能把微负载指令下降直接当作真实时间收益，也不能用总体收益隐去某个子项的退化。若一项设计保留了旧路径又增加准入成本，应考虑替换执行结构或删除旧限制，而非只缩小优化范围。

**默认启用与目标声明：** 先固定拟回答的问题、基线和测量矩阵，再做完整复核。报告八项原版 V8 Score、isolated 几何平均、combined Score、固定工作量、RSS 与受影响的重要路径；说明每项稳定回退的幅度、原因、受益交换及尚待处理的债务。是否接受性能取舍由整体目标和影响决定，没有跨所有架构方案通用的单项 2% 自动否决线。P2 的 −30% 指令／+3% 子项收益、RSS `max(3%, 1 MiB)`、编译／首次执行 3% 复核线和“两轮后撤销”均归档为旧系列的实验条件；不能用它们阻断新方向。

历史债务和负结果留在原系列；不能将跨系列数字相减，或用文档清理宣称它们
已消失。新路线的累计比较从 #52 开始。短轮固定迭代结果不冒充原版 Score。
旧 `FusionPlan` capture 工具只适用于其原源码，方法与 25 站点结果见
[历史 manifest](receipts/all-dense-6db6bfb0/README.md)；当前候选需检查实际
`ExecCode`，不能沿用旧 flags/PC 证明覆盖。

### 分开四项成本

| 维度 | 测量范围 |
| --- | --- |
| 编译 | parse、analysis/selection、encoding/verification、linking 与临时分配；未实现的阶段不填为已测 |
| 执行 | 编译后固定 guest 工作量、dispatch、operand traffic、owner 操作、heap borrow、driver 边界 |
| 适应 | 机制实现后分别测冷启动、升温、稳定、多态和类型变化；#52 的 test-only quickening 不是生产适应基线 |
| 内存与延迟 | code、边界/元数据、frame capacity、live heap、峰值 writable memory 与最长连续工作区间 |

现有 `fixed.py`/`iterate_v8.py` 的进程计数包含启动、编译和退出。隔离后的
execution-only、完整 retain/release、heap borrow 总数及最大无中断区间需要
相应测量入口；缺少这些数据时明确未测，不能从部分 counters 推算。

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

PR #48 后，focused／full 对比身份行之后的完整结果正文，不能手工修改 admission、诊断契约或 frozen outcome 来接受性能退化。语义无变化的候选不需要仅因 source SHA 改变就晋升结果基线。

纯文档修改核对源码声明、算术、链接/anchor 和收据身份，运行 `git diff --check`
及 `node scripts/test262/current-test262-metrics.mjs --check-docs`。无需为纯 prose
重跑引擎 benchmark；不得更新冻结语义数据或将旧验证改称当前验证。
引擎实施另行报告实际执行的 Cargo、Test262 和性能命令及完整源码身份。

# 完整 lexer/parser 性能

这是 PR #40 之后、基于 main 的下一轮优化。目标是固定语料上完整 parser
耗时相对 Boa 0.22.0 达到 3× 加速，并单独报告相对 main 的收益。

## 测量边界

`oxide-parse-probe` 调用生产 `Parser::parse`，完整扫描所有函数体并构造拥有
源码、名称、常量、作用域和操作数的 `FunctionTree`。parser/interner 初始化计入
时间，读取文件及结果销毁不计入。不执行 JavaScript，不跳过函数体，不把解析工作
移入 timer 之后；scope resolution、lowering 和 publication 不属于这一档。
Oxide 在解析时生成的 IR 和正则验证仍然计入。

`boa-parse-probe` 使用 `boa_parser::Parser::parse_eval(false)`：在固定的 Boa
0.22.0 中，它与 Script 入口调用同一个 `ScriptParser::new(false)`，但不调用
独立的 `analyze_scope`。保留 Annex B，计入 parser/interner 初始化并完整生成 AST。
这与旧 `boa_engine::Script::parse` 的 parse+scope 档不同。
runner 拒绝混用这两类探针；不能把本页与旧 Linux compile 矩阵直接相除。

正式构建均为同一 Rust toolchain 的 release、fat LTO、一个 codegen unit，
关闭 profiling。每次样本是新进程中的一次真实解析。交替引擎顺序，使用各 case
的中位耗时，再对固定全部 case 的速度比取几何平均。失败、超时和样本不足都使
对应完整几何平均失格。运行时分数与合并的 `all` 文件不参与主指标。

主语料固定为 `ahaoboy/js-engine-benchmark` 的
`2034d98fc8c5f8044e186267593f5d5ea5232caf`，按已有 `prepare_v8` 的规则生成
Richards、DeltaBlue、Crypto、RayTrace、Earley-Boyer、RegExp、Splay、Navier-Stokes
八个 Script bundle。文件、上游 revision、生成脚本及探针构建均保存哈希。
生成语法压力集和 `all` 只作为补充诊断。

## 实现与来源

- **词法分派与扫描**：ASCII 分类表直接选择 token scanner，过滤不可能的关键字，
  批量处理字符串中的 ASCII 段。保留 Unicode、原始字节 carrier、CRLF、HTML
  注释和字符串溢出的精确错误位置。设计参考 [V8 scanner](https://v8.dev/blog/scanner)。
- **token 生命周期**：parser 只持有当前 token 和上一个 token 的结束偏移；词法上下文与未来
  扫描上下文分别记录，相同上下文不重扫。lexer 直接写入当前 token，减少跨层返回值拷贝。
  模板的冷错误详情不扩大普通 token；
  仅未标记模板报错时按保存的位置恢复完整诊断。本机 `Token` 为 80 字节。参考 QuickJS 的单 token 与
  可恢复扫描位置，不采用跳过函数体的 lazy parsing。
- **名称与数值**：名称表共享一次文本分配，属性名复用不可变 `JsString`；小整数
  在 u64 内精确累积，溢出后保留原有 BigUint 转换与舍入路径。常量顺序不变。
  256 槽名称提示缓存核对完整文本，冲突回到保留随机 SipHash 的权威索引。
  名称驻留参考 V8 的 `AstValueFactory` 和 QuickJS atom ID。
- **表达式与探测**：用运算符优先级 continuation 消除多层空递归，保留指数、
  `in`、私有字段、逻辑与 nullish 的特殊规则。流式处理参数预扫描，以顺序游标
  读取 token 缓存；连续扫描直接追加，已提交部分按游标消费，命中仍校验词法 goal
  与上下文。完整的嵌套括号探测可复用摘要。普通表达式先排除非解构起始符，
  避免维护无用摘要。所有缓存有容量上限，错误或
  深度受限的探测不能成为成功摘要。优先级 continuation 参考 V8 `ParserBase`；
  255 层探测边界仍遵循固定 QuickJS。
- **IR 存储**：函数记录保持固定地址，函数结束后释放解析临时状态；小作用域
  线性查找，大作用域才分配索引；宽 span 和动态名称操作数存放在函数自己的表中，
  普通 `SpannedIrOp` 在本机为 32 字节。解析、解析后解析器状态销毁以及完整
  FunctionTree 的构造仍属于测量窗口。
- **长行**：转义标识符解码直接使用已验证 token 的绝对字节偏移，避免从文件
  开头重新计算无用的行列。参考
  [QuickJS-NG 的列位置缓存修复](https://github.com/quickjs-ng/quickjs/commit/0e194369a8650487c981e6128079ebac249a0769)。

参考源码固定为 V8 `7b50b62cb18f28617959e8452e2cd18195b38bcf`、
QuickJS 2026-06-04 和 QuickJS-NG `19dbe8524c1a7357d268cb586ee315616984fef8`。
这些是设计来源，主指标只比较 Oxide 与 Boa 的上述完整解析边界。

## 复现

```sh
git clone https://github.com/ahaoboy/js-engine-benchmark.git /tmp/js-engine-benchmark
git -C /tmp/js-engine-benchmark checkout 2034d98fc8c5f8044e186267593f5d5ea5232caf
python3 scripts/benchmark/prepare_frontend_corpus.py \
  --source /tmp/js-engine-benchmark --output target/parser-primary
python3 scripts/benchmark/build_frontend_probes.py \
  --engine boa --output target/parser-boa
python3 scripts/benchmark/build_frontend_probes.py \
  --engine oxide --repo "$PWD" --output target/parser-candidate
git worktree add --detach ../qjo-parser-main e2f79560
python3 scripts/benchmark/build_frontend_probes.py \
  --engine oxide --repo ../qjo-parser-main --output target/parser-main
python3 scripts/benchmark/compile_matrix.py \
  --corpus target/parser-primary --metric parse \
  --engine boa="$PWD/target/parser-boa/target/release/boa-parse-probe" \
  --engine main="$PWD/target/parser-main/target/release/oxide-parse-probe" \
  --engine candidate="$PWD/target/parser-candidate/target/release/oxide-parse-probe" \
  --repeat 15 --output target/parser-results
```

每个输出目录必须尚不存在。构建和测量时固定 checkout；正式计时前等待构建、
测试和采样退出。Boa 的完整依赖图固定在 `boa_parse_probe.lock`。main 的算法基线
使用 observation-only commit `e2f79560`，它只添加同一探针边界，没有解析算法优化。
确认轮使用同一组二进制，将 `--repeat` 改为 31，并指定新的输出目录。

补充压力集用 `python3 scripts/benchmark/compile_workloads.py --output target/parser-diagnostics`
生成默认的三个大小；合并语料用 `prepare_frontend_corpus.py --aggregate` 生成。
它们分别运行矩阵，重复 5 次，保持相同的引擎顺序与测量边界。

验证使用工作区测试、CI 固定 Rust 1.88 的 Clippy、QuickJS fixture 验证、
Test262 focused 和 full 的逐项冻结结果比较。不更新 Test262 基准来接纳回归。

## 2026-09-26 测量结果

Apple M1 / 16 GiB，macOS 26.6.2，Rust 1.96.0。引擎代码固定在
`5765fa00df7e6f25d0345305987bab19de27b5e4`；main 为 `2ed79f46`。
下表中的倍数均为参考耗时除以本分支耗时。

| 主语料批次 | 每引擎每项次数 | 相对 Boa | 相对 main |
| --- | ---: | ---: | ---: |
| 主矩阵 | 15 | 3.006× | 1.893× |
| 同一二进制确认轮 | 31 | 3.047× | 1.885× |

固定八项的两轮几何平均均超过 3×；确认轮各项相对 main 加速 1.659–2.929×。
两轮共 1,104 个独立进程样本全部解析成功。确认轮的逐项中位数如下：

| 语料 | Boa（ms） | main（ms） | 本分支（ms） | 相对 Boa | 相对 main |
| --- | ---: | ---: | ---: | ---: | ---: |
| richards | 1.595 | 0.933 | 0.553 | 2.885× | 1.688× |
| deltablue | 2.381 | 1.387 | 0.802 | 2.967× | 1.728× |
| crypto | 6.123 | 3.258 | 1.801 | 3.400× | 1.809× |
| raytrace | 2.889 | 2.143 | 1.099 | 2.628× | 1.949× |
| earley-boyer | 21.624 | 17.440 | 5.955 | 3.632× | 2.929× |
| regexp | 8.135 | 4.572 | 2.437 | 3.338× | 1.876× |
| splay | 1.256 | 0.769 | 0.464 | 2.707× | 1.659× |
| navier-stokes | 1.882 | 1.080 | 0.636 | 2.961× | 1.699× |

补充诊断每项每引擎 5 次，150 个样本全部成功；这些结果独立报告，不计入主指标：

| 补充集合 | 相对 Boa | 相对 main |
| --- | ---: | ---: |
| 9 项语法压力集（64 KiB / 512 KiB / 4 MiB） | 2.678× | 1.863× |
| 合并 all Script | 3.372× | 2.302× |

压力集各项相对 Boa 为 2.237–3.329×，相对 main 为 1.718–2.060×；
3× 验收结论适用于上述固定八项的几何平均。

正式测量时本任务的构建、测试和采样已退出；本机仍有其他 qjs/Node 基准和
桌面进程运行。两个独立批次均完整保留，未选择最优批次。
[测量回执](lexer-parser-results.json) 包含全部原始纳秒样本、逐项统计、源码及
二进制哈希、依赖锁哈希、语料版本和机器负载记录，可直接重算所有比值。

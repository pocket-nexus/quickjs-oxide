# Verify/Publication 简化计划（含 BC5 读取路径移除）

本文件承接 [issue #32](https://github.com/pocket-nexus/quickjs-oxide/issues/32)
的另一半（前端计划 `docs/lexer-parser-refactor.md` 明确“不含 verify/publish/VM
的改动”），并取代该计划 §3 P4 第 2 条的 P4-2 验收口径。基线是
`feat/parser` 的 P3 树（`172fde6e`），指标口径为
`functions-4194304` 与 `docs/compile-benchmark.md` §9.11 的 P3 实测。

## 0. 决策与目标

2026-09-24 会话确认的三项决策：

1. **删除 verify**。`code/verify/*` 是纯认证、无输出的额外阶段；上游 QuickJS
   2026-06-04 release 没有任何 verify pass（`compute_stack_size` 是数据流
   **计算**，对应我们的 `code::bytecode::verify_parts`，不是认证）。verify
   不是 VM/编译器该做的事，整块删除。
2. **删除 BC5 读取路径**。上游字节码是版本绑定的缓存而非可移植接口；上游
   自己也不在读取时验证。本项目不再承诺与上游 BC5 双向互读：公开读取 API、
   `binary_object` 解码/编码设施、门禁脚本与 C oracle fixtures 一并删除，
   `docs/parity.md` §11 契约改写。BJSON 对象序列化尚未实现，不属于本次
   保留范围；未来若需要，另立计划。
3. **publication 保留边界、删除中间层**。编译期不持有 runtime 身份
   （`docs/architecture.md` 边界），所以 atom interning、template/regexp
   实例化、堆节点分配 + 根保留 + 事务回滚必须保留；删除的是
   `flatten_unlinked_tree`/`FlatFunction` 中间层、每函数字段重收集与重复转换、
   堆边界重复的 `verify_parts`。

目标（P3 树实测，`functions-4194304`）：

| 项 | 时间 | alloc 次数 | alloc 字节 | 依据 |
| --- | ---: | ---: | ---: | --- |
| verify 阶段删除 | −15.0% | −20.7% | −13.9% | §9.11.1 阶段分布 |
| BC5 路径删除 | 无运行时指标，纯规模/CI 缩减 | — | — | 本文件 §1.2 |
| publication 单遍化 | publish 阶段 ≥ −25% | ≥ −25% | ≥ −25% | §9.11.3/9.11.4 站点 |

合并验收：verify+publish 两阶段时间合计相对 P3 ≥ −40%、alloc 次数 ≥ −40%、
realloc 字节 ≥ −5%；总编译时间与分配次数相对 P3 的降幅以实测记录为准
（§4）。

非目标：

- 不删除 publication 边界本身，不改 VM/执行器/`engine::api` 其余出口。
- 不改自有字节码/指令格式、atom 生成顺序、JS 语义与诊断文案。
- 不保留 BC5 读取/编码的任何公开或内部设施；不把解码器降级为 test-only。
- 不引入增量/延迟验证、fuzz 或新的验证替代物（本计划就是删除验证阶段）。
- 不做 publication 的进一步激进重构（如让 lowering 直接产出 runtime-ready
  结构）：atom 只能在 runtime 边界 interning，收益不成立。

## 1. 现状（事实，P3 树）

### 1.1 verify

- `src/engine/code/verify/`：14 个文件、约 7.5k 生产 LOC + 88 个测试
  （含 `tests/` 5 个文件，测试 LOC 约 6.4k）。纯认证、无输出；
  `VerifiedFunction`（`verified.rs:14`）只是所有权包装，publish 只取回原函数。
- 唯一被 publish 复用的东西是 `verify::private_elements` 的
  `private_binding_info` / `private_setter_local_pairs` / `PrivateBindingRole`
  （`bytecode_publish/private_elements.rs:7` 引用），删除前要搬进
  `bytecode_publish`。
- **必须保留**：`code::bytecode::verify_parts` / `verify_lowered_max_stack` 是
  lowering 计算 `max_stack` 的数据流计算（`compiler/flow.rs:8,15`），不是认证。
- **可删的重复调用**：`heap/allocation.rs:1291` 在堆边界又跑了一次
  `verify_parts`；随 verify 一起删。
- 依赖 verify 拒绝语义的测试：
  - `verify/` 内 88 个测试（随模块删除）；
  - `heap/runtime/tests/publication.rs` 的 `publication_rejects_*`（13 个
    malformed-draft 用例）——删除前逐条判定是否仍被堆边界检查拒绝；
  - `binary_publication.rs` 的 `verifier_rejected`（随 BC5 删除）。
- profiling：`CompilePhase::Verify`（`api/profiling/cost/phases.rs:15`）与其
  `costs.verify` 字段随阶段删除；`docs/compile-benchmark.md` 的阶段表同步。

### 1.2 BC5 读取路径

- 公开入口：`Context::read_trusted_scalar_script` /
  `Context::read_trusted_ordinary_function`（`api/context/bytecode.rs:14,33`）。
- 桥接：`code/binary_object_publish.rs`（765 LOC）把解码 DTO 翻译成
  `UnlinkedFunction` 后走 verify + publication。
- 解码/编码设施：`code/binary_object/` 12 个子模块、30,651 LOC：
  `wire`/`read_cursor`/`atoms`/`code`/`function_envelope`/`function_translate`/
  `bytecode_image`（decode + encode）/`graph`（whole-image model + writer）/
  `pinned_atoms`/`pinned_opcodes`/`scalar_script`/`ordinary_leaf`。
  消费方只有 `binary_object_publish` 与自身测试；`graph`/`bytecode_image`
  的 encoder 等是 staged dead code（`binary_object/mod.rs` 上成片的
  `#[allow(dead_code)]`）。
- 测试：`heap/runtime/tests/binary_*.rs` 10 个文件、72 个测试、833 LOC
  （`tests.rs:780-800` 的 `mod` 声明与 BC5 常量）。
- C oracle fixtures（只被 `scripts/quickjs/test-quickjs-c-oracles.sh` 编译
  上游 QuickJS 验证，无 Rust 消费方）：
  - `dev-support/quickjs-c-oracles.tsv` 的 7 个 `function-bytecode-*` +
    `module-bytecode-wire` + `shared-array-buffer-transport`；
  - `apps/cli/tests/fixtures/inputs/function_bytecode_*.c`（7）、
    `module_bytecode_wire.c`、`shared_array_buffer_transport.c` 及
    `fixtures/expected/*.quickjs-2026-06-04.txt` 对应文件；
  - 脚本内的 family 分支、schema 分支与 `required_id` 列表。
- 门禁脚本：`scripts/checks/check-bc5-pinned-atoms.mjs`（665 LOC）、
  `check-bc5-pinned-opcodes.mjs`（955 LOC）、
  `scripts/checks/lib/bc5-gate-primitives.mjs`（579 LOC），以及
  `.github/workflows/ci.yml:72-73,164-166` 的四处调用。
- 契约：`docs/parity.md` §11（双向互读门禁、malformed 不 panic 条款）；
  `docs/status.md` 的 BC5/bytecode 章节；`README.md:57` 的 trusted 读取说明；
  `scripts/README.md` 的 BC5 gate 说明。

### 1.3 publication

- 阶段占比（P3，functions-4MB）：时间 23.6%、alloc 次数 25.3%、alloc 字节
  28.7%。
- 现行结构（`code/runtime.rs:40-276`）：
  1. `flatten_unlinked_tree`（`bytecode_publish.rs:93`）把树展平成后序
     `Vec<FlatFunction>`，同时把 `UnlinkedConstant` 分类成 `FlatConstant`；
  2. 对每个 `FlatFunction`：链常量（template/regexp 实例化、atom string
     interning、属性键、eval 环境、闭包名、argument/local 定义）、
     `prepare_private_binding_publication`、`allocate_function_bytecode`；
  3. 用全局 `roots: Vec<Option<FunctionBytecodeRef>>` 与 `children: Vec<usize>`
     维护子节点根保留。
- 站点证据（§9.11.3/9.11.4）：`flatten_unlinked_tree` 43.0MB 大块分配
  （`bytecode_publish.rs:159`）、publish `Vec→Box` 18.7 万次
  （`runtime.rs:234/242/244/248`）、`FlattenFrame::new` 3.7 万次
  （`runtime.rs:457`）、`setter_storage_base` 27.4 万次、`Heap::
  allocate_function_bytecode` 7.1 万次。
- **必须保留**（`architecture.md` 边界）：属性键/eval 绑定名的 atom interning、
  template/regexp 实例化、堆节点分配 + 根保留 + 事务回滚（失败释放
  `auxiliary_atoms`、子根在父节点接管前保持存活）。

## 2. 设计

### 2.1 verify 删除

- 删除 `src/engine/code/verify/` 整目录与 `code/mod.rs:15` 的 `mod verify;`。
- 删除 `VerifiedFunction`；`publish_unlinked_function` 直接吃
  `UnlinkedFunction`，`publish_verified_unlinked_function` 合并进它。
- helper 搬家：把 `private_binding_info` / `private_setter_local_pairs` /
  `PrivateBindingRole` 的实现移入 `bytecode_publish/private_elements.rs`
  （该文件已存在，目前 `use` verify；改成自含实现，语义逐字节保持）。
- 调用点改写：
  - `code/runtime.rs:31-49`：单入口，去掉 verify 计时；
  - `builtins/eval.rs:21-31` 与 `vm/driver.rs:4252`：删除
    `EvalPublicationInput`/`EvalPublicationCapabilities` 校验，直接发布；
    `code/function/publication.rs` 中只为 verify 存在的类型一并删除；
  - `modules/mod.rs:1908-1919`：`module.into_parts()` 后直接
    `publish_unlinked_function`，`UnlinkedModuleParts` 不再需要泛型配对；
  - `heap/runtime/tests.rs:701,711,728,732` 等测试调用点适配。
- 堆边界：删 `allocation.rs:1291-1296` 的 `verify_parts` 调用与
  `"function bytecode failed generic verification"` 错误；其余堆结构检查
  （eval binding 归属、私有绑定、debug 等）保留不动。
- 测试处置原则：
  - verify 自有 88 个测试随模块删除；
  - `publication_rejects_*` 逐条判定：仍被堆边界/发布期检查拒绝的保留；
    只有 verify 能拒绝的删除，并在提交信息里列出；
  - 构造“合法但错误”draft 的测试不得改写为断言 panic/错误行为。
- `architecture.md:120` 的边界声明去掉 “Verification authenticates …”，保留
  “Publication links names, retains roots and rolls back failures.”。

### 2.2 BC5 读取路径删除

- 删除：`code/binary_object/`、`code/binary_object_publish.rs`、
  `api/context/bytecode.rs`（含 `api/context/mod.rs:27` 的 `mod bytecode;`）、
  `heap/runtime/tests/binary_*.rs`（10 个）与 `tests.rs` 的 `mod` 声明、
  BC5 常量（`QUICKJS_*_BC5` 等）与辅助函数。
- 删除 C oracle 字节码 fixtures、transcripts、manifest 行与脚本内
  family/schema/`required_id` 分支；保留其余 module fixtures。
- 删除 BC5 门禁脚本与 CI 调用（`ci.yml` 两处步骤内共 4 条命令）。
- `docs/parity.md` §11 改写：不再承诺与上游字节码双向互读；删除
  malformed/不 panic 门禁（该门禁的唯一实现就是 verify 在 BC5 路径上的调用）。
  新契约表述：BC5 与本项目无关；自有指令格式不是公开序列化格式；若未来
  恢复上游互读，必须重新引入独立验证，另立计划。
- `docs/status.md`、`README.md`、`scripts/README.md` 同步删除/改写 BC5 描述。

### 2.3 publication 单遍化

- 用**一次迭代后序 walk** 直接分配堆节点，替换 flatten + 逐函数发布两段式：
  - 显式帧栈（保持 iterative，深嵌套不爆栈；
    `deeply_nested_child_publication_and_release_are_iterative` 是守卫）；
  - 每帧持有该函数的 `UnlinkedFunction` parts、常量迭代器、已链
    `Vec<BytecodeConstant>`、子根 `Vec<FunctionBytecodeRef>`；
  - 遇到子函数常量就压栈；子帧完成即 `allocate_function_bytecode`，把
    `BytecodeConstant::Function(child_id)` 写进父帧常量，并把子根留在父帧
    的 `child_roots` 里；
  - 父节点分配成功后 drop 该帧 `child_roots`（父 cpool 边接管根保留）。
- 同一遍内完成：常量分类（不再有 `FlatConstant`）、template/regexp
  实例化、atom string/属性键/eval 环境/闭包名/argument+local 定义 interning、
  private binding 准备（去掉 `prepare_private_binding_publication` 的独立
  二次扫描，合并进定义 interning 循环）、`auxiliary_atoms` 事务回滚。
- 删除：`flatten_unlinked_tree`、`FlatFunction`、`FlattenFrame`、
  `FlatConstant`、全局 `roots`/`children` 表。
- test262-host 动态 import 策略检查改为在 walk 中逐函数检查
  （`runtime.rs:50-65` 的语义保持：任一函数含 `Instruction::Import` 即拒绝）。
- 目标：去掉 flatten 43MB 大块、`FlattenFrame` 3.7 万次分配、中间层带来的
  `Vec→Box` 转换与每函数字段重收集；必要的 `Vec→Rc<[T]>` 拷贝保留
  （heap 记录类型不动），除非实测证明可去。

## 3. 提交拆分

每个提交可编译、可跑 focused gate；阶段结束跑全量 gate 并更新实测。

1. `docs: add the verify/publication simplification plan`：本文件 +
   `docs/lexer-parser-refactor.md` P4-2 交叉引用（验收并入本计划）。
2. `refactor: drop the BC5 bytecode compatibility reader`：§2.2 全部内容
   （引擎、API、测试、fixtures、门禁、CI、parity/status/README）。
3. `perf(compiler): remove the standalone verification pass`：§2.1 全部内容
   （含 helper 搬家、堆内重复调用、测试处置、profiling 阶段、architecture
   边界声明）。
4. `perf(compiler): publish unlinked drafts in one post-order walk`：§2.3
   全部内容。
5. `docs: record the verify/publication simplification results`：实测数据、
   `docs/compile-benchmark.md` 阶段表、`docs/status.md`、issue #32 进度。

## 4. 验收与回滚

基线：P3 树（`172fde6e`），`functions-4194304`，方法与
`docs/compile-benchmark.md` §9.11 一致（阶段计数探针 + 分配计数探针 +
`perf stat` + 真实 bundle 矩阵）。

- 提交 2（BC5）：无运行时指标要求；`cargo test --locked --workspace
  --all-targets` 全绿、`test-quickjs-fixtures.sh --validate`、
  `test-quickjs-c-oracles.sh --validate`（删除后清单自洽）通过。
- 提交 3（verify）：编译时间相对 P3 ≥ −10%（预期 ≈ verify 阶段占比
  15.0%）；alloc 次数 ≥ −20%、alloc 字节 ≥ −13.9%；test262 报告 body 逐字节
  一致。
- 提交 4（publication）：publish 阶段时间 ≥ −25%、alloc 次数 ≥ −25%；
  verify+publish 合计时间 ≥ −40%、alloc ≥ −40%、realloc 字节 ≥ −5%；
  test262 报告 body 逐字节一致 + fixtures 字节一致。
- 回滚点：提交 2/3/4 各自独立回滚；任一提交实测中性即回滚该提交。
- 代价（已知并接受）：失去“编译器 bug → 编译期 internal error”的开发期
  安全网，编译器 bug 会变成运行期 panic 或错误行为。兜底是 test262 全量、
  oracle 差分、fixtures 与编译矩阵；本计划不引入 fuzz。

## 5. 测试与守卫

每个提交：

```sh
cargo test --locked --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings   # 5 组 feature 组合，见 CI
./scripts/quickjs/test-quickjs-fixtures.sh --validate
./scripts/quickjs/test-quickjs-c-oracles.sh --validate
./scripts/quickjs/test-quickjs-dynamic-import-trace.sh --validate
./scripts/checks/check-rust-only.sh
./scripts/checks/check-source-layout.py
```

阶段结束额外：`./scripts/test262/test-test262.sh --spec
dev-support/test262/current.conf --check`、真实 bundle 编译矩阵
（`scripts/benchmark/compile_matrix.py`）与分配计数
（`compile_alloc_probe`）。删除 BC5 门禁后，CI 不再调用
`check-bc5-pinned-*`；test262 host 构建与 oracle 差分保持。

## 6. 风险与后续

- **深嵌套**：单遍 walk 必须保持迭代；`deeply_nested_child_publication_and_
  release_are_iterative` 测试是回归守卫。
- **事务回滚**：单遍化后，父帧失败必须释放该帧已 intern 的
  `auxiliary_atoms` 与已分配的子节点；子节点在父节点接管前必须保根。
  回滚路径要有测试（现有 publication rollback 用例适配保留）。
- **BC5 恢复成本**：读取能力删除后如需恢复，解码器与验证都要重做；已记录
  为非目标。
- **后续候选**：P4-2 原“IR/常量侧分配削减”剩余项（JsString 构造、
  `validate_scope_graph` 缓冲、`lower_ops`/`FunctionIr` 表转换）在本计划
  之后按 `docs/lexer-parser-refactor.md` §3 P4 继续评估。

## 7. 执行结果（2026-09-24）

- 提交 2 `15410d3f`、提交 3 `ca88c763`、提交 4 `cf91f28a` 均已落地；
  提交 5 记录实测。
- 阶段时间（`functions-4194304`，release `qjs -d`，min of 3）：
  publish 281.83 → 223.41（verify 提交）→ 198.19ms（walk 提交，相对 P3
  −29.7%）；verify+publish 454.85 → 198.19ms（−56.4%）；总编译
  1173.31 → 918.79ms（−21.7%）。
- 分配探针（总量，无阶段插桩）：alloc 次数 −24.8%、alloc 字节 −16.9%、
  realloc 字节 −9.3%、窗口内 compile −23.6%；peak live 不变（parser
  缓冲主导）。
- 验收：verify 提交时间 −18.0%（目标 −10%）、alloc 次数 −23.7%
  （目标 −20%）、alloc 字节 −13.6%（目标 −13.9%，边际差）；publish 时间
  −29.7%（目标 −25%）、verify+publish 时间 −56.4%（目标 −40%）、
  verify+publish alloc 次数 −24.8%（目标 −40%，未达：publish 剩余成本是
  atom 驻留与堆节点注册）、realloc 字节 −9.3%（目标 −5%）。
- test262：P3 与 walk 各跑一次 `--full`（12 workers，102,037 variants），
  除首行 engine 哈希外 TSV/JSONL 逐字节一致；里程碑 `full_passes=79982`
  的 +28 差额是 P1a 既有漂移，未 promote。fixtures 13/13。
- §2.3 中 `prepare_private_binding_publication` 的独立扫描已由
  `PrivateBindingScanner` 合并进定义 interning；必要 `Vec→Box`/`Vec→Rc`
  拷贝按计划保留，heap 记录类型未动。
- 未完成项：issue #32 进度未更新（本环境无仓库写权限）；publish 阶段的
  独立分配计数未重建（P3 时代临时插桩补丁不在仓库）。

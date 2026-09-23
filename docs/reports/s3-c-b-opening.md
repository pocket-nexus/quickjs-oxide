# S3-C / B 开头实施记录

日期：2026-09-22。实施起点为 `bcfb4fe54eeea9b19e31f7b7eed2298a86de8aab`，
按更新后的 [C 计划](s3-c-plan.md)、[B 首批计划](s3-b-initial-plan.md) 和
[performance architecture](performance-architecture.md) 推进。

本轮交付 C0 的构建认证与历史输入恢复、C1 直接存储事务候选，以及 B1a
纯 QuickOp 编码/译码/验证模块。C 与 B 分文件并行开发，构建、测试和计时串行。
没有将已有 `d4f78697` gc/numeric 改动计入本轮产出。

## 1. 实际状态

| 工作包 | 本轮结果 | 尚未关闭的条件 |
| --- | --- | --- |
| C0 构建地基 | CLI 支持冻结源码导出、完整文件哈希、构建前后认证、实际产物认证 | 正式 saved/previous/pre-A 同协议重建与 A/A |
| C0 历史输入 | 找回并核验 58 fixed、67 compile、其中 9 original；另保存 3 个 bigint 输入 | 全矩阵重放、噪声、RSS、热点与回退归属 |
| C1 直接存储 | local/arg consume/keep 事务已接入，保持原 release/publication 边界 | C0 前置决策及普通 release 配对性能验收 |
| B0 编码约定 | 固定 8B word、规范 PC、保留位与有限 tag 集合 | C 的性能输入和最终 facade 交接 |
| B1a 纯模块 | 实现 codec、穷尽 translator、validator、独立测试 | 生产发布仍属 B1b |
| C2–C6 / B1b–B1e | 未启动 | 按各计划依赖顺序继续 |

**正式 C0 前置未满足。** 当前尚未找到足以关闭 E/RSS、有界残余分配和 A4
表示裁决的完整 receipts；现有 16B `JsValue` 不等于已完成 A4 数据裁决。
本轮不声称 C1 被性能接受，不将当前 HEAD 自动命名为 E 后 saved，也没有
把旧缺完整凭据或 LTO-off 二进制作为正式分母。

## 2. C0：可认证的构建与输入

`scripts/benchmark/build.py` 新增 `--repo`、`--source-manifest` 和 `--mode`：

- 干净 Git checkout 保留 commit 身份；未提交实现可导出完整源码，通过
  `files` / `source_files` 的 SHA-256 映射认证。新实现文件必须在清单中。
- 导出构建输出必须位于源码目录外；构建前、每次构建后、发布前核对 inventory
  与内容。新增文件、源码变化、manifest 替换都使构建失去准入资格。
- CLI 与 compile probe 复用 `frozen_source.py`。导出内 `target/` 也是源码
  inventory 的一部分，避免遗漏 `include!` 或 `#[path]` 输入。
- 读取 Cargo JSON 的唯一 `qjs` executable，支持 target triple；不能误将
  `<target>/release/qjs` 旧文件认证为新构建产物。构建前失效旧 host/triple receipt。
- 两种模式都构建成功且身份不变后才发布 receipt。记录二进制哈希、命令、
  rustc/cargo、Cargo manifest/lockfile、profile/target 环境覆盖；冻结 manifest
  原字节另存 `qjs.source.json`，嵌入身份为 `frozen:<manifest sha256>`。

回归测试覆盖构建中新增/修改源码、第二个模式失败时不认证第一个模式、manifest
变更、输出越界、祖先 Git 身份、Git revision 变化、target triple 旧产物误认证，
以及源码目录内 `target/` 输入的完整性。使用方法见
[benchmark README](../../scripts/benchmark/README.md#prepare-binaries-with-provenance)。

恢复输入位于本工作区 `target/s3-c-opening/inputs/`；这些是本机证据，不是
仓库内 vendored benchmark。恢复时保留原始 manifest 字节，并逐文件核验
历史 SHA-256、case、size 和 expected。没有重新生成或替换工作负载。

| 文件/目录 | 数量或 SHA-256 |
| --- | --- |
| `data-structure-fixed-final.json` | `90cf62d73d970dd1f9406b5ec1718ab769cb3a4c84cdab043217f6390b86a368` |
| `fixed-workloads/` | 58 文件，使用 manifest 原 basename |
| `workload-receipt.json` | `b3a65020ef063047093cedb8d9a8cfca014c4123d77d1fa77a6e6b7c77ccf656` |
| `replay-workloads/` | 67 文件，含 9 个 original score 契约 |
| `bigint-workloads/` | 3 文件，原 metadata 另存 `bigint-metadata.json` |
| `recovery-receipt.json` | `aa696d7c38ed6617448bb59e93e8695e07bc493f3d4f34686cf941f5108aafb6` |

`recovery-receipt.json` 记录原工作区、原路径、恢复路径、逐文件哈希与字节数。
fixed 原临时路径已经丢失，恢复文件来自 S07 replay 中 case/size/hash/expected
完全匹配的条目；仅重命名到原 manifest basename，不修改内容。

另保存实施前 `git archive` 导出 `target/s3-c-opening/baseline-source/`，
完整 1232 文件清单为 `baseline-source.json`，SHA-256：
`a839358df6a07396abd5a2cb5c86b5d2e531b3d1ae25cac48b0c738d8d7a4ff6`。
它只认证本轮起点源码，尚不是正式 C0 性能基线。

## 3. C1：operand → direct binding

新增 `vm/stack/store.rs`，由 `RunSlots` 暴露 local/parameter store facade。
事务在已认证 frame window 内先检查目标、栈深度、源槽类型及 keep 所需 dup：

1. 非 direct binding 返回 `None`，交由原 TDZ/const/captured/mapped 路径处理。
2. consume 将顶部 binding owner 直接移到目标，清空源槽、减少 depth。
3. keep 先复制一个 owner；复制失败时，源槽、目标槽和 depth 均不变。
4. 返回被替换 owner；不在事务里 release 或 drain deferred 队列。
5. `run.rs` 保留原 readiness / primitive / `release_outside_slots!` 三分支，
   继续由 handler 控制物化、规范 PC publication 与释放。

新增 `direct_store.consume` / `direct_store.keep` profiling 事件；普通构建没有
这些计数写入。`slot_moves` 仍表示逻辑 owning transfer；consume 去掉
pop→临时 owner 的交接后记录两次搬运。该计数不能解释成物理内存写入次数，
也不能替代 release 的 cycles/instructions 或耗时证据。

低层测试覆盖 consume/keep 精确所有权、self-alias、被替换 owner 的延后释放、
失败原子性、非 direct decline、deferred 不 drain，以及 String/BigInt/Symbol
在源槽释放后的存活。JS 测试覆盖赋值结果、别名、getter 一次求值、TDZ、const、
闭包和 mapped arguments；profiling 测试确认新事件实际命中。

## 4. B1a：8B QuickOp 与规范 PC

新增 `code/quick.rs`、`quick/translate.rs` 和独立测试；仅以 `#[cfg(test)]`
在 `code/mod.rs` 接线，没有增加生产 IR 常驻内存或改变执行派发。

- 固定 8B `u64`：tag 8 位、flags 8 位、aux 16 位、operand 32 位；flags/aux
  首批必须为零。保留完整 i32/u32 位模式，无 unsafe 或 owner。
- 7 个 tag：GenericCanonical、Nop、PushI32、Undefined、Null、Bool、Goto。
  198 个 canonical variants 显式分类，其中 191 个走 Generic。
- `CanonicalOnly` 不分配；`Words(Rc<Vec<QuickOp>>)` 保持只读共享，
  `words.len() == code.len()`，包括 Generic 和潜在 fusion 内部位置。
- 主 buffer 使用 `try_reserve_exact`；失败返回错误。小型 `Rc` 控制块的全局
  allocator OOM 按 Rust 原策略处理，没有伪称全部内存分配都可恢复。
- validator 检查长度、tag、全部保留位、参数和逐 PC 对应，并对热项校验四类
  canonical contracts：stack、control、operand、potential effects。

首轮测试发现 Goto 的 canonical exception contract 为 `MayThrow`；已据此
修正热项预期并添加独立回归断言，没有放宽 validator 或更改规范指令。
13 个 `quick_` 测试还覆盖全部 Generic variants、损坏编码、极值、reserve
失败、共享、PC 保留、线性大小和 10 类真实编译树。

## 5. 本轮验证

完整结果与日志保存在 `target/s3-c-opening/`。以下记录仅针对本轮源码；
性能矩阵未执行，不能用功能测试通过替代 C1 性能接受。

| 验证 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --locked --workspace --all-targets` | 3466 passed，1 个原有 ignored，0 failed | `workspace-tests-final.log` |
| `cargo test --locked -p quickjs-oxide --lib --features profiling` | 2565 passed，0 failed | `profiling-lib.log` |
| CLI profiling：普通 / feature 配置 | 2 / 6 passed | `profiling-cli-plain.log`、`profiling-cli-feature.log` |
| `cargo +1.88.0 clippy --locked --workspace --all-targets -- -D warnings` | 通过 | `msrv-clippy.log` |
| fmt / source-layout / rust-only | 全部通过；704 个 reachable Rust 文件 | 对应同名 `.log` |
| benchmark Python tests | 34 passed | `python-tests.log` |
| Test262 `--full` | 完整向量与基线一致：102037 total / 80032 eligible / 79982 pass | `test262/check.log`、`test262/full.log`、`test262/run.json` |
| 冻结导出 CLI release 构建及 store smoke | 通过；target triple 产物与 source/binary receipt 一致 | `candidate-build.log`、`candidate-cli-smoke.json` |

workspace 总数含 B1a 13 项与 C1 8 项；profiling 配置含 C1 第 9 项事件验证。
最初因 Goto 契约不符而失败的日志保留在 `quick-tests.log` 和
`workspace-tests.log`；修复后的完整运行才作为本轮通过依据。

Test262 先认证旧 receipt；因源码 stale，跳过会拒绝当前源码的 focused 路径，
直接完成 full，未修改 `current.conf`。7 个 parse failure、43 个 runtime
failure 为基线既有结果，全部状态向量匹配；未将它们隐藏为“全用例通过”。
当前 engine semantics SHA-256 为
`db5d1c879b33e364b26edb5c6f9b4bb386891273bea2cf991cff6cc167047b00`。
归档 TSV SHA-256：`fef0b553fb70d3a311a99460c565c80e26a99ac01dfdab61c437d6c62fb7b77e`；
JSONL SHA-256：`66483ae76c330e5f2522816b570474f15fc6251bc5566cee96b1b174f1293012`。

本轮候选完整导出为 `candidate-source/`，包含 1242 个文件；
`candidate-source.json` SHA-256 为
`c79e4a15207a56190bc76a4b83abbf9c91ac1825e311c606e40bfe4e827a894b`。
导出包含所有未跟踪的新实现文件；文档随后补录验证结果，运行时源码保持一致。
已用新 CLI builder 对该导出完成一次实际普通 release 构建：rustc 1.94.1，
`x86_64-unknown-linux-gnu`，fat LTO / CGU=1，无 PGO、无 profiling。
命令与环境存于 `candidate-build-command.json`。工具正确返回并认证
`candidate-build/x86_64-unknown-linux-gnu/release/qjs`；二进制 SHA-256 为
`a698722f2041fb7e1006d9afae658112aaee38e56a09568f5fad515e453ec822`。
同目录 `qjs.source.json` 与原 manifest 逐字节一致，`qjs.build.json` 哈希已回读
核验，local/arg 链式赋值 smoke 输出精确 `true\n`、stderr 为空。
这是构建工具与运行行为的验证，没有进行配对计时或宣称性能收益。

## 6. 下一步执行顺序

1. 补齐 E/RSS、有界残余归属和 A4 裁决 receipt；确认 saved、previous、
   pre-A 的精确身份，沿用 fat LTO / CGU=1 / 无 PGO / 无 profiling 协议。
2. 利用已恢复的输入与构建工具重建分母和候选，完成 A/A、C 定向 workload
   冻结及栈流量归因；保留 bigint256、typed-index、prop-delete、navier-stokes
   的 pre-A 回退台账，不能用相对当前 HEAD 的改善将它们关闭。
3. 对 C1 串行运行计划要求的定向、58 fixed、67 compile、9 original 与保护
   矩阵；按门槛决定保留或回退，再冻结 C facade。当前代码是待裁决候选。
4. B 可继续独立开发 B1b 发布认证：在 canonical verifier 后创建只读投影，
   覆盖 draft、BC5、fixture 构造点及分配失败；接入生产前补齐内存/编译成本证据。
5. C2 的 scalar TOS 与 B1c/B1d `run/stack` 接线按计划排序；先冻结 C 边界，
   再接 B 热 handler 和基础派发，不在本轮混入 owning TOS、IC 或 quickening。

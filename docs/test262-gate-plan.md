# Test262 门禁无状态化计划

## 背景

当前 `dev-support/test262/current.conf` 用 `engine_semantics_source=<commit>` 把冻结收据的来源锚定到一个 git 提交，`--check` 通过 `git rev-parse` 解析该提交、对 engine 路径重算指纹并与 `engine_semantics_sha256` 比对。

这套机制和 squash/restack 工作流冲突：

- PR 分支上的 promotion 会把 pin 指向分支内的提交；squash 合并后该提交不在 trunk 历史里，只能靠远端分支 ref 或 PR ref 可达。
- `gh stack` 在父 PR 合并后会 rebase 子分支，pin 立即失效，必须重做 promotion。
- 于是 main 的 CI 隐式依赖"已合并分支永远不删"。删除分支后 `--check` 以 `git rev-parse failed` 失败，报错也难懂。

根因是门禁把来源证明放在了一个可能只存在于分支上的 git 对象上，而不是树内数据。

## 目标

- 门禁完全由树内数据与当前 checkout 决定：不解析任何外部提交，不依赖分支 ref。
- 保留（并加强）防伪：
  - PR/push：真实重放 focused 向量（6,844 变体），逐字节比对冻结收据 body。
  - 每日/手动：真实重放全量 102,037 变体，比对冻结全量 body 哈希。
  - `--runner-provenance`：把编译进二进制的 workspace 指纹与 checkout 绑定。
- promotion 退化为纯数据更新：结果变化时更新冻结收据 body/哈希，不再维护提交 SHA 账本。

## spec v3（`dev-support/test262/current.conf`）

- `schema=test262-gate-v3`。
- 删除 `engine_semantics_source`。
- 保留 `engine_semantics_sha256`，语义改为：冻结 focused 收据首行记录、且 `--check` 必须与之相等的指纹（纯树内交叉校验）。
- `full_tsv_sha256`/`full_jsonl_sha256` 替换为 `full_tsv_body_sha256`/`full_jsonl_body_sha256`：剥离首行身份行后的哈希。
- `engine_semantics_files`/`engine_semantics_trees` 保留，用于构建期指纹与 provenance。

## 脚本（`scripts/test262/test-test262.sh`）

- `--check`：校验全部树内文件哈希、manifest 路径集、汇总契约与冻结 focused 收据一致性；不再 git 解析、不打印 stale、不构建。
- `--runner-provenance`：不变。
- `--focused`：去掉 workspace==baseline 前置；重放后校验首行身份等于 workspace 指纹，再剥离首行与冻结收据逐字节比对（TSV/JSONL）。可在任意 checkout 上运行。
- `--full`：重放后剥离首行，比对 `full_*_body_sha256`；删除归一化与 stale 提示。
- 保留 provenance 构建/校验与私有 runner 拷贝（host-boundary guard 依赖这些字符串）。

## CI（`.github/workflows/ci.yml`）

- 新增 `test262-focused` job：`pull_request`/`push`/手动（`tier=test262-focused`）运行 `--focused`，复用 test262 的 suite/oracle 缓存与依赖。
- `test262-full` 保持定时/手动；receipt-capture 分支改为校验新收据身份等于当前指纹、body 与 spec 不同，并打印新的 `full_*_body_sha256` 供更新。更新预期错误串。
- `workflow_dispatch` tier 增加 `test262-focused`。

## 守卫

- `scripts/checks/test262-host-metadata.py`：保留既有断言；新增 gate 不得引用 `engine_semantics_source`/`--commit`、workflow 必须包含 focused 重放、spec 必须使用 v3 body 哈希键。
- `scripts/test262/current-test262-metrics.mjs`：schema 校验改为 v3。
- `docs/test262.md`：更新 Reproduce 与 promotion 说明。

## 迁移数据

以 2026-09-25 合并后的 main 为准：

- `engine_semantics_sha256=2d299f99d79110c9b0433d7cc3cfa61502e9f2bfe6484e0c5fc114a7531e5f22`
- `focused_tsv_sha256=c354b11d4e91f2b64d26610e2a847cb276c7e19924e65a243da2b3476f232092`
- `focused_jsonl_sha256=32da244a2907cd18aca1725cc0b982a8df5104b2496561dd20d5c4e6a98a68a1`
- `full_tsv_body_sha256=971cc666767b3c4eb9b519340b5d7a77a80ff8405f6a8d800aada822d3230b19`
- `full_jsonl_body_sha256=8447c3de7a70959ff9bcff40102b469d51d810fff0033d7c78ba3065dbd55418`

## 验证

- `./scripts/test262/test-test262.sh --check`
- `./scripts/test262/test-test262.sh --focused`（重放 6,844）
- `TEST262_WORKERS=12 ./scripts/test262/test-test262.sh --full`（重放 102,037）
- `./scripts/checks/check-test262-host-boundary.sh`、`node scripts/test262/current-test262-metrics.mjs --check-docs` 及其余 fast 检查。

## 后续清理

门禁无状态化合并进 main 后，已合并的分支、遗留 worktree 与 `refs/backup/pr-stack/*` 备份引用即可删除。

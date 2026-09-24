# Lexer/Parser 前端重构计划

本文件是 lexer/parser 重构的实施计划，对应
[issue #32](https://github.com/pocket-nexus/quickjs-oxide/issues/32) 的 P1/P2/P3 前端部分。
目标是以可度量的方式消除前端（lexer → parser → resolution → lowering 名字流）
的堆分配与重复扫描，把 `functions` 密集语料上的指令数、cache miss、RSS 和编译
时间拉下来。**本计划不含 verify/publish/VM 的改动**（另立计划）。

## 0. 目标与非目标

目标（按实测口径，`docs/compile-benchmark.md` §6/§7）：

1. 标识符名字从“每个出现 6–10 次堆分配”降为“每个**不同**名字 1 次分配”，
   理想情况下最终 0 次（驻留后只存 `NameId`）。
2. `Token` 变为 `Copy`，token 缓存不再持有任何堆载荷；31 处整 token clone、
   5 处 `kind.clone()` 全部消失。
3. 17 处 clone-lexer 重扫与 `parenthesized_parameter_tokens` 的整段 token 复制
   改为 token 缓冲复用，同一段源码最多扫一次。
4. `functions-4194304` 上：指令/KB、cache-miss/KB、RSS/源MB、编译时间相对
   基线（2.90M/KB、17.1k/KB、100.3MB/MB、1789ms）逐阶段下降；终态吞吐至少
   翻倍（超过 Boa 的 4.69 MB/s，向 QuickJS 靠拢）。

非目标：

- 不改 verify/publish/执行器（属 issue #32 的另一半）。
- 不改 JS 语义与 QuickJS 对齐行为：token span、`line_terminator_before`、
  ASI、错误文案、Unicode 标识符表（pinned QuickJS 17 表）必须逐字节保持。
- 不改公开 API（`src/engine/api/*` 出口不变）；改动限定在 crate 内部。
- 不改变 Atom 的生成**顺序**（BC5 pinned atoms 与 binary-object 字节一致性依赖它）。

## 1. 现状（事实，带 file:line）

### 1.1 词法

- `Lexer<'a>` 是纯 `&str` 状态机，`Clone` 只复制 9 个标量 + 2 个引用
  （`lexer.rs:547-558`），无堆数据；`seek(Position)`（`lexer.rs:616-624`）可重放。
- 非 Copy 载荷只有 4 类：
  - `Identifier.value: String`（`lexer.rs:476`，构造 `1073`，唯一分配点 `942`）；
  - `StringLiteral.value: lexer::JsString{Vec<u16>}`（`lexer.rs:432`，构造 `1367`）；
  - `TemplatePart.raw_value/cooked: JsString`、`invalid_escape: Option<TemplateEscapeError{message: String}>`
    （`lexer.rs:452-464`）；
  - `LexError.message: String`（`lexer.rs:531`，仅错误路径）。
- `NumberLiteral`/`RegExpLiteral` 已全部是 `&'a str`，只需补 `Copy`
  （`lexer.rs:342-346`、`466-471`）。
- 数字解析已惰性（`parser/literals.rs:409-437`），但做 `raw.replace('_', "")`
  且走 `BigUint`（`literals.rs:412,459`）。
- 转义解码路径：`scan_escape_sequence` 返回 Copy 的 `EscapeValue`
  （`lexer.rs:1437-1541,2020-2047`），累积靠
  `push_char_with_limit/push_code_unit_with_limit`（`lexer.rs:382-416`）。
- 字符串/模板的 cooked 值只在 lexer 内构建；`StringLiteral` 已有 `has_escape`
  （`lexer.rs:434`），模板有 `cooked: Option<JsString>` 区分 tagged/untagged
  非法转义（`lexer.rs:459-461`）。

### 1.2 解析

- `Parser.tokens: Vec<Token<'source>>` + `cursor`，始终只预读 1 个 token；
  goal/context（regex vs div、strict/module/generator/async、template 续段）
  在 token 扫出**之后**才决定，所以有 `truncate + seek + relex` 机制
  （`parser/tokens.rs:417-522`）。
- 17 处 clone-lexer 前瞻：`parser/tokens.rs:77,177,296,328,348,371,429,531`；
  `arrow.rs:321,428,452,468`；`destructuring.rs:643,756`；`class.rs:555`；
  `module.rs:216`；`private_reference.rs:95`。
- 高成本前瞻（按解析产生式）：每个 `parse_assignment` 最多 5 个探针
  （`expressions.rs:95-119`）；每个 `for` 两次整头扫描（`loops.rs:151,288`）；
  每个括号组 `parenthesized_arrow_ahead` 平衡整组（`arrow.rs:320-423`）；
  每个 `{`/`[`/`(` 形参形态 `binding_pattern_scan`（`destructuring.rs:736+`），
  `object_binding_has_rest` 二次整段重扫（`destructuring.rs:1315,1821`）；
  `parenthesized_parameter_tokens` **复制整段 token**
  （`destructuring.rs:639-676`）；每个函数体 `directive_prologue_has_use_strict`
  整段指令序言重扫（`parser/tokens.rs:524-568`）。
- 31 处 `self.current().clone()` 的动机都是“结束借用后 move `identifier.value`”
  （典型 `statements.rs:812,903`；`loops.rs:508,566`；`calls.rs:101`）。
- `FunctionTree` 组装时 `SourceText` 深拷贝（`parser/entry.rs:281-283`，
  `SourceText` 为多 `Box<[..]>`，`source/text.rs:55-63`）。

### 1.3 名字流

- 未解析 IR 存 `String`：`IrOp::Identifier/IdentifierReference/PrivateField`
  （`model/ir.rs:160,166,176`）；已解析的 `DynamicIdentifier{name: u32}` 已是
  常量索引（`model/ir.rs:142,151`），说明“解析后按索引”与现架构一致。
- `resolution.rs:149,161,180` 每个未解析引用 `name.clone()` 进临时 Vec；
  `ensure_string_constant(&mut FunctionIr, name: &str)`（`resolution.rs:2428-2435`）
  是名字→常量的唯一入口，**命中缓存也会新建 `JsString`**（2-3 次分配），
  13 处调用 + `capture_global_path` 逐祖先调用（`resolution.rs:2395-2420`）。
- `bindings_by_name: HashMap<String, BindingId>`（`model/scope.rs:47`）；
  `IrBinding.name: String` 与 `IrGlobalDeclaration.name: String`
  （`model/bindings.rs`）；`FunctionIr.name/parameter_names/locals/parameters`
  均为 `String`（`model/ir/function.rs:69,111,159,201`）。
- 降级期每绑定一次 `JsString::try_from_utf8`（`lowering.rs:341,370,612`），
  `code` 侧类型是 `UnlinkedVariableDefinition.name: Option<JsString>`；
  `EvalEnvironment<Name>`/`EvalRootBinding<Name>` 已是泛型
  （`code/function/metadata.rs:450,467`），`code` 可以保持 `JsString` 不动。
- 发布期 `runtime.rs:138-220` 把名字 intern 成 Atom；`verify/closures.rs:13-14`
  与 `verify/private_elements.rs:68` 用 `Vec<u16>` 键（本计划不涉及）。
- 诊断渲染需要真实字符串：`parser/diagnostics.rs:84-104`、
  `private_reference.rs:252-256`、`module.rs:1039`、若干 `format!` 点。

### 1.4 测试与守卫（每阶段必须过）

- `cargo test --locked --workspace --all-targets`（compiler 约 308 个 `#[test]`，
  `code` 约 445 个）。
- `cargo clippy`（4 组 feature 组合，`-D warnings`）、`cargo fmt --check`、
  `--doc`、test262-host。
- test262 frozen receipt：`./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --check`
  （focused）与 `--full`；负向诊断 `scripts/test262/audit-negative-diagnostics.mjs`。
- QuickJS 差分：`./scripts/quickjs/test-quickjs-fixtures.sh --validate`、
  `test-quickjs-c-oracles.sh --validate`、`test-quickjs-dynamic-import-trace.sh`。
- 结构性守卫：`check-bc5-pinned-atoms.mjs`/`check-bc5-pinned-opcodes.mjs`、
  `check-source-layout.py`、`check-rust-only.sh`、`check-oracle-registry.sh`。
- profiling feature：`cargo test -p quickjs-oxide --features profiling --lib profiling_`。

## 2. 核心设计

### 2.1 Token 变 Copy，cooked 值惰性化（单一解码实现）

目标类型（`'a` 为源生命周期）：

```rust
#[derive(Clone, Copy)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
    pub line_terminator_before: bool,
}

#[derive(Clone, Copy)]
pub enum TokenKind<'a> {
    Identifier(Identifier<'a>),
    PrivateIdentifier(Identifier<'a>),
    Keyword(Keyword),
    Number(NumberLiteral<'a>),          // 已 Copy
    String(StringLiteral<'a>),          // raw + flags，无 cooked
    Template(TemplatePart<'a>),         // raw + flags，无 cooked
    RegExp(RegExpLiteral<'a>),          // 已 Copy
    Punctuator(Punctuator),
    RawAscii(u8),
    Eof,
}

#[derive(Clone, Copy)]
pub struct Identifier<'a> {
    pub raw: &'a str,
    pub has_escape: bool,
    pub keyword_hint: Option<Keyword>,      // 扫描期已由解码值判定
    pub escaped_reserved_word: bool,
}
```

关键点：

1. **删除 `Identifier.value`**。`!has_escape` 时 `value == raw`；`has_escape` 时
   由 parser 在需要时解码（每个**不同**转义名一次，冷路径）。
2. **`StringLiteral`/`TemplatePart` 只保留 raw + 标志**（审计修正）：
   - `StringLiteral<'a> { raw, has_escape, has_legacy_octal_escape }`：`value`
     删除；`quote` 全仓库零读取，一并删除（`Quote` 随之无用）。
   - `TemplatePart<'a> { raw, kind, invalid_escape: Option<TemplateEscapeError> }`：
     `raw_value/cooked` 删除；cooked 是否存在由 `invalid_escape.is_some()` 判定
     （untagged 报错、tagged 传 `undefined` 的语义不变）。
   - `TemplateEscapeError { message: &'static str, span: Span }` 变 Copy（可达
     message 全是字面量：`"unexpected end of string"`、两条 octal 文案、
     `"malformed escape sequence in string literal"`；invalid UTF-8 立即返回不
     入结构）；`LexError.message: String` 保持不动（错误路径不在 token 里）。
   - 消费点按需解码：untagged 模板要 cooked、tagged 模板要 raw_value+cooked，
     字符串/属性键要 cooked；`raw` 是源切片不含 CRLF 归一化与转义还原。
3. **解码逻辑只保留一份实现**：把 `scan_string`/`scan_template` 的累积器抽象成
   sink：

   ```rust
   trait StringSink {
       fn push_code_unit(&mut self, unit: u16) -> Result<(), JsStringError>;
       fn push_char(&mut self, ch: char) -> Result<(), JsStringError> { /* 默认：encode_utf16 逐 unit */ }
       fn push_code_point(&mut self, value: u32) -> Result<(), JsStringError> { /* 默认：代理对拆分 */ }
   }
   struct ValidateSink { len: usize, limit: usize }   // 扫描期：只计数校验，0 分配
   struct Utf16Sink(Vec<u16>);                        // 消费期：物化
   ```

   扫描期用 `ValidateSink`（唯一错误是 `TooLong`，由调用点在同一游标位置映射成
   现有 `string_too_long(start)`，span/message 不变）；消费期重新扫描同一 span，
   用 `Utf16Sink` 物化。`push_char/push_code_point` 的多 unit 一次性长度检查语义
   由默认实现逐 unit 累加达成：`Err` 后立即中止并丢弃缓冲区，可观测结果相同。
   错误路径 `LexError` 仍由扫描器构造，sink 不接触 span。
4. **`SourceText` 约束（审计新增，最重要）**：`StringLiteral.value`/
   `TemplatePart.raw_value`/`cooked` 可包含由 `SourceText` 还原的孤立 UTF-16
   代理项与瘦身字节（`lexer.rs:2810-2867` 测试钉死）。`raw: &str` 无法重建它们，
   所以**解码必须通过带 `source_text` 的 lexer 复制体**执行，不能用纯 `&str`
   实现。API 设计（Parser 方法，内部 `self.lexer.clone()` + `seek`）：

   ```rust
   fn identifier_text(&self, id: &Identifier) -> Cow<'a, str>;   // !has_escape => Borrowed(raw)
   fn decode_string(&self, lit: &StringLiteral) -> Result<JsString, Error>;
   fn decode_template_raw_value(&self, part: &TemplatePart) -> Result<JsString, Error>;
   fn decode_template_cooked(&self, part: &TemplatePart) -> Result<Option<JsString>, Error>;
   ```

   约束：clone 保留 `string_limit`（测试注入的长度上限）与 `source_text`；解码只
   对已提交 token 调用（不做前瞻）；`strict` 只影响 `\0-\7`/`\8\9` 分支，由
   `has_legacy_octal_escape` 反推（该标志为真时按非严格扫）；扫描期已保证除模板
   非法转义外全部合法，消费期错误可 `debug_assert`。

收益：lexer 对标识符/数字/正则/**字符串/模板**全部零堆分配；token 缓存只存
`Token`（约 64–72 字节，其中 `Span` 占 32，`Copy` 无堆载荷），任意 clone 为
Copy。真正的收益不是 token 变小，而是每个 token 背后不再挂堆对象。

### 2.2 名字驻留：`NameTable` + `NameId`

- 新增 `compiler/names.rs`：

  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub struct NameId(u32);

  pub struct NameTable {
      names: Vec<Box<str>>,                 // 插入序，稳定
      by_name: HashMap<Box<str>, NameId>,   // 内容键，保留 SipHash（防 HashDoS）
  }
  impl NameTable {
      pub fn intern(&mut self, name: &str) -> NameId;   // 命中 0 分配（仅 parser 调用）
      pub fn name(&self, id: NameId) -> &str;
      pub fn lookup(&self, name: &str) -> Option<NameId>; // 只读查询（resolution 用）
  }
  ```

  - **不放 Lexer**：lexer 保持纯函数式（clone+seek 可重放），前瞻不会污染表。
    驻留发生在 parser 边界：`Parser` 持有 `NameTable`，提供
    `intern_identifier(&Token) -> NameId`（`has_escape` 才解码，冷路径）与
    `intern_name(&str)`（合成名）。
  - 生命周期：`NameTable` 在 `parse_root` 结束时随 `FunctionTree` 一起交给
    resolution/lowering（`FunctionTree.names`），诊断用它渲染文本。
  - **唯一 `intern` 方是 parser**（`&mut self.names`）；parser 在构造期预驻留
    resolution 需要的合成名（模块 goal 的 `MODULE_IMPORT_META_BINDING_NAME` 等），
    resolution/lowering 只拿 `&NameTable` 做 `name/lookup`，避免
    `tree.functions` 可变借用与名字表可变借用冲突。
  - **private 名是两个独立名字**（审计修正）：标识符本体驻留解码名（不含 `#`，
    长度上限仍按含 `#` 计，`lexer.rs:946`）；绑定键 `#name`（
    `private_reference.rs:32-37`）与 `#name<set>`（`39-44`）仍是 `format!` 合成后
    intern 的独立 `NameId`。因此 `IrOp::PrivateField.name` 迁移后语义 = 当前存放
    的字符串（含 `#`），不是一个"去 `#`"的名字。
  - 字符串键保留 `std::HashMap` 默认 SipHash：编译输入不可信，`FxHash` 只用于
    整数键（见 2.4）。P3 再评估替代方案。

- IR 改为携带 `NameId`：`IrOp::{Identifier,IdentifierReference,PrivateField}.name:
  NameId`；`IrBinding/IrGlobalDeclaration/IrEvalDeclaration.name: NameId`；
  `IrScope.bindings_by_name: HashMap<NameId, BindingId>`；
  `FunctionIr.name/parameter_names/locals/parameters: NameId`；
  `IrParameterPatternBinding.name: NameId`；module 的 `local_name: NameId`
  （`export_name: JsString` 是用户可见字符串，**保持 JsString**，因为
  `export { x as "任意字符串" }` 不是标识符）。
- 比较语义：`identifier.value == "of"` 这类约 25 处必须改为
  `identical_name(token, "of")`，其实现为
  `if has_escape { decode(raw) == X } else { raw == X }`，**不能**简单写成
  `raw == X && !has_escape`（会改变转义上下文关键字的既有行为）。
  `keyword_hint`/`escaped_reserved_word` 仍由 lexer 计算，语义不变。
- duplicate-parameter 检测（`function.rs:641-648`、`arrow.rs:273-280`）
  改用 `NameId` 相等，等价且更快。
- 合成名：`SyntheticLocalKind::name()`（`&'static str`）与
  `class/fields.rs:175-177` 的 `format!` 名字照旧生成字符串后 `intern_name`，
  命名序号语义不变。
- resolver：`ensure_string_constant(function, id: NameId)` 用 `names.name(id)`
  取文本，调用顺序**逐点保持不变**（Atom 顺序钉死）。未解析收集 Vec
  （`resolution.rs:149,161,180`）由 `name.clone()` 变为直接拷贝 `NameId`。
- lowering 边界：`lowering.rs:341,370,612` 从 `NameId` 经 `names.name(id)` 生成
  `JsString`；可加 per-`NameTable` 的 `JsString` 缓存（每个不同名字只物化一次），
  `code` 侧类型完全不动。
- eval 外部绑定（`entry.rs:306-453`）从 `JsString` 解码后 `intern_name`。

### 2.3 `TokenBuffer`：带词法状态标记的 token 缓冲

**审计结论：分两步实施（P2a 前瞻备忘 / P2b 提交缓冲）**。现状
`tokens.len() == cursor + 1` 恒成立（`set_future_lex_context` 的 `truncate` 是
no-op，`advance_expression_start` 的 truncate/seek 分支是死代码），全部前瞻都走
clone-lexer。先用"前瞻备忘缓存"（键 `(start_offset, goal, LexContext)`，只服务
17 处探针、不改提交路径）低风险消掉重复扫描；再让 `TokenBuffer` 接管提交路径与
relex。必须保持的现状不变量逐条见附录 C.1，分步方案见 C.2/C.3。

P2b 用缓冲替换 `Vec<Token> + cursor` 与整套 `truncate/seek/relex`：

```rust
struct TokenEntry<'a> {
    token: Token<'a>,
    goal: LexicalGoal,      // 该 token 扫出时使用的 goal
    context: LexContext,    // 扫出时的 strict/module/generator/async
}
struct TokenBuffer<'a> {
    entries: Vec<TokenEntry<'a>>,
    cursor: usize,
    // 每个 entry 还记录其起点 Position，便于回退 lexer
}
```

API（取代 `parser/tokens.rs` 现有机制）：

- `current()/advance()`：commit 路径，语义不变；
- `ensure(index, goal, context)`：若 `entries[index]` 存在且 `(goal, context)`
  匹配则直接复用；否则 `truncate(index)` 并把 lexer `seek` 到该 entry 起点后
  重新扫到 index；
- `mark()/restore(mark)`：前瞻用，把扫描游标退回，**已扫 token 默认保留**
  （下次匹配 goal 时直接复用，不匹配才重扫）；
- `relex_current(goal, context)` / `set_future_context(context)`：用
  `ensure/truncate` 表达，`line_terminator_before` 保留逻辑不变
  （`parser/tokens.rs:479,505,510`）。

17 处 clone-lexer 前瞻逐个改为 `mark + ensure(...) + restore`；手写 goal 状态机
（regex/template）保留判定逻辑，但输入来自缓冲切片。
`parenthesized_parameter_tokens`（`destructuring.rs:639-676`）改为
`&entries[a..b]` 切片；`object_binding_has_rest` 的二次重扫改为复用缓冲。

语义红线：

- `set_future_lex_context` 不重读当前 token（`parser/tokens.rs:517-522`）；
- EOF 粘性（`advance_with_goal:447`）；
- 每个 token 的 `line_terminator_before`、`span` 与现行完全一致（前瞻重扫不改变
  已提交 token 的这两个字段）；
- regex/div 与 template 续段的判定顺序与今天一致（由 parser 侧调用点决定，
  buffer 只负责缓存与最少重扫）。

### 2.4 分配与哈希策略

- 整数键（`ScopeId/BindingId/FunctionId/pc`、`HashSet<(usize,u16)>` 等）引入
  `rustc-hash`（本地已缓存 2.1.3）替换默认 SipHash。
- 内容键（`NameTable.by_name`、`string_constants`）保留 SipHash：源不可信，
  避免 HashDoS。名字数量远小于 token 数，分配才是瓶颈。
- 小集合（每控制结构 3-4 个 Vec、闭包描述符）在 P3 用 `smallvec/thin-vec`
  （本地已缓存），先测量再替换。
- 依赖以工作区 `workspace.dependencies` 钉版本；本计划最多新增
  `rustc-hash`，P3 再议 `smallvec/thin-vec/memchr`。

## 3. 分阶段实施

每阶段独立可交付、可回滚；提交保持小而可编译。

### P0 工具与基线（不改语义）

1. 新增 `scripts/benchmark/probes/compile_alloc_probe.rs`（独立于矩阵探针）：
   `#[global_allocator]` 计数包装，在 `compile_ns:` 之后输出
   `alloc_count/alloc_bytes/realloc_count/realloc_bytes/dealloc_count/
   peak_live_bytes`。**不能放 `apps/cli/examples/`**：workspace 的
   `unsafe_code = "forbid"` 不可被局部 allow 覆盖；改由
   `build_compile_probe.py --probe/--name` 生成独立 crate（同一套依赖钉版与
   回执），`--version` 故意不匹配矩阵 magic，`compile_matrix.py` 会拒绝消费。
   （已实施）
2. 固化基线：在 `functions`/`expressions`/`syntax-mixed` 的 4MB 档上跑
   `perf stat`（instructions/cycles/cache-misses/task-clock，扣 64KB tiny 档）
   + RSS + 分配计数（9 个语料 × 3 次验证线性），写入
   `docs/compile-benchmark.md` §9。（已实施）
3. 检查：`check-source-layout.py`、`check-rust-only.sh`、fmt/clippy、
   `scripts/benchmark` 单测。

验收：工具产出可复现的基线表；无任何生产代码行为变化。
P0 实测基线（§9）：`functions-4194304` 1,882 alloc/KB（+229 realloc/KB）、
242 KB 分配量/KB、peak live 114,818 KB/KB、2.897M instr/KB、17,566 miss/KB、
task-clock 0.435ms/KB。§5 的目标表以该表为参照。

### P1a Token Copy + 惰性 cooked + 去掉 token clone

逐点清单见附录 A（A.1 类型、A.2 sink/解码、A.3 cooked 消费、A.4 比较点、
A.5 move/绑定点、A.6 token 复制、A.7 标志规则、A.8 测试适配）。

改动点：

- `lexer.rs`：按 §2.1 改 `Token/TokenKind/Identifier/StringLiteral/TemplatePart`；
  `NumberLiteral/RegExpLiteral` 补 `Copy`；`scan_*` 改用 `ValidateSink`；
  新增 `decode_string_literal`/`decode_template_cooked`（与 scan 同实现）；
  lexer 内测试适配（90 处 `TokenKind` 断言，`lexer.rs` 测试区）。
- 消费点适配（惰性 cooked）：`parser/literals.rs:105`、`module.rs:414,437,461,712,715`、
  `class.rs:468`、`object_literal.rs:128`、`destructuring.rs:1942`、
  `template.rs:40-42,135-140`、`parser/tokens.rs:542`（`use strict` 判定）。
- 删除 31 处 `current().clone()` 与 5 处 `.kind.clone()`
  （`module.rs`7、`destructuring.rs`4、`statements.rs`3、`template.rs`2、
  `loops.rs`2、`calls.rs`2、`object_literal.rs`2、`class.rs`2、`arrow.rs`2、
  `private_reference.rs`1、`literals.rs`1、`expressions.rs`1、`optional_chain.rs`1、
  `function.rs`1）；改法统一为“借 `&Token`（Copy）→ 需要名字时走 §2.2 的
  `intern_identifier`（本阶段先临时 `raw/decoded.to_owned()`，P1b 换 `NameId`）”。
- `parser/literals.rs:409-437` 的数字解析本阶段只做“去 `replace` 分配”的
  等价改写（`raw` 切片是否含 `_` 分支），BigUint 优化留 P3。
- lexer 外构造 `Identifier` 的 2 处（`function.rs:188-194,207-213`）改为新字段集。
- 测试适配：`tests/parameters.rs:471-474` 手工构造 `Parser`/token；
  `class/private.rs:323`；oracle 的 `identifier.raw` 比较（语义保持）。

语义风险与对策：

- `Identifier.value` 的 70 处消费全部逐个审计：比较类改
  `identical_name`（含转义分支），move 类改为“取 raw/解码后 to_owned”。
- `StringLiteral.has_escape` 已存在；模板新增 `has_invalid_escape`，
  保持 `cooked=None` 的 tagged/untagged 行为（`lexer.rs:1747-1781` 的事务性
  两遍扫描改为 validate sink，不消费闭合反引号/`${` 的不变式不变）。
- 长度上限（`StringTooLong`）与按 message 文本分支的 3 处
  （`lexer.rs:1390-1413,1747-1752,1565-1576`）保持行为；message 文本不变。

验收：

- `cargo test --workspace --all-targets`、clippy 5 组（CI 命令）、fmt、doc；
- lexer/parser 相关测试 + oracle lexical + `test-quickjs-fixtures --validate`；
- `audit-negative-diagnostics`、focused test262；
- 指标：`functions-4194304` 分配次数下降 ≥60%（分配探针），
  instr/KB 下降 ≥15%，cache-miss/KB 下降 ≥30%，时间下降 ≥15%。

实施记录（P1a 完成，HEAD `a5b651be`）：

- 提交：`970a11be`（C1：`StringSink`/`Utf16Sink`，字符串/模板累积走 sink）、
  `ba593f9e`（C2：`Identifier` 惰性解码 + Parser `identifier_text`/
  `identical_name`/`is_unescaped_name`）、`f29dd568`（C3：
  `StringLiteral`/`TemplatePart` 只存 raw+标志、`ValidateSink`、Parser
  `decode_string_literal`/`decode_template_raw_value`/`decode_template_cooked`、
  `Token`/`TokenKind`/`LexError` Copy、清 38 处 token clone）、
  `a5b651be`（C4：数字去 `replace` 分配，仅含 `_` 时 `Cow::Owned`）。
- 偏差：`LexError.message` 由 `String` 改 `&'static str`（全部消息为字面量，
  唯一外部构造是 `tests/scopes.rs`；`TemplateEscapeError.message` 直接取用）；
  Parser `decode_*` 以 token `Span` 为参数（避免 `position_at` 回放造成
  O(n²) 解码）；`ValidateSink` 在 C3 引入（C1 先只删 eager 值，避免
  dead_code）；`decode_template_cooked` 先查 `invalid_escape` 再解码，语义
  等价于旧 `cooked=None`。
- 验收：全量 Rust 测试（2283 lib + 全 target）、doc、CI 5 组 clippy、fmt、
  oracle 全量 912 passed（`QJS_ORACLE` 指向 qjs 二进制绝对路径）、
  `check-rust-only.sh`、`check-source-layout.py`、bc5 pinned atoms/opcodes、
  fixtures/c-oracles `--validate`。
- test262 全量对照：`--full`（12 workers，需清空 `GIT_*` 调用者环境）在当前
  HEAD 与父提交 `0e59f836`（`git worktree` 控制组）各跑一次，
  102,037 variants 的结果向量除首行 metadata 的 engine 哈希外逐行完全一致
  （pass=80010、fail-parse=7、fail-runtime=43、unsupported=3,502、
  skipped=18,475），证明 P1a 语义中立。pinned milestone（`full_summary`）
  比两者多 2,562 个 `unsupported-negative-provenance`，即 28 个用例在父提交
  就已成为 pass——是里程碑提升以来的既有漂移，与本次重构无关；因此本阶段
  不 promote 里程碑（focused replay 仍因 stale 被拒），promotion 留到分支
  合并/阶段收尾时一次性完成。
- 指标：见 §5 校准段与 `docs/compile-benchmark.md` §9.5。

### P1b `NameId` 驻留（IR/resolution/bindings/lowering）

逐点清单见附录 B（B.1 表与所有权、B.2 字段迁移、B.3 合成名、B.4 常量/Atom
顺序、B.5 哈希审计、B.6 诊断、B.7 测试）。

改动点：

- 新增 `compiler/names.rs`（§2.2）；`Parser` 持有 `NameTable`；
  `FunctionTree.names`；`finish()` 路径传递（`parser/entry.rs:275-287`）。
- IR/绑定/作用域字段换 `NameId`：`model/ir.rs:160,166,176`；
  `model/ir/function.rs:69,111,159,201,295,548`；`model/bindings.rs`；
  `model/scope.rs:47`；`compiler/module.rs:68,78-83`（`local_name`）。
- parser 全部名字消费点：emit（`parser/builder.rs:191,208,223`）、
  `promote_tail_identifier_get`/`keep_tail_identifier_reference`
  （`expressions.rs:1201-1333`）、`capture` 路径、Annex B、class private、
  destructuring、module 等（详见调研清单），共约 70 处。
- resolution：未解析收集（`resolution.rs:101-238`）改 `NameId`；
  `ensure_string_constant(function, id)`（`2428-2435`）用
  `names.name(id)` 生成 `JsString`，新增 per-`NameTable` `JsString` 缓存；
  保持 13 处调用顺序不变。
- lowering：`lowering.rs:341,370,612` 取 `names.name(id)`；
  `code` 侧 `UnlinkedVariableDefinition{name: Option<JsString>}` 不动。
- 诊断：需要真实文本处传 `&NameTable` 渲染
  （`parser/diagnostics.rs:84-104`、`private_reference.rs:252-256`、
  `module.rs:1039`、`destructuring.rs:509`、`class/fields.rs:175-177` 等）。
- profiling：`sample_ir_storage`（`compiler/diagnostics.rs`）字段类型变化，
  更新采样；`CompilePhase` 不变。
- 测试：约 308 个 compiler 测试中所有断言 `.name` 字符串处改用
  `tree.names.name(id)`（提供测试助手，机械替换）。

验收：

- 全量 Rust 测试 + `check-bc5-pinned-atoms --self-test` +
  `test-quickjs-fixtures --all --oxide`（字节一致性）+ focused test262；
- 指标：`functions-4194304` 分配次数相对 P1a 再降 ≥70%，
  instr/KB 再降 ≥15%，cache-miss/KB 再降 ≥40%，时间再降 ≥15%。

实施记录（P1b 完成，HEAD `bd3eb461`）：

- 提交：`343acdfd`（`compiler/names.rs` 新增 `NameTable`/`NameId`，parser 为
  唯一 intern 方；IR/bindings/scope/module 字段换 `NameId`；resolution/
  lowering/诊断改 `names.name(id)` 渲染；测试机械适配）、`2a606c5a`（clippy
  清理：去冗余 clone 与 `iter().any()`）、`bd3eb461`（per-NameTable
  `JsString` 缓存，`ensure_string_constant` 改收 `JsString`）。
- 偏差：动态 eval 绑定的运行时 `JsString` 名在 resolution 期由
  `tree.names.intern` 驻留（只写内部表，不产生输出影响；parser 之外的唯一
  intern 点）；合成名由 parser 预驻留，`pseudo_name` 以 `expect` 断言该
  不变量；private 名统一存含 `#` 的合成拼写；`ensure_string_constant` 的
  14 个调用点相对顺序不变。
- 验收：全量 Rust 测试（workspace all-targets 3375 passed / 0 failed、
  `--doc` 3 passed、test262-host lib/bins 2414 passed、
  `unsupported_diagnostics` 6 passed）、CI 5 组 clippy、fmt、oracle 全量
  912 passed、`check-rust-only.sh`、`check-source-layout.py`（694 文件）、
  bc5 pinned atoms/opcodes self-test、fixtures `--all --oxide`（13/13
  字节一致）、c-oracles `--validate`。
- test262：`--full` 在当前 HEAD 跑通全部 102,037 variants（pass=80010、
  fail-parse=7、fail-runtime=43、unsupported=3502、skipped=18475），
  `target/test262-full.tsv/.jsonl`（engine hash `5dbb43ca`）除首行 engine
  哈希外与 P1a HEAD `a5b651be` 的 full report（`09740268`，备份
  `target/p1a-full-report.*`）逐字节一致（TSV 主体 sha `971cc666…`、JSONL
  主体 sha `8447c3de…`），证明 P1b 相对 P1a 语义中立。脚本对 frozen
  milestone `full_summary` 的最终比对以 exit 5 失败（当前 pass=80010 /
  unsupported-negative-provenance=2534 vs 里程碑 79982/2562，28 例漂移），
  与 P1a 记录的既有漂移一致、与本次重构无关；`--focused` 仍因 stale 被拒。
  不 promote，留到分支合并/阶段收尾一次性完成。
- 指标：见 §5 校准段与 `docs/compile-benchmark.md` §9.6。未达 §5 的 P1b
  方向目标（分配 ≥70%、instr ≥15%、miss ≥40%、时间 ≥15%），仅 functions 的
  miss（−68.5%）/时间（−16.5%）达标；原因与后续校准见 §9.6。

### P2a 前瞻备忘缓存（低风险第一步）

改动点：新增只服务探针的 `LookaheadCache`（键
`(start_offset, goal, LexContext)`，值 `Token`，见附录 C.2）。17 处 clone-lexer
探针改为"先查缓存、未命中才 clone 扫描并写入"；提交路径、`tokens`/`cursor`、
`relex*`/`set_future*` 全部不动。缓存按 offset 有序，`set_future_lex_context`/
`relex` 清掉 `start >= future_offset` 的条目。

验收：全量 Rust 测试 + focused test262 + fixtures；新增缓存命中/失效单测
（C.4）；分配次数下降（探针缓存省掉重复 token 扫描的分配）；instr/KB 下降
≥10%。此阶段不追求 P2 的总目标。

### P2b 提交路径 `TokenBuffer`

改动点：按 §2.3 在 `parser/context.rs` 引入 `TokenBuffer`，重写
`parser/tokens.rs` 的 `advance*/ensure*/relex*/set_future*`；把 17 处
clone-lexer 前瞻换成 `mark/ensure/restore`；`destructuring.rs` 的
`parenthesized_parameter_tokens`/`object_binding_has_rest`、
`arrow.rs` 的 `parenthesized_arrow_ahead`、`loops.rs` 的 for-head 探针逐个迁移；
历史读取点（`statements.rs:846`、`arrow.rs:296-300`、directive 的
`tokens[start]`）保留按索引访问。

语义风险与对策：附录 C.1 的 15 条不变量逐条实现并测试；每个迁移点配对应语法
测试（for/箭头/解构/指令序言/正则 goal）；一次性只迁移 1–2 个产生式并跑
Oracle；`line_terminator_before` 与 span 保持不变；旧 clone-lexer 路径保留为
test-only 对拍（差异测试，C.4）。

验收：全量测试 + fixtures/C-oracles + focused test262；指标再降
instr/KB ≥15%、cache-miss/KB ≥30%、时间 ≥15%；`parenthesized_parameter_tokens`
不再出现 Vec 复制（代码审查 + 分配计数）。

### P3 杂项收尾

- `SourceText` 深拷贝改 `Rc`/共享（`parser/entry.rs:281-283`）。
- 数字字面量快路：无 `_` 且为小整数时直接 `i64/u64`，BigUint 仅大数
  （`parser/literals.rs:409-462`）。
- 小集合换 `SmallVec`/`ThinVec`（每函数 Vec 字段、break/continue 跳转列表、
  闭包描述符），以分配计数与 RSS 决定取舍。
- lexer 扫描快路：ASCII 分类表 + `memchr` 批量扫 trivia/标识符
  （`lexer.rs:710` 的逐字符 UTF-8 解码、`skip_trivia`、`scan_punctuator`）。
- 评估 `ensure_closure_variable` 的线性扫描（`resolution.rs`）与
  `HashSet<(usize,u16)>` 等 verify 侧暂不动的热点是否受前端改动收益。

验收：全量 gate（含 test262 `--full` 或 receipt 更新流程）；终态指标见 §5。

## 4. 语义红线与高风险清单

1. **转义标识符比较**：所有 `value == X` 改 `identical_name`，含转义解码分支；
   禁止直接 raw 比较。
2. **Atom 顺序**：`ensure_string_constant` 调用顺序与 `IrConstant` 追加顺序
   逐点不变；`check-bc5-pinned-atoms` + fixtures 字节一致性守护。
3. **错误文案与 span**：lexer/parser 诊断文本、`StringTooLong` 特判、
   负向诊断 TSV 全覆盖；message 保持 `String` 不改 `'static`。
4. **模板语义**：`cooked: Option` 的 tagged/untagged 区别、首次 `invalid_escape`
   的 span/文案；两遍扫描事务性改为 validate sink 时不得消费闭合反引号/`${`。
5. **ASI**：`line_terminator_before` 在每个 token 上必须与现行逐位一致，
   前瞻重扫/缓冲复用不得改写已提交 token 的该字段。
6. **`SourceText` 畸形字节**：惰性 cooked 不能推迟掉扫描期就必须报的
   malformed-byte/长度错误（`tests/raw_source.rs:128-142` 已覆盖）。
7. **private 标识符**：标识符解码名不含 `#` 但长度上限含 `#`（`lexer.rs:946`）；
   绑定键是独立合成名 `#name`/`#name<set>`（`private_reference.rs:32-44`）。
   `NameTable` 按“一个完整字符串一个 `NameId`”处理这两类名字，不得只驻留去 `#` 的
   版本再靠拼接比较。`#constructor` 检查按解码后名字（`class.rs:493` 无
   `has_escape` 守卫，转义私有名同样要解码）。
8. **测试直接构造**：`tests/parameters.rs:471-474`、`class/private.rs:323`、
   oracle 的 `Lexer::new + raw` 依赖，需同步适配且保持 `raw` 语义。

## 5. 验收矩阵与目标

度量命令（每次改动/每阶段）：

```sh
# 编译吞吐（真实 bundle + 生成语料）
python3 scripts/benchmark/compile_matrix.py --metric compile ... --repeat 5
# 指令/缓存/RSS（functions/expressions 4MB，tiny 基线扣除）
perf stat -x, -e instructions,cycles,branches,branch-misses,cache-references,cache-misses,task-clock -- <probe> FILE
# 分配计数（P0 工具；构建见 docs/compile-benchmark.md §9.4）
target/p0-alloc-probe/target/release/oxide-compile-alloc-probe FILE
# 语义 gate
cargo test --locked --workspace --all-targets
./scripts/test262/test-test262.sh --spec dev-support/test262/current.conf --check
./scripts/quickjs/test-quickjs-fixtures.sh --validate
./scripts/quickjs/test-quickjs-c-oracles.sh --validate
```

阶段性目标（`functions-4194304`，相对基线 1789ms / 2.90M instr/KB / 17.1k miss/KB / 100.3 MB/源MB）：

| 阶段 | 时间 | instr/KB | cache-miss/KB | 分配次数 |
| --- | ---: | ---: | ---: | ---: |
| P1a | −15% | −15% | −30% | −60% |
| P1b | −35% | −30% | −60% | −85% |
| P2 | −50% | −45% | −75% | −90% |
| P3 | −55% | −50% | −80% | −90% |

P2 分 P2a（前瞻备忘）与 P2b（提交缓冲）两步，表中 P2 行为两步合并目标。

真实 bundle 中位吞吐目标：从 4.79 MB/s 到 >7 MB/s（P2 后 >6 MB/s），并保持
test262/QuickJS 差分全绿。目标值是方向性检查点，P0 基线出来后按实测校准；
P1a 首个 checkpoint（分配探针 + perf）用于校准后续阶段的数字。

P1a checkpoint 实测（`docs/compile-benchmark.md` §9.5，HEAD `a5b651be`）：
alloc 次数 −13.0%~−19.5%、alloc+realloc −13.8%~−21.7%、instr/KB
−3.6%~−4.9%、cache-miss/KB −8.7%~−17.5%、task-clock −5.6%~−9.3%、RSS
−15.7%~−21.2%。方向正确但低于表中方向性目标：4MB 语料的分配大头在
parse 之后的 IR/常量/绑定路径，lexer 侧每 token 分配消除只覆盖一部分。
后续阶段分配目标按“相对上一 checkpoint 再降”执行（P1b ≥70% 相对 P1a），
instr/miss/时间目标保持“相对 P0 基线”方向；每阶段 checkpoint 后更新
§9.5 对照。

P1b checkpoint 实测（`docs/compile-benchmark.md` §9.6，HEAD `bd3eb461`）：
alloc 次数相对 P1a −9.6%~−16.9%、instr/KB −0.9%~−6.2%、cache-miss/KB
−18.6%~−68.5%、task-clock −7.5%~−16.5%、RSS −12.4%~−18.5%；相对 P0
基线 alloc −22.4%~−32.4%、instr −4.9%~−9.6%、cache-miss −32.9%~−71.2%。
仍低于 P1b 方向目标：分配大头在 IR/常量/绑定/字节码路径（名字字符串仅约
1/6），per-NameTable `JsString` 缓存只再贡献约 1–4 个百分点；P2/P3 分配
目标继续按“相对上一 checkpoint 再降”执行，P2 后若分配仍为瓶颈需重估。

## 6. 提交与分支

- 从 `feat/lexerparser-boaquickjsv8`（benchmark 与 issue 证据所在分支）拉出
  `feat/lexer-parser-refactor`。
- 提交粒度：P0 工具/基线 1–2 个；P1a 3–5 个（Copy 化 → 惰性解码 → 消费点 → 清 clone）；
  P1b 4–6 个（NameTable → IR 字段 → parser → resolution/lowering → 诊断/测试）；
  P2a 1–2 个（前瞻备忘）+ P2b 3–5 个（缓冲核心 → 逐产生式迁移）；P3 2–4 个。
- 每个提交可编译、可跑 focused gate；每阶段结束跑全量 gate + 矩阵并更新
  `docs/compile-benchmark.md` 的指标表。

## 7. 交付物

- 代码：上述阶段全部落地，`code`/API/VM 无行为变化。
- 工具：`compile_alloc_probe`（分配计数）、更新后的矩阵/剖析口径。
- 文档：本计划 + `docs/compile-benchmark.md` 指标更新 + issue #32 进度勾选。
- 度量：每阶段一份基线对比（MB/s、instr/KB、miss/KB、RSS/源MB、分配次数）。

## 附录 A P1a 逐点执行清单

> 行号基于 0e59f836；执行前用
> `rg -n 'identifier\.value|\.value\.utf16|\.cooked|\.raw_value|current\(\)\.clone|kind\.clone' src/engine/compiler`
> 复核，行号漂移以符号为准。

### A.1 类型变更（最终形态）

- `NumberLiteral<'a>`/`RegExpLiteral<'a>`：derive 增加 `Copy`（字段已全 Copy）。
- `Identifier<'a> { raw, has_escape, keyword_hint, escaped_reserved_word }`（删
  `value`），derive `Copy`。
- `StringLiteral<'a> { raw, has_escape, has_legacy_octal_escape }`（删 `value`、
  `quote`），derive `Copy`。
- `TemplateEscapeError { message: &'static str, span: Span }`，derive `Copy`。
- `TemplatePart<'a> { raw, kind, invalid_escape: Option<TemplateEscapeError> }`
  （删 `raw_value`/`cooked`），derive `Copy`。
- `TokenKind`/`Token` derive `Copy`；`LexError`/`LexErrorKind` 不进 token，保持现状。
- `Quote` 枚举在 `quote` 字段删除后无引用，删除。

### A.2 sink 与解码

- `StringSink`/`ValidateSink`/`Utf16Sink` 按 §2.1 第 3 点实现。
- `scan_string`（1337-1435）：`value: JsString` 改 sink 参数；错误构造与 span 全部
  保持（含 1388-1411 的 re-anchor、1417-1432 的 `StringTooLong` 位置）。
- `scan_template`（1650-1814）：拆出
  `scan_template_into(initial, raw_sink, cooked_sink)`，Token 包装层构造
  `TemplatePart`。保持：raw 先于 cooked 检查（1759 先于 1769）、事务性 clone 探测
  （1745-1758）、首次 `invalid_escape` 优先（1774）、`cooked=None` 后继续累积 raw、
  1,5,6 三个 kind 的 raw 边界（backtick/`${` 前）。
- `append_template_raw_source`（1816-1826）：`semantic_utf16_units` 的 `Vec<u16>`
  改流式写 sink；`append_template_raw_units`（2309-2326）的 CRLF→LF 保持。
- `scan_identifier`（935-1092）：删 `value`；`has_escape` 分支为
  `keyword_from_str`（1071）临时组装 decoded `String`，非转义分支直接用 `raw`；
  962-991 的回滚与 946 的 `#` 长度计数不变。
- Parser 解码方法（§2.1 第 4 点）：`self.lexer.clone()` + `seek` + sink 扫描；
  模板按 `part.kind` 决定 `initial`（Head/NoSubstitution 从 backtick，Middle/Tail
  从 `}`）；只对已提交 token 调用。
- `parse_number`（parser/literals.rs:409-437）：`raw.replace('_', "")` 改 `Cow`
  （无 `_` 借用原切片）；BigUint 快路留 P3，行为不变。
- `parser/literals.rs:244` 等诊断继续用 `identifier.raw`（源拼写）。

### A.3 cooked/raw_value 消费点（改调用解码方法）

- 字符串 cooked：parser/literals.rs:105；parser/tokens.rs:542（directive，可先
  `has_escape` 短路）；class.rs:468；destructuring.rs:1975；object_literal.rs:128,310；
  module.rs:414,437,461,712,715（712+715 同一值解码一次复用）。
- 模板：untagged `template.rs:40-50`（解码 cooked；`invalid_escape` 提供诊断
  span/message）；tagged `template.rs:135-140`（cooked 可 `None`，raw_value 必解码）。
- `has_legacy_octal_escape` 预检查保持原位（class.rs:461、destructuring.rs:1967、
  object_literal.rs:121,303、parser/literals.rs:98）。

### A.4 转义标识符比较点（30 处，改 `identical_name`）

- arrow.rs:465；calls.rs:105,314；class.rs:309,313,493,516；
  destructuring.rs:582,1452,1686,2107,2253,2263；parser/diagnostics.rs:34；
  function.rs:245,265；parser/literals.rs:122；parser/loops.rs:518,607；
  parser/statements.rs:577,822；parser/tokens.rs:58,107,292,368,634；
  module.rs:729；object_literal.rs:108-109（162-169/185/188 的 `method_prefix`
  局部 String 比较保持现状）。
- 实现为 Parser 方法（需要解码能力）：

  ```rust
  fn identical_name(&self, identifier: &Identifier<'_>, expected: &str) -> bool {
      self.identifier_text(identifier) == expected
  }
  ```

  必须含转义解码分支；禁止 `raw == expected && !has_escape`。

### A.5 move/绑定/渲染点（P1a 用 `identifier_text`；P1b 换 `NameId`）

- arrow.rs:143,209；calls.rs:281；class.rs:104,454,503；declarations.rs:895,976；
  destructuring.rs:1460,1687,1942,2111,2268,2277,2282,2303,2326,2334；
  function.rs:421,559,723；module.rs:320,438,491,700,717,909；
  object_literal.rs:101,268,296；optional_chain.rs:99,107；parser/control.rs:30；
  parser/expressions.rs:124,1085,1093；parser/literals.rs:129,138；
  parser/loops.rs:524,577；parser/statements.rs:578,828,914；parser/tokens.rs:327；
  private_reference.rs:106；parser/diagnostics.rs:56,70。
- 转义场景 `identifier_text` 返回 `Cow::Owned`（解码一次）；非转义 `Borrowed`。
- `private_binding_name(&text)` 在解码之后拼 `#`（顺序不影响语义，名字内容必须一致）。

### A.6 token 复制清理（31 + 5 + 3 处）

- `current().clone()` 31 处：template.rs:34,129；private_reference.rs:102；
  optional_chain.rs:96；object_literal.rs:96,292；function.rs:553；
  destructuring.rs:1655,1939,2084,2234；module.rs:301,410,435,445,475,698,709；
  class.rs:81,450；arrow.rs:136,196；parser/expressions.rs:1082；
  parser/loops.rs:508,566；parser/statements.rs:563,812,903；parser/calls.rs:101,273；
  parser/literals.rs:48。
- `.kind.clone()` 5 处：destructuring.rs:1940,2085,2235；function.rs:172；class.rs:96。
- 计划外整 token 复制 3 处：destructuring.rs:676（P2b 改切片）、
  object_literal.rs:106、function.rs:558（Copy 后成本仅拷贝）。
- 改法：`Token` Copy 后直接按值/引用持有；比较与 move 按 A.4/A.5。

### A.7 标识符标志/关键字规则（不得改变）

- `keyword_hint` 基于解码值（1070-1072）；`escaped_reserved_word =
  has_escape && active_keyword.is_some()`（1078）；`keyword_is_active`（1111-1121）
  的 strict/module/generator/async 门控不变。
- 失败尾随 `\u` 回滚后 `raw` 不含反斜杠但 `has_escape` 保持 true（962-991）。
- 消费期解码的 `strict` 只影响 `\0-\7`/`\8\9` 分支，按 `has_legacy_octal_escape`
  反推；`source_text` 经 clone 保留（孤立代理项）。
- 合成 `Identifier` 2 处（function.rs:188-194,207-213）改为
  `{ raw: "yield"/"await", has_escape: false, keyword_hint: Some(...),
  escaped_reserved_word: false }`。

### A.8 测试适配与新增

- lexer 内 19 个读 payload 的测试：字符串 5（2684,2701,2810,3054,3194）、
  模板 6（2810,3066,3106,3122,3182,3264）、标识符 8（2421,2451,2479,2543,
  2631,3026,3410,3453）、数字 2（2584,2870）、regexp 2（2761,2810）——
  改为解码 helper/flag 断言；错误与元数据测试不动。
- `tests/parameters.rs:471-509`、`class/private.rs:323-363` 只构造 Parser + token，
  预计零改。
- oracle `oracle_unicode_identifiers.rs:348,358` 保持 `identifier.raw`。
- 新增：`decode_* ≡ 旧 value` 对偶测试（复用上述语料）；`identical_name` 三种形态
  （`if`/`\u0069f`/`if\u{}`）；tagged/untagged `invalid_escape` span/message 快照。

## 附录 B P1b 逐点执行清单

### B.1 `NameTable`/`NameId`

按 §2.2：`FunctionTree.names: NameTable`；parser 唯一 `intern`；
resolution/lowering 只读 `name/lookup`；parser 预驻留 resolution 需要的合成名。

### B.2 字段迁移表（String → NameId）

| 位置 | 字段 | 处理 |
| --- | --- | --- |
| model/ir.rs:159,165,175 | `IrOp::{Identifier,IdentifierReference,PrivateField}.name` | `NameId`（PrivateField 存当前含 `#` 的合成名） |
| model/ir.rs:141,150 | `DynamicIdentifier*{name: u32}` | 不变（常量索引） |
| model/ir/function.rs | `function_name/parameters/parameter_names/locals/eval_redeclaration` | `Option<NameId>`/`Vec<Option<NameId>>`/`Vec<NameId>` |
| model/bindings.rs:176,200,234 | `IrBinding/IrGlobalDeclaration/IrEvalDeclaration.name` | `NameId` |
| model/scope.rs:47 | `bindings_by_name` | `HashMap<NameId, BindingId>` |
| compiler/module.rs:68,78 | `IrModuleBinding.name`/`IrModuleLocalExport.local_name` | `NameId` |
| compiler/module.rs | `export_name` | 保持 `JsString`（可为任意字符串） |
| parser/context.rs:104,113 | `MemberReference::{Private,IdentifierReference}.name` | `NameId` |
| parser/context.rs:220 | `label_name` | 保持 `String`（parser 局部冷路径，缩爆炸半径） |
| function.rs:34,29 | `FunctionDefinitionHeader.name` | 保持 `Identifier`（Copy），421/723 转 `NameId` |
| destructuring.rs:53 | `ObjectBindingPropertyKey::Fixed.shorthand` | 保持 `Identifier`（判定 escaped_reserved_word） |
| 属性键/字符串边界 | class.rs:454、destructuring.rs:1942、object_literal.rs:114,296、module.rs:438,717、parser/statements.rs:850,947、destructuring.rs:1430 | `JsString::try_from_utf8(names.name(id))`，保持 `JsString` |
| constants | `IrConstant::Primitive(Value::String)`、`string_constants` | 不变 |

### B.3 合成名（intern 时机保住原相对顺序）

- `pseudo_binding.rs:21-24`（4 个）、`parser/literals.rs:72`、
  `parser/calls.rs:122-126,202,208,228,233`、`parser/parameters.rs:288`、
  `class/fields.rs:113-117`、`class/static_block.rs:48`、`class.rs:206-209`、
  `function.rs:188-213`、`module.rs:668-684,837-866,912`、
  `private_reference.rs:32-44`、`parser/entry.rs:228`、
  `resolution.rs:352,356,382`（`"arguments"`）、`bindings.rs:20-24`。
- `MODULE_IMPORT_META_BINDING_NAME` 在 module goal 解析期预驻留（resolution.rs:168
  当前 `to_owned` 改为 `lookup` + invariant 断言）。
- `entry.rs:306-453` eval external bindings 的 `JsString → String → intern` 顺序
  不变（仍在 parse 期、resolution 前完成，含 412/422/432 的畸形名校验）。

### B.4 常量/Atom 顺序保护

- `append_constant`（model/ir/function.rs:529-542）以 `constants` Vec 序号为准，
  `string_constants` 首次出现去重；`tests/scopes.rs:4-25` 钉死字面量与 ensure 的
  交错顺序。
- parser 解析期先追加属性键/数组索引常量，resolution 期才 `ensure_string_constant`；
  **NameTable 不得追加常量**，也不得改变 `ensure_string_constant` 的调用时机。
- 14 个调用点相对顺序（A3 审计）：parser/entry.rs:382 → resolution.rs:1026 →
  1067 → private_reference.rs:233 → resolution.rs:1666 → 2259 → 2377 → 2413 →
  2715 → 2788 → 2807 → 2913 → 1301 → 1325；改造后仍以同样顺序调用。
- per-NameTable `JsString` 缓存只做 `name → JsString` 复用，不替代
  `string_constants` 的 get/entry 语义（首个字面量仍优先命中）。
- 守卫：`tests/scopes.rs:4-25,28-40`、`check-bc5-pinned-atoms`、fixtures 字节一致性。

### B.5 哈希审计结论

编译器内所有 name 相关 map 均只 get/insert/entry，无迭代驱动输出：
`bindings_by_name`（scope.rs:52 / function.rs:568,613-614 / 610 clear）、
`string_constants`（resolution.rs:2430 / function.rs:538）、module.rs:1027 局部
map、lowering 测试 map。切 `HashMap<NameId, BindingId>` **无字节序风险**；唯一
义务是同一解码名必得同一 `NameId`。整数键换 `rustc-hash` 属 §2.4。

### B.6 诊断渲染

`parser/diagnostics.rs:56,70,84-104`；`private_reference.rs:252-256`；
`module.rs:1039`；`destructuring.rs:509`；`class/fields.rs:175-177`。parser 期
`self.names.name(id)`，resolution/lowering 期 `tree.names.name(id)`；错误构造
时机与文案不变。

### B.7 测试适配

- 约 308 个 compiler 测试的 `.name` 断言：加
  `fn name<'a>(tree: &'a FunctionTree, id: NameId) -> &'a str` 助手后机械替换。
- 手工构造 IR 的测试（lowering.rs:1547、tests/scopes.rs:165,175,209 等）需
  NameTable/构造助手。
- 关键回归：tests/scopes.rs:4-25,28-40；tests/debug.rs:292,321；oracle 全量；
  fixtures 字节一致；focused test262。

## 附录 C P2 状态机结论与分步

### C.1 现状不变量（P2a/P2b 都必须保持）

1. `tokens.len() == cursor + 1` 恒成立；`set_future_lex_context` 的 truncate 现为
   no-op；`advance_expression_start` 的 truncate/seek 死分支不得复活。
2. relex 保留已观测的 `line_terminator_before`（tokens.rs:479/483、505/510），
   rescan 从 `span.start`（trivia 之后）；`span.start` 跨 goal 稳定（`/` 与 RegExp
   同起点，模板续段起点是 `}`）。
3. `relex_current_with_context` 固定回 `Div` goal（tokens.rs:454-456,509）；
   arrow.rs:240-245 依赖之后 `parse_primary` 重新选 RegExp。
4. EOF：`advance_with_goal` 在 current 为 Eof 时 no-op（447）；EOF 可重扫，
   span=len..len，bit 由 relex 保留；错误后 `tokens` 短于 cursor 时不得粘住
   failed 状态。
5. `set_future_lex_context` 不改当前 token（arrow.rs:48-51 的 `async await =>`
   不对称性），只影响其后扫描。
6. `TemplateContinuation` 条目必须带 goal 缓存；绝不能用 `Div` 从 `}` 重扫。
7. 历史/任意索引读取：statements.rs:846（`tokens[cursor-1]`）、arrow.rs:296-300、
   tokens.rs:530（`tokens[start]`）。
8. 探针错误合约：吞掉错误的站点 1,2,4,6,9,11,12,13,14；传播的站点
   3,5,7,8,15,16,17。缓存不得把吞掉的 LexError 变硬错误，也不得缓存失败扫描。
9. 裸源探测不进缓存：`next_token_is_for_of_keyword`（tokens.rs:65-70）、
   `quickjs_simple_lookahead_has_line_terminator`（lexer.rs:2058）。
10. `advance_expression_start` 的 goal 选择发生在该 entry 首次扫描之前；entry 的
    bit 来自该次扫描。
11. 探针最多各 clone 独立、目标机相同：goal 机（regex/template 重扫）都在 clone
    内自洽；唯一写回提交状态的是 7（`advance_expression_start`）。
12. 探针距离与深度上限按站点保持：`parenthesized_parameter_tokens` 无 255 上限，
    站点 14（`binding_pattern_scan`）有；不得统一。

### C.2 P2a `LookaheadCache`

```rust
struct LookaheadCache<'a> {
    entries: Vec<LookaheadEntry<'a>>, // 按 start_offset 有序
}
struct LookaheadEntry<'a> {
    start: usize,
    goal: LexicalGoal,
    context: LexContext,
    token: Token<'a>,
}
```

- 查询 `peek(start, goal, context) -> Option<Token>`；未命中由调用方按现逻辑
  clone 扫描并 `insert`；命中直接返回（Token 为 Copy）。
- 失效：`set_future_lex_context`/`relex_current_with_*` 清 `start >= 边界`；
  `advance` 清 `start <= current().span.start` 的已提交区间。
- 命中/未命中计数放 `#[cfg(feature = "profiling")]`，仅诊断。
- 迁移顺序：先单 token 简单探针（3-7、10-12、15-17），再 two-token following
  （13/14），最后整头扫描（1、2、9、13 主体）；每步跑对应 oracle/单元测试。

### C.3 P2b `TokenBuffer`

- `entries: Vec<{ token, goal, context, start }>` + `cursor`；提交 entry 不可变，
  relex 时整体替换并保留 bit。
- `ensure(index, goal, context)`：三者匹配则复用；否则 truncate 到 index 并从
  `entries[index].start`（或 lexer 当前位置）重扫；EOF 粘性由调用方保留。
- `mark/restore`：记录 `entries.len()`/cursor，restore 只回退游标、不清条目；
  下次 goal/context 匹配即可复用。
- `parenthesized_parameter_tokens` 改 `&entries[a..b]`；
  `object_binding_has_rest` 二次重扫改复用。
- 旧 clone 路径保留 test-only 差异测试（C.4）。

### C.4 P2 新增测试

- 缓存复用：同 `(start, goal, context)` 二次请求不再扫描（计数断言）；不同
  goal/context 强制重扫且 span/bit 一致。
- 失效：`set_future_lex_context` 后 `cursor+1` 按新 context 分类（`await`/`yield`
  用例）；`Divide`→`RegExp` 替换；模板续段条目不被 `Div` 复用。
- EOF：越过不前进；relex EOF bit 保持。
- 差异测试：17 处探针在迁移期与旧实现逐 token 对拍。
- 既有回归重点：`apps/cli/tests/oracle/arrow_functions.rs`、
  `parameters/oracle_parameter_binding_patterns.rs`、
  `control_flow/oracle_for_*.rs`、`templates/`、`regexp/`；`tests/syntax.rs:392`、
  `tests/async_functions.rs:746`、`tests/parameters.rs:398,426,465`、
  `lexer.rs:3066-3342`。

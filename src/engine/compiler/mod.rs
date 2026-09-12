//! Source-to-bytecode compilation with late lexical-name resolution.
//!
//! QuickJS first emits scope-variable operations, then `resolve_scope_var`
//! rewrites them after every nested function and lexical scope is known.  Its
//! `get_closure_var` helper also installs relay closure slots on intervening
//! functions.  This module keeps the same boundary in typed form: parsing emits
//! `IrOp`s into a recursive `FunctionIr` arena, identifier resolution runs
//! child-first, and only then are VM instructions and recursive unlinked
//! function constants produced. The parser owns its Lexer and requests tokens
//! through fallible advances, so an error on later source cannot preempt a
//! diagnostic on the current token. Directive-prologue probes clone and seek
//! the lexer, then the committed stream is rescanned under its strict context.

pub mod lexer;
mod model;
mod parser;
#[cfg(test)]
use model::bindings::EvalDeclarationMode;
#[cfg(test)]
use model::bindings::EvalDeclarationTarget;
#[cfg(test)]
use model::bindings::EvalDeclarationValue;
#[cfg(test)]
use model::bindings::IrAnnexBinding;
#[cfg(test)]
use model::ir::CallArguments;
#[cfg(test)]
use crate::engine::code::bytecode::ApplyKind;
#[cfg(test)]
use crate::engine::compiler::lexer::LexerOptions;
#[cfg(test)]
use crate::engine::compiler::lexer::quickjs_simple_lookahead_is_of;
#[cfg(test)]
use pseudo_binding::HOME_OBJECT_LOCAL_NAME;
#[cfg(test)]
use pseudo_binding::NEW_TARGET_LOCAL_NAME;

use parser::builder::FunctionBuilder;
use model::bindings::{BindingId, BindingKind, BindingStorage, IrBinding, IrEvalDeclaration, IrGlobalDeclaration, IrHoistedFunction, IrProgramAnnexFunction, IrScopedFunction, SyntheticLocal, SyntheticLocalKind, binding_kind_from_closure_flags, binding_kinds_compatible};
use model::ir::{FunctionId, IdentifierAccess, IdentifierReferenceAccess, IrConstant, IrOp, PrivateFieldAccess, SpannedIrOp};
use model::scope::{IrScope, ScopeId, ScopeKind};

mod scope_validation;
use scope_validation::validate_scope_graph;
mod resolution;
#[cfg(test)]
use resolution::ensure_closure_variable;
use resolution::{ResolvedBinding, apply_quickjs_late_throw_sites, capture_binding_path, ensure_string_constant, find_or_create_own_binding, insert_hoist_fragment, ordered_hoisted_functions, prepend_hoist_prefix, push_closure_variable, resolve_identifiers};
mod lowering;
use crate::engine::api::error::{Error, ErrorKind, NativeErrorMessage};
#[cfg(test)]
use crate::engine::atom::AtomTable;
#[cfg(test)]
use crate::engine::code::bytecode::DetachedBytecode;
use crate::source::{SourceLocation, SourceSpan};

use crate::engine::code::bytecode::{ArgumentsKind, DynamicEnvironmentSource, EvalVariableSource, Instruction, MAX_LOCAL_SLOTS, PrivateNameSource, WithObjectSource, verify_parts};
use crate::engine::code::bytecode_validation::quickjs_copies_defined_argument_count;
use crate::engine::code::debug::{DebugInfoMode, Pc2LineEntry, Pc2LineTable};
use crate::source::{QuickJsSourceLocator, SourceOffset};

use crate::engine::code::function::metadata::{ClassInitializerKind, ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName, ConstructorKind, EvalBinding, EvalBindingSource, EvalCallerProfile, EvalCallerVariableTarget, EvalEnvironment, EvalKind, EvalRootBinding, EvalScope, EvalScopeKind, EvalVariableEnvironment, FunctionKind as BytecodeFunctionKind, FunctionMetadata, ParameterArgumentCell, ParameterBodyStorage, ParameterDefaultSource, ParameterEnvironmentLayout, ParameterPatternCopy};
use crate::engine::code::function::{UnlinkedConstant, UnlinkedFunction, UnlinkedFunctionDebug, UnlinkedVariableDefinition};
use crate::engine::code::module::{ModuleImportAttribute, ModuleRequest, UnlinkedModule};
use crate::engine::compiler::lexer::{Identifier, Keyword, LexContext, LexError, LexErrorKind, Lexer, LexicalGoal, NumberKind, NumericRadix, Punctuator, Span, TemplatePartKind, Token, TokenKind};
use crate::engine::value::bigint::JsBigInt;
use crate::engine::value::{JsString, JsStringError, PrimitiveValue as Value};
use crate::source::text::SourceText;
#[cfg(test)]
use lowering::lower_detached_script;
use lowering::lower_unlinked_tree;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use std::collections::HashMap;
use std::ops::Range;

mod arrow;
mod class;
mod destructuring;
mod function;
mod generator;
mod module;
mod object_literal;
mod optional_chain;
mod private_reference;
mod pseudo_binding;
mod template;

use pseudo_binding::{ACTIVE_FUNCTION_LOCAL_NAME, PseudoBinding, THIS_LOCAL_NAME, ensure_eval_visible_pseudo_bindings, find_or_create_own_pseudo_binding, function_owns_pseudo_binding, install_pseudo_binding_prologues};

/// Default filename used by the Rust convenience compile/eval APIs.
pub const DEFAULT_EVAL_FILENAME: &str = "<input>";

/// Named source compilation options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileOptions {
    pub filename: String,
}

impl CompileOptions {
    #[must_use]
    pub fn new(filename: impl Into<String>) -> Self {
        Self {
            filename: filename.into(),
        }
    }
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self::new(DEFAULT_EVAL_FILENAME)
    }
}

/// Internal compilation context for one synthetic eval root.
///
/// Direct eval imports the exact live caller bindings described by R1w.
/// Indirect eval has no external bindings and resolves against the defining
/// realm's global environment. `caller_strict` is ignored for indirect eval,
/// matching QuickJS's `JS_EVAL_TYPE_INDIRECT` path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalCompileContext {
    pub kind: EvalKind,
    pub caller_strict: bool,
    pub bindings: Box<[EvalRootBinding<JsString>]>,
    pub caller_profile: EvalCallerProfile,
    pub super_call_allowed: bool,
    pub super_allowed: bool,
    pub arguments_forbidden: bool,
}

impl EvalCompileContext {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn direct(caller_strict: bool, bindings: Vec<EvalRootBinding<JsString>>) -> Self {
        let scope_count = bindings
            .iter()
            .map(|binding| usize::from(binding.scope) + 1)
            .max()
            .unwrap_or(0);
        let mut scope_kinds = vec![EvalScopeKind::FunctionRoot; scope_count];
        for binding in &bindings {
            if binding.kind == ClosureVariableKind::WithObject {
                scope_kinds[usize::from(binding.scope)] = EvalScopeKind::With;
            } else if binding.is_catch_parameter {
                scope_kinds[usize::from(binding.scope)] = EvalScopeKind::Catch;
            }
        }
        for (scope, scope_kind) in scope_kinds.iter_mut().enumerate() {
            let has_parameter_object = bindings.iter().any(|binding| {
                usize::from(binding.scope) == scope
                    && binding.kind == ClosureVariableKind::ArgEvalVariableObject
            });
            let has_body_object = bindings.iter().any(|binding| {
                usize::from(binding.scope) == scope
                    && binding.kind == ClosureVariableKind::EvalVariableObject
            });
            if has_parameter_object && !has_body_object {
                *scope_kind = EvalScopeKind::Parameter;
            }
        }
        let variable_target = if caller_strict {
            EvalCallerVariableTarget::StrictLocal
        } else {
            bindings
                .iter()
                .position(|binding| {
                    matches!(
                        binding.kind,
                        ClosureVariableKind::EvalVariableObject
                            | ClosureVariableKind::ArgEvalVariableObject
                    )
                })
                .and_then(|index| u16::try_from(index).ok())
                .map(EvalCallerVariableTarget::ExternalBinding)
                .unwrap_or(EvalCallerVariableTarget::Global)
        };
        Self::direct_with_profile(
            caller_strict,
            bindings,
            EvalCallerProfile {
                scope_kinds: scope_kinds.into_boxed_slice(),
                variable_target,
            },
            false,
            false,
        )
    }

    pub fn direct_with_profile(
        caller_strict: bool,
        bindings: Vec<EvalRootBinding<JsString>>,
        caller_profile: EvalCallerProfile,
        super_call_allowed: bool,
        super_allowed: bool,
    ) -> Self {
        Self {
            kind: EvalKind::Direct,
            caller_strict,
            bindings: bindings.into_boxed_slice(),
            caller_profile,
            super_call_allowed,
            super_allowed,
            arguments_forbidden: false,
        }
    }

    pub fn direct_with_profile_and_arguments(
        caller_strict: bool,
        bindings: Vec<EvalRootBinding<JsString>>,
        caller_profile: EvalCallerProfile,
        super_call_allowed: bool,
        super_allowed: bool,
        arguments_forbidden: bool,
    ) -> Self {
        let mut context = Self::direct_with_profile(
            caller_strict,
            bindings,
            caller_profile,
            super_call_allowed,
            super_allowed,
        );
        context.arguments_forbidden = arguments_forbidden;
        context
    }

    pub fn indirect() -> Self {
        Self {
            kind: EvalKind::Indirect,
            caller_strict: false,
            bindings: Box::new([]),
            caller_profile: EvalCallerProfile {
                scope_kinds: Box::new([]),
                variable_target: EvalCallerVariableTarget::Global,
            },
            super_call_allowed: false,
            super_allowed: false,
            arguments_forbidden: false,
        }
    }
}

/// Compile one ECMAScript script directly to stack bytecode.
///
/// # Errors
/// Returns a syntax error for invalid source and an unsupported diagnostic for
/// grammar which has not yet reached the feature-parity implementation path.
#[cfg(test)]
pub fn compile_script(source: &str) -> Result<DetachedBytecode<Value>, Error> {
    let mut tree = Parser::parse(source, JsString::from_static(DEFAULT_EVAL_FILENAME))?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    if tree.functions.len() != 1 {
        return Err(Error::unsupported(
            "nested function bytecode requires runtime publication; use Context::compile or Context::eval",
            source_span(tree.functions[1].source.span),
        ));
    }
    lower_detached_script(tree)
}

/// Compile a script into a runtime-independent draft ready for publication.
///
/// Runtime publication uses this production boundary to keep primitive
/// constants structural and to carry execution metadata into the heap node.
/// The older detached compiler result is test-only.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_script(source: &str) -> Result<UnlinkedFunction, Error> {
    compile_unlinked_script_with_filename(source, DEFAULT_EVAL_FILENAME, DebugInfoMode::Full)
}

pub fn compile_unlinked_script_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse(source, JsString::try_from_utf8(filename)?)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Reject source buffers which cannot fit QuickJS's signed debug-source
/// length before any byte-oriented carrier allocation is attempted.
pub fn validate_source_length(source_len: usize) -> Result<(), Error> {
    i32::try_from(source_len).map(|_| ()).map_err(|_| {
        Error::new(
            ErrorKind::JsInternal,
            "source is too large for QuickJS debug metadata",
        )
    })
}

/// Compile one explicitly sized Script buffer after applying QuickJS's signed
/// source-length guard, before allocating its byte-exact carrier.
pub fn compile_unlinked_script_bytes_with_filename(
    source: &[u8],
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    validate_source_length(source.len())?;
    let source = SourceText::try_from_raw_bytes(source)?;
    compile_unlinked_script_source_with_filename(&source, filename, debug_info)
}

/// Compile one byte-exact Script carrier for the public byte-oriented
/// embedding boundary.
pub fn compile_unlinked_script_source_with_filename(
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_script_source(source, JsString::try_from_utf8(filename)?)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Compile one ECMAScript module into its runtime-independent module record.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_module_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    compile_unlinked_module_with_name(source, JsString::try_from_utf8(filename)?, debug_info)
}

/// Compile a module whose host-resolved identity may contain any ECMAScript
/// String code unit, including lone UTF-16 surrogates.
pub fn compile_unlinked_module_with_name(
    source: &str,
    name: JsString,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, Error> {
    compile_unlinked_module_with_name_and_attribute_checker(source, name, debug_info, None)
        .map_err(ModuleCompileFailure::into_engine_without_checker)
}

/// Synchronous host validation performed as soon as one non-empty static
/// import-attribute clause has been parsed.
///
/// Pinned QuickJS invokes `JSModuleCheckAttributes` before consuming the
/// clause's closing brace and before parsing any following source. Returning
/// an error here therefore must stop parsing immediately.
pub trait ModuleImportAttributeChecker {
    /// Observe one source-order request after it has entered the parser-owned
    /// requested-module table.
    ///
    /// Called after decoding attributes and before checking them, while the
    /// closing brace is still current.
    fn publish_request(&mut self, request: &ModuleRequest) -> Result<(), ModuleCompileFailure>;

    fn check(&mut self, attributes: &[ModuleImportAttribute]) -> Result<(), ModuleCompileFailure>;
}

/// Abrupt completion of the checked module-compilation path.
///
/// Ordinary compiler diagnostics remain [`Error`] values. The distinct
/// host-abort arm stops parsing after a module attribute callback throws.
/// The caller retains the exact JavaScript value; the compiler never owns
/// an engine handle or converts that value into a diagnostic.
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleCompileFailure {
    Engine(Error),
    Host,
}

impl From<Error> for ModuleCompileFailure {
    fn from(error: Error) -> Self {
        Self::Engine(error)
    }
}

impl From<JsStringError> for ModuleCompileFailure {
    fn from(error: JsStringError) -> Self {
        Self::Engine(error.into())
    }
}

impl ModuleCompileFailure {
    fn into_engine_without_checker(self) -> Error {
        match self {
            Self::Engine(error) => error,
            Self::Host => Error::internal(
                "module compiler produced a host throw without an attribute checker",
            ),
        }
    }
}

/// Compile a module while exposing QuickJS's parse-time attribute-check hook.
pub fn compile_unlinked_module_with_name_and_attribute_checker(
    source: &str,
    name: JsString,
    debug_info: DebugInfoMode,
    checker: Option<&mut dyn ModuleImportAttributeChecker>,
) -> Result<UnlinkedModule, ModuleCompileFailure> {
    let tree = Parser::parse_module(source, name, checker)?;
    finish_unlinked_module_tree(tree, debug_info)
}

/// Compile one explicitly sized Module buffer after applying QuickJS's signed
/// source-length guard, before allocating its byte-exact carrier.
pub fn compile_unlinked_module_bytes_with_name_and_attribute_checker(
    source: &[u8],
    name: JsString,
    debug_info: DebugInfoMode,
    checker: Option<&mut dyn ModuleImportAttributeChecker>,
) -> Result<UnlinkedModule, ModuleCompileFailure> {
    validate_source_length(source.len())?;
    let source = SourceText::try_from_raw_bytes(source)?;
    let tree = Parser::parse_module_source(&source, name, checker)?;
    finish_unlinked_module_tree(tree, debug_info)
}

fn finish_unlinked_module_tree(
    mut tree: FunctionTree,
    debug_info: DebugInfoMode,
) -> Result<UnlinkedModule, ModuleCompileFailure> {
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error.into());
    }
    let name = tree.filename.clone();
    let module = tree
        .module
        .take()
        .ok_or_else(|| Error::internal("module compiler produced no module record"))?;
    let has_top_level_await = tree.functions.first().is_some_and(|function| {
        function
            .ops
            .iter()
            .any(|operation| matches!(operation.op, IrOp::Bytecode(Instruction::Await)))
    });
    let function = lower_unlinked_tree(tree, debug_info)?;
    module::finish_module(name, function, has_top_level_await, module).map_err(Into::into)
}

/// Compile one primitive-String eval as an independent synthetic root.
///
/// This deliberately does not reuse the ordinary Script root. QuickJS gives
/// eval its own body environment and, for direct eval, attaches that root to
/// the active caller's VarRefs only while publishing and executing it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn compile_unlinked_eval_with_filename(
    source: &str,
    filename: &str,
    debug_info: DebugInfoMode,
    context: EvalCompileContext,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_eval(source, JsString::try_from_utf8(filename)?, context)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

/// Compile dynamic eval source whose carrier may represent lone UTF-16
/// surrogates. Public Rust `&str` compilation remains on the ordinary wrapper
/// above; only JavaScript String eval uses this reversible internal boundary.
pub fn compile_unlinked_eval_source_with_filename(
    source: &SourceText,
    filename: &str,
    debug_info: DebugInfoMode,
    context: EvalCompileContext,
) -> Result<UnlinkedFunction, Error> {
    let mut tree = Parser::parse_eval_source(source, JsString::try_from_utf8(filename)?, context)?;
    resolve_identifiers(&mut tree)?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    lower_unlinked_tree(tree, debug_info)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParentLink {
    function: FunctionId,
    definition_scope: ScopeId,
}

// QuickJS 2026-06-04 `JS_MAX_LOCAL_VARS` and `JS_STACK_SIZE_MAX` are both
// 65,534. Call opcodes encode one more argument count value; the resulting
// operand stack is checked against the smaller stack limit during lowering.
const MAX_LOCAL_VARIABLES: usize = MAX_LOCAL_SLOTS as usize;
const MAX_BYTECODE_STACK: usize = 65_534;
const MAX_CALL_ARGUMENTS: usize = 65_535;
// QuickJS `js_parse_program` allocates `JS_ATOM__ret_` as the first local of
// every script. Source text cannot spell this sentinel as an IdentifierName.
const EVAL_RET_LOCAL_NAME: &str = "<ret>";
// QuickJS `JS_ATOM__var_`: the null-prototype variable object used by sloppy
// direct eval. Source text cannot spell this binding identity.
const EVAL_VARIABLE_OBJECT_LOCAL_NAME: &str = "<var>";
// QuickJS `JS_ATOM__arg_var_`: the independent null-prototype variable
// object selected by sloppy direct eval while a non-simple parameter list is
// being evaluated. It remains live for the whole activation so body eval can
// consult it after the ordinary `<var>` object.
const ARG_EVAL_VARIABLE_OBJECT_LOCAL_NAME: &str = "<arg_var>";
// QuickJS `JS_ATOM__with_`: the object-environment binding owned by one
// sloppy `with` scope. Source text cannot spell this binding identity.
const WITH_OBJECT_LOCAL_NAME: &str = "<with>";
// A finally clause in script code must preserve the incoming completion value
// when it terminates normally. Keep those implementation-only save slots in
// the same explicit metadata domain as `<ret>` rather than letting an unbound
// ordinary local silently escape the scope-graph trust boundary.
const FINALLY_EVAL_RET_LOCAL_NAME: &str = "<finally-ret>";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionKind {
    Script,
    Module,
    Eval(EvalKind),
    Ordinary,
    /// Compiler-only object-literal concise method. Like an ordinary function
    /// it owns `this`, `arguments`, and `new.target`, but publication lowers it
    /// as a non-constructor with no `prototype` property.
    Method,
    /// Compiler-only parse/binding kind. QuickJS publishes synchronous arrow
    /// bytecode as a normal function with no prototype or constructor bit.
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatementCompletion {
    Eval,
    Discard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatementPosition {
    ProgramBody,
    FunctionBody,
    NestedList,
    /// Sloppy `if` consequent/alternate: ordinary functions are permitted,
    /// but a label may not forward that permission to its body.
    AnnexBIfArm,
    /// Sloppy labelled statement reached from a declaration list. Ordinary
    /// functions and further labels are permitted, but other declarations are
    /// still single-statement syntax errors.
    AnnexBLabelBody,
    Single,
}

impl StatementPosition {
    const fn allows_other_declaration(self) -> bool {
        matches!(
            self,
            Self::ProgramBody | Self::FunctionBody | Self::NestedList
        )
    }

    const fn allows_labelled_annex_b(self) -> bool {
        matches!(
            self,
            Self::ProgramBody | Self::FunctionBody | Self::NestedList | Self::AnnexBLabelBody
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MemberReference {
    Field {
        key: u32,
        site: SourceOffset,
    },
    Computed {
        site: SourceOffset,
    },
    /// `this, frozen HomeObject prototype, raw/canonical key` reference used
    /// by QuickJS's get/put-super-value lowering.
    Super {
        site: SourceOffset,
    },
    Private {
        name: String,
        span: Span,
        scope: ScopeId,
        site: SourceOffset,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IdentifierReference {
    name: String,
    span: Span,
    scope: ScopeId,
    object_environment: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LogicalAssignment {
    And,
    Or,
    Nullish,
}

/// Mirrors QuickJS's `PF_POW_ALLOWED`, `PF_POW_FORBIDDEN`, and zero flag.
/// The zero mode is reserved for prefix-update operands: `++x ** 2` may use
/// the updated value as the left operand, while ordinary unary expressions
/// such as `-x ** 2` are early errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PowerMode {
    Allowed,
    Forbidden,
    None,
}

/// QuickJS `PF_IN_ACCEPTED`, kept as parser state so recursive assignment RHS
/// inherits ExpressionNoIn while parentheses and selected grammar entries can
/// temporarily restore the ordinary Expression grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InMode {
    Allow,
    Disallow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ForHeadDelimiter {
    Parenthesis,
    Bracket,
    Brace,
    Template,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ForIterationKind {
    In,
    Of,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ForAssignmentDeclaration {
    Assignment,
    Var,
    Lexical,
}

#[derive(Clone, Debug)]
struct ForAssignmentTargetInfo {
    declaration: ForAssignmentDeclaration,
    var_initializer: Option<IdentifierReference>,
    is_destructuring: bool,
}

/// Parser-only counterpart of the breakable-statement part of QuickJS
/// `BlockEnv`. Each function owns its own stack so a nested function cannot
/// target an outer statement. `drop_count` models the values which must be
/// removed when an abrupt jump crosses a control (the retained switch
/// discriminant today). Try/finally unwinding is represented by the dedicated
/// control kinds below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BreakControlKind {
    RegularStatement,
    Loop,
    /// QuickJS installs a transient `BlockEnv` with `has_iterator` while
    /// lowering every ArrayBindingPattern or ArrayAssignmentPattern. It has
    /// no break/continue target, but a generator `.return(value)` injected at
    /// a `yield` inside the pattern must still close this iterator before the
    /// frame returns.
    DestructuringIterator,
    /// QuickJS precompiles the assignment fragment of a for-of/for-await head
    /// before the right-hand side installs the loop iterator record. At
    /// runtime that fragment executes with the record active, but a generator
    /// return from the fragment abandons it without calling the outer
    /// iterator's `return` method.
    ForOfAssignmentFragment,
    /// QuickJS's `has_iterator` BlockEnv, shared by for-of and for-await. Its
    /// target depth retains the conceptual `iterator`, `next`, and private
    /// unwind marker slots. A same-loop continue keeps that record, a break
    /// reaches the shared close tail, and an edge crossing the loop closes it
    /// immediately.
    ForOf,
    /// QuickJS for-in retains one hidden enumeration object. Same-loop
    /// continue keeps it, the shared break tail drops it, and a jump crossing
    /// the loop removes it without IteratorClose.
    ForIn,
    Switch,
    /// QuickJS's catch-marker BlockEnv. It is not itself breakable, but every
    /// abrupt edge crossing it must discard the marker and call its finally
    /// subroutine (which may be the empty `Ret` used by try/catch).
    TryFinally,
    /// The BlockEnv active while parsing a finally body. A break/continue
    /// leaving it discards the pending value and gosub return address so the
    /// new abrupt completion overrides the old one.
    FinallyBody,
}

#[derive(Debug)]
struct BreakControlContext {
    kind: BreakControlKind,
    label_name: Option<String>,
    /// Parser scope active when QuickJS pushes this `BlockEnv`. Abrupt jumps
    /// leave descendant lexical scopes, but keep the matched control's own
    /// scope active until its shared tail runs.
    scope: ScopeId,
    entry_depth: usize,
    drop_count: usize,
    break_jumps: Vec<usize>,
    continue_jumps: Vec<usize>,
    /// Parser-IR Gosub sites whose common target is known only after the catch
    /// and optional finally clauses have been parsed.
    finally_gosubs: Vec<usize>,
}

#[derive(Debug)]
struct IrParameterPatternBinding {
    name: String,
    parameter_local: u16,
    body_local: Option<u16>,
    declaration_span: Span,
}

#[derive(Debug)]
struct FunctionIr {
    /// Completed body boundary, retained for suspension metadata validation.
    body_parsed: bool,
    /// Parent function plus the scope which was current at this function's
    /// definition. This is QuickJS `parent` + `parent_scope_level` as one
    /// invariant-preserving typed link.
    parent: Option<ParentLink>,
    kind: FunctionKind,
    /// Callable execution semantics are independent from the grammar role.
    /// A class or object generator remains a concise `Method` for bindings
    /// and HomeObject purposes while publishing generator bytecode.
    execution_kind: BytecodeFunctionKind,
    /// Base/derived class constructors share the compiler's concise-method
    /// binding model but publish constructor bytecode without an ordinary
    /// function's eagerly visible `.prototype` shape. `DefineClass` owns that
    /// descriptor, while `CheckCtor` enforces construct-only invocation.
    class_constructor: bool,
    /// Whether this class constructor uses the derived [[Construct]]
    /// protocol. Keeping this separate from ordinary constructability lets
    /// arrows and direct eval inherit `super()` authority without pretending
    /// to be constructors themselves.
    derived_class_constructor: bool,
    /// Synthetic QuickJS class-element program role. This is assigned only by
    /// class lowering after the ordinary method-shaped FunctionIr is created.
    class_initializer_kind: Option<ClassInitializerKind>,
    /// Whether this authenticated instance/static aggregate installs the
    /// private-method brand for its class side before executing element code.
    class_private_brand: bool,
    /// QuickJS parser authority copied independently from HomeObject storage.
    super_call_allowed: bool,
    super_allowed: bool,
    arguments_forbidden: bool,
    source: FunctionSourceInfo,
    /// Intrinsic function name, independent of contextual `SetName` inference
    /// for anonymous definitions.
    function_name: Option<String>,
    /// Whether a named expression may lazily create QuickJS's private
    /// `JS_VAR_FUNCTION_NAME` self binding. Declarations carry an intrinsic
    /// name but resolve recursion through their authored environment.
    private_name_binding: bool,
    /// Lazily allocated private self-binding local.
    function_name_local: Option<u16>,
    /// Root local initialized by the typed arguments-object entry prologue.
    ///
    /// Like QuickJS's `arguments_var_idx`, this is selected only when source
    /// resolution (or a function-scoped `var`/function declaration) needs the
    /// implicit binding. A named physical `arguments` parameter suppresses it;
    /// a BindingPattern BoundName does not, because QuickJS reserves an
    /// anonymous argument slot and initializes the arguments object first.
    arguments_local: Option<u16>,
    /// Lazily materialized QuickJS pseudo variables captured by descendant
    /// arrows or exposed to direct eval. Arrow frames never own these locals;
    /// only concise methods can own the HomeObject cell.
    home_object_local: Option<u16>,
    /// QuickJS's hidden `this_active_func`, captured by arrows/direct eval so
    /// `super()` dynamically reads the active constructor's [[Prototype]].
    active_function_local: Option<u16>,
    this_local: Option<u16>,
    new_target_local: Option<u16>,
    /// Hidden null-prototype variable object for sloppy authored function code
    /// containing syntactic direct eval. Its identity is explicit rather than
    /// inferred from local allocation order.
    eval_variable_object_local: Option<u16>,
    /// Hidden `<arg_var>` object used by sloppy direct eval in a parentless
    /// Parameter Environment. Unlike authored parameter cells, this slot is
    /// rooted for the full activation and is projected into both parameter
    /// and body eval descriptors.
    arg_eval_variable_object_local: Option<u16>,
    /// Sloppy Parameter Environment alias of the ordinary function's
    /// unmapped arguments object. QuickJS skips this cell's ordinary TDZ reset
    /// and initializes it together with the body arguments binding.
    synthetic_parameter_arguments_local: Option<u16>,
    /// A lazily allocated HomeObject pseudo local requires the published
    /// method function to retain its object literal as HomeObject. Descendant
    /// arrows relay the local without carrying this metadata themselves.
    needs_home_object: bool,
    /// Physical call-frame argument slots. Destructuring parameters use an
    /// unnamed slot, matching QuickJS's `JS_ATOM_NULL` argument descriptor;
    /// their individual BoundNames live in root locals instead.
    parameters: Vec<Option<String>>,
    /// Every authored BoundName in formal-list order, including leaves of a
    /// BindingPattern. This is the authority for duplicate-parameter policy,
    /// `arguments` shadowing, and Annex B parameter-name checks.
    parameter_names: Vec<String>,
    /// QuickJS `defined_arg_count`, exposed as the function's public `length`.
    /// An identifier rest parameter owns a physical argument slot but is not
    /// included in this count.
    defined_argument_count: usize,
    /// QuickJS `has_simple_parameter_list`. Besides early-error policy, this
    /// selects mapped versus unmapped `arguments` for sloppy functions.
    has_simple_parameter_list: bool,
    /// Physical argument slot overwritten by the entry-time `OP_rest` result.
    rest_parameter: Option<u16>,
    /// First actual argument collected for a terminal `...BindingPattern`.
    /// Unlike an identifier rest parameter this does not reserve a physical
    /// frame slot; the fresh Array is consumed directly by destructuring.
    rest_pattern_start: Option<u16>,
    /// Independent declarative scope used by identifier default parameters.
    /// QuickJS calls this its argument scope; keeping the identity explicit
    /// lets resolution enforce the body-variable visibility barrier.
    parameter_scope: Option<ScopeId>,
    /// Every initializer-visible mutable cell owned by `parameter_scope`, in
    /// FormalParameters BoundName order. Identifier formals contribute one
    /// cell while a BindingPattern contributes one cell per leaf.
    parameter_locals: Vec<u16>,
    /// Exact whole-list pre-scan reservation for authored parameter cells.
    /// Reserving this leading local prefix before parsing any initializer
    /// prevents nested class/function compilation from interleaving scratch
    /// locals with the heap-visible Parameter Environment ABI.
    parameter_local_reservation_count: Option<usize>,
    /// Parameter-scope cell selected by each physical named argument. An
    /// anonymous BindingPattern slot has no direct cell because destructuring
    /// initializes its individual BoundNames instead.
    parameter_argument_locals: Vec<Option<u16>>,
    /// Parameter-scope BindingPattern leaves which must be copied into fresh
    /// FunctionRoot variables after every parameter expression has run.
    parameter_pattern_bindings: Vec<IrParameterPatternBinding>,
    /// Top-level formal initializers in source order. Pattern-leaf defaults
    /// create the argument scope but do not cut Function.length, so they are
    /// intentionally absent from this list.
    parameter_default_sources: Vec<ParameterDefaultSource>,
    /// At least one BindingPattern is initialized before the authored body.
    /// Without a Parameter Environment it runs in FunctionRoot; with one it
    /// runs in the parentless parameter scope and is copied out at the end.
    pattern_parameter_initialization: bool,
    locals: Vec<String>,
    scopes: Vec<IrScope>,
    bindings: Vec<IrBinding>,
    global_declarations: Vec<IrGlobalDeclaration>,
    /// Last direct function declaration attached to each ordinary
    /// function-scoped argument/local binding.
    hoisted_functions: Vec<IrHoistedFunction>,
    /// Source-ordered declaration records for sloppy direct eval targeting a
    /// caller function's variable environment.
    eval_declarations: Vec<IrEvalDeclaration>,
    eval_declarations_installed: bool,
    /// First caller lexical name which conflicts with an eval `var`/function.
    /// The eval still compiles so global declaration instantiation can run
    /// before this typed SyntaxError is thrown at bytecode entry.
    eval_redeclaration: Option<String>,
    function_hoists_installed: bool,
    /// Phase marker for the final hidden-frame entry prefix. Unlike ordinary
    /// body hoists this also applies to scripts and eval roots, so it cannot
    /// be inferred from `function_hoists_installed`.
    pseudo_binding_prologues_installed: bool,
    /// Scoped lexical function slots, including one slot per sloppy same-scope
    /// duplicate as in QuickJS `JS_VAR_FUNCTION_DECL`.
    scoped_functions: Vec<IrScopedFunction>,
    /// ProgramBody's labelled-function exception has authored closure writes
    /// but no lexical scope-entry slot.
    program_annex_functions: Vec<IrProgramAnnexFunction>,
    var_scope: ScopeId,
    body_scope: ScopeId,
    /// QuickJS `eval_ret_idx`: the script-only hidden completion local.
    /// Keeping the typed slot separate from its unspellable debug name avoids
    /// confusing it with future source bindings or other synthetic locals.
    eval_ret_local: Option<u16>,
    /// Every local which deliberately has no source binding identity. This is
    /// validated separately from authored locals before publication.
    synthetic_locals: Vec<SyntheticLocal>,
    ops: Vec<SpannedIrOp>,
    constants: Vec<IrConstant>,
    /// First primitive string occurrence; constant ordinals remain append-only.
    string_constants: HashMap<JsString, u32>,
    closure_variables: Vec<ClosureVariable>,
    /// Exact flattened caller bindings imported by a synthetic direct-eval
    /// root. Entries retain their original R1w descriptor indices even though
    /// bindings are inserted into the synthetic root in outer-to-inner order
    /// so ordinary reverse lookup selects the innermost duplicate name.
    external_bindings: Vec<EvalRootBinding<JsString>>,
    /// Exact imported caller scope topology and variable target.  The root's
    /// flat external binding vector remains the closure-prefix ABI, while this
    /// profile reconstructs the original ordered suffix for nested eval.
    eval_caller_profile: EvalCallerProfile,
    /// Immutable QuickJS-shaped scope chains linked for syntactic direct-eval
    /// call sites. Multiple calls from the same parser scope share one entry.
    eval_environments: Vec<EvalEnvironment<JsString>>,
    strict: bool,
}

#[derive(Clone, Debug)]
struct FunctionSourceInfo {
    span: Span,
    definition: SourceOffset,
    range: Option<Range<SourceOffset>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SuperCapabilities {
    super_call_allowed: bool,
    super_allowed: bool,
}

impl SuperCapabilities {
    const NONE: Self = Self {
        super_call_allowed: false,
        super_allowed: false,
    };
    const PROPERTY: Self = Self {
        super_call_allowed: false,
        super_allowed: true,
    };
    const CALL_AND_PROPERTY: Self = Self {
        super_call_allowed: true,
        super_allowed: true,
    };

    fn validated(self) -> Result<Self, Error> {
        if self.super_call_allowed && !self.super_allowed {
            return Err(Error::internal(
                "function permits super() without SuperProperty",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug)]
struct FunctionIrOptions {
    function_name: Option<String>,
    private_name_binding: bool,
    class_constructor: bool,
    derived_class_constructor: bool,
    parameters: Vec<Option<String>>,
    defined_argument_count: usize,
    has_simple_parameter_list: bool,
    rest_parameter: Option<u16>,
    strict: bool,
    super_capabilities: SuperCapabilities,
}

impl FunctionIr {
    fn new(
        parent: Option<ParentLink>,
        kind: FunctionKind,
        source: FunctionSourceInfo,
        options: FunctionIrOptions,
    ) -> Result<Self, Error> {
        let super_capabilities = options.super_capabilities.validated()?;
        if options.derived_class_constructor
            && (!options.class_constructor
                || kind != FunctionKind::Method
                || super_capabilities != SuperCapabilities::CALL_AND_PROPERTY)
        {
            return Err(Error::internal("derived constructor metadata is malformed"));
        }
        let parameter_count = options.parameters.len();
        if options.defined_argument_count > options.parameters.len()
            || (options.has_simple_parameter_list
                && (options.defined_argument_count != options.parameters.len()
                    || options.rest_parameter.is_some()
                    || options.parameters.iter().any(Option::is_none)))
            || options.rest_parameter.is_some_and(|rest| {
                usize::from(rest) + 1 != options.parameters.len()
                    || options.defined_argument_count != usize::from(rest)
                    || options.has_simple_parameter_list
                    || options.parameters[usize::from(rest)].is_none()
            })
        {
            return Err(Error::internal("formal parameter metadata is malformed"));
        }
        let (locals, eval_ret_local, synthetic_locals) =
            if matches!(kind, FunctionKind::Script | FunctionKind::Eval(_)) {
                (
                    vec![EVAL_RET_LOCAL_NAME.to_owned()],
                    Some(0),
                    vec![SyntheticLocal {
                        index: 0,
                        kind: SyntheticLocalKind::EvalCompletion,
                    }],
                )
            } else {
                (Vec::new(), None, Vec::new())
            };
        // QuickJS reserves scope zero for arguments/function-scoped storage,
        // then pushes the authored body scope. Named-expression self storage
        // is a lazy local in the root, not a synthetic lexical parent scope.
        let function_root = ScopeId(0);
        let body = ScopeId(1);
        let scopes = vec![
            IrScope {
                parent: None,
                kind: ScopeKind::FunctionRoot,
                is_parameter_initializer: false,
                bindings: Vec::new(),
                bindings_by_name: Default::default(),
            },
            IrScope {
                parent: Some(function_root),
                kind: if matches!(
                    kind,
                    FunctionKind::Script | FunctionKind::Module | FunctionKind::Eval(_)
                ) {
                    ScopeKind::ProgramBody
                } else {
                    ScopeKind::FunctionBody
                },
                is_parameter_initializer: false,
                bindings: Vec::new(),
                bindings_by_name: Default::default(),
            },
        ];
        let var_scope = function_root;
        let ops = if matches!(
            kind,
            FunctionKind::Ordinary
                | FunctionKind::Method
                | FunctionKind::Arrow
                | FunctionKind::Eval(_)
        ) {
            vec![SpannedIrOp {
                op: IrOp::EnterScope(body),
                pc_site: None,
            }]
        } else {
            Vec::new()
        };
        let mut function = Self {
            body_parsed: false,
            parent,
            kind,
            execution_kind: BytecodeFunctionKind::Normal,
            class_constructor: options.class_constructor,
            derived_class_constructor: options.derived_class_constructor,
            class_initializer_kind: None,
            class_private_brand: false,
            super_call_allowed: super_capabilities.super_call_allowed,
            super_allowed: super_capabilities.super_allowed,
            arguments_forbidden: false,
            source,
            function_name: options.function_name,
            private_name_binding: options.private_name_binding,
            function_name_local: None,
            arguments_local: None,
            home_object_local: None,
            active_function_local: None,
            this_local: None,
            new_target_local: None,
            eval_variable_object_local: None,
            arg_eval_variable_object_local: None,
            synthetic_parameter_arguments_local: None,
            needs_home_object: false,
            parameters: options.parameters,
            parameter_names: Vec::new(),
            defined_argument_count: options.defined_argument_count,
            has_simple_parameter_list: options.has_simple_parameter_list,
            rest_parameter: options.rest_parameter,
            rest_pattern_start: None,
            parameter_scope: None,
            parameter_locals: Vec::new(),
            parameter_local_reservation_count: None,
            parameter_argument_locals: vec![None; parameter_count],
            parameter_pattern_bindings: Vec::new(),
            parameter_default_sources: Vec::new(),
            pattern_parameter_initialization: false,
            locals,
            scopes,
            bindings: Vec::new(),
            global_declarations: Vec::new(),
            hoisted_functions: Vec::new(),
            eval_declarations: Vec::new(),
            eval_declarations_installed: false,
            eval_redeclaration: None,
            function_hoists_installed: false,
            pseudo_binding_prologues_installed: false,
            scoped_functions: Vec::new(),
            program_annex_functions: Vec::new(),
            var_scope,
            body_scope: body,
            eval_ret_local,
            synthetic_locals,
            ops,
            constants: Vec::new(),
            string_constants: HashMap::new(),
            closure_variables: Vec::new(),
            external_bindings: Vec::new(),
            eval_caller_profile: EvalCallerProfile {
                scope_kinds: Box::new([]),
                variable_target: EvalCallerVariableTarget::Global,
            },
            eval_environments: Vec::new(),
            strict: options.strict,
        };
        for (index, name) in function.parameters.clone().into_iter().enumerate() {
            let Some(name) = name else {
                continue;
            };
            let index = u16::try_from(index)
                .map_err(|_| Error::new(ErrorKind::JsInternal, "too many arguments"))?;
            function.parameter_names.push(name.clone());
            function.add_binding(
                function.var_scope,
                function.var_scope,
                name,
                BindingStorage::Argument(index),
                BindingKind::Normal,
                None,
            );
        }
        Ok(function)
    }

    /// Allocate the derived constructor's hidden cells after formal parsing.
    /// Parameter-environment cells must remain the leading locals, but
    /// unresolved `super()`/`this` operations in parameter initializers do not
    /// need physical operands until the later identifier-linking pass.
    fn allocate_derived_constructor_pseudo_bindings(&mut self) -> Result<(), Error> {
        if !self.derived_class_constructor
            || !self.class_constructor
            || self.kind != FunctionKind::Method
            || self.active_function_local.is_some()
            || self.this_local.is_some()
        {
            return Err(Error::internal(
                "derived constructor pseudo bindings were allocated in an invalid phase",
            ));
        }
        if self.locals.len().saturating_add(2) > MAX_LOCAL_VARIABLES {
            return Err(Error::new(
                ErrorKind::JsInternal,
                "too many local variables",
            ));
        }
        let active_function = u16::try_from(self.locals.len())
            .map_err(|_| Error::new(ErrorKind::JsInternal, "too many local variables"))?;
        self.locals.push(ACTIVE_FUNCTION_LOCAL_NAME.to_owned());
        self.active_function_local = Some(active_function);
        self.add_binding(
            self.var_scope,
            self.var_scope,
            ACTIVE_FUNCTION_LOCAL_NAME.to_owned(),
            BindingStorage::Local(active_function),
            BindingKind::Normal,
            None,
        );

        let this = u16::try_from(self.locals.len())
            .map_err(|_| Error::new(ErrorKind::JsInternal, "too many local variables"))?;
        self.locals.push(THIS_LOCAL_NAME.to_owned());
        self.this_local = Some(this);
        self.add_binding(
            self.var_scope,
            self.var_scope,
            THIS_LOCAL_NAME.to_owned(),
            BindingStorage::Local(this),
            BindingKind::Lexical { is_const: false },
            None,
        );
        Ok(())
    }

    /// Preserve every authored constant and its ordinal. Only name-constant
    /// reuse consults the derived first-occurrence index.
    fn append_constant(&mut self, constant: IrConstant) -> Result<u32, Error> {
        let index = u32::try_from(self.constants.len())
            .map_err(|_| Error::new(ErrorKind::JsInternal, "out of memory"))?;
        if let IrConstant::Primitive(Value::String(value)) = &constant {
            self.string_constants.entry(value.clone()).or_insert(index);
        }
        self.constants.push(constant);
        Ok(index)
    }

    fn add_binding(
        &mut self,
        storage_scope: ScopeId,
        declaration_scope: ScopeId,
        name: String,
        storage: BindingStorage,
        kind: BindingKind,
        declaration_span: Option<Span>,
    ) -> BindingId {
        let binding = BindingId(self.bindings.len());
        self.bindings.push(IrBinding {
            name,
            storage_scope,
            declaration_scope,
            storage,
            kind,
            is_scoped_function: false,
            is_scoped_generator: false,
            is_catch_parameter: false,
            declaration_span,
        });
        self.scopes[storage_scope.0].bindings.push(binding);
        self.scopes[storage_scope.0]
            .bindings_by_name
            .insert(self.bindings[binding.0].name.clone(), binding);
        binding
    }

    fn add_synthetic_local(&mut self, kind: SyntheticLocalKind) -> Result<u16, Error> {
        if self.locals.len() >= MAX_LOCAL_VARIABLES {
            return Err(Error::new(
                ErrorKind::JsInternal,
                "too many local variables",
            ));
        }
        let index = u16::try_from(self.locals.len())
            .map_err(|_| Error::new(ErrorKind::JsInternal, "too many local variables"))?;
        self.locals.push(kind.name().to_owned());
        self.synthetic_locals.push(SyntheticLocal { index, kind });
        Ok(index)
    }

    fn binding_in_scope(&self, scope: ScopeId, name: &str) -> Option<&IrBinding> {
        self.binding_id_in_scope(scope, name)
            .map(|binding| &self.bindings[binding.0])
    }

    fn binding_id_in_scope(&self, scope: ScopeId, name: &str) -> Option<BindingId> {
        self.scopes[scope.0].binding_named(name)
    }

    /// Rare late function-name insertion changes order after normal appends.
    /// Rebuild once there, rather than burdening every name lookup with a scan.
    fn rebuild_scope_name_index(&mut self, scope: ScopeId) {
        let scope = &mut self.scopes[scope.0];
        scope.bindings_by_name.clear();
        for &binding in &scope.bindings {
            scope
                .bindings_by_name
                .insert(self.bindings[binding.0].name.clone(), binding);
        }
    }

    fn binding_id_from_scope(
        &self,
        mut scope: ScopeId,
        name: &str,
    ) -> Option<(ScopeId, BindingId)> {
        loop {
            if let Some(binding) = self.binding_id_in_scope(scope, name) {
                return Some((scope, binding));
            }
            scope = self.scopes[scope.0].parent?;
        }
    }

    fn first_global_declaration_is_normal(&self, name: &str) -> bool {
        self.global_declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .is_some_and(|declaration| !declaration.is_lexical)
    }

    fn binding_from_scope(&self, mut scope: ScopeId, name: &str) -> Option<ResolvedBinding> {
        loop {
            if let Some(binding) = self.binding_in_scope(scope, name) {
                return Some(ResolvedBinding {
                    storage: binding.storage,
                    kind: binding.kind,
                });
            }
            scope = self.scopes[scope.0].parent?;
        }
    }

    fn scope_is_within(&self, mut scope: ScopeId, ancestor: ScopeId) -> bool {
        loop {
            if scope == ancestor {
                return true;
            }
            let Some(parent) = self.scopes[scope.0].parent else {
                return false;
            };
            scope = parent;
        }
    }
}

fn install_eval_external_bindings(
    function: &mut FunctionIr,
    bindings: Box<[EvalRootBinding<JsString>]>,
    caller_profile: EvalCallerProfile,
    caller_strict: bool,
) -> Result<(), Error> {
    let FunctionKind::Eval(kind) = function.kind else {
        return Err(Error::internal(
            "eval caller bindings escaped a synthetic eval root",
        ));
    };
    if kind == EvalKind::Indirect && !bindings.is_empty() {
        return Err(Error::internal(
            "indirect eval root received external caller bindings",
        ));
    }
    if !function.closure_variables.is_empty() || !function.external_bindings.is_empty() {
        return Err(Error::internal(
            "eval caller bindings were installed more than once",
        ));
    }
    if bindings.iter().any(|binding| {
        let Some(&scope_kind) = caller_profile.scope_kinds.get(usize::from(binding.scope)) else {
            return true;
        };
        (binding.is_catch_parameter && scope_kind != EvalScopeKind::Catch)
            || (binding.kind == ClosureVariableKind::WithObject)
                != (scope_kind == EvalScopeKind::With)
    }) || caller_profile
        .scope_kinds
        .iter()
        .enumerate()
        .any(|(scope, kind)| {
            *kind == EvalScopeKind::With
                && bindings
                    .iter()
                    .filter(|binding| usize::from(binding.scope) == scope)
                    .count()
                    != 1
        })
    {
        return Err(Error::internal(
            "eval caller bindings disagree with their scope profile",
        ));
    }
    let has_variable_object = bindings.iter().any(|binding| {
        matches!(
            binding.kind,
            ClosureVariableKind::EvalVariableObject | ClosureVariableKind::ArgEvalVariableObject
        )
    });
    match (caller_strict, caller_profile.variable_target) {
        (false, EvalCallerVariableTarget::Global) if !has_variable_object => {}
        (true, EvalCallerVariableTarget::StrictLocal) if kind == EvalKind::Direct => {}
        (false, EvalCallerVariableTarget::ExternalBinding(index))
            if bindings.get(usize::from(index)).is_some_and(|binding| {
                matches!(
                    binding.kind,
                    ClosureVariableKind::EvalVariableObject
                        | ClosureVariableKind::ArgEvalVariableObject
                ) && !binding.is_lexical
                    && !binding.is_const
                    && !binding.is_catch_parameter
            }) => {}
        _ => {
            return Err(Error::internal(
                "eval caller variable target is not authenticated",
            ));
        }
    }

    for (index, binding) in bindings.iter().enumerate() {
        let index = u16::try_from(index)
            .map_err(|_| Error::new(ErrorKind::JsInternal, "too many closure variables"))?;
        let name = String::from_utf16(&binding.name.utf16_units().collect::<Vec<_>>())
            .map_err(|_| Error::internal("eval caller binding name is not well formed"))?;
        let name = ensure_string_constant(function, &name)?;
        let descriptor = ClosureVariable {
            source: ClosureSource::EvalEnvironment(index),
            name: ClosureVariableName::Constant(name),
            is_lexical: binding.is_lexical,
            is_const: binding.is_const,
            kind: binding.kind,
        };
        let installed = push_closure_variable(function, descriptor)?;
        if installed != index {
            return Err(Error::internal(
                "eval caller closure indices are not contiguous",
            ));
        }
    }

    // Scope bindings are searched newest-first. Install outer-to-inner so the
    // innermost exact descriptor wins for duplicate names while every closure
    // slot remains available to the specialized publication verifier. The
    // `<var>` remains unspellable source metadata, but it must still have a
    // binding identity in the synthetic root.  QuickJS relays the same hidden
    // closure VarRef when eval source itself contains a direct eval; retaining
    // it here lets that later call authenticate the exact variable target.
    for (index, binding) in bindings.iter().enumerate().rev() {
        let index = u16::try_from(index)
            .map_err(|_| Error::new(ErrorKind::JsInternal, "too many closure variables"))?;
        if binding.kind == ClosureVariableKind::EvalVariableObject
            && (binding.is_lexical
                || binding.is_const
                || binding.is_catch_parameter
                || binding.name.to_utf8_lossy() != EVAL_VARIABLE_OBJECT_LOCAL_NAME)
        {
            return Err(Error::internal(
                "eval variable object binding metadata is malformed",
            ));
        }
        if binding.kind == ClosureVariableKind::ArgEvalVariableObject
            && (binding.is_lexical
                || binding.is_const
                || binding.is_catch_parameter
                || binding.name.to_utf8_lossy() != ARG_EVAL_VARIABLE_OBJECT_LOCAL_NAME)
        {
            return Err(Error::internal(
                "argument eval variable object binding metadata is malformed",
            ));
        }
        if binding.kind == ClosureVariableKind::WithObject
            && (binding.is_lexical
                || binding.is_const
                || binding.is_catch_parameter
                || binding.name.to_utf8_lossy() != WITH_OBJECT_LOCAL_NAME)
        {
            return Err(Error::internal("with object binding metadata is malformed"));
        }
        let name = String::from_utf16(&binding.name.utf16_units().collect::<Vec<_>>())
            .map_err(|_| Error::internal("eval caller binding name is not well formed"))?;
        let kind =
            binding_kind_from_closure_flags(binding.kind, binding.is_lexical, binding.is_const)
                .ok_or_else(|| Error::internal("eval caller binding flags are inconsistent"))?;
        let installed = function.add_binding(
            function.var_scope,
            function.var_scope,
            name,
            BindingStorage::External(index),
            kind,
            None,
        );
        function.bindings[installed.0].is_catch_parameter = binding.is_catch_parameter;
    }
    function.external_bindings = bindings.into_vec();
    function.eval_caller_profile = caller_profile;
    Ok(())
}

#[derive(Debug)]
struct FunctionTree {
    functions: Vec<FunctionIr>,
    source: SourceText,
    filename: JsString,
    module: Option<module::IrModule>,
    pending_unsupported: Option<Error>,
}

#[derive(Clone, Copy, Debug)]
struct PreparedScopedFunction {
    binding: BindingId,
    create_annex_binding: bool,
}

#[derive(Clone, Copy, Debug)]
enum AnonymousFunctionDefinition {
    Function,
    Class {
        owner: FunctionId,
        /// IR insertion point immediately before the static initializer
        /// closure. NamedEvaluation is moved here so static elements observe
        /// the inferred class name.
        static_initializer_start: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModuleDeclarationExport {
    None,
    Named,
    Default,
}

struct Parser<'source> {
    lexer: Lexer<'source>,
    tokens: Vec<Token<'source>>,
    cursor: usize,
    current_function: FunctionId,
    in_mode: InMode,
    functions: Vec<FunctionBuilder>,
    module: Option<module::IrModule>,
    module_declaration_export: ModuleDeclarationExport,
    module_declaration_export_target: Option<(FunctionId, ScopeId)>,
    /// Function expression eligible for QuickJS's assignment-name inference.
    /// Operators which make the surrounding expression cease to be an
    /// AnonymousFunctionDefinition clear this marker.
    anonymous_function_definition: Option<AnonymousFunctionDefinition>,
    /// First syntactically valid construct which belongs to an unimplemented
    /// engine frontier. The parser keeps going so later grammar and early
    /// errors retain QuickJS priority over the implementation diagnostic.
    pending_unsupported: Option<Error>,
}

enum RootCompileContext {
    Script,
    Module,
    Eval(EvalCompileContext),
}


fn relocate_ir_fragment(
    operations: &mut [SpannedIrOp],
    old_range: Range<usize>,
    new_start: usize,
) -> Result<(), Error> {
    for operation in operations {
        let IrOp::Bytecode(
            Instruction::Goto(target)
            | Instruction::IfFalse(target)
            | Instruction::IfTrue(target)
            | Instruction::Catch(target)
            | Instruction::Gosub(target),
        ) = &mut operation.op
        else {
            continue;
        };
        let Ok(old_target) = usize::try_from(*target) else {
            continue;
        };
        if old_range.contains(&old_target) {
            let relocated = new_start
                .checked_add(old_target - old_range.start)
                .ok_or_else(|| Error::new(ErrorKind::JsInternal, "out of memory"))?;
            *target = u32::try_from(relocated)
                .map_err(|_| Error::new(ErrorKind::JsInternal, "out of memory"))?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum IdentifierContext {
    Reference,
    Variable,
    FunctionName,
    Argument,
}

fn validate_identifier(
    identifier: &Identifier<'_>,
    span: Span,
    strict: bool,
    context: IdentifierContext,
) -> Result<(), Error> {
    validate_identifier_reservation(identifier, span, strict, context)?;
    if strict
        && !matches!(context, IdentifierContext::Reference)
        && matches!(identifier.value.as_str(), "eval" | "arguments")
    {
        let message = match context {
            IdentifierContext::Variable => "invalid variable name in strict mode",
            IdentifierContext::FunctionName => "invalid function name in strict code",
            IdentifierContext::Argument => "invalid argument name in strict code",
            IdentifierContext::Reference => unreachable!("reference context was excluded"),
        };
        return Err(Error::syntax(message, source_span(span)));
    }
    Ok(())
}

fn validate_identifier_reservation(
    identifier: &Identifier<'_>,
    span: Span,
    strict: bool,
    context: IdentifierContext,
) -> Result<(), Error> {
    if identifier.escaped_reserved_word {
        return Err(syntax_atom_error(
            "'",
            &identifier.value,
            "' is a reserved identifier",
            span,
        )?);
    }
    if strict
        && identifier
            .keyword_hint
            .is_some_and(strict_reserved_identifier)
    {
        let message = match context {
            IdentifierContext::Reference => {
                return Err(syntax_atom_error(
                    "'",
                    &identifier.value,
                    "' is a reserved identifier",
                    span,
                )?);
            }
            IdentifierContext::Variable => "invalid variable name in strict mode",
            IdentifierContext::FunctionName => "invalid function name in strict code",
            IdentifierContext::Argument => "invalid argument name in strict code",
        };
        return Err(Error::syntax(message, source_span(span)));
    }
    Ok(())
}

fn syntax_atom_error(
    prefix: &str,
    atom: &str,
    suffix: &str,
    span: Span,
) -> Result<Error, JsStringError> {
    Ok(syntax_atom_error_without_span(prefix, atom, suffix)?.with_span(source_span(span)))
}

fn syntax_atom_error_without_span(
    prefix: &str,
    atom: &str,
    suffix: &str,
) -> Result<Error, JsStringError> {
    let atom = JsString::try_from_utf8(atom)?;
    let mut message = NativeErrorMessage::new();
    message.push_utf8(prefix);
    atom.push_atom_get_str_to(&mut message);
    message.push_utf8(suffix);
    Ok(Error::from_native_message(ErrorKind::Syntax, message))
}

const fn strict_reserved_identifier(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Implements
            | Keyword::Interface
            | Keyword::Let
            | Keyword::Package
            | Keyword::Private
            | Keyword::Protected
            | Keyword::Public
            | Keyword::Static
            | Keyword::Yield
    )
}

fn unlinked_primitive(value: Value) -> Result<UnlinkedConstant, Error> {
    UnlinkedConstant::primitive(value).map_err(|error| {
        Error::internal(format!(
            "compiler emitted a runtime-bound constant into an unlinked function: {error}"
        ))
    })
}

fn parse_number(
    number: &crate::engine::compiler::lexer::NumberLiteral<'_>,
) -> Result<Value, String> {
    let raw = number.raw.replace('_', "");
    if let NumberKind::BigInt(radix) = number.kind {
        let literal = raw
            .strip_suffix('n')
            .ok_or_else(|| "BigInt literal is missing its suffix".to_owned())?;
        let (digits, base) = match radix {
            NumericRadix::Binary => (literal.get(2..).unwrap_or_default(), 2),
            NumericRadix::Octal => (literal.get(2..).unwrap_or_default(), 8),
            NumericRadix::Decimal => (literal, 10),
            NumericRadix::Hexadecimal => (literal.get(2..).unwrap_or_default(), 16),
        };
        return JsBigInt::parse_radix(digits, base)
            .map(Value::BigInt)
            .map_err(|error| error.to_string());
    }

    let value = match number.kind {
        NumberKind::Integer(radix) => parse_radix_literal(&raw, radix)?,
        NumberKind::Float | NumberKind::LegacyDecimal => raw
            .parse::<f64>()
            .map_err(|_| format!("invalid numeric literal '{raw}'"))?,
        NumberKind::LegacyOctal => parse_digits(&raw, 8)?,
        NumberKind::BigInt(_) => unreachable!("handled above"),
    };
    Ok(Value::number(value))
}

/// Mirrors the token switch in QuickJS 2026-06-04 `js_parse_directives`.
/// Its observable ASI behavior is intentionally narrower than a generic
/// "can this token continue an expression" test.
fn quickjs_directive_asi_token(kind: &TokenKind<'_>) -> bool {
    matches!(
        kind,
        TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::Template(_)
            | TokenKind::Identifier(_)
            | TokenKind::RegExp(_)
            | TokenKind::Punctuator(Punctuator::Decrement | Punctuator::Increment)
            | TokenKind::Keyword(
                Keyword::Null
                    | Keyword::False
                    | Keyword::True
                    | Keyword::If
                    | Keyword::Return
                    | Keyword::Var
                    | Keyword::This
                    | Keyword::Delete
                    | Keyword::Typeof
                    | Keyword::New
                    | Keyword::Do
                    | Keyword::While
                    | Keyword::For
                    | Keyword::Switch
                    | Keyword::Throw
                    | Keyword::Try
                    | Keyword::Function
                    | Keyword::Debugger
                    | Keyword::With
                    | Keyword::Class
                    | Keyword::Const
                    | Keyword::Enum
                    | Keyword::Export
                    | Keyword::Import
                    | Keyword::Super
                    | Keyword::Interface
                    | Keyword::Let
                    | Keyword::Package
                    | Keyword::Private
                    | Keyword::Protected
                    | Keyword::Public
                    | Keyword::Static
            )
    )
}

/// QuickJS `is_regexp_allowed`, used only by the non-committing `for`-head
/// probe. The real parser still owns the eventual lexical goal and diagnostic.
fn for_head_regexp_allowed_after(kind: &TokenKind<'_>) -> bool {
    if matches!(
        kind,
        TokenKind::Identifier(identifier)
            if !identifier.has_escape && matches!(identifier.value.as_str(), "of" | "yield")
    ) {
        return true;
    }
    !matches!(
        kind,
        TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::RegExp(_)
            | TokenKind::Identifier(_)
            | TokenKind::Keyword(Keyword::Null | Keyword::False | Keyword::True | Keyword::This)
            | TokenKind::Punctuator(
                Punctuator::RightParen
                    | Punctuator::RightBracket
                    | Punctuator::RightBrace
                    | Punctuator::Increment
                    | Punctuator::Decrement
            )
    )
}

fn parse_radix_literal(raw: &str, radix: NumericRadix) -> Result<f64, String> {
    let (digits, base) = match radix {
        NumericRadix::Binary => (raw.get(2..).unwrap_or_default(), 2),
        NumericRadix::Octal => (raw.get(2..).unwrap_or_default(), 8),
        NumericRadix::Decimal => (raw, 10),
        NumericRadix::Hexadecimal => (raw.get(2..).unwrap_or_default(), 16),
    };
    parse_digits(digits, base)
}

fn parse_digits(digits: &str, radix: u32) -> Result<f64, String> {
    if digits.is_empty() {
        return Err("numeric literal has no digits".to_owned());
    }
    let value = BigUint::parse_bytes(digits.as_bytes(), radix)
        .ok_or_else(|| format!("invalid base-{radix} numeric literal"))?;
    Ok(value.to_f64().unwrap_or(f64::INFINITY))
}

fn lex_error(error: LexError) -> Error {
    if error.kind == LexErrorKind::StringTooLong {
        Error::new(ErrorKind::JsInternal, error.message)
    } else {
        Error::syntax(error.message, source_span(error.span))
    }
}

fn source_offset(span: Span) -> Result<SourceOffset, Error> {
    SourceOffset::try_from_usize(span.start.byte_offset)
        .map_err(|error| Error::internal(error.to_string()))
}

const fn source_span(span: Span) -> SourceSpan {
    SourceSpan::new(
        SourceLocation::new(span.start.byte_offset, span.start.line, span.start.column),
        SourceLocation::new(span.end.byte_offset, span.end.line, span.end.column),
    )
}

#[cfg(test)]
mod tests;

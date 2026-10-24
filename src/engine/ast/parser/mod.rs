//! The parser: source text to [`Program`].
//!
//! The grammar decisions follow QuickJS 2026-06-04 as the stack-code parser
//! in `compiler::parser` did: the same non-committing lookahead probes choose
//! arrow functions, destructuring assignments, `for` heads and contextual
//! keywords, the same early errors are reported at the same tokens in the
//! same order, and the same recursion weights bound nesting. Instead of
//! emitting code, every production returns its tree.

mod classes;
mod declare;
mod expressions;
mod functions;
mod lookahead;
mod modules;
mod objects;
mod patterns;
mod statements;
mod support;
pub(crate) use support::syntax_atom_error_without_span;
#[cfg(test)]
mod tests;

use super::{
    DeclId, FuncId, Function, FunctionInfo, FunctionKind, LabelId, MethodRole, ModuleRecord,
    Program, ProgramKind, Scope, ScopeId, ScopeKind,
};
use crate::engine::api::error::Error;
use crate::engine::compiler::lexer::{LexContext, Lexer, LexerOptions, LexicalGoal, Span, Token};
use crate::engine::compiler::names::{NameId, NameTable};
use crate::engine::compiler::stack_guard::ParserStackGuard;
use crate::source::text::SourceText;
use lookahead::LookaheadCache;
use std::cell::RefCell;

/// QuickJS `PF_IN_ACCEPTED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InMode {
    Allow,
    Disallow,
}

/// Where a statement occurs, which decides the declarations it may be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StatementPosition {
    ProgramBody,
    FunctionBody,
    NestedList,
    /// A sloppy `if` arm: an ordinary function declaration is allowed.
    AnnexBIfArm,
    /// A sloppy labelled statement in a declaration list.
    AnnexBLabelBody,
    Single,
}

impl StatementPosition {
    pub(super) const fn allows_other_declaration(self) -> bool {
        matches!(
            self,
            Self::ProgramBody | Self::FunctionBody | Self::NestedList
        )
    }

    pub(super) const fn allows_labelled_annex_b(self) -> bool {
        matches!(
            self,
            Self::ProgramBody | Self::FunctionBody | Self::NestedList | Self::AnnexBLabelBody
        )
    }
}

/// A breakable statement on the current function's control stack.
#[derive(Clone, Debug)]
pub(super) struct BreakControl {
    pub(super) kind: BreakKind,
    pub(super) label: Option<String>,
    pub(super) id: LabelId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BreakKind {
    /// A labelled non-loop statement.
    Regular,
    Loop,
    Switch,
}

/// The parser state of the function being parsed.
#[derive(Debug)]
pub(super) struct FunctionState {
    pub(super) id: FuncId,
    pub(super) kind: FunctionKind,
    pub(super) is_async: bool,
    pub(super) is_generator: bool,
    pub(super) strict: bool,
    pub(super) current_scope: ScopeId,
    pub(super) var_scope: ScopeId,
    pub(super) body_scope: ScopeId,
    pub(super) parameter_scope: Option<ScopeId>,
    /// `yield` and `await` are allowed only once the body starts.
    pub(super) in_function_body: bool,
    pub(super) super_call_allowed: bool,
    pub(super) super_allowed: bool,
    pub(super) arguments_forbidden: bool,
    pub(super) breaks: Vec<BreakControl>,
    /// Every bound parameter name, for the duplicate checks.
    pub(super) parameter_names: Vec<NameId>,
    /// Pattern parameters bind in the function root.
    pub(super) pattern_parameter_initialization: bool,
    pub(super) simple_params: bool,
    /// Script: the order of global declarations, for QuickJS's
    /// first-global-record rules (name, whether lexical).
    pub(super) global_declarations: Vec<(NameId, bool)>,
}

impl FunctionState {
    pub(super) const fn is_static_block(&self) -> bool {
        matches!(self.kind, FunctionKind::StaticBlock)
    }

    pub(super) fn first_global_declaration_is_normal(&self, name: NameId) -> bool {
        self.global_declarations
            .iter()
            .find(|(n, _)| *n == name)
            .is_some_and(|(_, lexical)| !lexical)
    }
}

pub(crate) struct Parser<'source> {
    pub(super) lexer: Lexer<'source>,
    pub(super) names: NameTable,
    pub(super) token: Token<'source>,
    /// The lexical context and goal the current token was scanned with.
    pub(super) token_context: (LexContext, LexicalGoal),
    pub(super) previous_end: Option<usize>,
    /// Where the previous token ended, for node spans.
    pub(super) previous_end_location: Option<crate::source::SourceLocation>,
    pub(super) in_mode: InMode,
    pub(super) program_kind: ProgramKind,
    /// The functions being parsed, innermost last.
    pub(super) stack: Vec<FunctionState>,
    pub(super) scopes: Vec<Scope>,
    pub(super) decls: Vec<super::Decl>,
    pub(super) functions: Vec<FunctionInfo>,
    pub(super) module: Option<ModuleRecord>,
    /// The scope a module export declaration is parsed in, and its kind.
    pub(super) module_export: Option<(FuncId, ScopeId, ModuleExport)>,
    pub(super) has_with: bool,
    pub(super) next_label: u32,
    lookahead: RefCell<LookaheadCache<'source>>,
    pub(super) stack_guard: ParserStackGuard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModuleExport {
    Named,
    Default,
}

impl<'source> Parser<'source> {
    /// Parses a script.
    ///
    /// # Errors
    /// Returns the first syntax error, as the stack-code parser reports it.
    pub(crate) fn parse_script(source: &'source SourceText) -> Result<Program, Error> {
        Self::parse_root(source.carrier(), Some(source), ProgramKind::Script)
    }

    pub(crate) fn parse_module(source: &'source SourceText) -> Result<Program, Error> {
        Self::parse_root(source.carrier(), Some(source), ProgramKind::Module)
    }

    /// Parses indirect eval code.
    pub(crate) fn parse_eval(source: &'source SourceText) -> Result<Program, Error> {
        Self::parse_root(source.carrier(), Some(source), ProgramKind::Eval)
    }

    #[cfg(test)]
    pub(crate) fn parse_str(source: &'source str, kind: ProgramKind) -> Result<Program, Error> {
        Self::parse_root(source, None, kind)
    }

    fn parse_root(
        source: &'source str,
        source_text: Option<&'source SourceText>,
        kind: ProgramKind,
    ) -> Result<Program, Error> {
        crate::engine::compiler::validate_source_length(source.len())?;
        let is_module = kind == ProgramKind::Module;
        let options = LexerOptions {
            context: LexContext {
                strict: is_module,
                module: is_module,
                ..LexContext::default()
            },
            // QuickJS enables Annex B HTML comments outside modules.
            allow_html_comments: !is_module,
        };
        let mut lexer = match source_text {
            Some(source) => Lexer::with_source_text(source, options),
            None => Lexer::with_options(source, options),
        };
        let first = lexer.next_token().map_err(support::lex_error)?;
        let source_span = first.span;
        let mut parser = Self {
            token_context: (lexer.context(), LexicalGoal::Div),
            lexer,
            names: NameTable::new(),
            token: first,
            previous_end: None,
            previous_end_location: None,
            in_mode: InMode::Allow,
            program_kind: kind,
            stack: Vec::new(),
            scopes: Vec::new(),
            decls: Vec::new(),
            functions: Vec::new(),
            module: is_module.then(ModuleRecord::default),
            module_export: None,
            has_with: false,
            next_label: 0,
            lookahead: RefCell::default(),
            stack_guard: ParserStackGuard::new(),
        };
        let root_kind = match kind {
            ProgramKind::Script => FunctionKind::Script,
            ProgramKind::Module => FunctionKind::Module,
            ProgramKind::Eval => FunctionKind::Eval,
        };
        let id = parser.begin_function(root_kind, is_module, None);
        if is_module {
            // QuickJS compiles a module as an async function.
            let state = parser.state_mut();
            state.is_async = true;
            state.in_function_body = true;
        }
        let strict = is_module || parser.directive_prologue_has_use_strict(is_module)?;
        parser.relex_current_with_strict(strict)?;
        parser.state_mut().strict = strict;
        let body = if is_module {
            parser.parse_module_body()?
        } else {
            parser.parse_script_body()?
        };
        let state = parser.end_function();
        let end = parser.lexer.source().len() as u32;
        let root = Box::new(Function {
            id,
            kind: root_kind,
            role: MethodRole::Method,
            is_async: state.is_async,
            is_generator: false,
            name: None,
            self_binding: None,
            params: Vec::new(),
            rest: None,
            simple_params: true,
            length: 0,
            body,
            concise: false,
            strict,
            span: source_span,
            source: (0, end),
        });
        if is_module {
            parser.resolve_module_exports()?;
        }
        Ok(Program {
            kind,
            root,
            names: parser.names,
            scopes: parser.scopes,
            decls: parser.decls,
            functions: parser.functions,
            module: parser.module,
            has_with: parser.has_with,
        })
    }

    // ---- functions and scopes ----

    pub(super) fn state(&self) -> &FunctionState {
        self.stack.last().expect("a function is being parsed")
    }

    pub(super) fn state_mut(&mut self) -> &mut FunctionState {
        self.stack.last_mut().expect("a function is being parsed")
    }

    pub(super) fn strict(&self) -> bool {
        self.state().strict
    }

    /// Starts a function: its root and body scopes, inheriting strictness.
    pub(super) fn begin_function(
        &mut self,
        kind: FunctionKind,
        strict: bool,
        definition_scope: Option<ScopeId>,
    ) -> FuncId {
        let id = FuncId(self.functions.len() as u32);
        let parent = self.stack.last().map(|state| state.id);
        let var_scope = self.new_scope(None, ScopeKind::FunctionRoot, id);
        let body_kind = match kind {
            FunctionKind::Script | FunctionKind::Module | FunctionKind::Eval => {
                ScopeKind::ProgramBody
            }
            _ => ScopeKind::FunctionBody,
        };
        let body_scope = self.new_scope(Some(var_scope), body_kind, id);
        self.functions.push(FunctionInfo {
            parent,
            kind,
            var_scope,
            body_scope,
            parameter_scope: None,
            definition_scope,
            strict,
        });
        let (super_call_allowed, super_allowed) = match kind {
            FunctionKind::ClassConstructor { derived } => (derived, true),
            FunctionKind::Method
            | FunctionKind::ClassFieldInit { .. }
            | FunctionKind::StaticBlock => (false, true),
            FunctionKind::Arrow => self
                .stack
                .last()
                .map_or((false, false), |p| (p.super_call_allowed, p.super_allowed)),
            _ => (false, false),
        };
        let arguments_forbidden = match kind {
            FunctionKind::ClassFieldInit { .. } | FunctionKind::StaticBlock => true,
            FunctionKind::Arrow => self.stack.last().is_some_and(|p| p.arguments_forbidden),
            _ => false,
        };
        self.stack.push(FunctionState {
            id,
            kind,
            is_async: false,
            is_generator: false,
            strict,
            current_scope: body_scope,
            var_scope,
            body_scope,
            parameter_scope: None,
            in_function_body: false,
            super_call_allowed,
            super_allowed,
            arguments_forbidden,
            breaks: Vec::new(),
            parameter_names: Vec::new(),
            pattern_parameter_initialization: false,
            simple_params: true,
            global_declarations: Vec::new(),
        });
        id
    }

    /// Ends the innermost function, recording its final strictness.
    pub(super) fn end_function(&mut self) -> FunctionState {
        let state = self.stack.pop().expect("a function is being parsed");
        let info = &mut self.functions[state.id.index()];
        info.strict = state.strict;
        info.parameter_scope = state.parameter_scope;
        state
    }

    pub(super) fn new_scope(
        &mut self,
        parent: Option<ScopeId>,
        kind: ScopeKind,
        function: FuncId,
    ) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope {
            parent,
            kind,
            function,
            decls: Vec::new(),
        });
        id
    }

    pub(super) fn push_scope(&mut self, kind: ScopeKind) -> ScopeId {
        let state = self.state();
        let (parent, function) = (state.current_scope, state.id);
        let scope = self.new_scope(Some(parent), kind, function);
        self.state_mut().current_scope = scope;
        scope
    }

    pub(super) fn pop_scope(&mut self, expected: ScopeId) -> Result<(), Error> {
        if self.state().current_scope != expected {
            return Err(Error::internal("parser scope stack is unbalanced"));
        }
        let parent = self.scopes[expected.index()]
            .parent
            .ok_or_else(|| Error::internal("cannot pop a function root scope"))?;
        self.state_mut().current_scope = parent;
        Ok(())
    }

    pub(super) fn current_scope(&self) -> ScopeId {
        self.state().current_scope
    }

    pub(super) fn new_label(&mut self) -> LabelId {
        let id = LabelId(self.next_label);
        self.next_label += 1;
        id
    }

    pub(super) fn decl_in_scope(&self, scope: ScopeId, name: NameId) -> Option<DeclId> {
        self.scopes[scope.index()]
            .decls
            .iter()
            .rev()
            .copied()
            .find(|decl| self.decls[decl.index()].name == name)
    }

    /// The nearest declaration of `name` from `scope` up to its function's
    /// root, with the scope that holds it.
    pub(super) fn decl_from_scope(
        &self,
        mut scope: ScopeId,
        name: NameId,
    ) -> Option<(ScopeId, DeclId)> {
        loop {
            if let Some(decl) = self.decl_in_scope(scope, name) {
                return Some((scope, decl));
            }
            scope = self.scopes[scope.index()].parent?;
        }
    }

    /// Whether `inner` is `outer` or nested in it.
    pub(super) fn scope_is_within(&self, mut inner: ScopeId, outer: ScopeId) -> bool {
        loop {
            if inner == outer {
                return true;
            }
            match self.scopes[inner.index()].parent {
                Some(parent) => inner = parent,
                None => return false,
            }
        }
    }

    pub(super) fn syntax_here(&self, message: impl Into<String>) -> Error {
        Error::syntax(message, self.current().span)
    }

    /// The span from `start` to the end of the previous token.
    pub(super) fn span_from(&self, start: Span) -> Span {
        let mut span = start;
        if let Some(end) = self.previous_end_location
            && end.byte_offset >= start.start.byte_offset
        {
            span.end = end;
        }
        span
    }
}

pub(super) fn pos(span: Span) -> u32 {
    span.start.byte_offset as u32
}

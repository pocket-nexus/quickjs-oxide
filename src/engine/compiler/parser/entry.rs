//! Root compilation and consuming parser completion.

use crate::engine::compiler::ModuleImportAttributeChecker;
use crate::engine::code::function::metadata::FunctionKind as BytecodeFunctionKind;
use crate::engine::api::error::Error;
use crate::engine::code::function::metadata::EvalCallerProfile;
use crate::engine::code::function::metadata::EvalCallerVariableTarget;
use crate::engine::compiler::EvalCompileContext;
use crate::engine::code::function::metadata::EvalKind;
use crate::engine::code::function::metadata::EvalRootBinding;
use crate::engine::compiler::parser::builder::FunctionBuilder;
use crate::engine::compiler::FunctionIrOptions;
use crate::engine::compiler::FunctionKind;
use crate::engine::compiler::FunctionSourceInfo;
use crate::engine::compiler::FunctionTree;
use crate::engine::compiler::InMode;
use crate::engine::value::JsString;
use crate::engine::compiler::lexer::LexContext;
use crate::engine::compiler::lexer::Lexer;
use crate::engine::compiler::lexer::LexerOptions;
use crate::engine::compiler::ModuleCompileFailure;
use crate::engine::compiler::ModuleDeclarationExport;
use crate::engine::compiler::Parser;
use crate::engine::compiler::RootCompileContext;
use crate::source::SourceOffset;
use crate::source::text::SourceText;
use crate::engine::compiler::SuperCapabilities;
use crate::engine::compiler::install_eval_external_bindings;
use crate::engine::compiler::lex_error;
use crate::engine::compiler::module;
use crate::engine::compiler::validate_source_length;

impl<'source> Parser<'source> {
    pub(in crate::engine::compiler) fn parse(source: &'source str, filename: JsString) -> Result<FunctionTree, Error> {
        Self::parse_root(source, None, filename, RootCompileContext::Script, None)
            .map_err(ModuleCompileFailure::into_engine_without_checker)
    }

    pub(in crate::engine::compiler) fn parse_module(
        source: &'source str,
        filename: JsString,
        checker: Option<&mut dyn ModuleImportAttributeChecker>,
    ) -> Result<FunctionTree, ModuleCompileFailure> {
        Self::parse_root(source, None, filename, RootCompileContext::Module, checker)
    }

    pub(in crate::engine::compiler) fn parse_module_source(
        source: &'source SourceText,
        filename: JsString,
        checker: Option<&mut dyn ModuleImportAttributeChecker>,
    ) -> Result<FunctionTree, ModuleCompileFailure> {
        Self::parse_root(
            source.carrier(),
            Some(source),
            filename,
            RootCompileContext::Module,
            checker,
        )
    }

    pub(in crate::engine::compiler) fn parse_script_source(
        source: &'source SourceText,
        filename: JsString,
    ) -> Result<FunctionTree, Error> {
        Self::parse_root(
            source.carrier(),
            Some(source),
            filename,
            RootCompileContext::Script,
            None,
        )
        .map_err(ModuleCompileFailure::into_engine_without_checker)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::engine::compiler) fn parse_eval(
        source: &'source str,
        filename: JsString,
        context: EvalCompileContext,
    ) -> Result<FunctionTree, Error> {
        Self::parse_root(
            source,
            None,
            filename,
            RootCompileContext::Eval(context),
            None,
        )
        .map_err(ModuleCompileFailure::into_engine_without_checker)
    }

    pub(in crate::engine::compiler) fn parse_eval_source(
        source: &'source SourceText,
        filename: JsString,
        context: EvalCompileContext,
    ) -> Result<FunctionTree, Error> {
        Self::parse_root(
            source.carrier(),
            Some(source),
            filename,
            RootCompileContext::Eval(context),
            None,
        )
        .map_err(ModuleCompileFailure::into_engine_without_checker)
    }

    pub(in crate::engine::compiler) fn parse_root(
        source: &'source str,
        source_text: Option<&'source SourceText>,
        filename: JsString,
        context: RootCompileContext,
        mut module_attribute_checker: Option<&mut dyn ModuleImportAttributeChecker>,
    ) -> Result<FunctionTree, ModuleCompileFailure> {
        validate_source_length(source.len())?;
        let is_module = matches!(&context, RootCompileContext::Module);
        let (
            root_kind,
            inherited_strict,
            external_bindings,
            caller_profile,
            super_capabilities,
            arguments_forbidden,
        ) = match context {
            RootCompileContext::Script => (
                FunctionKind::Script,
                false,
                Vec::<EvalRootBinding<JsString>>::new().into_boxed_slice(),
                EvalCallerProfile {
                    scope_kinds: Box::new([]),
                    variable_target: EvalCallerVariableTarget::Global,
                },
                SuperCapabilities::NONE,
                false,
            ),
            RootCompileContext::Module => (
                FunctionKind::Module,
                true,
                Vec::<EvalRootBinding<JsString>>::new().into_boxed_slice(),
                EvalCallerProfile {
                    scope_kinds: Box::new([]),
                    variable_target: EvalCallerVariableTarget::StrictLocal,
                },
                SuperCapabilities::NONE,
                false,
            ),
            RootCompileContext::Eval(context) => {
                if !matches!(context.kind, EvalKind::Direct | EvalKind::Indirect) {
                    return Err(
                        Error::internal("eval compiler received a non-eval root kind").into(),
                    );
                }
                if context.kind == EvalKind::Indirect
                    && (!context.bindings.is_empty()
                        || !context.caller_profile.scope_kinds.is_empty()
                        || context.caller_profile.variable_target
                            != EvalCallerVariableTarget::Global
                        || context.super_call_allowed
                        || context.super_allowed
                        || context.arguments_forbidden)
                {
                    return Err(Error::internal(
                        "indirect eval compiler received a caller environment",
                    )
                    .into());
                }
                let super_capabilities = SuperCapabilities {
                    super_call_allowed: context.super_call_allowed,
                    super_allowed: context.super_allowed,
                }
                .validated()
                .map_err(|_| {
                    Error::internal("eval compiler permits super() without SuperProperty")
                })?;
                (
                    FunctionKind::Eval(context.kind),
                    context.kind == EvalKind::Direct && context.caller_strict,
                    context.bindings,
                    context.caller_profile,
                    super_capabilities,
                    context.arguments_forbidden,
                )
            }
        };
        // QuickJS enables Annex B HTML comments for every Script and Eval
        // parse, independently of strict mode. Module parsing keeps them
        // disabled (`allow_html_comments = !is_module`).
        let lexer_options = LexerOptions {
            context: LexContext {
                strict: inherited_strict,
                module: is_module,
                ..LexContext::default()
            },
            allow_html_comments: !is_module,
        };
        let mut lexer = match source_text {
            Some(source) => Lexer::with_source_text(source, lexer_options),
            None => Lexer::with_options(source, lexer_options),
        };
        let first_token = lexer.next_token().map_err(lex_error)?;
        let source_span = first_token.span;
        let mut parser = Self {
            lexer,
            tokens: vec![first_token],
            cursor: 0,
            current_function: 0,
            in_mode: InMode::Allow,
            anonymous_function_definition: None,
            pending_unsupported: None,
            module: is_module.then(module::IrModule::default),
            module_declaration_export: ModuleDeclarationExport::None,
            module_declaration_export_target: None,
            functions: vec![FunctionBuilder::new(
                None,
                root_kind,
                FunctionSourceInfo {
                    span: source_span,
                    definition: SourceOffset::try_from_usize(0)
                        .map_err(|error| Error::internal(error.to_string()))?,
                    range: None,
                },
                FunctionIrOptions {
                    function_name: (!is_module).then(|| "<eval>".to_owned()),
                    private_name_binding: false,
                    class_constructor: false,
                    derived_class_constructor: false,
                    parameters: Vec::new(),
                    defined_argument_count: 0,
                    has_simple_parameter_list: true,
                    rest_parameter: None,
                    strict: inherited_strict,
                    super_capabilities,
                },
            )?],
        };
        if is_module {
            // QuickJS compiles every module root as an async function. The
            // separate module record bit records whether authored evaluation
            // can actually suspend; keeping the callable kind async here lets
            // top-level AwaitExpression and `for await` reuse the ordinary
            // async-function lowering and continuation machinery.
            parser.functions[0].execution_kind = BytecodeFunctionKind::Async;
            parser.functions[0].context.in_function_body = true;
            parser.functions[0].eval_caller_profile = caller_profile.clone();
        }
        if matches!(root_kind, FunctionKind::Eval(_)) {
            install_eval_external_bindings(
                &mut parser.functions[0],
                external_bindings,
                caller_profile,
                inherited_strict,
            )?;
        }
        let strict =
            inherited_strict || parser.directive_prologue_has_use_strict(0, inherited_strict)?;
        parser.relex_current_with_strict(strict)?;
        parser.functions[0].strict = strict;
        parser.functions[0].arguments_forbidden = arguments_forbidden;
        if is_module {
            parser.parse_module_body(&mut module_attribute_checker)?;
        } else {
            parser.parse_script_body()?;
        }
        Ok(FunctionTree {
            functions: parser.functions.into_iter().map(FunctionBuilder::finish).collect::<Result<_, _>>()?,
            source: source_text
                .cloned()
                .unwrap_or_else(|| SourceText::from_utf8(source)),
            filename,
            module: parser.module,
            pending_unsupported: parser.pending_unsupported,
        })
    }

}

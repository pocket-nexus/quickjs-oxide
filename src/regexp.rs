//! The regular-expression engine of the `oxide-regexp` crate at its
//! historical path, plus the two entry points that need engine types: the
//! `JsString` compile wrapper and the `ErrorKind` classification.

pub use oxide_regexp::{
    CharacterRange, CompileError, CompileErrorKind, CompileErrorSource, CompiledRegExp, ExecError,
    GroupName, Instruction, ProgramError, RegExpFlags, RegExpMatch, ValidatedProgram, execute,
    execute_latin1_with_interrupt, execute_with_interrupt, javascript_compile_error_message,
};

use crate::engine::api::error::ErrorKind;
use crate::engine::value::JsString;

/// Compile exact UTF-16 pattern and flag strings without consulting runtime
/// objects or host regular-expression facilities.
pub fn compile(pattern: &JsString, flags: &JsString) -> Result<CompiledRegExp, CompileError> {
    oxide_regexp::compile(
        &pattern.utf16_units().collect::<Vec<_>>(),
        &flags.utf16_units().collect::<Vec<_>>(),
    )
}

/// Classify a pure-Rust compiler failure at the JavaScript compilation
/// boundary.
#[must_use]
pub const fn javascript_compile_error_kind(error: &CompileError) -> ErrorKind {
    match error.kind() {
        CompileErrorKind::Internal => ErrorKind::JsInternal,
        CompileErrorKind::Syntax
        | CompileErrorKind::TooManyCaptures
        | CompileErrorKind::TooManyRegisters => ErrorKind::Syntax,
    }
}

//! Pure Rust RegExp engine for pinned QuickJS 2026-06-04, and the
//! checksum-pinned Unicode 17 tables it and the String intrinsics use.
//!
//! The crate is runtime independent: it owns typed flags, compiler errors,
//! instruction IR and the executor, all over UTF-16 code units (`&[u16]`) and
//! plain `u32` code points. Heap objects and ECMAScript intrinsics are wired
//! by the engines that depend on it (the root crate and `oxide-core`).

#![forbid(unsafe_code)]

mod compiler;
mod executor;
mod flags;
mod group_name;
mod opcode;

pub use compiler::{
    CompileError, CompileErrorKind, CompileErrorSource, CompiledRegExp, ValidatedProgram,
};
pub use executor::{
    ExecError, ProgramError, RegExpMatch, execute, execute_latin1_with_interrupt,
    execute_with_interrupt,
};
pub use flags::RegExpFlags;
pub use group_name::GroupName;
pub use opcode::{CharacterRange, Instruction};

pub mod unicode;

/// Compile exact UTF-16 pattern and flag code units without consulting
/// runtime objects or host regular-expression facilities.
pub fn compile(pattern: &[u16], flags: &[u16]) -> Result<CompiledRegExp, CompileError> {
    compiler::compile_units(pattern, flags)
}

/// Return the short diagnostic exposed by pinned QuickJS `lre_compile`.
///
/// The typed compiler retains source and UTF-16 position metadata in its
/// `Display` form; JavaScript compilation deliberately exposes only this
/// QuickJS-compatible message.
#[must_use]
pub fn javascript_compile_error_message(error: &CompileError) -> &str {
    if error.source() == CompileErrorSource::Flags {
        return "invalid regular expression flags";
    }
    match error.message() {
        "unexpected end of pattern" | "unterminated character class" | "trailing backslash" => {
            "unexpected end"
        }
        "unterminated group" => "expecting ')'",
        "unexpected closing parenthesis" => "extraneous characters at the end",
        "invalid group specifier" => "invalid group",
        "regular expression syntax error" => "syntax error",
        "invalid quantifier target" => "nothing to repeat",
        "invalid control escape"
        | "invalid hexadecimal escape"
        | "invalid Unicode escape"
        | "invalid identity escape" => "invalid escape sequence in regular expression",
        message => message,
    }
}

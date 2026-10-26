//! The engine's compile-time limits, as QuickJS sets them.

use crate::engine::api::error::{Error, ErrorKind};

/// QuickJS 2026-06-04 `JS_MAX_LOCAL_VARS`: the most locals one function
/// declares.
pub(crate) const MAX_LOCAL_VARIABLES: usize = 65_534;

/// QuickJS 2026-06-04 `JS_STACK_SIZE_MAX`: the deepest operand stack one
/// function's code reaches; a call's operands count against it.
pub(crate) const MAX_BYTECODE_STACK: usize = 65_534;

/// Rejects source buffers which cannot fit QuickJS's signed debug-source
/// length, before any byte-oriented carrier allocation is attempted.
pub fn validate_source_length(source_len: usize) -> Result<(), Error> {
    i32::try_from(source_len).map(|_| ()).map_err(|_| {
        Error::new(
            ErrorKind::JsInternal,
            "source is too large for QuickJS debug metadata",
        )
    })
}

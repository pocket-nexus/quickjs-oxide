//! Source parsing and construction-only function state.

pub(super) mod builder;
pub(super) mod context;
mod entry;
mod statements;
mod loops;
mod control;
mod declarations;
mod expressions;
mod calls;
mod literals;
mod tokens;
mod parameters;
mod scopes;

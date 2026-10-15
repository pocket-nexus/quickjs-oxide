use std::ffi::OsStr;
use std::process::{Command, Output};

// Keep source in argv so parser diagnostics retain qjs's <cmdline> filename,
// line and column behavior. Callers compare the complete process result.
pub(crate) fn run_cli_exact(program: &OsStr, source: &str, description: &str) -> Output {
    Command::new(program)
        .args(["-e", source])
        .output()
        .unwrap_or_else(|error| panic!("could not run CLI for {description}: {error}"))
}

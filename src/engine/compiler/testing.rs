//! Opt-in access to the production parser, without resolution or publication.
use super::model::ir::function::FunctionTree;
use super::parser::context::Parser;
use crate::engine::api::error::Error;
use crate::engine::value::JsString;

/// Owns the complete production parse result, including source and name storage.
/// Keep this alive until after stopping a benchmark's timer.
pub struct ParsedScriptForBenchmark {
    _tree: FunctionTree,
}

/// Parse an entire Script, including every nested function body.
/// Parser/name-table construction and source retention are part of this call.
pub fn parse_script_for_benchmark(source: &str) -> Result<ParsedScriptForBenchmark, Error> {
    let mut tree = Parser::parse(source, JsString::from_static("<frontend-benchmark>"))?;
    if let Some(error) = tree.pending_unsupported.take() {
        return Err(error);
    }
    Ok(ParsedScriptForBenchmark { _tree: tree })
}

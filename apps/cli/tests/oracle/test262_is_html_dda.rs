use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{eval, string_text as text};

const FIXTURE: &str = include_str!("../fixtures/inputs/is_html_dda.js");
const QUICKJS_2026_06_04: &str =
    include_str!("../fixtures/expected/is_html_dda.quickjs-2026-06-04.txt");

#[test]
fn is_html_dda_semantics_match_pinned_quickjs_transcript() {
    let (_runtime, mut context) = host_runtime_context();
    context
        .install_test262_host()
        .expect("install Test262 host surface");

    // `Value::to_boolean` sees only an object handle: ask the engine.
    assert_eq!(
        eval(&mut context, "!$262.IsHTMLDDA"),
        quickjs_oxide::engine::api::core::Value::Bool(true)
    );

    drop(eval(&mut context, FIXTURE));
    let transcript = text(eval(&mut context, "isHtmlDdaTranscript.join('\\n')"));
    assert_eq!(format!("{transcript}\n"), QUICKJS_2026_06_04);
}

use crate::runtime_observation::{eval, string_text as text};
use quickjs_oxide::engine::api::Runtime;

const FIXTURE: &str = include_str!("../fixtures/inputs/is_html_dda.js");
const QUICKJS_2026_06_04: &str =
    include_str!("../fixtures/expected/is_html_dda.quickjs-2026-06-04.txt");

#[test]
fn is_html_dda_semantics_match_pinned_quickjs_transcript() {
    let runtime =
        Runtime::new_with_host_services(quickjs_oxide_host::SystemHostServices::default());
    let mut context = runtime.new_context().expect("create context");
    context
        .install_test262_host()
        .expect("install Test262 host surface");

    let html_dda = eval(&mut context, "$262.IsHTMLDDA");
    assert!(!html_dda.to_boolean().unwrap());

    drop(eval(&mut context, FIXTURE));
    let transcript = text(eval(&mut context, "isHtmlDdaTranscript.join('\\n')"));
    assert_eq!(format!("{transcript}\n"), QUICKJS_2026_06_04);
}

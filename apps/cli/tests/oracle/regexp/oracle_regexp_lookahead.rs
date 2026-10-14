use crate::oracle_cases::{CompileCase, StartMatchCase as MatchCase};
use crate::runtime_observation::utf16;
use crate::support::regexp_probe_oracle::{compare_lookaround, self_check_lookaround};

// Differential lock for pinned QuickJS 2026-06-04 forward lookahead.
//
// QuickJS compiles assertions as begin/end opcode pairs in `libregexp.c`
// `re_parse_term` (1901-1981), then executes them with typed positive and
// negative backtracking frames (2704-2925, 2967-2978). These vectors pin the
// semantics that are easy to lose when adapting that model to Rust: zero-width
// input restoration, positive capture retention, rollback on every negative or
// abandoned path, positive assertion atomicity, nesting, Annex B quantified
// assertions, scoped modifiers, and UTF-16/code-point boundaries.

const COMPILE_CASES: &[CompileCase] = &[
    CompileCase {
        label: "positive lookahead is valid in Unicode mode",
        pattern: "(?=.)",
        flags: "u",
        expected: "ok",
    },
    CompileCase {
        label: "negative lookahead is valid in Unicode mode",
        pattern: "(?!.)",
        flags: "u",
        expected: "ok",
    },
    CompileCase {
        label: "Annex B permits a quantified positive lookahead",
        pattern: "(?=.)*",
        flags: "",
        expected: "ok",
    },
    CompileCase {
        label: "Annex B permits a lazy quantified negative lookahead",
        pattern: "(?!.)??",
        flags: "",
        expected: "ok",
    },
    CompileCase {
        label: "Unicode positive lookahead star is a syntax error",
        pattern: "(?=.)*",
        flags: "u",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "Unicode negative lookahead plus is a syntax error",
        pattern: "(?!.)+",
        flags: "u",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "Unicode positive lookahead brace quantifier is a syntax error",
        pattern: "(?=.){1,2}",
        flags: "u",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "Unicode lazy lookahead quantifier is a syntax error",
        pattern: "(?=.)??",
        flags: "u",
        expected: "SyntaxError",
    },
];

#[test]
fn regexp_lookahead_oracle_vectors_self_check() {
    let oracle = require_qjs_oracle!("RegExp lookahead oracle self-check");
    self_check_lookaround(&oracle, "lookahead", &match_cases(), COMPILE_CASES);
}

#[test]
fn regexp_lookahead_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("RegExp lookahead differential");
    compare_lookaround(
        &oracle,
        "lookahead",
        "RegExp forward lookahead",
        &match_cases(),
        COMPILE_CASES,
    );
}

fn match_cases() -> Vec<MatchCase> {
    vec![
        ascii(
            "positive assertion succeeds without consuming twice",
            r"^a(?=b)b$",
            "",
            "ab",
            0,
        ),
        ascii(
            "positive assertion failure rejects the branch",
            r"^a(?=b)c$",
            "",
            "ac",
            0,
        ),
        ascii(
            "negative assertion succeeds after its body fails",
            r"^a(?!b)c$",
            "",
            "ac",
            0,
        ),
        ascii(
            "negative assertion fails after its body succeeds",
            r"^a(?!b)b$",
            "",
            "ab",
            0,
        ),
        ascii(
            "positive assertion retains captures for an outer backreference",
            r"^(?=(a+))\1$",
            "",
            "aaa",
            0,
        ),
        ascii(
            "positive body failure rolls its capture back before an outer alternative",
            r"^(?:(?=(a))b|a)\1?$",
            "",
            "a",
            0,
        ),
        ascii(
            "outer backtracking rolls a committed positive capture back",
            r"^(?:(?=(a))abX|a)\1?$",
            "",
            "a",
            0,
        ),
        ascii(
            "negative body failure rolls its temporary capture back",
            r"^(?!(a)b)a\1?$",
            "",
            "a",
            0,
        ),
        ascii(
            "negative body success rolls its capture back before an outer alternative",
            r"^(?:(?!(a))b|a)\1?$",
            "",
            "a",
            0,
        ),
        ascii(
            "positive assertion is atomic after choosing its first alternative",
            r"^(?=(a|aa))\1b$",
            "",
            "aab",
            0,
        ),
        ascii(
            "positive assertion succeeds when its chosen alternative is sufficient",
            r"^(?=(aa|a))\1b$",
            "",
            "aab",
            0,
        ),
        ascii(
            "negative assertion exhausts internal alternatives before succeeding",
            r"^(?!(a|aa)c)aab$",
            "",
            "aab",
            0,
        ),
        ascii(
            "negative assertion fails when a later internal alternative succeeds",
            r"^(?!(a|aa)b)aab$",
            "",
            "aab",
            0,
        ),
        ascii(
            "nested positive assertions retain the innermost capture",
            r"^(?=(?=(a))\1)\1$",
            "",
            "a",
            0,
        ),
        ascii(
            "positive assertion can contain a successful negative assertion",
            r"^(?=(?!b)(a))\1$",
            "",
            "a",
            0,
        ),
        ascii(
            "Annex B quantified positive assertion remains zero width",
            r".(?=Z)+",
            "",
            "a bZ cZZ",
            0,
        ),
        ascii(
            "Annex B quantified negative assertion may choose zero repetitions",
            r".(?!Z)*",
            "",
            "aZ",
            0,
        ),
        ascii(
            "zero-count optional assertion resets its nested capture",
            r"(?:(?=(abc)))?a",
            "",
            "abc",
            0,
        ),
        ascii(
            "one exact assertion repetition retains its nested capture",
            r"(?:(?=(abc))){1,1}a",
            "",
            "abc",
            0,
        ),
        ascii(
            "empty Annex B lookahead star terminates",
            r"^(?=)*$",
            "",
            "",
            0,
        ),
        ascii(
            "Unicode quantified wrapper stops after a zero-advance assertion",
            r"^(?:(?=(a)))*a$",
            "u",
            "a",
            0,
        ),
        ascii(
            "scoped ignoreCase applies to an enclosing lookahead",
            r"^(?i:(?=a)a)$",
            "",
            "A",
            0,
        ),
        ascii(
            "scoped ignoreCase is restored after the lookahead group",
            r"^(?i:(?=(a)))a$",
            "",
            "A",
            0,
        ),
        ascii(
            "capture text leaves an ignoreCase scope intact",
            r"^(?=(?i:(a)))\1$",
            "",
            "A",
            0,
        ),
        ascii(
            "scoped removal of global ignoreCase applies inside lookahead",
            r"^(?=(?-i:a))a$",
            "i",
            "A",
            0,
        ),
        ascii(
            "Unicode lookahead dot captures one astral code point",
            r"^(?=(.)).$",
            "u",
            "😀",
            0,
        ),
        ascii(
            "legacy lookahead dot captures one astral UTF-16 code unit",
            r"^(?=(.))..$",
            "",
            "😀",
            0,
        ),
        ascii(
            "Unicode ignoreCase folds an astral capture and backreference",
            r"^(?=(𐐀))\1$",
            "iu",
            "𐐨",
            0,
        ),
        ascii(
            "Unicode search start inside a surrogate pair normalizes before lookahead",
            r"(?=😀)😀",
            "u",
            "x😀",
            2,
        ),
        ascii("empty positive assertion succeeds", r"^(?=)a$", "", "a", 0),
        ascii("empty negative assertion fails", r"^(?!)a$", "", "a", 0),
    ]
}

fn ascii(
    label: &'static str,
    pattern: &str,
    flags: &'static str,
    input: &str,
    start: usize,
) -> MatchCase {
    MatchCase {
        label,
        pattern: utf16(pattern),
        flags,
        input: utf16(input),
        start,
    }
}

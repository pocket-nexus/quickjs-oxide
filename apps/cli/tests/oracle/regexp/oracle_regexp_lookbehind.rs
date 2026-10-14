use crate::oracle_cases::{CompileCase, StartMatchCase as MatchCase};
use crate::runtime_observation::utf16;
use crate::support::regexp_probe_oracle::{compare_lookaround, self_check_lookaround};

// Differential lock for pinned QuickJS 2026-06-04 backward lookaround.
//
// QuickJS compiles a lookbehind through the same typed assertion frames as a
// lookahead, but reverses each alternative's term order, brackets consuming
// atoms with `prev`, swaps capture boundaries, and selects backward
// backreferences. These vectors pin the interactions that are easiest to lose
// in a Rust rewrite: variable-length reverse matching, left-to-right
// alternative priority, reverse greediness, assertion atomicity and capture
// rollback, forward and mutually recursive references, nested direction
// changes, anchors, word boundaries, and UTF-16/code-point positioning.

const COMPILE_CASES: &[CompileCase] = &[
    CompileCase {
        label: "positive lookbehind is valid in legacy mode",
        pattern: "(?<=a)",
        flags: "",
        expected: "ok",
    },
    CompileCase {
        label: "negative lookbehind is valid in legacy mode",
        pattern: "(?<!a)",
        flags: "",
        expected: "ok",
    },
    CompileCase {
        label: "positive lookbehind is valid in Unicode mode",
        pattern: "(?<=.)",
        flags: "u",
        expected: "ok",
    },
    CompileCase {
        label: "negative lookbehind is valid in Unicode mode",
        pattern: "(?<!.)",
        flags: "u",
        expected: "ok",
    },
    CompileCase {
        label: "a grouped lookbehind can be quantified",
        pattern: "(?:(?<=a))*",
        flags: "",
        expected: "ok",
    },
    CompileCase {
        label: "legacy positive lookbehind star is a syntax error",
        pattern: "(?<=a)*",
        flags: "",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "legacy negative lookbehind optional is a syntax error",
        pattern: "(?<!a)?",
        flags: "",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "Unicode positive lookbehind plus is a syntax error",
        pattern: "(?<=a)+",
        flags: "u",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "legacy negative lookbehind brace quantifier is a syntax error",
        pattern: "(?<!a){1,2}",
        flags: "",
        expected: "SyntaxError",
    },
    CompileCase {
        label: "legacy lazy lookbehind quantifier is a syntax error",
        pattern: "(?<=a)??",
        flags: "",
        expected: "SyntaxError",
    },
];

#[test]
fn regexp_lookbehind_oracle_vectors_self_check() {
    let oracle = require_qjs_oracle!("RegExp lookbehind oracle self-check");
    self_check_lookaround(&oracle, "lookbehind", &match_cases(), COMPILE_CASES);
}

#[test]
fn regexp_lookbehind_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("RegExp lookbehind differential");
    compare_lookaround(
        &oracle,
        "lookbehind",
        "RegExp backward lookaround",
        &match_cases(),
        COMPILE_CASES,
    );
}

fn match_cases() -> Vec<MatchCase> {
    vec![
        ascii(
            "positive fixed-length lookbehind succeeds without consuming its body",
            r"(?<=abc)def",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "positive fixed-length lookbehind rejects a mismatched body",
            r"(?<=abd)def",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "negative lookbehind succeeds where its body cannot match",
            r"(?<!abc)\w{3}",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "negative lookbehind rejects a matching predecessor",
            r"(?<!abc)def",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "unbounded variable-length lookbehind reaches the available prefix",
            r"(?<=\w*)[^abc]{3}",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "a greedy reverse loop captures the complete suffix",
            r"(?<=(b+))c",
            "",
            "abbbbbbc",
            0,
        ),
        ascii(
            "a lazy reverse loop stops at the nearest predecessor",
            r"(?<=(b+?))c",
            "",
            "abbbc",
            0,
        ),
        ascii(
            "reverse execution makes the right capture greedy first",
            r"(?<=([ab]+)([bc]+))$",
            "",
            "abc",
            0,
        ),
        ascii(
            "a repeated capture retains the leftmost reverse iteration",
            r"(?<=(\w){3})def",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "lookbehind alternatives retain source-order priority",
            r".*(?<=(..|...|....))(.*)",
            "",
            "xabcd",
            0,
        ),
        ascii(
            "a later lookbehind alternative runs after the first one fails",
            r".*(?<=(xx|...|....))(.*)",
            "",
            "xabcd",
            0,
        ),
        ascii(
            "a successful lookbehind is atomic under outer failure",
            r"(?<=([abc]+)).\1",
            "",
            "abcdbc",
            0,
        ),
        ascii(
            "a positive lookbehind retains captures for the outer expression",
            r"(?<=(c))def\1?",
            "",
            "abcdefc",
            0,
        ),
        ascii(
            "a successful negative lookbehind rolls its body capture back",
            r"(?<!(^|[ab]))\w{2}",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "outer backtracking rolls an abandoned positive capture back",
            r"(?:(?<=(a))bX|b)\1?$",
            "",
            "ab",
            0,
        ),
        ascii(
            "a numeric forward reference executes after its capture in reverse order",
            r"(?<=\1(\w))d",
            "i",
            "abcCd",
            0,
        ),
        ascii(
            "a variable capture satisfies a numeric forward reference",
            r"(?<=\1(\w+))c",
            "",
            "ababc",
            0,
        ),
        ascii(
            "an outer capture can be compared by a backward backreference",
            r"(.)(?<=(\1\1))",
            "",
            "abb",
            0,
        ),
        ascii(
            "mutually recursive captures preserve QuickJS undefined-reference semantics",
            r"(?<=a(.\2)b(\1)).{4}",
            "",
            "aabcacbc",
            0,
        ),
        ascii(
            "a forward reference and a later backreference can share reverse state",
            r"(?<=a(\2)b(..\1))b",
            "",
            "aacbacb",
            0,
        ),
        ascii(
            "a forward self-reference is empty before its capture is established",
            r"(?<=(?:\1b)(aa)).",
            "",
            "aabaax",
            0,
        ),
        ascii(
            "a backward body can switch to a nested forward lookahead",
            r"(?<=ab(?=c)\wd)\w\w",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "nested lookahead and lookbehind retain their successful capture",
            r"(?<=a(?=([bc]{2}(?<!a{2}))d)\w{3})\w\w",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "a failing nested negative lookbehind rejects the outer assertion",
            r"(?<=a(?=([bc]{2}(?<!a*))d)\w{3})\w\w",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "a forward lookahead can contain a backward lookbehind",
            r"(?=(?<=ab)c)c",
            "",
            "abc",
            0,
        ),
        ascii(
            "start-of-input anchors are evaluated at the reversed position",
            r"(?<=^abc)def",
            "",
            "abcdef",
            0,
        ),
        ascii(
            "multiline start anchors work inside lookbehind",
            r"(?<=^[a-c]{3})def",
            "m",
            "xyz\nabcdef",
            0,
        ),
        ascii(
            "a variable reverse loop can extend to a start anchor",
            r"^foo(?<=^fo+)$",
            "",
            "foo",
            0,
        ),
        ascii(
            "end-of-input remains zero width after a lookbehind",
            r"^foo(?<=foo)$",
            "",
            "foo",
            0,
        ),
        ascii(
            "a word boundary can be tested inside lookbehind",
            r"(?<=\b)[d-f]{3}",
            "",
            "abc def",
            0,
        ),
        ascii(
            "a non-word boundary can be tested inside lookbehind",
            r"(?<=\B)\w{3}",
            "",
            "ab cdef",
            0,
        ),
        ascii(
            "nested backward assertions share the correct word-boundary position",
            r"(?<=\B)(?<=c(?<=\w))\w{3}",
            "",
            "ab cdef",
            0,
        ),
        ascii(
            "ignoreCase applies while matching backward",
            r"(?<=abc)d",
            "i",
            "ABCd",
            0,
        ),
        ascii(
            "Unicode lookbehind consumes one astral code point backward",
            r"(?<=😀)x",
            "u",
            "😀x",
            0,
        ),
        ascii(
            "legacy reverse dot captures only the low surrogate",
            r"(?<=(.))x",
            "",
            "😀x",
            0,
        ),
        ascii(
            "Unicode reverse dot captures the complete astral code point",
            r"(?<=(.))x",
            "u",
            "😀x",
            0,
        ),
        raw(
            "Unicode reverse dot treats a lone high surrogate as one code point",
            r"(?<=(.))x",
            "u",
            vec![0xd83d, u16::from(b'x')],
            0,
        ),
        ascii(
            "Unicode ignoreCase folds an astral literal while moving backward",
            r"(?<=𐐀)x",
            "iu",
            "𐐨x",
            0,
        ),
        ascii(
            "sticky lookbehind succeeds at the exact requested position",
            r"(?<=a)b",
            "y",
            "ab",
            1,
        ),
        ascii(
            "sticky lookbehind does not search past a failing requested position",
            r"(?<=a)b",
            "y",
            "ab",
            0,
        ),
        ascii(
            "Unicode sticky start inside a surrogate pair normalizes backward",
            r"(?<=x)😀",
            "uy",
            "x😀",
            2,
        ),
        ascii(
            "QuickJS does not reverse a multi-code-point Unicode Sets string",
            r"(?<=[\q{ab}])c",
            "v",
            "abc",
            0,
        ),
        ascii(
            "QuickJS Unicode Sets string can read forward across the assertion position",
            r"(?<=[\q{ab}])b",
            "v",
            "ab",
            0,
        ),
        ascii(
            "QuickJS Unicode Sets string preserves its class-wide prev placement",
            r"(?<=[\q{ba}])a",
            "v",
            "baa",
            0,
        ),
    ]
}

fn ascii(
    label: &'static str,
    pattern: &str,
    flags: &'static str,
    input: &str,
    start: usize,
) -> MatchCase {
    raw(label, pattern, flags, utf16(input), start)
}

fn raw(
    label: &'static str,
    pattern: &str,
    flags: &'static str,
    input: Vec<u16>,
    start: usize,
) -> MatchCase {
    MatchCase {
        label,
        pattern: utf16(pattern),
        flags,
        input,
        start,
    }
}

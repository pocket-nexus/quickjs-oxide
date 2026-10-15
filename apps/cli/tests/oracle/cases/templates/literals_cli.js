// mode: print
// case: empty unterminated template
`
// case: unterminated template text
`abc
// case: unterminated substitution
`a${1
// case: unterminated continuation
`a${1}
// case: empty substitution
`${}`
// case: extra expression token
`a${1 2}b`
// case: semicolon in substitution
`a${1;2}b`
// case: mismatched parenthesis
`a${(1}b`
// case: empty later substitution
`a${1}b${}c`
// case: short hex escape
`\x`
// case: non-hex escape
`\xG0`
// case: short Unicode escape
`\u`
// case: non-hex Unicode escape
`\u00G0`
// case: empty braced Unicode escape
`\u{}`
// case: out-of-range Unicode escape
`\u{110000}`
// case: legacy octal escape
`\1`
// case: legacy zero-prefixed escape
`\01`
// case: legacy eight escape
`\8`
// case: malformed tail escape
`a${1}\xG0`
// case: multiline empty substitution
`a${
}`
// case: multiline unterminated continuation
`a${
1
`
// case: initial malformed escape beats substitution grammar
`\x${1 2}\x`
// case: substitution grammar beats later malformed escape
`a${1 2}\x`
// case: malformed tail follows valid substitution
`a${1}\x`
// case: nested malformed escape
`a${`\x`}b`

// mode: print
// --- C6: unterminated strings point at the opening quote ---
// case: newline-terminated single-quoted string
var s = 'abc
// case: crlf-terminated single-quoted string
var s = 'abc
// case: newline-terminated string on the second line
var t = 1;
var s = 'abc
// case: eof-terminated string already matched and keeps the quote
var s = 'abc
// case: bare backslash before a newline
var s = '\
// case: bare backslash at end of file
var s = '\
// --- C5: every malformed \x / \u escape shares one pinned message ---
// case: short fixed unicode escape
var s = '\u00'
// case: non-hex fixed unicode escape
var s = '\uz'
// case: unicode escape exhausted at end of file
var s = '\u'
// case: non-hex hex escape
var s = '\xZZ'
// case: hex escape exhausted at end of file
var s = '\x'
// case: empty braced unicode escape
var s = '\u{}'
// case: non-hex braced unicode escape
var s = '\u{zz}'
// case: unterminated braced unicode escape
var s = '\u{41'
// case: braced unicode escape beyond U+10FFFF
var s = '\u{110000}'
// case: double-quoted malformed escape behaves identically
var s = "\u00"
// case: strict mode \8 uses the generic message
"use strict"; var s = "a\8b"
// case: strict mode \9 uses the generic message
"use strict"; var s = "a\9b"
// --- unaffected surfaces that must not regress ---
// case: strict legacy octal keeps its dedicated message
"use strict"; var s = "a\07b"

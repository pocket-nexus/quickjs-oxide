// Differential lock for pinned QuickJS 2026-06-04 named RegExp captures.
//
// The vectors deliberately cover the QuickJS implementation, including its
// observable parser quirks: named-capture scopes are a wrapping global u8
// advanced by every alternative, and the lexical forward-reference scan skips
// the first byte in a named capture body.  Runtime cases pin the capture-name
// trailer consumers (`groups`, `indices.groups`, and replacement) rather than
// testing only the matcher in isolation.

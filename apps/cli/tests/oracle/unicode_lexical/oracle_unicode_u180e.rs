// Differential lock for the pinned QuickJS 2026-06-04 treatment of U+180E
// MONGOLIAN VOWEL SEPARATOR. Unicode 17 classifies it as a format character,
// not ECMAScript whitespace: the lexer must reject it between tokens while
// literals and comments retain it as ordinary source content. The same choice
// flows through numeric parsing, trimming, Final_Sigma context, and RegExp
// dot/space classes. JavaScript eval and JSON are intentionally outside this
// slice because they remain independent runtime frontiers.

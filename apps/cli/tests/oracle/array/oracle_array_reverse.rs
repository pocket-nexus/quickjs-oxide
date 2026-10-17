// This target pins QuickJS 2026-06-04 `Array.prototype.reverse` and
// `Array.prototype.toReversed`. The former mutates generic array-like receivers
// pair by pair; the latter is a dense defining-realm base-Array copy and never
// consults constructor or @@species.

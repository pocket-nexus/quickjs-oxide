// Differential lock for pinned QuickJS 2026-06-04 RegExp match indices.
//
// These vectors keep `d` observable all the way through the consumers that
// either preserve the `indices` graph (`exec`, non-global match, and matchAll)
// or intentionally discard it (global match, split, and replacement).  They
// also pin QuickJS's UTF-16 `lastIndex` behavior when execution starts on the
// low surrogate of a pair.

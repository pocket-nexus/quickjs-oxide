// Differential lock for pinned QuickJS 2026-06-04 RegExp dotAll semantics.
// QuickJS carries `s` through `LRE_FLAG_DOTALL`: `libregexp.c` selects
// REOP_any instead of REOP_dot, scoped modifiers save and restore dotall state,
// and `quickjs.c` exposes the flag through the RegExp accessors and species
// consumers. These vectors focus on interactions not isolated by the broader
// RegExp intrinsic, modifier, matchAll, and split oracle suites.

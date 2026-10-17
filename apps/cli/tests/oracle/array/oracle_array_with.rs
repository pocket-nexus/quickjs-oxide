// This target pins QuickJS 2026-06-04 `Array.prototype.with` as one complete
// change-by-copy slice. Source probes deliberately avoid later Array methods
// and later Object reflection APIs on the Rust side. The graph probe uses the
// host API so lazy native descriptors remain observable without expanding the
// source-level Object surface.

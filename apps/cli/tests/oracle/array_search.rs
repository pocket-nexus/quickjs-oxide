// This differential pins the first Array.prototype algorithm slice after the
// iterator surface to QuickJS 2026-06-04. Source probes avoid later Array
// methods and later Object reflection APIs on the Rust side. The graph probe
// uses the host API so own-key order and lazy native metadata remain observable
// without pulling Object.getOwnPropertyDescriptor into this milestone.

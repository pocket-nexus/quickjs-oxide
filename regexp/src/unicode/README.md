# Unicode text rules

unicode implements identifier character classification, case mapping,
character properties and normalization.

It handles text only; generated tables come from a pinned upstream version,
and the generators live in `scripts/unicode`.

## Files and subdirectories

- [case.rs](case.rs): checksum-pinned Unicode 17 case conversion used by String intrinsics.
- [generated/](generated/README.md): pinned generated Unicode tables.
- [mod.rs](mod.rs): module entry, shared interfaces and submodule declarations.
- [normalize.rs](normalize.rs): checksum-pinned Unicode 17 normalization used by String intrinsics.
- [property.rs](property.rs): checksum-pinned Unicode 17 property sets used by RegExp property escapes.

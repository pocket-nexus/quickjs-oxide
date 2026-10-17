// Pins QuickJS 2026-06-04 `js_object_hasOwn`. Unlike the legacy
// `Object.prototype.hasOwnProperty`, the static first performs ToObject on its
// first argument and only then ToPropertyKey on its second argument. The final
// query is an own-property descriptor probe and must not Get the property.
// Proxy, TypedArray, and Arguments exotic own-property paths participate in
// this differential. Module namespace objects remain a separate module
// boundary.

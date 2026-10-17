// Pins QuickJS 2026-06-04 `js_object_fromEntries`. In particular, the
// implementation allocates a defining-realm ordinary result, acquires and
// caches `next`, reads entry[0] before entry[1], converts the key only after
// both reads, and routes every abrupt completion after iterator acquisition
// through `JS_IteratorClose(..., TRUE)`.
//
// Proxy and TypedArray entries participate in the same differential. Strong
// Map/Set iterators and generator IteratorClose are active as well. Module
// namespace entries remain a separate module/object-model boundary.

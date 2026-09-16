# Property cache validity

S16 uses up to two guarded locations per static read site. Runtime domain,
realm, generational receiver shape and layout revision are checked on each hit.
Prototype hits additionally check the global layout epoch and walk the recorded
depth. Revision changes replace stale entries for the same shape. A third live
shape or unsupported access cools the site down for 1024 reads before retrying.

Ordinary storage payloads include collection/date/regexp/function methods.
Proxy and module namespace semantics always use their domain driver. Indexed
exotics decline potentially numeric spellings, including TypedArray -0, NaN
and Infinity; conservative extra declines are permitted. Getter/AutoInit/VarRef
slots never become cached Data values.

Read hits that retain the receiver only retain the selected result under an
exclusive heap borrow. They release no owner and do not drain the zero queue,
so pending cleanup cannot change the guarded shape before result retention.
Replacing receiver reads retain the existing cleanup/release preflight.

Unresolved-global cells cache realm, atom, shape identity/revision and slot.
The value remains a live read (including mapped VarRef); lexical and initialized
cells retain their original path. The non-owning location dies with its cell.
Delete, defineProperty, prototype changes, dictionary transitions and realm
changes invalidate the applicable dependencies before another cache hit.

Admission is an exhaustive `ObjectKind` match. Ordinary, Iterator, ArrayIterator,
ForInIterator, Date, RegExp, RegExpStringIterator, Map, MapIterator, Set,
SetIterator, WeakMap, WeakSet, WeakRef, FinalizationRegistry, GlobalObject, Error,
StringIterator, IteratorHelper, IteratorWrap, AsyncFromSyncIterator,
IteratorConcat, ArrayBuffer, SharedArrayBuffer, DataView, NativeFunction,
BoundFunction, BytecodeFunction, Generator, AsyncGenerator, AsyncFunctionState
and Promise use the common ordinary named-property shape lookup; their payload
brands affect builtin methods, not that lookup. Array, Arguments, Primitive and
TypedArray additionally decline numeric spellings because indexed reads can be
intercepted before shape lookup. Proxy traps and ModuleNamespace live bindings
always retain their domain algorithms. The same classification applies at each
prototype holder. A newly added class must make an explicit admission choice.

## Proxy trap selection cache (S21)

`MethodStep::start` resolves one of the 13 Proxy traps by reading
`handler[trapName]` on every operation. S21 reuses the same guarded
`PropertyReadCache` location protocol for that read. One cache per runtime is
indexed by a closed trap index (`RuntimeState.proxy_trap_reads`), so the cache
key is the handler's shape lineage, not a specific proxy: proxies that share a
handler shape share the cache entry. The cache owns no object, atom or value,
and lives outside `ProxyData`, so it adds no GC edge and does not change
`ProxyData`/`ArenaSlot` layout.

The cacheable judgement is deliberately narrower than an ordinary read:
the trap location must be an own or inherited **Data** slot. `locate` reuses the
read-IC admission test, so an accessor trap (`get get(){...}`), a
`VarRef`/`AutoInit` slot, a dictionary layout, or a handler that is itself a
Proxy declines and keeps the canonical dynamic read. A declined handler cools
the trap entry down (megamorphic) for the same 1024 reads as a read site, which
only degrades to today's every-read cost. Accessors therefore run their getter
on every read; nested Proxy handlers keep their trap ordering.

Invalidation is the read-IC invalidation verbatim: adding/removing properties or
`defineProperty` changes the layout revision, a same-shape overwrite of
`handler.get` is read as today's slot value (the cache stores a location, never
a value), a prototype hit checks the global layout epoch, a collected shape
fails the generational `heap.shape(id)` lookup, and a revoked Proxy is rejected
*before* the cache is consulted. Realm and runtime domain mismatches are
`Location` guards. The dynamic `handler[trapName]` read is always retained as the
semantic fallback, so a cache miss never changes operation order or observable
results.

# Front-end primitive values

value holds the primitives the front end, its constant pool and the
embedding API share. JavaScript values at run time are `quickjs-oxide-core`'s
(`oxide_core::Value`, NaN-boxed, with strings, symbols, BigInts and objects
as heap records); the values here are Rust data. The lowering copies constants into the core, and the API
copies primitives out of it.

| Item | Content |
| --- | --- |
| `JsString` | an ECMAScript string as UTF-16 code units, including lone surrogates, stored as a Latin-1 or UTF-16 leaf or a bounded rope; comparison and hashing agree across the storage choices. Source text, identifiers, string literals and the API's `Value::String` use it |
| `JsStringBuilder`, `JsStringError` | building strings within QuickJS's length limit; `TooLong` and `OutOfMemory` |
| `JsBigInt` ([bigint.rs](bigint.rs)) | QuickJS's two BigInt representations (an immediate `i64` or a heap integer over `num-bigint`), for BigInt literals and the API's `Value::BigInt` |
| `PrimitiveValue` | a constant-pool value: `Int`, `Float`, `BigInt` or `String` |
| `number_to_string` | `Number::toString(value, 10)` as the core prints it (it calls the core's radix conversion), exported by the API |
| `decode_quickjs_utf8` | QuickJS's UTF-8 decoding (surrogate encodings accepted), used by `SourceText`, the lexer and JSON module text |

Conversions that can run JavaScript (`ToPrimitive`, `ToString` of an
object) are the core's; `PrimitiveValue::to_js_string` covers only the
primitive kinds.

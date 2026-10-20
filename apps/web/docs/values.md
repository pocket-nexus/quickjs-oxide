# Values and calls

Every value that crosses between Rust and JavaScript is a `Value`. Primitives are copied out of the engine as Rust data; objects and symbols stay in the engine's heap and Rust holds handles to them. This page shows each kind, then how to read and create objects, call functions and convert values. The code comes from [`examples/values.rs`](https://github.com/pocket-nexus/quickjs-oxide/blob/main/examples/values.rs), which asserts every result shown.

All types on this page are in `quickjs_oxide::engine::api::core`.

## The Value enum

```rust
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Int(i32),
    Float(f64),
    BigInt(JsBigInt),
    String(JsString),
    Symbol(SymbolRef),
    Object(ObjectRef),
}
```

A JavaScript number is `Int` when it is an integer in `i32` range other than `-0`, and `Float` otherwise. `Value::number(f64)` makes the right one, and `as_number()` reads either as `f64`.

`==` on `Value` (`PartialEq`) compares representations: `Int(1)` and `Float(1.0)` differ, and two `ObjectRef`s are equal when they name the same object. For JavaScript's comparisons use `strict_equal` (`===`), `same_value` (`Object.is`) and `same_value_zero`.

<!-- include: examples/values.rs#primitives -->

`Value::type_of` answers from the variant alone, so every object is `"object"`. `Runtime::type_of` asks the engine and reports `"function"` for callable objects.

`From` conversions build values from `bool`, `i32`, `f64` (through `Value::number`), `JsString`, `ObjectRef` and `CallableRef`.

## Strings

`JsString` holds UTF-16 code units, as JavaScript strings do. It can hold lone surrogates, which UTF-8 cannot, so conversion to a Rust `String` is lossy: `to_utf8_lossy` replaces each lone surrogate with U+FFFD.

<!-- include: examples/values.rs#strings -->

| Method | Result |
| --- | --- |
| `JsString::try_from_utf8(&str)` | a string from UTF-8 |
| `JsString::try_from_utf16(units)` | a string from UTF-16 code units |
| `JsString::from_static(&'static str)` | a string from a constant |
| `len()`, `is_empty()` | length in UTF-16 code units |
| `code_unit_at(i)`, `code_point_at(i)` | one code unit or code point |
| `utf16_units()` | an iterator over the code units |
| `is_well_formed()` | `false` when the string has a lone surrogate |
| `to_utf8_lossy()` | a Rust `String` |

## BigInts

`JsBigInt` is an arbitrary-precision integer. It implements `Display`, `FromStr` and `From` for the Rust integer types.

<!-- include: examples/values.rs#bigints -->

## Objects, functions and symbols

Objects and symbols are handles into the engine's heap:

| Type | Holds |
| --- | --- |
| `ObjectRef` | any object: plain objects, arrays, functions, errors, promises |
| `CallableRef` | an object known to be callable; it dereferences to `ObjectRef` |
| `SymbolRef` | a symbol |

A handle keeps its value alive until the last clone of the handle drops. Cloning a handle is cheap and names the same object. A handle belongs to the runtime that made it (`belongs_to(&runtime)`); passing it to another runtime returns `RuntimeError::Invariant`.

`Runtime::as_callable(&object)` returns `Some(CallableRef)` when the object is callable, and `Runtime::is_constructor` tells whether it accepts `new`. `CallableRef::as_object` and `into_object` give back the `ObjectRef`.

Symbols are made with `Runtime::new_symbol`, read with `Runtime::symbol_description`, and used as property keys through `PropertyKey::from(symbol)`. `Runtime::well_known_symbol` returns the symbols JavaScript sees as `Symbol.iterator`, `Symbol.asyncIterator` and the others listed by `WellKnownSymbol::ALL`.

<!-- include: examples/values.rs#symbols -->

## Read properties

A property name is a `PropertyKey`: a string, an array index or a symbol. `Runtime::intern_property_key("name")` makes a string key (or an index key when the text is an array index such as `"0"`), and `PropertyKey::index(n)` makes an index key directly.

`Context::get_property` runs the full `[[Get]]`, including getters and proxies, and returns `Value::Undefined` for a missing property. `Runtime::has_property` and `has_own_property` test presence, and `own_property_keys` lists own keys in JavaScript's property order.

<!-- include: examples/values.rs#read-properties -->

`Context::get_own_property` returns an own property's full descriptor (`CompleteOrdinaryPropertyDescriptor::Data` or `Accessor`); an accessor's getter is returned as a `CallableRef`, not called. `Runtime::delete_property` deletes, and `get_prototype_of` and `set_prototype_of` read and change the prototype.

## Create objects and arrays

| Method | Creates |
| --- | --- |
| `Context::new_object()` | `{}` with `Object.prototype` |
| `Context::new_object_with_prototype(Some(&proto))` | an object with the given prototype, or none with `None` |
| `Context::new_array()` | `[]` |
| `Context::new_array_from_values(vec)` | an array of the given values |
| `Context::new_native_error(kind, message)` | an error object such as a `TypeError`, not thrown |

`Context::set_property` assigns as `object[key] = value` does and returns whether the assignment succeeded. `Context::define_own_property` defines a property from an `OrdinaryPropertyDescriptor`; `OrdinaryPropertyDescriptor::data(value, writable, enumerable, configurable)` builds a data descriptor.

<!-- include: examples/values.rs#create-objects -->

## Call functions

`Context::call(&callable, this, &arguments)` calls a function as `callable.call(this, ...arguments)` does. Pass `Value::Undefined` as `this` for a plain function call, or the object for a method call.

When the function throws, `call` returns `Err(RuntimeError::Exception)`; `Context::take_exception` returns the thrown value and clears it.

<!-- include: examples/values.rs#call -->

Calls run synchronously to completion. A function that returns a promise returns the promise object; [Promises and jobs](./jobs.md) shows how to run the jobs that settle it.

## Construct objects

`Context::construct(&constructor, &arguments)` runs `new constructor(...arguments)`. It throws a `TypeError` when the object is not a constructor. `construct_with_new_target` sets a different `new.target`, as `Reflect.construct` does.

<!-- include: examples/values.rs#construct -->

## Convert values

`Context::to_js_string` and `Context::to_number` apply JavaScript's `ToString` and `ToNumber`. For objects they call `Symbol.toPrimitive`, `valueOf` or `toString`, so they run JavaScript and can throw. `Value::to_boolean` applies `ToBoolean`, which never runs JavaScript.

`Runtime::print_value` formats a value the way QuickJS prints it, without running any JavaScript. `qjs` uses it for uncaught exceptions.

<!-- include: examples/values.rs#conversions -->

## The global object

`Context::global_object` returns the realm's global object. Properties set on it from Rust are global variables in JavaScript, and global `var` and function declarations are readable from Rust.

<!-- include: examples/values.rs#globals -->

Other intrinsics have their own accessors: `object_prototype`, `array_prototype`, `function_prototype`, `iterator_prototype`, `string_iterator_prototype`, `number_prototype`, `boolean_prototype`, `string_prototype`, `symbol_prototype`, `bigint_prototype` and `function_constructor`.

JavaScript reaches the application through the `Host` trait: output, the clock, the time zone, the random seed, file reads and promise-rejection events. See [Host services](./host.md).

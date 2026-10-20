//! The examples of the "Values and calls" docs page; every value the page
//! shows is asserted here.
//!
//! ```sh
//! cargo run --quiet --example values
//! ```

use quickjs_oxide::engine::api::core::{
    Context, JsBigInt, JsString, OrdinaryPropertyDescriptor, PropertyKey, Runtime, RuntimeError,
    Value, WellKnownSymbol,
};
use quickjs_oxide_host::SystemHost;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    primitives(&mut context)?;
    strings()?;
    bigints(&mut context)?;
    symbols(&runtime, &mut context)?;
    read_properties(&runtime, &mut context)?;
    create_objects(&runtime, &mut context)?;
    call_functions(&runtime, &mut context)?;
    construct(&runtime, &mut context)?;
    conversions(&runtime, &mut context)?;
    globals(&runtime, &mut context)?;
    Ok(())
}

fn primitives(context: &mut Context) -> Result {
    // docs:start primitives
    assert_eq!(context.eval("undefined")?, Value::Undefined);
    assert_eq!(context.eval("null")?, Value::Null);
    assert_eq!(context.eval("!0")?, Value::Bool(true));
    assert_eq!(context.eval("2 ** 10")?, Value::Int(1024));
    assert_eq!(context.eval("1 / 3")?, Value::Float(1.0 / 3.0));
    assert_eq!(context.eval("-0")?, Value::Float(-0.0));

    // Value::number picks Int for integers in i32 range and Float otherwise.
    assert_eq!(Value::number(7.0), Value::Int(7));
    assert_eq!(Value::number(2.5), Value::Float(2.5));

    // Equality as JavaScript defines it.
    assert!(Value::Int(1).strict_equal(&Value::Float(1.0)));
    assert!(!Value::Float(f64::NAN).strict_equal(&Value::Float(f64::NAN)));
    assert!(Value::Float(f64::NAN).same_value(&Value::Float(f64::NAN)));

    // ToBoolean and typeof for primitives never run JavaScript.
    assert!(!Value::Int(0).to_boolean());
    assert_eq!(Value::Null.type_of(), "object");
    // docs:end primitives
    Ok(())
}

fn strings() -> Result {
    // docs:start strings
    let text = JsString::try_from_utf8("héllo 😀")?;
    // Length counts UTF-16 code units, as `"héllo 😀".length` does.
    assert_eq!(text.len(), 8);
    assert_eq!(text.code_unit_at(1), Some(0x00e9));
    assert_eq!(text.to_utf8_lossy(), "héllo 😀");

    // A lone surrogate is a valid JavaScript string but not valid UTF-8.
    let lone = JsString::try_from_utf16([0xd800])?;
    assert!(!lone.is_well_formed());
    assert_eq!(lone.to_utf8_lossy(), "\u{fffd}");

    let constant = JsString::from_static("ready");
    assert_eq!(
        Value::from(constant),
        Value::String(JsString::try_from_utf8("ready")?)
    );
    // docs:end strings
    Ok(())
}

fn bigints(context: &mut Context) -> Result {
    // docs:start bigints
    let Value::BigInt(big) = context.eval("2n ** 70n")? else {
        panic!("expected a BigInt");
    };
    assert_eq!(big.to_string(), "1180591620717411303424");
    assert_eq!(big.as_i64(), None); // does not fit in an i64

    let small = JsBigInt::from(-5_i64);
    assert_eq!(small.as_i64(), Some(-5));
    assert_eq!(small.to_string_radix(16)?, "-5");
    // docs:end bigints
    Ok(())
}

fn symbols(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start symbols
    let tag = runtime.new_symbol(Some(JsString::try_from_utf8("tag")?))?;
    let description = runtime.symbol_description(&tag)?.expect("a description");
    assert_eq!(description.to_utf8_lossy(), "tag");

    // A symbol is a property key.
    let object = context.new_object()?;
    let key = PropertyKey::from(tag.clone());
    context.set_property(&object, &key, Value::Int(1))?;
    assert_eq!(context.get_property(&object, &key)?, Value::Int(1));

    // The well-known symbols are the same ones JavaScript sees.
    let iterator = runtime.well_known_symbol(WellKnownSymbol::Iterator)?;
    assert_eq!(context.eval("Symbol.iterator")?, Value::Symbol(iterator));
    // docs:end symbols
    Ok(())
}

fn read_properties(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start read-properties
    let Value::Object(user) = context.eval("({ name: 'Ada', langs: ['en', 'fr'] })")? else {
        panic!("expected an object");
    };

    let name_key = runtime.intern_property_key("name")?;
    let name = context.get_property(&user, &name_key)?;
    assert_eq!(context.to_js_string(&name)?.to_utf8_lossy(), "Ada");

    // A missing property reads as undefined.
    let age_key = runtime.intern_property_key("age")?;
    assert_eq!(context.get_property(&user, &age_key)?, Value::Undefined);
    assert!(!runtime.has_property(&user, &age_key)?);

    // Array elements use index keys.
    let langs_key = runtime.intern_property_key("langs")?;
    let Value::Object(langs) = context.get_property(&user, &langs_key)? else {
        panic!("expected an array");
    };
    assert!(runtime.is_array_object(&langs)?);
    let second = context.get_property(&langs, &PropertyKey::index(1))?;
    assert_eq!(context.to_js_string(&second)?.to_utf8_lossy(), "fr");

    // Own keys in property order.
    let keys: Vec<String> = runtime
        .own_property_keys(&user)?
        .iter()
        .filter_map(|key| key.to_js_string())
        .map(|key| key.to_utf8_lossy())
        .collect();
    assert_eq!(keys, ["name", "langs"]);
    // docs:end read-properties
    Ok(())
}

fn create_objects(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start create-objects
    let config = context.new_object()?;
    let debug = runtime.intern_property_key("debug")?;
    context.set_property(&config, &debug, Value::Bool(true))?;

    // A read-only, non-enumerable property.
    let version = runtime.intern_property_key("version")?;
    let descriptor = OrdinaryPropertyDescriptor::data(Value::Int(3), false, false, false);
    assert!(context.define_own_property(&config, &version, &descriptor)?);

    let ports = context.new_array_from_values(vec![Value::Int(80), Value::Int(443)])?;
    let ports_key = runtime.intern_property_key("ports")?;
    context.set_property(&config, &ports_key, Value::Object(ports))?;

    // Hand the object to JavaScript.
    let stringify = context.eval("(value) => JSON.stringify(value)")?;
    let stringify = runtime
        .as_callable(stringify.as_object().expect("an object"))?
        .expect("a function");
    let json = context.call(&stringify, Value::Undefined, &[Value::Object(config)])?;
    assert_eq!(
        context.to_js_string(&json)?.to_utf8_lossy(),
        r#"{"debug":true,"ports":[80,443]}"#
    );
    // docs:end create-objects
    Ok(())
}

fn call_functions(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start call
    let Value::Object(counter) =
        context.eval("({ count: 10, add(n) { this.count += n; return this.count; } })")?
    else {
        panic!("expected an object");
    };
    let add_key = runtime.intern_property_key("add")?;
    let Value::Object(add) = context.get_property(&counter, &add_key)? else {
        panic!("expected a function");
    };
    let add = runtime.as_callable(&add)?.expect("add is callable");

    // The second argument is `this`.
    let result = context.call(&add, Value::Object(counter.clone()), &[Value::Int(5)])?;
    assert_eq!(result, Value::Int(15));

    // A function that throws returns RuntimeError::Exception.
    let thrower = context.eval("(function () { throw new RangeError('too far'); })")?;
    let thrower = runtime
        .as_callable(thrower.as_object().expect("an object"))?
        .expect("a function");
    assert_eq!(
        context.call(&thrower, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let error = context.take_exception()?.expect("the thrown error");
    assert_eq!(
        context.to_js_string(&error)?.to_utf8_lossy(),
        "RangeError: too far"
    );
    // docs:end call
    Ok(())
}

fn construct(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start construct
    let global = context.global_object()?;
    let map_key = runtime.intern_property_key("Map")?;
    let Value::Object(map_constructor) = context.get_property(&global, &map_key)? else {
        panic!("expected Map");
    };
    let map_constructor = runtime
        .as_callable(&map_constructor)?
        .expect("Map is callable");
    assert!(runtime.is_constructor(&map_constructor)?);

    // new Map([["a", 1]])
    let entry = context.new_array_from_values(vec![
        Value::String(JsString::from_static("a")),
        Value::Int(1),
    ])?;
    let entries = context.new_array_from_values(vec![Value::Object(entry)])?;
    let map = context.construct(&map_constructor, &[Value::Object(entries)])?;
    assert_eq!(runtime.type_of(&map)?, "object");

    let size_key = runtime.intern_property_key("size")?;
    let map = map.as_object().expect("an object");
    assert_eq!(context.get_property(map, &size_key)?, Value::Int(1));
    // docs:end construct
    Ok(())
}

fn conversions(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start conversions
    // ToString and ToNumber run JavaScript: toString, valueOf, Symbol.toPrimitive.
    let money = context.eval("({ valueOf() { return 42; }, toString() { return '$42'; } })")?;
    assert_eq!(context.to_js_string(&money)?.to_utf8_lossy(), "$42");
    assert_eq!(context.to_number(&money)?, 42.0);

    assert!(
        context
            .to_number(&Value::String(JsString::from_static("abc")))?
            .is_nan()
    );

    // typeof as the engine reports it (a function is "function").
    let function = context.eval("Math.max")?;
    assert_eq!(runtime.type_of(&function)?, "function");

    // QuickJS's printer, as qjs prints an uncaught exception; it runs no JavaScript.
    let value = context.eval("[1, 'two', { three: 3 }]")?;
    assert_eq!(runtime.print_value(&value)?, "[ 1, \"two\", { three: 3 } ]");
    // docs:end conversions
    Ok(())
}

fn globals(runtime: &Runtime, context: &mut Context) -> Result {
    // docs:start globals
    let global = context.global_object()?;

    // Rust sets a global; JavaScript reads it.
    let limit = runtime.intern_property_key("LIMIT")?;
    context.set_property(&global, &limit, Value::Int(100))?;
    assert_eq!(context.eval("LIMIT * 2")?, Value::Int(200));

    // JavaScript sets a global; Rust reads it.
    context.eval("var status = 'done'")?;
    let status = runtime.intern_property_key("status")?;
    let value = context.get_property(&global, &status)?;
    assert_eq!(context.to_js_string(&value)?.to_utf8_lossy(), "done");
    // docs:end globals
    Ok(())
}

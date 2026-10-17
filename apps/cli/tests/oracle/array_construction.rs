use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::string_units;
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::core::{
    CompleteOrdinaryPropertyDescriptor, Context, DescriptorField, ObjectRef,
    OrdinaryPropertyDescriptor, Runtime, Value, number_to_string,
};
use std::ffi::OsStr;
use std::process::Command;

// This target deliberately describes the complete first Array vertical slice,
// rather than the parser-only boundary which exists today.  Keep the cases
// usable without the global Object/Reflect builtins on the Rust side: source
// cases return primitives, while descriptor and own-key observations use the
// public host API and compare them with an equivalent QuickJS observer.

const HOST_SNAPSHOT_CASES: &[(&str, &str)] = &[
    ("empty array own shape", "[]"),
    ("dense array own shape", "[10,20,30]"),
    ("sparse array own shape", "[,10,,30,,]"),
    ("single length constructor own shape", "Array(4)"),
    (
        "32 boundary literal own shape",
        "[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]",
    ),
    ("spread literal own shape", "[0,...'ab',3]"),
    (
        "numeric keys precede length and later strings",
        "(function(){var a=[];a.foo=1;a[3]=3;a[1]=1;return a})()",
    ),
];

#[test]
fn array_host_own_keys_and_descriptors_match_pinned_quickjs() {
    let oracle = require_qjs_oracle!("Array host descriptor differential");
    for &(description, source) in HOST_SNAPSHOT_CASES {
        let expected = oracle_snapshot(&oracle, source, description);
        let (runtime, mut context) = host_runtime_context();
        let array = eval_object(&mut context, source, description);
        assert_eq!(
            array_snapshot(&runtime, &mut context, &array),
            expected,
            "Array own-key/descriptor drifted for {description}: {source:?}",
        );
    }
}

#[test]
fn array_constructor_graph_matches_pinned_quickjs() {
    let oracle = require_qjs_oracle!("Array constructor graph differential");
    let expected = oracle_constructor_graph(&oracle);
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        rust_constructor_graph(&runtime, &mut context),
        expected,
        "Array constructor/prototype/global descriptor graph drifted",
    );
}

#[test]
fn array_host_definitions_use_array_set_length_semantics() {
    let oracle = require_qjs_oracle!("Array host mutation differential");
    let expected = oracle_host_mutation(&oracle);
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        rust_host_mutation(&runtime, &mut context),
        expected,
        "host property operations bypassed ArraySetLength/index semantics",
    );
}

fn array_snapshot(runtime: &Runtime, context: &mut Context, array: &ObjectRef) -> String {
    runtime
        .own_property_keys(array)
        .expect("Array own keys")
        .into_iter()
        .map(|key| {
            let key_text = string_units(&runtime.property_key_to_js_string(&key).unwrap());
            let descriptor = context
                .get_own_property(array, &key)
                .expect("Array own descriptor")
                .expect("Array own key disappeared during snapshot");
            format!("{key_text}={}", descriptor_token(descriptor))
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn descriptor_token(descriptor: CompleteOrdinaryPropertyDescriptor) -> String {
    match descriptor {
        CompleteOrdinaryPropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        } => format!(
            "D:{}:{}:{}:{}",
            value_token(value),
            u8::from(writable),
            u8::from(enumerable),
            u8::from(configurable),
        ),
        CompleteOrdinaryPropertyDescriptor::Accessor {
            get,
            set,
            enumerable,
            configurable,
        } => format!(
            "A:{}:{}:{}:{}",
            u8::from(get.is_some()),
            u8::from(set.is_some()),
            u8::from(enumerable),
            u8::from(configurable),
        ),
    }
}

fn oracle_snapshot(oracle: &OsStr, source: &str, description: &str) -> String {
    let script = r#"
function units(s) {
  var out='';
  for (var i=0;i<s.length;i++) {
    if (i) out+=',';
    out+=('0000'+s.charCodeAt(i).toString(16)).slice(-4);
  }
  return out;
}
function valueToken(value) {
  if (value === undefined) return 'u';
  if (value === null) return 'n';
  if (typeof value === 'boolean') return value ? 'b1' : 'b0';
  if (typeof value === 'number') {
    if (value !== value) return 'dNaN';
    if (value === 0 && 1/value === -Infinity) return 'd-0';
    return 'd'+String(value);
  }
  if (typeof value === 'string') return 's'+units(value);
  if (typeof value === 'bigint') return 'i'+String(value);
  if (typeof value === 'symbol') return 'y';
  return 'o';
}
function descToken(d) {
  if ('value' in d)
    return 'D:'+valueToken(d.value)+':'+Number(d.writable)+':'+Number(d.enumerable)+':'+Number(d.configurable);
  return 'A:'+Number(d.get!==undefined)+':'+Number(d.set!==undefined)+':'+Number(d.enumerable)+':'+Number(d.configurable);
}
var array=std.evalScript(scriptArgs[0]);
var keys=Reflect.ownKeys(array),out='';
for(var i=0;i<keys.length;i++) {
  if(i) out+=';';
  out+=units(String(keys[i]))+'='+descToken(Object.getOwnPropertyDescriptor(array,keys[i]));
}
print(out);
"#;
    let output = Command::new(oracle)
        .args(["--std", "-e", script, source])
        .output()
        .unwrap_or_else(|error| panic!("could not snapshot QuickJS {description}: {error}"));
    assert!(
        output.status.success(),
        "QuickJS snapshot failed for {description}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("QuickJS snapshot was not UTF-8 for {description}: {error}"))
        .trim_end()
        .to_owned()
}

fn rust_constructor_graph(runtime: &Runtime, context: &mut Context) -> String {
    let Value::Object(constructor) = context.eval("Array").expect("evaluate global Array") else {
        panic!("global Array was not an object");
    };
    let global = context.global_object().unwrap();
    let function_prototype = context.function_prototype().unwrap();
    let object_prototype = context.object_prototype().unwrap();
    let length = own_descriptor(runtime, context, &constructor, "length");
    let name = own_descriptor(runtime, context, &constructor, "name");
    let prototype_descriptor = own_descriptor(runtime, context, &constructor, "prototype");
    let Value::Object(array_prototype) = descriptor_value(&prototype_descriptor) else {
        panic!("Array.prototype descriptor did not contain an object");
    };
    let global_descriptor = own_descriptor(runtime, context, &global, "Array");
    let back_descriptor = own_descriptor(runtime, context, &array_prototype, "constructor");
    let callable = runtime.as_callable(&constructor).unwrap().is_some();
    let constructable = runtime.is_constructor(&constructor).unwrap();
    let constructor_prototype = runtime.get_prototype_of(&constructor).unwrap();
    let array_parent = runtime.get_prototype_of(&array_prototype).unwrap();
    let global_identity = matches!(descriptor_value(&global_descriptor), Value::Object(value) if value == constructor);
    let back_identity =
        matches!(descriptor_value(&back_descriptor), Value::Object(value) if value == constructor);
    let global_token = format!("{}:{global_identity}", descriptor_token(global_descriptor));
    let back_token = format!("{}:{back_identity}", descriptor_token(back_descriptor));
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        callable,
        constructable,
        constructor_prototype.as_ref() == Some(&function_prototype),
        array_parent.as_ref() == Some(&object_prototype),
        descriptor_token(length),
        descriptor_token(name),
        descriptor_token(prototype_descriptor),
        global_token,
        back_token,
    )
}

fn oracle_constructor_graph(oracle: &OsStr) -> String {
    let script = r#"
function valueToken(value) {
  if (value === undefined) return 'u';
  if (value === null) return 'n';
  if (typeof value === 'boolean') return value ? 'b1' : 'b0';
  if (typeof value === 'number') return 'd'+String(value);
  if (typeof value === 'string') {
    var out='';for(var i=0;i<value.length;i++){if(i)out+=',';out+=('0000'+value.charCodeAt(i).toString(16)).slice(-4)}return 's'+out;
  }
  if (typeof value === 'bigint') return 'i'+String(value);
  if (typeof value === 'symbol') return 'y';
  return 'o';
}
function token(d) {
  if ('value' in d) return 'D:'+valueToken(d.value)+':'+Number(d.writable)+':'+Number(d.enumerable)+':'+Number(d.configurable);
  return 'A:'+Number(d.get!==undefined)+':'+Number(d.set!==undefined)+':'+Number(d.enumerable)+':'+Number(d.configurable);
}
var p=Array.prototype;
var globalDescriptor=Object.getOwnPropertyDescriptor(globalThis,'Array');
var backDescriptor=Object.getOwnPropertyDescriptor(p,'constructor');
print(
  (typeof Array==='function')+'|'+
  (typeof Array==='function')+'|'+
  (Object.getPrototypeOf(Array)===Function.prototype)+'|'+
  (Object.getPrototypeOf(p)===Object.prototype)+'|'+
  token(Object.getOwnPropertyDescriptor(Array,'length'))+'|'+
  token(Object.getOwnPropertyDescriptor(Array,'name'))+'|'+
  token(Object.getOwnPropertyDescriptor(Array,'prototype'))+'|'+
  token(globalDescriptor)+':'+(globalDescriptor.value===Array)+'|'+
  token(backDescriptor)+':'+(backDescriptor.value===Array)
);
"#;
    command_stdout(oracle, script, "Array constructor graph")
}

fn rust_host_mutation(runtime: &Runtime, context: &mut Context) -> String {
    let array = eval_object(context, "[]", "host mutation array");
    let index_two = runtime.intern_property_key("2").unwrap();
    let index_four = runtime.intern_property_key("4").unwrap();
    let index_six = runtime.intern_property_key("6").unwrap();
    let length = runtime.intern_property_key("length").unwrap();

    let set_two = context
        .set_property(&array, &index_two, Value::Int(7))
        .expect("set Array index 2");
    let define_four = context
        .define_own_property(
            &array,
            &index_four,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::Int(9)),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
        .expect("define fixed Array index 4");
    let delete_two = runtime
        .delete_property(&array, &index_two)
        .expect("delete Array index 2");
    let shrink = context
        .set_property(&array, &length, Value::Int(1))
        .expect("attempt Array length shrink");
    let freeze_length = context
        .define_own_property(
            &array,
            &length,
            &OrdinaryPropertyDescriptor {
                writable: DescriptorField::Present(false),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
        .expect("make Array length non-writable");
    let define_six = context
        .define_own_property(
            &array,
            &index_six,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::Int(11)),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(true),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
        .expect("attempt index past non-writable Array length");
    format!(
        "{set_two}|{define_four}|{delete_two}|{shrink}|{freeze_length}|{define_six}|{}",
        array_snapshot(runtime, context, &array),
    )
}

fn oracle_host_mutation(oracle: &OsStr) -> String {
    let script = r#"
function units(s){var out='';for(var i=0;i<s.length;i++){if(i)out+=',';out+=('0000'+s.charCodeAt(i).toString(16)).slice(-4)}return out}
function valueToken(value){if(value===undefined)return'u';if(value===null)return'n';if(typeof value==='boolean')return value?'b1':'b0';if(typeof value==='number')return'd'+String(value);if(typeof value==='string')return's'+units(value);if(typeof value==='bigint')return'i'+String(value);if(typeof value==='symbol')return'y';return'o'}
function descToken(d){if('value'in d)return'D:'+valueToken(d.value)+':'+Number(d.writable)+':'+Number(d.enumerable)+':'+Number(d.configurable);return'A:'+Number(d.get!==undefined)+':'+Number(d.set!==undefined)+':'+Number(d.enumerable)+':'+Number(d.configurable)}
function snapshot(a){var keys=Reflect.ownKeys(a),out='';for(var i=0;i<keys.length;i++){if(i)out+=';';out+=units(String(keys[i]))+'='+descToken(Object.getOwnPropertyDescriptor(a,keys[i]))}return out}
var a=[];
var setTwo=Reflect.set(a,'2',7);
var defineFour=Reflect.defineProperty(a,'4',{value:9});
var deleteTwo=delete a[2];
var shrink=Reflect.set(a,'length',1);
var freezeLength=Reflect.defineProperty(a,'length',{writable:false});
var defineSix=Reflect.defineProperty(a,'6',{value:11,writable:true,enumerable:true,configurable:true});
print(setTwo+'|'+defineFour+'|'+deleteTwo+'|'+shrink+'|'+freezeLength+'|'+defineSix+'|'+snapshot(a));
"#;
    command_stdout(oracle, script, "Array host mutation")
}

fn own_descriptor(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> CompleteOrdinaryPropertyDescriptor {
    let key = runtime.intern_property_key(name).unwrap();
    context
        .get_own_property(object, &key)
        .unwrap_or_else(|error| panic!("get own {name}: {error}"))
        .unwrap_or_else(|| panic!("missing own {name}"))
}

fn descriptor_value(descriptor: &CompleteOrdinaryPropertyDescriptor) -> Value {
    match descriptor {
        CompleteOrdinaryPropertyDescriptor::Data { value, .. } => {
            value.try_clone().expect("duplicate root")
        }
        CompleteOrdinaryPropertyDescriptor::Accessor { .. } => {
            panic!("expected data property descriptor")
        }
    }
}

fn command_stdout(oracle: &OsStr, script: &str, description: &str) -> String {
    let output = Command::new(oracle)
        .args(["--std", "-e", script])
        .output()
        .unwrap_or_else(|error| panic!("could not run QuickJS {description}: {error}"));
    assert!(
        output.status.success(),
        "QuickJS {description} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("QuickJS {description} output was not UTF-8: {error}"))
        .trim_end()
        .to_owned()
}

fn value_token(value: Value) -> String {
    match value {
        Value::Undefined => "u".to_owned(),
        Value::Null => "n".to_owned(),
        Value::Bool(value) => format!("b{}", u8::from(value)),
        Value::Int(value) => format!("d{value}"),
        Value::Float(value) if value.is_nan() => "dNaN".to_owned(),
        Value::Float(value) if value == 0.0 && value.is_sign_negative() => "d-0".to_owned(),
        Value::Float(value) => format!("d{}", number_to_string(value)),
        Value::BigInt(value) => format!("i{value}"),
        Value::String(value) => format!("s{}", string_units(&value)),
        Value::Object(_) => "o".to_owned(),
        Value::Symbol(_) => "y".to_owned(),
    }
}

use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;

use crate::object_graph_observation::{
    define_global_with_message as define_global, eval_string_with_global,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

#[test]
fn typed_array_from_cross_realm_result_map_errors_and_abrupt_values() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let mut custom = runtime.new_context().expect("create context");

    let from = eval_callable(
        &runtime,
        &mut defining,
        "Object.getPrototypeOf(Uint8Array).from",
        "defining TypedArray.from",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let custom_uint16_prototype = eval_object(
        &mut custom,
        "Uint16Array.prototype",
        "custom Uint16Array prototype",
    );
    let constructor = eval_callable(
        &runtime,
        &mut custom,
        "(function Result(length){return new Uint16Array(length)})",
        "custom result constructor",
    );
    let source = eval_object(
        &mut caller,
        "({0:40,1:41,length:2})",
        "caller array-like source",
    );
    let map_this = eval_object(&mut caller, "({mapThis:1})", "caller map this");
    define_global(
        &runtime,
        &mut caller,
        "__typedArrayFromMapThis",
        Value::Object(map_this.try_clone().expect("duplicate root")),
    );
    let map = eval_callable(
        &runtime,
        &mut caller,
        "(function(value,index){\
            globalThis.__typedArrayFromSawThis=\
                this===__typedArrayFromMapThis;\
            return value+index;\
        })",
        "caller mapper",
    );

    let result = caller
        .call(
            &from,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[
                Value::Object(source),
                Value::Object(map.as_object().try_clone().expect("duplicate root")),
                Value::Object(map_this),
            ],
        )
        .expect("cross-realm TypedArray.from");
    let Value::Object(result) = result else {
        panic!("cross-realm TypedArray.from did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(custom_uint16_prototype),
        "custom constructor result escaped its defining realm",
    );
    assert_eq!(
        eval_string_with_global(
            &runtime,
            &mut caller,
            "__typedArrayFromResult",
            Value::Object(result),
            "[__typedArrayFromResult.length,__typedArrayFromResult[0],\
             __typedArrayFromResult[1],__typedArrayFromSawThis].join('|')",
        ),
        "2|40|42|true",
    );

    let sloppy_map = eval_callable(
        &runtime,
        &mut caller,
        "(function(value){\
            globalThis.__typedArrayFromSloppyThis=this===globalThis;\
            return value;\
        })",
        "caller sloppy mapper",
    );
    let sloppy_source = eval_object(
        &mut caller,
        "({0:1,length:1})",
        "caller sloppy-mapper source",
    );
    drop(
        caller
            .call(
                &from,
                Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
                &[
                    Value::Object(sloppy_source),
                    Value::Object(sloppy_map.as_object().try_clone().expect("duplicate root")),
                ],
            )
            .expect("cross-realm sloppy mapper"),
    );
    assert_eq!(
        caller
            .eval("__typedArrayFromSloppyThis")
            .expect("sloppy mapper this observation"),
        Value::Bool(true),
        "sloppy mapper did not normalize undefined in its own realm",
    );

    let strict_map = eval_callable(
        &runtime,
        &mut caller,
        "(function(value){\
            'use strict';\
            globalThis.__typedArrayFromStrictThis=this===undefined;\
            return value;\
        })",
        "caller strict mapper",
    );
    let strict_source = eval_object(
        &mut caller,
        "({0:1,length:1})",
        "caller strict-mapper source",
    );
    drop(
        caller
            .call(
                &from,
                Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
                &[
                    Value::Object(strict_source),
                    Value::Object(strict_map.as_object().try_clone().expect("duplicate root")),
                ],
            )
            .expect("cross-realm strict mapper"),
    );
    assert_eq!(
        caller
            .eval("__typedArrayFromStrictThis")
            .expect("strict mapper this observation"),
        Value::Bool(true),
        "strict mapper did not preserve undefined",
    );

    assert!(matches!(
        caller.call(
            &from,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[Value::Null],
        ),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm null source TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "null source TypeError did not use the builtin defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "null source TypeError leaked into the caller realm",
    );

    let conversion_source = eval_object(
        &mut caller,
        "({0:Symbol('typed-array-from'),length:1})",
        "caller conversion-error source",
    );
    assert!(matches!(
        caller.call(
            &from,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[Value::Object(conversion_source)],
        ),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm conversion TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error),
        "element conversion TypeError did not use the builtin defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
        "element conversion TypeError leaked into the caller realm",
    );

    let token = eval_object(&mut caller, "({token:1})", "caller abrupt token");
    define_global(
        &runtime,
        &mut caller,
        "__typedArrayFromToken",
        Value::Object(token.try_clone().expect("duplicate root")),
    );
    let abrupt_source = eval_object(
        &mut caller,
        "Object.defineProperty({},Symbol.iterator,{\
            get:function(){throw __typedArrayFromToken}\
        })",
        "caller abrupt iterator source",
    );
    assert!(matches!(
        caller.call(
            &from,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[Value::Object(abrupt_source)],
        ),
        Err(RuntimeError::Exception),
    ));
    let thrown = caller
        .take_exception()
        .expect("take cross-realm abrupt value")
        .expect("cross-realm abrupt value was missing");
    assert_eq!(thrown, Value::Object(token));
}

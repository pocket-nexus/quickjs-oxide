//! Instruction fixtures execute the published core; protocol probes use real JS.
use super::numeric::{number_to_int32, number_to_uint32};
use crate::engine::api::{Error, Runtime};
use crate::engine::code::bytecode::{DetachedBytecode, Instruction};
use crate::engine::code::function::{
    UnlinkedConstant, UnlinkedFunction, metadata::FunctionMetadata,
};
use crate::engine::value::{JsString, Value};

struct PublishedFixture;
impl PublishedFixture {
    fn execute(&self, function: &DetachedBytecode<Value>) -> Result<Value, Error> {
        function.verify()?;
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let constants = function
            .constants
            .iter()
            .cloned()
            .map(UnlinkedConstant::primitive)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::internal(e.to_string()))?;
        let draft = UnlinkedFunction::fixture(
            function.code.clone(),
            constants,
            FunctionMetadata {
                local_count: function.local_count,
                max_stack: function.max_stack,
                ..FunctionMetadata::default()
            },
        );
        let published = runtime
            .publish_unlinked_function(context.realm, draft)
            .map_err(|e| Error::internal(e.to_string()))?;
        context
            .execute(&published)
            .map_err(|e| Error::internal(e.to_string()))
    }
}

fn assert_js(source: &str) {
    assert_eq!(
        Runtime::new().new_context().eval(source).unwrap(),
        Value::Bool(true),
        "{source}"
    );
}

#[test]
fn executes_arithmetic_stack_bytecode() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(6),
            Instruction::PushI32(7),
            Instruction::Mul,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };

    assert_eq!(PublishedFixture.execute(&function).unwrap(), Value::Int(42));
}

#[test]
fn dup3_clones_the_three_values_in_order_without_a_temporary_buffer() {
    for (drop_count, expected) in [(0, 3), (1, 2), (2, 1)] {
        let mut code = vec![
            Instruction::PushI32(1),
            Instruction::PushI32(2),
            Instruction::PushI32(3),
            Instruction::Dup3,
        ];
        code.extend(std::iter::repeat_n(Instruction::Drop, drop_count));
        code.push(Instruction::Return);
        let function = DetachedBytecode::<Value> {
            code,
            constants: vec![],
            local_count: 0,
            max_stack: 6,
        };

        assert_eq!(
            PublishedFixture.execute(&function).unwrap(),
            Value::Int(expected)
        );
    }
}

#[test]
fn unary_arithmetic_preserves_quickjs_numeric_tags_and_float_bits() {
    fn execute(value: Value, instruction: Instruction) -> Value {
        PublishedFixture
            .execute(&DetachedBytecode::<Value> {
                code: vec![Instruction::PushConst(0), instruction, Instruction::Return],
                constants: vec![value],
                local_count: 0,
                max_stack: 1,
            })
            .unwrap()
    }

    fn execute_post(value: Value, instruction: Instruction, selector: Instruction) -> Value {
        PublishedFixture
            .execute(&DetachedBytecode::<Value> {
                code: vec![
                    Instruction::PushConst(0),
                    instruction,
                    selector,
                    Instruction::Return,
                ],
                constants: vec![value],
                local_count: 0,
                max_stack: 2,
            })
            .unwrap()
    }

    for (instruction, expected) in [
        (Instruction::Neg, (-42.0_f64).to_bits()),
        (Instruction::Plus, 42.0_f64.to_bits()),
        (Instruction::Inc, 43.0_f64.to_bits()),
        (Instruction::Dec, 41.0_f64.to_bits()),
    ] {
        let Value::Float(actual) = execute(Value::Float(42.0), instruction) else {
            panic!("integral Float64 unary result lost its Float tag");
        };
        assert_eq!(actual.to_bits(), expected);
    }

    let nan_bits = 0x7ff8_0000_0000_0042;
    let Value::Float(negated_nan) =
        execute(Value::Float(f64::from_bits(nan_bits)), Instruction::Neg)
    else {
        panic!("negated NaN lost its Float tag");
    };
    assert_eq!(negated_nan.to_bits(), nan_bits ^ (1_u64 << 63));

    for (value, instruction, expected) in [
        (0, Instruction::Neg, (-0.0_f64).to_bits()),
        (i32::MIN, Instruction::Neg, 2_147_483_648.0_f64.to_bits()),
        (i32::MAX, Instruction::Inc, 2_147_483_648.0_f64.to_bits()),
        (i32::MIN, Instruction::Dec, (-2_147_483_649.0_f64).to_bits()),
    ] {
        let Value::Float(actual) = execute(Value::Int(value), instruction) else {
            panic!("Int32 unary boundary did not promote to Float64");
        };
        assert_eq!(actual.to_bits(), expected);
    }

    for (instruction, old_bits, new_bits) in [
        (Instruction::PostInc, 42.0_f64.to_bits(), 43.0_f64.to_bits()),
        (Instruction::PostDec, 42.0_f64.to_bits(), 41.0_f64.to_bits()),
    ] {
        let Value::Float(old) =
            execute_post(Value::Float(42.0), instruction.clone(), Instruction::Drop)
        else {
            panic!("postfix Float64 old value lost its Float tag");
        };
        let Value::Float(new) = execute_post(Value::Float(42.0), instruction, Instruction::Nip)
        else {
            panic!("postfix Float64 replacement lost its Float tag");
        };
        assert_eq!(old.to_bits(), old_bits);
        assert_eq!(new.to_bits(), new_bits);
    }

    assert_eq!(
        execute_post(
            Value::Int(i32::MAX),
            Instruction::PostInc,
            Instruction::Drop
        ),
        Value::Int(i32::MAX)
    );
    let Value::Float(promoted) =
        execute_post(Value::Int(i32::MAX), Instruction::PostInc, Instruction::Nip)
    else {
        panic!("postfix Int32 boundary replacement did not promote to Float64");
    };
    assert_eq!(promoted.to_bits(), 2_147_483_648.0_f64.to_bits());
}

#[test]
fn swap_exchanges_only_the_top_two_values() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(20),
            Instruction::PushI32(22),
            Instruction::Swap,
            Instruction::Sub,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };
    assert_eq!(PublishedFixture.execute(&function).unwrap(), Value::Int(2));
}

#[test]
fn detached_vm_catches_values_and_manages_private_handlers() {
    let thrown = DetachedBytecode::<Value> {
        code: vec![
            Instruction::Catch(3),
            Instruction::PushI32(7),
            Instruction::Throw,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };
    assert_eq!(PublishedFixture.execute(&thrown).unwrap(), Value::Int(7));

    let normal = DetachedBytecode::<Value> {
        code: vec![
            Instruction::Catch(4),
            Instruction::DropCatch,
            Instruction::PushI32(3),
            Instruction::Return,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 1,
    };
    assert_eq!(PublishedFixture.execute(&normal).unwrap(), Value::Int(3));

    let nip = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(10),
            Instruction::Catch(6),
            Instruction::PushI32(20),
            Instruction::PushI32(30),
            Instruction::NipCatch,
            Instruction::Return,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 4,
    };
    assert_eq!(PublishedFixture.execute(&nip).unwrap(), Value::Int(30));

    let nested = DetachedBytecode::<Value> {
        code: vec![
            Instruction::Catch(7),
            Instruction::Catch(5),
            Instruction::PushI32(11),
            Instruction::Throw,
            Instruction::Nop,
            Instruction::Throw,
            Instruction::Nop,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 3,
    };
    assert_eq!(PublishedFixture.execute(&nested).unwrap(), Value::Int(11));
}

#[test]
fn published_core_allocates_array_with_realm_intrinsics() {
    let function = DetachedBytecode::<Value> {
        code: vec![Instruction::ArrayFrom(0), Instruction::Return],
        constants: vec![],
        local_count: 0,
        max_stack: 1,
    };
    assert!(matches!(
        PublishedFixture.execute(&function).unwrap(),
        Value::Object(_)
    ));
}

#[test]
fn detached_vm_executes_typed_gosub_return_and_cleanup() {
    let returning = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(9),
            Instruction::Gosub(4),
            Instruction::Return,
            Instruction::Nop,
            Instruction::Ret,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };
    assert_eq!(PublishedFixture.execute(&returning).unwrap(), Value::Int(9));

    let abrupt = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(9),
            Instruction::Gosub(4),
            Instruction::Return,
            Instruction::Nop,
            Instruction::DropGosub,
            Instruction::Drop,
            Instruction::PushI32(4),
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };
    assert_eq!(PublishedFixture.execute(&abrupt).unwrap(), Value::Int(4));

    let caught_inside_gosub = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(9),
            Instruction::Gosub(4),
            Instruction::Return,
            Instruction::Nop,
            Instruction::Catch(8),
            Instruction::PushI32(7),
            Instruction::Throw,
            Instruction::Nop,
            Instruction::Drop,
            Instruction::Ret,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 4,
    };
    assert_eq!(
        PublishedFixture.execute(&caught_inside_gosub).unwrap(),
        Value::Int(9)
    );
}

#[test]
fn detached_vm_uses_the_declared_undefined_local_frame() {
    let initial = DetachedBytecode::<Value> {
        code: vec![Instruction::GetLocal(0), Instruction::Return],
        constants: vec![],
        local_count: 1,
        max_stack: 1,
    };
    assert_eq!(
        PublishedFixture.execute(&initial).unwrap(),
        Value::Undefined
    );

    let written = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(42),
            Instruction::PutLocal(0),
            Instruction::GetLocal(0),
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 1,
        max_stack: 1,
    };
    assert_eq!(PublishedFixture.execute(&written).unwrap(), Value::Int(42));
}

#[test]
fn ordinary_local_and_argument_writes_keep_assignment_results_and_owner_fallback() {
    assert_js(
        r#"(function(a) {
            var x = 1;
            var keptLocal = (x = 2);
            if (keptLocal !== 2 || x !== 2) return false;
            x = 3;
            var keptArgument = (a = 4);
            if (keptArgument !== 4 || a !== 4) return false;
            a = 5;
            var owner = { marker: 7 };
            x = owner;
            x = 6;
            a = owner;
            a = 8;
            if (owner.marker !== 7 || x !== 6 || a !== 8) return false;
            x = -0;
            a = -0;
            return 1 / x === -Infinity && 1 / a === -Infinity;
        })(0)"#,
    );
}

#[test]
fn executes_power_stack_bytecode_and_quickjs_number_edges() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(2),
            Instruction::PushI32(3),
            Instruction::PushI32(2),
            Instruction::Pow,
            Instruction::Pow,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 3,
    };

    assert_eq!(
        PublishedFixture.execute(&function).unwrap(),
        Value::Int(512)
    );
    assert!(crate::engine::value::number::pow(1.0, f64::INFINITY).is_nan());
    assert!(crate::engine::value::number::pow(-1.0, f64::NEG_INFINITY).is_nan());
    assert!(crate::engine::value::number::pow(-2.0, 0.5).is_nan());
    assert_eq!(crate::engine::value::number::pow(f64::NAN, 0.0), 1.0);
    assert_eq!(crate::engine::value::number::pow(2.0, -2.0), 0.25);
    assert_eq!(
        crate::engine::value::number::pow(-0.0, 3.0).to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(
        crate::engine::value::number::pow(-0.0, -3.0),
        f64::NEG_INFINITY
    );
}

#[test]
fn executes_string_addition() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushConst(0),
            Instruction::PushConst(1),
            Instruction::Add,
            Instruction::Return,
        ],
        constants: vec![
            Value::String(JsString::from_static("quick")),
            Value::String(JsString::from_static("js")),
        ],
        local_count: 0,
        max_stack: 2,
    };

    assert_eq!(
        PublishedFixture.execute(&function).unwrap(),
        Value::String(JsString::from_static("quickjs"))
    );
}

#[test]
fn executes_bitwise_stack_bytecode() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(0b1010),
            Instruction::PushI32(0b1100),
            Instruction::BitAnd,
            Instruction::BitNot,
            Instruction::PushI32(0b0011),
            Instruction::BitXor,
            Instruction::PushI32(0b0100),
            Instruction::BitOr,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };

    assert_eq!(
        PublishedFixture.execute(&function).unwrap(),
        Value::Int(-12)
    );
}

#[test]
fn to_int32_uses_ecmascript_modulo_semantics() {
    for (value, expected) in [
        (0.0, 0),
        (-0.0, 0),
        (f64::NAN, 0),
        (f64::INFINITY, 0),
        (f64::NEG_INFINITY, 0),
        (1.9, 1),
        (-1.9, -1),
        (2_147_483_647.0, i32::MAX),
        (2_147_483_648.0, i32::MIN),
        (4_294_967_295.0, -1),
        (4_294_967_296.0, 0),
        (4_294_967_297.0, 1),
        (-2_147_483_649.0, i32::MAX),
        (1.0e300, 0),
    ] {
        assert_eq!(number_to_int32(value), expected, "input {value:?}");
    }

    for (value, expected) in [
        (-1.0, u32::MAX),
        (2_147_483_648.0, 2_147_483_648),
        (4_294_967_295.0, u32::MAX),
        (4_294_967_296.0, 0),
        (-4_294_967_295.0, 1),
    ] {
        assert_eq!(number_to_uint32(value), expected, "input {value:?}");
    }
}

#[test]
fn executes_shift_stack_bytecode() {
    let function = DetachedBytecode::<Value> {
        code: vec![
            Instruction::PushI32(-8),
            Instruction::PushI32(1),
            Instruction::Sar,
            Instruction::PushI32(1),
            Instruction::Shr,
            Instruction::PushI32(2),
            Instruction::Shl,
            Instruction::Return,
        ],
        constants: vec![],
        local_count: 0,
        max_stack: 2,
    };

    assert_eq!(PublishedFixture.execute(&function).unwrap(), Value::Int(-8));
}

#[test]
fn generator_initial_yield_resumes_without_an_input_operand() {
    assert_js(
        r#"(function(){var n=0;function* g(){n++;return 3;}var it=g();return n===0&&it.next(99).value===3&&n===1;})()"#,
    );
}

#[test]
fn generator_yield_snapshot_and_next_resume_preserve_quickjs_stack_abi() {
    assert_js(
        r#"(function(){function* g(){var a=yield 2;return a+3;}var it=g();return it.next().value===2&&it.next(4).value===7;})()"#,
    );
}

#[test]
fn generator_yield_return_resume_pushes_magic_one() {
    assert_js(
        r#"(function(){function* g(){yield 1;return 2;}var it=g();it.next();var r=it.return(7);return r.done&&r.value===7;})()"#,
    );
}

#[test]
fn generator_plain_yield_throw_enters_existing_unwind_path() {
    assert_js(
        r#"(function(){function* g(){try{yield 1;}catch(e){return e+2;}}var it=g();it.next();return it.throw(5).value===7;})()"#,
    );
}

#[test]
fn generator_yield_return_runs_compiled_finally_unwind_path() {
    assert_js(
        r#"(function(){var n=0;function* g(){try{yield 1;}finally{n=9;}}var it=g();it.next();return it.return(7).value===7&&n===9;})()"#,
    );
}

#[test]
fn generator_yield_star_throw_resume_injects_magic_two() {
    assert_js(
        r#"(function(){function* inner(){try{yield 1;}catch(e){yield e;}}function* g(){yield* inner();}var it=g();it.next();return it.throw(8).value===8;})()"#,
    );
}

#[test]
fn yield_star_iterator_start_and_next_keep_an_ordinary_four_slot_record() {
    assert_js(
        r#"(function(){function* g(){return yield* [2,3];}var it=g();return it.next().value===2&&it.next().value===3&&it.next().done;})()"#,
    );
}

#[test]
fn yield_star_iterator_call_uses_typed_method_and_argument_modes() {
    assert_js(
        r#"(function(){var trace="";var obj={[Symbol.iterator](){return this;},next(v){trace+="n"+v;return {value:1,done:false};},return(v){trace+="r"+v;return {value:v,done:true};}};function* g(){yield* obj;}var it=g();it.next();var r=it.return(7);return r.value===7&&r.done&&trace==="nundefinedr7";})()"#,
    );
}

#[test]
fn yield_star_iterator_protocol_errors_match_quickjs() {
    assert_js(
        r#"(function(){function* g(){yield* {[Symbol.iterator](){return {next(){return 1;}};}};}try{g().next();return false;}catch(e){return e instanceof TypeError;}})()"#,
    );
}

#[test]
fn class_definition_opcodes_preserve_quickjs_stack_order() {
    assert_js(
        r#"(function(){var a=[];class A{[a.push("key")&&"m"](){return 7;}static x=3;}return new A().m()===7&&A.x===3&&a.join()==="key";})()"#,
    );
}

#[test]
fn check_ctor_rejects_calls_and_accepts_construction_frames() {
    assert_js(
        r#"(function(){class A{};var ok=new A() instanceof A;try{A();return false;}catch(e){return ok&&e instanceof TypeError;}})()"#,
    );
}

#[test]
fn borrowed_call_window_keeps_lower_operands_and_cleans_up_all_exit_kinds() {
    assert_js(
        r#"(function(){function f(x,y){return x+y;}var x=3+f(4,5)+6;try{f({valueOf(){throw 7;}},1);}catch(e){return x===18&&e===7;}return false;})()"#,
    );
}

#[test]
fn tail_invocations_complete_the_frame_with_exact_call_operands() {
    assert_js(
        r#"(function(){function f(){return this.x+arguments.length+arguments[0];}function g(){return f.call({x:3},4,5);}return g()===9;})()"#,
    );
}

#[test]
fn tail_invocation_throws_use_the_activation_backtrace_and_catch_path() {
    assert_js(
        r#"(function(){function f(){throw 4;}function g(){return f();}try{g();}catch(e){return e===4;}return false;})()"#,
    );
}

#[test]
fn eval_opcode_gates_original_identity_and_preserves_fallback_arguments() {
    assert_js(
        r#"(function(){var a=3;var direct=eval("a+1");return direct===4&&(function(eval){return eval(2,5);})(function(a,b){return a+b;})===7;})()"#,
    );
}

#[test]
fn string_direct_eval_forwards_environment_and_lazily_normalizes_this() {
    assert_js(r#"(function(){var x=1;var o={};return eval(o)===o&&eval("x=5; x")===5&&x===5;})()"#);
}

#[test]
fn arguments_opcode_forwards_kind_and_host_completion() {
    assert_js(
        r#"(function(a){arguments[0]=9;return a===9&&arguments.length===2;})(1,2)&& (function(a){"use strict";arguments[0]=9;return a===1;})(1)"#,
    );
}

#[test]
fn rest_opcode_forwards_start_and_host_completion() {
    assert_js(r#"(function(a,...rest){return a===1&&rest.join() === "2,3";})(1,2,3)"#);
}

#[test]
fn eval_variable_object_opcodes_preserve_stack_and_host_operands() {
    assert_js(r#"(function(){eval("var x=7;");var a=x;eval("x=8;");return a===7&&x===8;})()"#);
}

#[test]
fn to_object_boxes_primitives_and_rejects_nullish_values() {
    assert_js(
        r#"(function(){var r={..."ab"};let rejected=0;try{let {}=null;}catch(e){if(e instanceof TypeError)rejected++;}try{let {}=undefined;}catch(e){if(e instanceof TypeError)rejected++;}return rejected===2&&Object.keys(r).length===0&&Object(2).valueOf()===2;})()"#,
    );
}

#[test]
fn dynamic_environment_opcodes_forward_sources_strictness_and_stack_values() {
    assert_js(r#"(function(){var x=1,o={x:4};with(o){x+=2;}return o.x===6&&x===1;})()"#);
}

#[test]
fn iterator_unwind_preserves_exception_and_completion_precedence() {
    assert_js(
        r#"(function(){var closed=0;var it={[Symbol.iterator](){return this;},next(){return {value:1,done:false};},return(){closed++;throw 9;}};try{for(var x of it){throw 7;}}catch(e){return e===7&&closed===1;}return false;})()"#,
    );
}

#[test]
fn for_of_next_disables_done_and_throwing_iterators() {
    assert_js(
        r#"(function(){var closed=0;var it={[Symbol.iterator](){return this;},next(){throw 7;},return(){closed++;return {};}};try{for(var x of it){}}catch(e){return e===7&&closed===0;}return false;})()"#,
    );
}

#[test]
fn array_literal_opcodes_preserve_operands_and_element_order() {
    assert_js(r#"(function(){var n=0,a=[++n,...[2,3],++n];return a.join()==="1,2,3,2";})()"#);
}

#[test]
fn object_literal_opcodes_preserve_target_and_operand_order() {
    assert_js(
        r#"(function(){var n=0,o={a:++n,["b"]:++n,...{c:3}};return Object.keys(o).join()==="a,b,c"&&o.a===1&&o.b===2;})()"#,
    );
}

#[test]
fn object_rest_copy_reads_depth_operands_after_to_object_and_preserves_the_stack() {
    assert_js(
        r#"(function(){var {a,...rest}={a:1,b:2,c:3};return a===1&&rest.b===2&&rest.c===3&&!("a" in rest);})()"#,
    );
}

#[test]
fn object_literal_opcodes_forward_host_throws() {
    assert_js(
        r#"(function(){try{var o={...{get x(){throw 7;}}};return false;}catch(e){return e===7;}})()"#,
    );
}

#[test]
fn append_uses_iterator_protocol_and_preserves_pending_throw_on_close() {
    assert_js(
        r#"(function(){var n=0;var it={[Symbol.iterator](){return this;},next(){return ++n===3?{done:true}:{value:n,done:false};}};if([...it].join()!=="1,2")return false;let closed=0,pending={};let abrupt={[Symbol.iterator](){return this;},next(){return {value:1,done:false};},return(){closed++;throw 9;}};try{for(let x of abrupt){throw pending;}}catch(e){return e===pending&&closed===1;}return false;})()"#,
    );
}

#[test]
fn iterator_region_above_gosub_address_closes_without_consuming_it() {
    assert_js(
        r#"(function(){var trace="";var it={[Symbol.iterator](){return this;},next(){return {value:1,done:false};},return(){trace+="c";return {};}};function f(){try{for(var x of it){return 4;}}finally{trace+="f";}}return f()===4&&trace==="cf";})()"#,
    );
}

#[test]
fn captured_local_reuse_hook_is_limited_to_abrupt_resume_boundaries() {
    assert_js(
        r#"(function(){var fs=[];for(var i=0;i<3;i++){try{let x=i;fs.push(()=>x);if(i===1)throw 7;}catch(e){}}return fs.map(f=>f()).join()==="0,2,2";})()"#,
    );
}

#[test]
fn detached_vm_enforces_lexical_local_tdz_and_initialization() {
    assert_js(
        r#"(function(){try{let y=x;let x=1;return false;}catch(e){return e instanceof ReferenceError;}})()"#,
    );
}

#[test]
fn detached_vm_enforces_derived_this_one_shot_and_return_shape() {
    assert_js(
        r#"(function(){class A{}class B extends A{constructor(){super();try{super();}catch(e){if(e instanceof ReferenceError)return {ok:true};throw e;}}}return new B().ok;})()"#,
    );
}

#[test]
fn detached_vm_rejects_checked_writes_in_the_tdz_and_allows_plain_reinitialization() {
    assert_js(
        r#"(function(){try{x=2;let x;return false;}catch(e){if(!(e instanceof ReferenceError))return false;}let a=1;a=2;return a===2;})()"#,
    );
}

#[test]
fn detached_membership_rejects_primitive_right_operands_before_host_dispatch() {
    assert_js(
        r#"(function(){try{"x" in 1;return false;}catch(e){if(!(e instanceof TypeError))return false;}try{({}) instanceof 1;return false;}catch(e){return e instanceof TypeError;}})()"#,
    );
}

#[test]
fn string_addition_builds_ropes_and_reports_the_quickjs_length_error() {
    assert_js(
        r#"(function(){var x="";for(var i=0;i<1000;i++)x+="ab";return x.length===2000&&x.slice(-4)==="abab";})()"#,
    );
}

#[test]
fn await_fulfilment_and_rejection_use_the_same_published_core() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    drop(context.eval("var fulfilled=0,rejected=0; async function f(){return 2+await 5;} async function g(){try{await Promise.reject(7);}catch(e){return e+3;}} f().then(x=>fulfilled=x);g().then(x=>rejected=x);").unwrap());
    while runtime.is_job_pending() {
        runtime.execute_pending_job().unwrap();
    }
    assert_eq!(
        context.eval("fulfilled===7&&rejected===10").unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn dynamic_import_preserves_argument_evaluation_before_async_conversion() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    drop(context.eval("var importTrace='';function spec(){importTrace+='s';return {toString(){importTrace+='t';throw 7;}};}function opts(){importTrace+='o';return {};}var importError;import(spec(),opts()).catch(e=>importError=e);").unwrap());
    while runtime.is_job_pending() {
        runtime.execute_pending_job().unwrap();
    }
    assert_eq!(
        context
            .eval("importTrace==='sot'&&importError===7")
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn native_getter_and_conversion_callbacks_share_the_explicit_call_continuation() {
    assert_js(
        r#"(function(){
        const token={}, trace=[];
        const target={x:42};
        const boundGetter=Function.prototype.call.bind(function(){trace.push(this.x);return this.x;},target);
        const object={};
        Object.defineProperty(object,'answer',{get:boundGetter});
        Object.defineProperty(object,'maximum',{get:Math.max.bind(null,20,42)});
        const numeric={valueOf:Math.max.bind(null,40,42)};
        const abrupt={valueOf:Function.prototype.call.bind(function(){throw token;},null)};
        function tail(value){return Math.max.apply(null,[value,42]);}
        let caught=false;
        try{abrupt-1;}catch(error){caught=error===token;}
        let result=object.answer===42&&object.maximum===42&&numeric-1===41&&tail(2)===42;
        function recurse(depth){return depth===0?42:recurse.call(null,depth-1);}
        return result&&caught&&trace.join()==='42'&&recurse(128)===42;
    })()"#,
    );
}

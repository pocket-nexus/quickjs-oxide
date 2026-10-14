//! JS-observable witnesses harvested before retiring PR88.
use crate::engine::api::Value;
use crate::engine::test_support::assert_script_true;
use crate::engine::test_support::test_context;

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/arrays.rs
#[test]
fn definition_array_length_conversion_reloads_flags_and_preserves_two_number_calls() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "(()=>{let calls=0; const a=[1,2,3]; const n={valueOf(){calls++; if(calls===2)Object.defineProperty(a,'length',{writable:false}); return 1;}}; const ok=Reflect.defineProperty(a,'length',{value:n}); return !ok&&calls===2&&a.length===3;})()",
    );
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_proxy_descriptor_trap_receives_owned_edges_and_rejection_stays_observable() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "(()=>{let calls=0;const v={tag:1};const p=new Proxy({}, {defineProperty(t,k,d){calls++;return k==='x'&&d.value===v&&d.writable&&d.enumerable&&d.configurable;}});p.x=v;return calls===1&&!Object.hasOwn(p,'x');})()",
    );
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_shared_typed_postconversion_word_uses_selected_arc_outside_state() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "(()=>{const b=new SharedArrayBuffer(8,{maxByteLength:16});const t=new Uint16Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.grow(16);return 513;}}});return ok&&calls===1&&t[0]===513&&t.length===8;})()",
    );
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_typed_conversion_observes_resize_then_revalidates_current_range() {
    // TypedWriteStep preserves its existing true completion after conversion,
    // while the selected word kernel suppresses a now-out-of-range write.
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "(()=>{const b=new ArrayBuffer(8,{maxByteLength:16});const t=new Uint8Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.resize(0);return 9;}}});return ok&&calls===1&&t.length===0&&t[0]===undefined;})()",
    );
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_typed_numeric_miss_does_not_coerce_value_named_key_uses_ordinary_kernel() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "(()=>{const t=new Uint8Array(1);let calls=0;const v={valueOf(){calls++;return 7;}};const bad=Reflect.defineProperty(t,'-0',{value:v});const named=Reflect.defineProperty(t,'01',{value:v,writable:true});return !bad&&named&&calls===0&&t['01']===v&&t[0]===0;})()",
    );
}

// Source: 3b0ee61aa42b1aea9e1526849f856d31804042f7:src/engine/vm/property_driver/named_read_tests.rs
#[test]
fn primitive_getter_this_uses_the_existing_strict_and_sloppy_binding_contract() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"(()=>{
        Object.defineProperty(Number.prototype,'strictNamedThis',{get:function(){'use strict';return this},configurable:true});
        Object.defineProperty(Number.prototype,'sloppyNamedThis',{get:function(){return this},configurable:true});
        return (23).strictNamedThis===23 && typeof (23).sloppyNamedThis==='object';
    })()"#,
    );
}

// Source: 55c154a891fc2dab074c0ec0909f8cf8a79a35ea:src/engine/vm/call/bound/tests.rs
#[test]
fn bound_getters_and_toprimitive_callbacks_preserve_the_actual_receiver_hint_and_effects() {
    let (_, mut context) = test_context();
    for source in [
        "(()=>{let n=0,r={v:40};let g=(function(a){n++;return this.v+a}).bind(r,2).bind({v:100});let o=Object.defineProperty({},'x',{get:g});const {x:v}=o;return v===42&&n===1})()",
        "(()=>{let n=0,r={v:42};let m=(function(h){n++;return h==='number'?this.v:0}).bind(r);let o={[Symbol.toPrimitive]:m};return Math.abs(o)===42&&n===1})()",
        "(()=>{let log='';let o={[Symbol.toPrimitive]:(function(h){log+=h;return this.text}).bind({text:' 42 '})};let v=String.prototype.trim.call(o);return v==='42'&&log==='string'})()",
    ] {
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
    }
}

// Source: d1e455245221659ddfbb6d0328057d0ffc814fd2:src/engine/builtins/date/prototype/operation/tests.rs
#[test]
fn date_converting_vm_forced_ordinary_hints_preserve_order_and_never_call_exotic_method() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"(() => {
        let trace = '';
        const value = {
            get [Symbol.toPrimitive]() { throw 'exotic lookup'; },
            get valueOf() { trace += 'vg'; return function() { trace += 'vc'; return 7; }; },
            get toString() { trace += 'sg'; return function() { trace += 'sc'; return 'S'; }; }
        };
        const primitive = Date.prototype[Symbol.toPrimitive];
        if (primitive.call(value,'number') !== 7 || primitive.call(value,'integer') !== 7 ||
            primitive.call(value,'string') !== 'S' || primitive.call(value,'default') !== 'S') return false;
        try { primitive.call(value,{valueOf(){throw 'hint conversion';}}); return false; }
        catch (e) { if (!(e instanceof TypeError) || e.message !== 'invalid hint') return false; }
        try { primitive.call(1,'number'); return false; }
        catch (e) { if (!(e instanceof TypeError) || e.message !== 'not an object') return false; }
        return trace === 'vgvcvgvcsgscsgsc';
    })()"#,
    );
}

// Source: 82e107ff7bc392812f3a862c739ba838392bbe7a:src/engine/vm/property_driver/computed_read_tests.rs
#[test]
fn computed_proxy_conversion_and_throw_callbacks_are_not_replayed() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"(()=>{let keys=0,gets=0,trace='';
        const key={toString(){keys++;return 'x'}};
        const object=new Proxy({x:7},{get(t,k,r){gets++;if(k!=='x'||r!==object)throw 'roles';return t[k]}});
        const a=object[key];
        const throwing=new Proxy({}, {get(){trace+='p';throw 13}});
        let b=0;try{b=5+throwing[key]}catch(e){b=e;trace+='c'}
        return a===7&&b===13&&keys===2&&gets===1&&trace==='pc';
    })()"#,
    );
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/delete/tests.rs
#[test]
fn ordinary_dictionary_delete_preserves_policy_and_does_not_invoke_accessor() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "var deletedReads=0; var deletedObject={a:1,b:2}; Object.defineProperty(deletedObject,'long_deletion_accessor',{get(){deletedReads++;return 3},configurable:true}); delete deletedObject.long_deletion_accessor && deletedReads===0 && deletedObject.a===1 && deletedObject.b===2",
    );
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/delete/tests.rs
#[test]
fn typed_and_string_virtual_delete_use_the_canonical_selected_dependencies() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        "var deletionTyped=new Uint8Array([1,2]); var deletionString=new String('ab'); !Reflect.deleteProperty(deletionTyped,'0') && Reflect.deleteProperty(deletionTyped,'5') && Reflect.deleteProperty(deletionTyped,'-0') && !Reflect.deleteProperty(deletionString,'0') && Reflect.deleteProperty(deletionString,'5')",
    );
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/vm/property_driver/write_state_tests.rs
#[test]
fn array_length_and_typed_conversion_keep_selected_effect_order_and_authoritative_storage() {
    let (_, mut context) = test_context();
    assert_script_true(
        &mut context,
        r#"(()=>{
        let trace='';const a=[1,2,3];
        const length={valueOf(){trace+='l';return 2}};a.length=length;
        const typed=new Uint8Array(1),value={valueOf(){trace+='v';return 257}};
        typed[0]=value;typed[-0]=3;
        const invalid={valueOf(){trace+='i';return 4}};typed['-0']=invalid;
        const receiver={};const different=Reflect.set(typed,'0',7,receiver);
        return a.length===2&&a[1]===2&&a[2]===undefined&&typed[0]===3&&different&&receiver[0]===7&&trace==='llvi';
    })()"#,
    );
}

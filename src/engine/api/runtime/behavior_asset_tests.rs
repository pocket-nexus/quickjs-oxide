//! JS-observable witnesses harvested before retiring PR88.
use crate::engine::api::{Runtime, Value};

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/arrays.rs
#[test]
fn definition_array_length_conversion_reloads_flags_and_preserves_two_number_calls() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let calls=0; const a=[1,2,3]; const n={valueOf(){calls++; if(calls===2)Object.defineProperty(a,'length',{writable:false}); return 1;}}; const ok=Reflect.defineProperty(a,'length',{value:n}); return !ok&&calls===2&&a.length===3;})()").unwrap(), Value::Bool(true));
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_proxy_descriptor_trap_receives_owned_edges_and_rejection_stays_observable() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let calls=0;const v={tag:1};const p=new Proxy({}, {defineProperty(t,k,d){calls++;return k==='x'&&d.value===v&&d.writable&&d.enumerable&&d.configurable;}});p.x=v;return calls===1&&!Object.hasOwn(p,'x');})()").unwrap(), Value::Bool(true));
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_shared_typed_postconversion_word_uses_selected_arc_outside_state() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const b=new SharedArrayBuffer(8,{maxByteLength:16});const t=new Uint16Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.grow(16);return 513;}}});return ok&&calls===1&&t[0]===513&&t.length===8;})()").unwrap(), Value::Bool(true));
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_typed_conversion_observes_resize_then_revalidates_current_range() {
    // TypedWriteStep preserves its existing true completion after conversion,
    // while the selected word kernel suppresses a now-out-of-range write.
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const b=new ArrayBuffer(8,{maxByteLength:16});const t=new Uint8Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.resize(0);return 9;}}});return ok&&calls===1&&t.length===0&&t[0]===undefined;})()").unwrap(), Value::Bool(true));
}

// Source: a66f422b93b6de138fc0d1cace44a45ebb16ed20:src/engine/heap/runtime/tests/ordinary_definition/typed.rs
#[test]
fn definition_typed_numeric_miss_does_not_coerce_value_named_key_uses_ordinary_kernel() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const t=new Uint8Array(1);let calls=0;const v={valueOf(){calls++;return 7;}};const bad=Reflect.defineProperty(t,'-0',{value:v});const named=Reflect.defineProperty(t,'01',{value:v,writable:true});return !bad&&named&&calls===0&&t['01']===v&&t[0]===0;})()").unwrap(), Value::Bool(true));
}

// Source: 3b0ee61aa42b1aea9e1526849f856d31804042f7:src/engine/vm/property_driver/named_read_tests.rs
#[test]
fn primitive_getter_this_uses_the_existing_strict_and_sloppy_binding_contract() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        Object.defineProperty(Number.prototype,'strictNamedThis',{get:function(){'use strict';return this},configurable:true});
        Object.defineProperty(Number.prototype,'sloppyNamedThis',{get:function(){return this},configurable:true});
        return (23).strictNamedThis===23 && typeof (23).sloppyNamedThis==='object';
    })()"#).unwrap(),Value::Bool(true));
}

// Source: 55c154a891fc2dab074c0ec0909f8cf8a79a35ea:src/engine/vm/call/bound/tests.rs
#[test]
fn bound_getters_and_toprimitive_callbacks_preserve_the_actual_receiver_hint_and_effects() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
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
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(() => {
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
    })()"#).unwrap(), Value::Bool(true));
}

// Source: 82e107ff7bc392812f3a862c739ba838392bbe7a:src/engine/vm/property_driver/computed_read_tests.rs
#[test]
fn computed_proxy_conversion_and_throw_callbacks_are_not_replayed() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{let keys=0,gets=0,trace='';
        const key={toString(){keys++;return 'x'}};
        const object=new Proxy({x:7},{get(t,k,r){gets++;if(k!=='x'||r!==object)throw 'roles';return t[k]}});
        const a=object[key];
        const throwing=new Proxy({}, {get(){trace+='p';throw 13}});
        let b=0;try{b=5+throwing[key]}catch(e){b=e;trace+='c'}
        return a===7&&b===13&&keys===2&&gets===1&&trace==='pc';
    })()"#).unwrap(), Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/delete/tests.rs
#[test]
fn ordinary_dictionary_delete_preserves_policy_and_does_not_invoke_accessor() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("var deletedReads=0; var deletedObject={a:1,b:2}; Object.defineProperty(deletedObject,'long_deletion_accessor',{get(){deletedReads++;return 3},configurable:true}); delete deletedObject.long_deletion_accessor && deletedReads===0 && deletedObject.a===1 && deletedObject.b===2").unwrap(),Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/delete/tests.rs
#[test]
fn typed_and_string_virtual_delete_use_the_canonical_selected_dependencies() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("var deletionTyped=new Uint8Array([1,2]); var deletionString=new String('ab'); !Reflect.deleteProperty(deletionTyped,'0') && Reflect.deleteProperty(deletionTyped,'5') && Reflect.deleteProperty(deletionTyped,'-0') && !Reflect.deleteProperty(deletionString,'0') && Reflect.deleteProperty(deletionString,'5')").unwrap(),Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/ordinary/set/legacy_tests.rs
#[test]
fn selected_dense_append_preserves_array_permissions_holes_and_special_keys() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(context.eval(r#"(() => {
            const a = [], marker = {}, symbol = Symbol();
            a[0] = marker; a[1] = 2;
            if (a.length !== 2 || a[0] !== marker) return false;
            a.length = 5; a[2] = 3;
            if (a.length !== 5 || 3 in a || 4 in a) return false;
            Object.defineProperty(a, 'length', { writable: false });
            a[3] = 4;
            if (a[3] !== 4 || Reflect.set(a, '5', 6)) return false;
            a['4294967295'] = 9; a[symbol] = marker;
            if (a.length !== 5 || a['4294967295'] !== 9 || a[symbol] !== marker) return false;
            const b = []; Object.preventExtensions(b);
            if (Reflect.set(b, '0', marker) || b.length !== 0) return false;
            let seen = 0;
            const prototype = Object.create(Array.prototype);
            Object.defineProperty(prototype, '0', { set(v) { seen++; if(v !== marker) throw 'bad'; } });
            const c = []; Object.setPrototypeOf(c, prototype); c[0] = marker;
            if (seen !== 1 || c.length !== 0 || Object.hasOwn(c, '0')) return false;
            const receiver = []; Object.setPrototypeOf(receiver, prototype);
            if (!Reflect.set({0: 1}, '0', marker, receiver) || seen !== 1 || receiver[0] !== marker || receiver.length !== 1) return false;
            return true;
        })()"#).unwrap(), Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/ordinary/set/legacy_tests.rs
#[test]
fn selected_missing_append_keeps_unique_dictionary_and_shared_shape_isolation() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(context.eval(r#"(() => {
            const a = {}, b = {};
            for (let i=0; i<12; i++) { a['k'+i]=i; b['k'+i]=i; }
            a.onlyA=12;
            if ('onlyA' in b || Object.keys(b).length !== 12) return false;
            delete a.k1; a.afterDelete=13; a.k1=14;
            const symbol=Symbol(); a[symbol]=15;
            if (a.k0 !== 0 || a.k1 !== 14 || a.afterDelete !== 13 || a[symbol] !== 15) return false;
            if (Object.keys(a).join(',') !== 'k0,k2,k3,k4,k5,k6,k7,k8,k9,k10,k11,onlyA,afterDelete,k1') return false;
            Object.preventExtensions(a);
            return !Reflect.set(a,'rejected',1) && a.k1 === 14 && b.k1 === 1;
        })()"#).unwrap(), Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/object/ordinary/set/legacy_tests.rs
#[test]
fn selected_missing_set_preserves_prototype_and_distinct_receiver_semantics() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(context.eval(r#"(() => {
            const symbol = Symbol('slot'), marker = {};
            let trace = '';
            const proto = { writable: 1, set setter(v) { trace += 's'; this.seen = v; } };
            Object.defineProperty(proto, 'readonly', { value: 1 });
            const object = Object.create(proto);
            object.writable = marker; object[symbol] = marker; object.setter = marker;
            if (object.writable !== marker || object[symbol] !== marker || object.seen !== marker || trace !== 's') return false;
            if (Reflect.set(object, 'readonly', marker) || Object.hasOwn(object, 'readonly')) return false;
            try { (function(){ 'use strict'; object.readonly = marker; })(); return false; }
            catch (e) { if (!(e instanceof TypeError)) return false; }
            const sealed = Object.preventExtensions(Object.create(proto));
            if (Reflect.set(sealed, 'newKey', marker)) return false;
            // Receiver's prototype is irrelevant once target has selected data.
            const receiver = Object.create({ set writable(v) { throw 'wrong receiver prototype'; } });
            if (!Reflect.set(proto, 'writable', marker, receiver) || receiver.writable !== marker) return false;
            const proxy = new Proxy({}, {
                set(t,k,v,r) { trace += 'p'; return Reflect.set(t,k,v,r); },
                getOwnPropertyDescriptor(t,k) { trace += 'd'; return Reflect.getOwnPropertyDescriptor(t,k); }
            });
            const child = Object.create(Object.create(proxy));
            child.key = marker;
            if (trace !== 'sp' || child.key !== marker) return false;
            const mutating = new Proxy({}, {set(t,k,v,r) {
                Object.defineProperty(r,k,{value:7,writable:false});
                return Reflect.set(t,k,v,r);
            }});
            const afterBoundary = Object.create(mutating);
            if (Reflect.set(afterBoundary,'key',marker) || afterBoundary.key !== 7) return false;
            return true;
        })()"#).unwrap(), Value::Bool(true));
}

// Source: 285d97a486c1264d8bd95d787521ad4e60233336:src/engine/vm/property_driver/write_state_tests.rs
#[test]
fn array_length_and_typed_conversion_keep_selected_effect_order_and_authoritative_storage() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        let trace='';const a=[1,2,3];
        const length={valueOf(){trace+='l';return 2}};a.length=length;
        const typed=new Uint8Array(1),value={valueOf(){trace+='v';return 257}};
        typed[0]=value;typed[-0]=3;
        const invalid={valueOf(){trace+='i';return 4}};typed['-0']=invalid;
        const receiver={};const different=Reflect.set(typed,'0',7,receiver);
        return a.length===2&&a[1]===2&&a[2]===undefined&&typed[0]===3&&different&&receiver[0]===7&&trace==='llvi';
    })()"#).unwrap(), Value::Bool(true));
}

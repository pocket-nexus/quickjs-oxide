use super::*;

#[test]
fn definition_typed_conversion_observes_resize_then_revalidates_current_range() {
    // TypedWriteStep preserves its existing true completion after conversion,
    // while the selected word kernel suppresses a now-out-of-range write.
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const b=new ArrayBuffer(8,{maxByteLength:16});const t=new Uint8Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.resize(0);return 9;}}});return ok&&calls===1&&t.length===0&&t[0]===undefined;})()").unwrap(), Value::Bool(true));
}

#[test]
fn definition_shared_typed_postconversion_word_uses_selected_arc_outside_state() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const b=new SharedArrayBuffer(8,{maxByteLength:16});const t=new Uint16Array(b);let calls=0;const ok=Reflect.defineProperty(t,'0',{value:{valueOf(){calls++;b.grow(16);return 513;}}});return ok&&calls===1&&t[0]===513&&t.length===8;})()").unwrap(), Value::Bool(true));
}

#[test]
fn definition_typed_numeric_miss_does_not_coerce_value_named_key_uses_ordinary_kernel() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{const t=new Uint8Array(1);let calls=0;const v={valueOf(){calls++;return 7;}};const bad=Reflect.defineProperty(t,'-0',{value:v});const named=Reflect.defineProperty(t,'01',{value:v,writable:true});return !bad&&named&&calls===0&&t['01']===v&&t[0]===0;})()").unwrap(), Value::Bool(true));
}

#[test]
fn definition_proxy_descriptor_trap_receives_owned_edges_and_rejection_stays_observable() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let calls=0;const v={tag:1};const p=new Proxy({}, {defineProperty(t,k,d){calls++;return k==='x'&&d.value===v&&d.writable&&d.enumerable&&d.configurable;}});p.x=v;return calls===1&&!Object.hasOwn(p,'x');})()").unwrap(), Value::Bool(true));
}

//! Observable Array mutation witnesses retained from the retired migration.
use crate::engine::api::{Runtime, Value};

fn eval(source: &str) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
}
#[test]
fn all_four_actual_vm_selectors_preserve_all_value_kinds_and_alias_results() {
    eval(
        r#"(()=>{
 const o={},s=Symbol('value'),v=[undefined,null,true,3,-0,'leaf',123456789012345678901234567890n,s,o];
 for(const x of v){const a=[];if(a.push(x)!==1||!Object.is(a.pop(),x)||a.length!==0)return false;
 const b=[9];if(b.unshift(x)!==2||!Object.is(b.shift(),x)||b[0]!==9)return false;}return true;
})()"#,
    );
}
#[test]
fn holes_and_inherited_last_getter_are_read_before_pop_delete_and_length() {
    eval(
        r#"(()=>{
 let log='';const p={get 2(){log+='g';return 17;}};const a=Object.create(p);a.length=3;
 if(Array.prototype.pop.call(a)!==17||a.length!==2||log!=='g'||Object.hasOwn(a,'2'))return false;
 const b=[,,];return b.pop()===undefined&&b.length===1&&!(0 in b);
})()"#,
    );
}
#[test]
fn length_conversion_snapshot_is_kept_when_callbacks_change_actual_length() {
    eval(
        r#"(()=>{
 let log='';const a={get length(){log+='l';return {valueOf(){log+='n';return 2;}}},set length(v){log+='L'+v;},get 1(){log+='g';this.extra=4;return 8;}};
 return Array.prototype.pop.call(a)===8&&log==='lngL1'&&a.extra===4;
})()"#,
    );
}
#[test]
fn selected_getter_setter_and_proxy_traps_run_once_in_exact_order() {
    eval(
        r#"(()=>{
 let log='';const t={length:1,0:6};const p=new Proxy(t,{get(o,k,r){log+='g'+k+';';return Reflect.get(o,k,r)},set(o,k,v,r){log+='s'+k+';';return Reflect.set(o,k,v,r)},deleteProperty(o,k){log+='d'+k+';';return Reflect.deleteProperty(o,k)}});
 if(Array.prototype.pop.call(p)!==6||log!=='glength;g0;d0;slength;')return false;
 log='';return Array.prototype.push.call(p,8)===1&&log==='glength;s0;slength;'&&t[0]===8;
})()"#,
    );
}
#[test]
fn shift_and_unshift_resume_the_selected_copy_phase_without_restarting_length() {
    eval(
        r#"(()=>{
 let n=0;const t={0:'a',2:'c',get length(){n++;return 3},set length(v){this.final=v}};
 if(Array.prototype.shift.call(t)!=='a'||n!==1||t[1]!=='c'||Object.hasOwn(t,'0')||t.final!==2)return false;
 let reads=0;const u={0:4,1:5,get length(){reads++;return 2},set length(v){this.final=v}};
 return Array.prototype.unshift.call(u,1,2)===4&&reads===1&&u[0]===1&&u[1]===2&&u[2]===4&&u[3]===5&&u.final===4;
})()"#,
    );
}
#[test]
fn partial_writes_and_delete_rejection_preserve_the_original_failure_order() {
    eval(
        r#"(()=>{
 const a={length:0};Object.defineProperty(a,'length',{writable:false});let caught=false;
 try{Array.prototype.push.call(a,7,8)}catch(e){caught=e instanceof TypeError}
 if(!caught||a[0]!==7||a[1]!==8||a.length!==0)return false;
 const b={length:1};Object.defineProperty(b,'0',{value:5,configurable:false});caught=false;
 try{Array.prototype.pop.call(b)}catch(e){caught=e instanceof TypeError}
 return caught&&b[0]===5&&b.length===1;
})()"#,
    );
}
#[test]
fn empty_pop_still_sets_length_and_push_checks_safe_limit_before_first_write() {
    eval(
        r#"(()=>{
 let log='';const a={get length(){log+='g';return -5},set length(v){log+='s'+v}};
 if(Array.prototype.pop.call(a)!==undefined||log!=='gs0')return false;
 const b={length:9007199254740991};let caught=false;try{Array.prototype.push.call(b,1)}catch(e){caught=e instanceof TypeError}
 return caught&&b.length===9007199254740991&&!Object.hasOwn(b,'9007199254740991');
})()"#,
    );
}
#[test]
fn string_virtual_and_typed_index_rejections_keep_language_semantics() {
    eval(
        r#"(()=>{
 let caught=0;try{Array.prototype.pop.call('ab')}catch(e){if(e instanceof TypeError)caught++}
 const a=new Int32Array([3,4]);try{Array.prototype.pop.call(a)}catch(e){if(e instanceof TypeError)caught++}
 try{Array.prototype.push.call(a,9)}catch(e){if(e instanceof TypeError)caught++}
 return caught===3&&a.length===2&&a[0]===3&&a[1]===4;
})()"#,
    );
}
#[test]
fn mapped_arguments_detach_delete_but_keep_unremoved_aliases() {
    eval(
        r#"(function(a,b){
 if(Array.prototype.pop.call(arguments)!==b||arguments.length!==1||Object.hasOwn(arguments,'1'))return false;
 a=11;if(arguments[0]!==11)return false;return Array.prototype.push.call(arguments,12)===2&&arguments[1]===12&&b===2;
})(1,2)"#,
    );
}
#[test]
fn nested_getter_mutation_restores_parent_and_lower_operand_owners() {
    eval(
        r#"(()=>{
 const marker={},a={get length(){const b=[4,5];if(b.shift()!==4||b.unshift(8)!==2)throw 1;return 1},get 0(){return marker},set length(v){this.n=v}};
 return [marker,Array.prototype.pop.call(a),marker].every(x=>x===marker)&&a.n===0;
})()"#,
    );
}
#[test]
fn getter_throw_catch_preserves_fault_owner_and_aborts_delete_length() {
    eval(
        r#"(()=>{
 const marker={},a={length:1,get 0(){throw marker}};let actual;
 try{Array.prototype.pop.call(a)}catch(e){actual=e}
 return actual===marker&&a.length===1&&Object.hasOwn(a,'0');
})()"#,
    );
}

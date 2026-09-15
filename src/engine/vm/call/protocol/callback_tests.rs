use crate::engine::api::{Runtime, Value, profiling::CostProfile};

#[test]
fn recycled_callback_frames_do_not_reuse_another_functions_static_key() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(context.eval("(()=>{let p=new Proxy({},{get(t,k){return k}});function first(){return p.first}function second(){return p.second}for(let i=0;i<10;i++){if(first()!=='first'||second()!=='second')return false}return true})()").unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn lazy_getters_and_proxy_traps_reuse_storage_and_return_directly() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    context
        .eval(
            "var getterObject={get x(){return 1}};var trapObject=new Proxy({},{get(){return 1}});",
        )
        .unwrap();
    let profile = CostProfile::start();
    assert_eq!(
        context
            .eval(
                "var total=0;for(var i=0;i<20;i++){total+=getterObject.x;total+=trapObject.x;}total"
            )
            .unwrap(),
        Value::Int(40)
    );
    let costs = profile.snapshot();
    let event = |name| costs.owned_execution_events.get(name).copied().unwrap_or(0);
    assert!(
        event("property_callback_lazy_install") >= 40,
        "{:?}",
        costs.owned_execution_events
    );
    assert_eq!(event("property_return_direct"), 20);
    assert!(event("method_resume_allocation") <= 1);
    assert!(event("get_resume_allocation") <= 1);
    // The root execution query and one child property query coexist. Only
    // these two cold boxes may grow; all twenty trap replies reuse the latter.
    assert!(
        costs
            .call_buffers
            .get("query.pending_box")
            .is_none_or(|cost| cost.capacity_growths <= 2),
        "{:?}",
        costs.call_buffers.get("query.pending_box")
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn lazy_property_callbacks_preserve_observer_stacks_trap_invariants_and_reentry() {
    let weak = {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "(()=>{function caller(){return ({get x(){return new Error().stack}}).x}let s=caller();return s.includes('caller')&&s.includes('get x')})()",
            "(()=>{function trap(){throw new Error('trap')}function caller(){return new Proxy({},{get:trap}).x}try{caller()}catch(e){return e.stack.includes('trap')&&e.stack.includes('caller')}return false})()",
            "(()=>{let t={};Object.defineProperty(t,'x',{value:1,writable:false,configurable:false});try{new Proxy(t,{get(){return 2}}).x}catch(e){return e instanceof TypeError}return false})()",
            "(()=>{let t={};Object.defineProperty(t,'x',{get:undefined,configurable:false});try{new Proxy(t,{get(){return 2}}).x}catch(e){return e instanceof TypeError}return false})()",
            "(()=>{let log='';let inner={get x(){log+='i';return 2}};let outer={get x(){log+='o';return inner.x}};let p=new Proxy(outer,{get(t,k,r){log+='p';return Reflect.get(t,k,r)}});return p.x===2&&log==='poi'})()",
            "(()=>{function original(x){return this.tag+x}let o={};Object.defineProperty(o,'x',{get:original.bind({tag:40},2)});return o.x===42})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        std::rc::Rc::downgrade(&runtime.0)
    };
    assert!(
        weak.upgrade().is_none(),
        "empty callback pools retained a Runtime"
    );
}

#[test]
fn native_leaf_proofs_skip_materialization_but_errors_and_objects_observe() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    context.eval("function mathLeaf(x){return Math.min(x,7)}; function mathOuter(x){return mathLeaf(x)};").unwrap();
    let profile = CostProfile::start();
    assert_eq!(
        context
            .eval("var mathResult=0;for(var i=0;i<20;i++)mathResult=mathOuter(i);mathResult")
            .unwrap(),
        Value::Int(7)
    );
    let costs = profile.snapshot();
    // GetField itself may observe, but native entry with proven numeric inputs
    // must not add a materialization event after every native selection.
    assert!(
        costs
            .owned_execution_events
            .get("native_unobserved_entry")
            .copied()
            .unwrap_or(0)
            >= 20
    );
    drop(profile);
    for source in [
        "(()=>{function bad(){return Math.min(Symbol())}try{bad()}catch(e){return e instanceof TypeError&&e.stack.includes('bad')}return false})()",
        "(()=>{function objectCaller(){return Math.min({valueOf(){throw new Error('converted')}})}try{objectCaller()}catch(e){return e.stack.includes('objectCaller')&&e.stack.includes('valueOf')}return false})()",
        "(()=>{function invalidBrand(){return Map.prototype.get.call({},0)}try{invalidBrand()}catch(e){return e instanceof TypeError&&e.stack.includes('invalidBrand')}return false})()",
    ] {
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
    }
}

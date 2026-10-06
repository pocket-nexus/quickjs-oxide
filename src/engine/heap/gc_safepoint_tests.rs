use crate::engine::api::{GcPolicy, Runtime, RuntimeError, Value};

fn pressure(runtime: &Runtime) {
    runtime.0.gc_pressure.remaining.set(1);
}

#[test]
fn automatic_cycle_gc_bounds_unreachable_loops_and_keeps_active_roots() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(
        runtime.gc_policy().expect("runtime state"),
        GcPolicy::Automatic
    );
    pressure(&runtime);
    assert_eq!(
        context
            .eval(
                r#"(function(arg){
        let local={n:20}; const capture=()=>local.n;
        for(let i=0;i<20000;i++){let x={};x.self=x;}
        return this.n+arg.n+capture();
    }).call({n:10},{n:12})"#
            )
            .unwrap(),
        Value::Int(42)
    );
    assert!(
        runtime.heap_counts().expect("runtime state").object_nodes
            < super::gc_pressure::MIN_GC_HEADROOM + 1024
    );
    assert!(!runtime.0.gc_pressure.collecting.get());
}

#[test]
fn automatic_cycle_gc_keeps_caller_operand_through_allocating_callback() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    pressure(&runtime);
    // The receiver has no global/local owner in the caller. It remains on the
    // caller's operand stack while the key callback allocates and collects.
    assert_eq!(
        context
            .eval(
                r#"
        function receiver() { let o={tag:42}; o.self=o; return o; }
        function key() {
            for(let i=0;i<20000;i++) { let dead={}; dead.self=dead; }
            return 'tag';
        }
        receiver()[key()]
    "#
            )
            .unwrap(),
        Value::Int(42)
    );
    assert!(
        runtime.heap_counts().unwrap().object_nodes < super::gc_pressure::MIN_GC_HEADROOM + 1024
    );
}

#[test]
fn automatic_cycle_gc_keeps_weak_targets_until_outer_turn_and_queues_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    runtime
        .with_execution_turn(|| {
            pressure(&runtime);
            drop(context.eval(
                r#"
            var weakTarget={}; weakTarget.self=weakTarget;
            var weakRef=new WeakRef(weakTarget);
            var weakSymbol=Symbol('kept'), symbolRef=new WeakRef(weakSymbol);
            var finalizers=0;
            var registry=new FinalizationRegistry(()=>finalizers++);
            registry.register(weakTarget,1);
            weakTarget=null;weakSymbol=null;
            for(let i=0;i<10000;i++){let x={};x.self=x;}
        "#,
            )?);
            assert_eq!(
                context.eval(
                    "weakRef.deref()!==undefined && symbolRef.deref()!==undefined && finalizers===0"
                )?,
                Value::Bool(true)
            );
            runtime.run_gc()?;
            assert_eq!(
                context.eval("weakRef.deref()!==undefined && symbolRef.deref()!==undefined")?,
                Value::Bool(true)
            );
            Ok(())
        })
        .unwrap();
    runtime.run_gc().unwrap();
    assert_eq!(
        context
            .eval("weakRef.deref()===undefined && symbolRef.deref()===undefined && finalizers===0")
            .unwrap(),
        Value::Bool(true)
    );
    // The existing collector removes dead weak registrations before trial
    // deletion. A target first killed by trial deletion is delivered on the
    // next collection, just as with explicit GC before this change.
    runtime.run_gc().unwrap();
    assert!(runtime.is_job_pending().expect("runtime state"));
    let mut jobs = 0;
    while runtime.is_job_pending().expect("runtime state") {
        jobs += 1;
        assert!(jobs < 10);
        runtime.execute_pending_job().unwrap();
    }
    assert_eq!(context.eval("finalizers").unwrap(), Value::Int(1));
}

#[test]
fn automatic_cycle_gc_keeps_suspended_and_pending_reaction_roots() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    drop(
        context
            .eval(
                r#"
        var it=(function*(){let x={n:21};yield 1;return x.n;})();it.next();
        var answer=0;
        (function(){let x={n:21};Promise.resolve().then(()=>answer=x.n);})();
    "#,
            )
            .unwrap(),
    );
    pressure(&runtime);
    drop(
        context
            .eval("for(let i=0;i<10000;i++){let x={};x.self=x;}")
            .unwrap(),
    );
    assert_eq!(
        context.eval("answer===0 && it.next().value===21").unwrap(),
        Value::Bool(true)
    );
    runtime.execute_pending_job().unwrap();
    assert_eq!(context.eval("answer").unwrap(), Value::Int(21));
    pressure(&runtime);
    assert_eq!(
        context.eval("for(let i=0;i<5000;i++){let x={};x.self=x;} throw 42"),
        Err(RuntimeError::Exception)
    );
    assert_eq!(context.take_exception().unwrap(), Some(Value::Int(42)));
}

#[test]
fn automatic_cycle_gc_defers_borrows_reentry_and_manual_policy() {
    let runtime = Runtime::new();
    let _context = runtime.new_context().expect("create context");
    runtime.0.gc_pressure.remaining.set(0);
    {
        let _borrow = runtime.0.state.borrow();
        runtime.collect_if_requested().unwrap();
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
    }
    runtime
        .set_gc_policy(GcPolicy::Manual)
        .expect("set GC policy");
    runtime.collect_if_requested().unwrap();
    assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
    runtime
        .set_gc_policy(GcPolicy::Automatic)
        .expect("set GC policy");
    runtime.0.gc_pressure.collecting.set(true);
    runtime.collect_if_requested().unwrap();
    assert!(runtime.run_gc().is_err());
    assert!(runtime.0.gc_pressure.collecting.get());
    runtime.0.gc_pressure.collecting.set(false);
    runtime.collect_if_requested().unwrap();
    assert!(runtime.0.gc_pressure.remaining.get() > 0);
}

#[test]
fn automatic_cycle_gc_defers_unwinding_without_repeating_vm_checkpoints() {
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    struct EvaluateOnDrop<'a> {
        context: &'a mut crate::engine::api::Context,
        progressed: &'a Cell<bool>,
    }
    impl Drop for EvaluateOnDrop<'_> {
        fn drop(&mut self) {
            self.progressed.set(matches!(
                self.context
                    .eval("let n=0; for(let i=0;i<4;i++) n+=i; n+36"),
                Ok(Value::Int(42))
            ));
        }
    }

    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let progressed = Cell::new(false);
    runtime.0.gc_pressure.remaining.set(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = EvaluateOnDrop {
            context: &mut context,
            progressed: &progressed,
        };
        panic!("exercise embedding entry while unwinding");
    }));
    assert!(result.is_err());
    assert!(
        !progressed.get(),
        "entry during unwind must reject state access"
    );
    assert!(runtime.is_poisoned());
    assert!(matches!(
        runtime.run_gc(),
        Err(crate::engine::api::RuntimeError::Poisoned)
    ));
}

#[test]
fn automatic_cycle_gc_traces_ephemeron_chains_and_preserves_owned_value_edges() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    drop(
        context
            .eval(
                r#"
        var first = new WeakMap(), second = new WeakMap();
        var liveKey = {}, middle = {}, payload = {answer: 42};
        first.set(liveKey, middle); second.set(middle, payload);
        var liveKeyRef = new WeakRef(liveKey), payloadRef = new WeakRef(payload);
        var deadKey = {}, deadValue = {answer: 7};
        var cycleKey = {}, cycleValue = {back: cycleKey};
        first.set(cycleKey, cycleValue);
        var cycleKeyRef = new WeakRef(cycleKey);
        first.set(deadKey, deadValue);
        var deadKeyRef = new WeakRef(deadKey), deadValueRef = new WeakRef(deadValue);
        middle = payload = deadKey = deadValue = cycleKey = cycleValue = null;
    "#,
            )
            .unwrap(),
    );
    pressure(&runtime);
    assert_eq!(
        context
            .eval(
                r#"
        for (let i=0;i<10000;i++) { let x={}; x.self=x; }
        second.get(first.get(liveKey)).answer === 42 &&
        payloadRef.deref() !== undefined && deadKeyRef.deref() === undefined &&
        deadValueRef.deref() === undefined && cycleKeyRef.deref() !== undefined
    "#
            )
            .unwrap(),
        Value::Bool(true)
    );
    // Existing QuickJS-compatible tracing treats WeakMap values as ordinary
    // owned edges. A value -> key backreference remains live while its map is
    // rooted; automatic scheduling must preserve that graph contract. Dropping
    // the maps releases it. The preceding derefs keep targets only through
    // that completed turn.
    pressure(&runtime);
    assert_eq!(
        context
            .eval(
                r#"
        liveKey = first = second = null;
        for (let i=0;i<10000;i++) { let x={}; x.self=x; }
        liveKeyRef.deref() === undefined && payloadRef.deref() === undefined &&
        cycleKeyRef.deref() === undefined
    "#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn automatic_cycle_gc_runs_inside_long_allocating_turns() {
    // These loops cover allocation paths with plain/fused branches, ordinary
    // calls, constructors, captured cells and array payloads. Check before the
    // outer turn closes so its final collection cannot mask missing VM service.
    let sources = [
        "for(let i=0;i<20000;i++){let x={};x.self=x}",
        "function make(){let x={};x.self=x;return x} for(let i=0;i<20000;i++) make()",
        "class C{constructor(){this.self=this}} for(let i=0;i<20000;i++) new C()",
        "for(let i=0;i<20000;i++){let x={};x.f=()=>x}",
        "for(let i=0;i<20000;i++){let x=[];x.push(x)}",
    ];
    for source in sources {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        runtime
            .with_execution_turn(|| {
                drop(context.eval(source)?);
                assert!(
                    runtime.heap_counts().expect("runtime state").object_nodes
                        < super::gc_pressure::MIN_GC_HEADROOM + 1024,
                    "{source}"
                );
                Ok(())
            })
            .unwrap();
    }
}

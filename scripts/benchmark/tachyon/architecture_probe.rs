//! Small behavioral checks for architecture capabilities, not throughput tests.
use std::{
    future::Future,
    num::NonZeroU32,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};
use tachyon_bytecode::{Bytecode, CompiledModule, VerifyContext};
use tachyon_compiler::{
    CompileOptions, Compiler, MediaType, SourceId, SourceMode, SourceName, SourceText,
};
use tachyon_gc::HeapLimit;
use tachyon_vm::{
    AtomHashSeed, AtomTableConfig, ExecutionBudget, Isolate, IsolateConfig, PromiseOutcome,
    RealmLimits, RunOutcome, StackLimits, Value,
};
fn compile(id: u32, source: &str) -> CompiledModule {
    Compiler
        .compile(
            SourceText::new(
                SourceId::new(id),
                SourceName::new("architecture-probe.js"),
                MediaType::JavaScript,
                Arc::from(source),
            ),
            CompileOptions {
                source_mode: SourceMode::Script,
                direct_eval: false,
            },
        )
        .unwrap()
}
fn isolate() -> Isolate {
    Isolate::new(IsolateConfig::new(
        AtomTableConfig::new(65536, 8 * 1024 * 1024, AtomHashSeed::new(1, 2)),
        HeapLimit::new(256 * 1024 * 1024),
        StackLimits::new(16384, 4 * 1024 * 1024),
        RealmLimits::new(1024, 65536),
    ))
    .unwrap()
}
fn number(v: Value) -> f64 {
    v.as_i32().map(f64::from).or_else(|| v.as_f64()).unwrap()
}
fn run(vm: &mut Isolate, module: &CompiledModule) -> Value {
    match vm
        .execute(
            module,
            ExecutionBudget {
                fuel: u64::MAX,
                quantum: u32::MAX,
            },
        )
        .unwrap()
    {
        RunOutcome::Completed(value) => value,
        other => panic!("unexpected {other:?}"),
    }
}
fn main() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<CompiledModule>();
    let module = compile(
        0,
        "var counter=(typeof counter==='undefined'?0:counter)+1;counter;",
    );
    let mut a = isolate();
    let mut b = isolate();
    assert_eq!(number(run(&mut a, &module)), 1.0);
    assert_eq!(number(run(&mut b, &module)), 1.0);
    assert_eq!(number(run(&mut a, &module)), 2.0);
    println!("shared_code_isolated_globals=1,1,2; compiled_module=Send+Sync");
    let invalid = Bytecode::from_words(vec![u32::MAX]);
    assert!(
        invalid
            .verify(VerifyContext {
                register_count: 1,
                constants: &[],
                function_count: 1,
                scope_name_count: 0,
                max_environment_slot_count: 0
            })
            .is_err()
    );
    println!("invalid_bytecode_rejected=true");
    let job = compile(
        1,
        "Promise.resolve().then(function(){var sum=0;for(var i=0;i<10000;i++)sum+=i;return sum;});",
    );
    let mut vm = isolate();
    let promise = match vm
        .execute_without_promise_checkpoint(
            &job,
            ExecutionBudget {
                fuel: u64::MAX,
                quantum: u32::MAX,
            },
        )
        .unwrap()
    {
        RunOutcome::Completed(v) => v,
        other => panic!("unexpected {other:?}"),
    };
    let mut context = Context::from_waker(Waker::noop());
    let quantum = NonZeroU32::new(8).unwrap();
    {
        let mut driver = vm.drive_promise(promise, quantum).unwrap();
        for _ in 0..5 {
            assert!(Pin::new(&mut driver).poll(&mut context).is_pending());
        }
    }
    // Recreate the host Future: progress remains owned by the isolate.
    let mut driver = vm.drive_promise(promise, quantum).unwrap();
    let mut polls = 5;
    loop {
        polls += 1;
        assert!(polls < 1000000);
        match Pin::new(&mut driver).poll(&mut context) {
            Poll::Pending => (),
            Poll::Ready(Ok(PromiseOutcome::Fulfilled(v))) => {
                assert_eq!(number(v), 49_995_000.0);
                println!(
                    "resumable_quantum=8; polls={polls}; future_recreated=true; result=49995000"
                );
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    println!("tachyon_value_bytes={}", std::mem::size_of::<Value>());
}

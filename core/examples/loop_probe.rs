//! Loop probe: runs `empty_loop` or `numeric_local_loop` as the lowering emits
//! them, for `perf stat` slopes and for the size and assembly of
//! `vm::interp::run`.
//!
//! ```sh
//! cargo build --release -p oxide-core --example loop_probe
//! perf stat -e instructions:u target/release/examples/loop_probe empty 1000000
//! nm -C --size-sort -S target/release/examples/loop_probe | grep interp::run
//! ```

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{CodeBuilder, Opcode, flags};
use oxide_core::heap::gc::HeapLimits;
use oxide_core::{Runtime, Value};

fn main() {
    let mut args = std::env::args().skip(1);
    let probe = args.next().unwrap_or_else(|| "empty".into());
    let n: i32 = args.next().map_or(1_000_000, |n| n.parse().unwrap());
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let mut b = CodeBuilder::new();
    // this = r0, n = p0 = r1, l0 = r2.
    match probe.as_str() {
        // for (var i = 0; i < n; i++) {} return n;
        "empty" => {
            b.a_int(Opcode::LoadInt, 2, 0);
            b.ab_jump(Opcode::JNlt, 2, 1, 4);
            b.ab(Opcode::Inc, 2, 2);
            b.jump(Opcode::Jmp, 1);
            b.a(Opcode::Return, 1);
        }
        // var sum = 0; while (n > 0) { sum = sum + 1; n = n - 1; } return sum;
        "numeric" => {
            b.a_int(Opcode::LoadInt, 2, 0);
            b.a_short_jump(Opcode::JNgtI, 1, 0, 5);
            b.ab_int(Opcode::AddI, 2, 2, 1);
            b.ab_int(Opcode::SubI, 1, 1, 1);
            b.jump(Opcode::Jmp, 1);
            b.a(Opcode::Return, 2);
        }
        other => panic!("unknown probe {other}"),
    }
    let unit = b.finish(UnitLayout {
        register_count: 3,
        parameter_count: 1,
        local_count: 1,
        flags: flags::STRICT,
        ..UnitLayout::default()
    });
    let unit = runtime.publish(unit).unwrap();
    let function = runtime.instantiate(unit, &[&[]]).unwrap();
    let result = runtime
        .call(
            Value::object(function),
            Value::UNDEFINED,
            &[Value::int32(n)],
        )
        .unwrap();
    println!("{result:?}");
}

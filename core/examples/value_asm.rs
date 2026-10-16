//! Release-assembly probe for `Value::kind` and `Value::as_object` (phase 1,
//! M1 acceptance). Build with
//! `cargo build --release -p oxide-core --example value_asm` and disassemble
//! the `probe_*` symbols.

use oxide_core::{Kind, ObjectId, Value};
use std::hint::black_box;

#[inline(never)]
pub fn probe_kind(value: Value) -> Kind {
    value.kind()
}

#[inline(never)]
pub fn probe_as_object(value: Value) -> Option<ObjectId> {
    value.as_object()
}

fn main() {
    let values = [
        Value::int32(1),
        Value::number(0.5),
        Value::UNDEFINED,
        Value::TRUE,
    ];
    let mut count = 0usize;
    for value in black_box(values) {
        count += probe_kind(value) as usize;
        count += black_box(probe_as_object(value)).map_or(0, |id| id.index() as usize);
    }
    println!("{count}");
}

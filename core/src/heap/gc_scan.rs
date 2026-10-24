//! Read-only mark views and the WeakMap entry scan.

use super::gc_kernel::ephemeron_action;
use super::gc_waiters::Waiters;
use crate::value::{Kind, Value};

pub(super) struct ValueMarks<'a> {
    pub objects: &'a [u64],
    pub strings: &'a [u64],
    pub symbols: &'a [u64],
    pub bigints: &'a [u64],
}

fn marked(bits: &[u64], index: u32) -> bool {
    match bits.get((index / 64) as usize) {
        Some(word) => *word & (1u64 << (index % 64)) != 0,
        None => false,
    }
}

impl ValueMarks<'_> {
    pub(super) fn contains(&self, value: Value) -> bool {
        match value.kind() {
            Kind::Object => marked(self.objects, value.as_object().unwrap().0),
            Kind::String => marked(self.strings, value.as_string().unwrap().0),
            Kind::Symbol => marked(self.symbols, value.as_symbol().unwrap().0),
            Kind::BigInt => marked(self.bigints, value.as_bigint().unwrap().0),
            _ => true,
        }
    }
}

pub(super) fn scan_entries(
    slots: &[Value],
    marks: &ValueMarks<'_>,
    stack: &mut Vec<Value>,
    waiters: &mut Waiters,
) {
    let mut position = 0;
    while position < slots.len() {
        let key = slots[position];
        let value = slots[position + 1];
        let empty = key.is_empty();
        let value_live = empty || marks.contains(value);
        let key_live = !value_live && marks.contains(key);
        match ephemeron_action(empty, value_live, key_live) {
            0 => {}
            1 => stack.push(value),
            _ => waiters.defer(key, value),
        }
        position += 2;
    }
}

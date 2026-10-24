//! Read-only mark views and weak-entry processing.

use std::collections::VecDeque;

use super::gc_kernel::ephemeron_action;
use super::gc_waiters::Waiters;
use crate::value::{Kind, Value};
use crate::vm::jobs::Job;

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

/// Queues callbacks in cell order and clears unmarked targets and tokens.
/// The mark snapshot is fixed; the callback and held values are strong roots.
pub(super) fn finalize_entries(
    slots: &mut [Value],
    marks: &ValueMarks<'_>,
    jobs: &mut VecDeque<Job>,
) {
    let callback = slots[0];
    let mut position = 1;
    while position < slots.len() {
        let target = slots[position];
        let held = slots[position + 1];
        let token = slots[position + 2];
        if !target.is_empty() {
            if !marks.contains(target) {
                jobs.push_back(Job::Call {
                    function: callback,
                    this: Value::UNDEFINED,
                    args: vec![held],
                });
                slots[position] = Value::EMPTY;
                slots[position + 1] = Value::EMPTY;
                slots[position + 2] = Value::EMPTY;
            } else if !token.is_empty() && !marks.contains(token) {
                slots[position + 2] = Value::EMPTY;
            }
        }
        position += 3;
    }
}

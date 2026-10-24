//! Deferred ephemeron values, indexed by the exact boxed key bits.
//!
//! Primitive keys and the standard hasher have specifications in the pinned
//! Verus standard library. The source proofs need no custom key/hasher axioms.

use std::collections::HashMap;

use crate::value::Value;

struct Waiting {
    #[cfg(any(test, feature = "gc-stress"))]
    key: Value,
    values: Vec<Value>,
}

pub(super) struct Waiters {
    by_key: HashMap<u64, Waiting>,
}

impl Waiters {
    pub(super) fn new() -> Self {
        Self {
            by_key: HashMap::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }

    pub(super) fn defer(&mut self, key: Value, value: Value) {
        let waiting = self.by_key.entry(key.bits()).or_insert(Waiting {
            #[cfg(any(test, feature = "gc-stress"))]
            key,
            values: Vec::new(),
        });
        waiting.values.push(value);
    }

    pub(super) fn activate(&mut self, key: Value) -> Option<Vec<Value>> {
        match self.by_key.remove(&key.bits()) {
            Some(waiting) => Some(waiting.values),
            None => None,
        }
    }

    pub(super) fn clear(&mut self) {
        self.by_key.clear();
    }

    #[cfg(any(test, feature = "gc-stress"))]
    pub(super) fn keys(&self) -> impl Iterator<Item = Value> + '_ {
        self.by_key.values().map(|waiting| waiting.key)
    }
}

//! Snapshot-local accounting for the optional immutable QuickOp projection.
//!
//! Inline Option/enum fields already belong to arena_slots or the existing
//! executable projection category. Only separately allocated storage is added.

use super::{MemoryCategory, add_storage, logical, storage};
use crate::engine::code::function::metadata::ParameterEnvironmentLayout;
use crate::engine::code::quick::{QUICK_TAG_COUNT, QUICK_TAG_MEMORY_NAMES, QuickStorage};
use std::collections::HashSet;

const WORD_BYTES: usize = size_of::<u64>();
// This is an estimate based on std's Rc<Vec<T>> representation, not a stable
// allocator contract: one Vec header and the strong/weak reference counters.
// Rc padding and allocator bookkeeping are not independently observable here.
const CONTROL_BYTES_ESTIMATE: usize = size_of::<Vec<u64>>() + 2 * size_of::<usize>();

pub(super) struct QuickMemory {
    seen: HashSet<usize>,
    parameter_headers_seen: HashSet<usize>,
    parameter_headers: MemoryCategory,
    words: MemoryCategory,
    controls: MemoryCategory,
    canonical_only: MemoryCategory,
    tags: [MemoryCategory; QUICK_TAG_COUNT],
}

impl QuickMemory {
    pub(super) fn new() -> Self {
        let mut parameter_headers = storage(
            "bytecode_parameter_environment_headers",
            0,
            0,
            size_of::<ParameterEnvironmentLayout>(),
        );
        parameter_headers.basis = "deduplicated-experimental-Box-ParameterEnvironmentLayout-payload; inline Box pointer counted in arena; excludes nested array backing and allocator overhead";
        let mut words = storage("bytecode_quick_words", 0, 0, WORD_BYTES);
        words.basis = "deduplicated-QuickOp-Vec-length/capacity-times-8; excludes inline fields, Rc controls and allocator overhead";
        let mut controls = storage("bytecode_quick_controls", 0, 0, CONTROL_BYTES_ESTIMATE);
        controls.basis = "deduplicated-Rc-Vec-control-estimate: std Vec header plus two usize reference counts; excludes allocator overhead and unobserved padding";
        let mut canonical_only = logical("bytecode_quick_canonical_only", 0);
        canonical_only.basis = "canonical-only-published-function-count; no word buffer or Rc control allocation; inline fields counted elsewhere";
        let tags = QUICK_TAG_MEMORY_NAMES.map(|name| {
            let mut category = logical(name, 0);
            category.basis = "logical-PC-count: deduplicated-word-buffers plus canonical-only functions; not additional bytes";
            category
        });
        Self {
            seen: HashSet::new(),
            parameter_headers_seen: HashSet::new(),
            parameter_headers,
            words,
            controls,
            canonical_only,
            tags,
        }
    }

    pub(super) fn observe_parameter_environment(
        &mut self,
        layout: Option<&ParameterEnvironmentLayout>,
    ) {
        let Some(layout) = layout else {
            return;
        };
        if self
            .parameter_headers_seen
            .insert(layout as *const _ as usize)
        {
            add_storage(
                &mut self.parameter_headers,
                1,
                1,
                size_of::<ParameterEnvironmentLayout>(),
            );
        }
    }

    pub(super) fn observe(&mut self, projection: QuickStorage, canonical_len: usize) {
        let Some(identity) = projection.buffer_identity else {
            *self.canonical_only.count.as_mut().unwrap() += 1;
            // CanonicalOnly owns no words, but every canonical PC is Generic.
            // It is counted once per bytecode node visited by the heap scan.
            *self.tags[0].count.as_mut().unwrap() += canonical_len;
            return;
        };
        if !self.seen.insert(identity) {
            return;
        }
        add_storage(
            &mut self.words,
            projection.word_len,
            projection.word_capacity,
            WORD_BYTES,
        );
        add_storage(&mut self.controls, 1, 1, CONTROL_BYTES_ESTIMATE);
        for (category, count) in self.tags.iter_mut().zip(projection.tag_counts) {
            *category.count.as_mut().unwrap() += count;
        }
    }

    pub(super) fn extend_into(self, categories: &mut Vec<MemoryCategory>) {
        categories.push(self.parameter_headers);
        categories.extend([self.words, self.controls, self.canonical_only]);
        categories.extend(self.tags);
    }
}

#[cfg(test)]
mod tests;

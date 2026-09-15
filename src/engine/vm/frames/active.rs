//! Incremental physical-native budgets owned by the activation stack.
use super::{ActiveFrameKind, ActiveFrameRecord, ActiveFrameToken};
use crate::engine::vm::native_stack::{native_stack_family, native_stack_weight};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub(crate) struct ActiveFrames {
    records: Vec<ActiveFrameRecord>,
    // The common synchronous native tail never enters the allocated vector.
    // Observers see the descriptor through the same checked collection API.
    pending_native: Option<ActiveFrameRecord>,
    depth: Rc<Cell<usize>>,
    native_cost: usize,
    families: [usize; 14],
}
impl Clone for ActiveFrames {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
            pending_native: self.pending_native,
            depth: Rc::new(Cell::new(self.len())),
            native_cost: self.native_cost,
            families: self.families,
        }
    }
}
impl ActiveFrames {
    pub(crate) fn with_depth(depth: Rc<Cell<usize>>) -> Self {
        debug_assert_eq!(depth.get(), 0);
        Self {
            depth,
            ..Self::default()
        }
    }
    pub(crate) fn len(&self) -> usize { self.records.len() + usize::from(self.pending_native.is_some()) }
    pub(crate) fn is_empty(&self) -> bool { self.len() == 0 }
    pub(crate) fn get(&self, index: usize) -> Option<&ActiveFrameRecord> {
        if index == self.records.len() { self.pending_native.as_ref() } else { self.records.get(index) }
    }
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut ActiveFrameRecord> {
        if index == self.records.len() { self.pending_native.as_mut() } else { self.records.get_mut(index) }
    }
    pub(crate) fn last(&self) -> Option<&ActiveFrameRecord> { self.pending_native.as_ref().or_else(|| self.records.last()) }
    pub(crate) fn last_mut(&mut self) -> Option<&mut ActiveFrameRecord> {
        if self.pending_native.is_some() { self.pending_native.as_mut() } else { self.records.last_mut() }
    }
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item=&ActiveFrameRecord> + ExactSizeIterator {
        (0..self.len()).map(|index| self.get(index).unwrap())
    }
    pub(crate) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item=&mut ActiveFrameRecord> {
        self.records.iter_mut().chain(self.pending_native.iter_mut())
    }
    pub(crate) fn to_vec(&self) -> Vec<ActiveFrameRecord> { self.iter().copied().collect() }
    fn materialize_native_tail(&mut self) {
        if self.pending_native.is_some() {
            self.records.reserve(1);
            self.records.push(self.pending_native.take().unwrap());
        }
    }
    pub(crate) fn push_lazy_native(&mut self, record: ActiveFrameRecord) {
        self.materialize_native_tail();
        self.pending_native = Some(record);
        self.depth.set(self.len());
        self.charge(record, true);
    }
    pub(crate) fn native_cost(&self) -> usize {
        self.native_cost
    }
    pub(crate) fn native_family_depth(&self, family: usize) -> usize {
        self.families[family]
    }
    fn charge(&mut self, record: ActiveFrameRecord, add: bool) {
        if record.native_continuation {
            return;
        }
        let ActiveFrameKind::Native { target, .. } = record.kind else {
            return;
        };
        let weight = native_stack_weight(target);
        if add {
            self.native_cost += weight;
        } else {
            self.native_cost -= weight;
        }
        if let Some((family, _)) = native_stack_family(target) {
            if add {
                self.families[family] += 1;
            } else {
                self.families[family] -= 1;
            }
        }
    }
    pub(crate) fn push(&mut self, record: ActiveFrameRecord) {
        // Allocation precedes accounting so unwinding cannot leave a charge.
        self.materialize_native_tail();
        self.records.push(record);
        self.depth.set(self.len());
        self.charge(record, true);
    }
    pub(crate) fn pop(&mut self) -> Option<ActiveFrameRecord> {
        let record = self.pending_native.take().or_else(|| self.records.pop())?;
        self.depth.set(self.len());
        self.charge(record, false);
        Some(record)
    }
    pub(crate) fn truncate(&mut self, depth: usize) {
        while self.len() > depth {
            self.pop();
        }
    }
    pub(super) fn mark_native_continuation(&mut self, depth: usize, token: ActiveFrameToken) {
        let record = *self.get(depth).expect("native frame index");
        assert_eq!(record.token, token);
        self.charge(record, false);
        self.get_mut(depth).expect("native frame index").native_continuation = true;
    }
}
impl ActiveFrames {
    pub(crate) fn retire(&mut self, token: ActiveFrameToken, depth: usize) {
        let position = if self.get(depth).is_some_and(|frame| frame.token == token) {
            depth
        } else {
            self.iter().position(|frame| frame.token == token).unwrap_or(depth)
        };
        self.truncate(position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::{Runtime, Value},
        builtins::native::{NativeFunctionId as N, RegExpNativeKind as R},
        vm::frames::ActiveFrameFlags,
    };
    #[test]
    fn native_budget_matches_scan_across_mark_pop_and_exception_suffixes() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context.eval("function f(){}; f").unwrap() else {
            panic!()
        };
        let shared_depth = Rc::new(Cell::new(0));
        let mut frames = ActiveFrames::with_depth(shared_depth.clone());
        let verify = |frames: &ActiveFrames| {
            assert_eq!(shared_depth.get(), frames.len());
            let mut cost = 0;
            let mut families = [0; 14];
            for frame in frames.iter().filter(|f| !f.native_continuation) {
                if let ActiveFrameKind::Native { target, .. } = frame.kind {
                    cost += native_stack_weight(target);
                    if let Some((family, _)) = native_stack_family(target) {
                        families[family] += 1;
                    }
                }
            }
            assert_eq!(frames.native_cost, cost);
            assert_eq!(frames.families, families);
        };
        for (i, target) in [
            N::FunctionPrototypeCall,
            N::ObjectAssign,
            N::ObjectFromEntries,
            N::RegExp(R::Search),
            N::ArrayPrototypeSort,
            N::ObjectHasOwn,
        ]
        .into_iter()
        .enumerate()
        {
            frames.push_lazy_native(ActiveFrameRecord {
                token: ActiveFrameToken(i as u64 + 1),
                function: function.object_id(),
                realm: context.realm,
                flags: ActiveFrameFlags::default(),
                kind: ActiveFrameKind::Native {
                    target,
                    actual_arg_count: 0,
                    readable_arg_count: 0,
                },
                native_continuation: false,
            });
            verify(&frames);
            assert_eq!(frames.records.len(), i);
            assert!(frames.pending_native.is_some());
            assert_eq!(frames.to_vec().len(), i + 1);
        }
        let mut independent = frames.clone();
        independent.pop();
        assert_eq!(shared_depth.get(), frames.len());
        assert_eq!(independent.depth.get(), independent.len());
        frames.mark_native_continuation(2, ActiveFrameToken(3));
        verify(&frames);
        frames.pop();
        verify(&frames);
        frames.retire(ActiveFrameToken(3), 2);
        verify(&frames);
        frames.retire(ActiveFrameToken(1), 0);
        verify(&frames);
        assert!(frames.is_empty());
    }
}

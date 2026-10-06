//! Weak per-prototype index of empty shared ordinary shapes, used by `{}` and
//! ordinary `new` receivers. Entries own nothing: a hit re-authenticates the
//! shape's generation and proves it is still empty, shared-layout and names
//! the same prototype, so in-place mutation or collection only causes a miss.
use super::RuntimeState;
use crate::engine::api::RuntimeError;
use crate::engine::heap::{ObjectId, ShapeId};
use std::cell::Cell;

const WAYS: usize = 4;

#[derive(Default)]
pub(crate) struct EmptyShapes {
    entries: [Option<(ObjectId, ShapeId)>; WAYS],
}

impl RuntimeState {
    /// Allocate an extensible ordinary object with no own properties. A cached
    /// empty shape gains the object's reference; a miss moves the reference
    /// returned by shape creation into the object instead of releasing it.
    pub(crate) fn new_empty_ordinary_object(
        &mut self,
        poisoned: &Cell<bool>,
        prototype: ObjectId,
    ) -> Result<ObjectId, RuntimeError> {
        for way in 0..WAYS {
            let Some((owner, shape)) = self.empty_shapes.entries[way] else {
                break;
            };
            if owner != prototype {
                continue;
            }
            if self.heap.shape(shape).is_ok_and(|shape| {
                shape.entries().is_empty()
                    && !shape.is_dictionary()
                    && shape.prototype() == Some(prototype)
            }) {
                self.empty_shapes.entries[..=way].rotate_right(1);
                let object = self.heap.allocate_empty_ordinary_object(shape, false)?;
                return self.observe_pending_cleanup(poisoned, object);
            }
            break;
        }
        let shape = self.get_or_create_shape(Some(prototype), &[])?;
        match self.heap.allocate_empty_ordinary_object(shape, true) {
            Ok(object) => {
                let entries = &mut self.empty_shapes.entries;
                let stale = entries
                    .iter()
                    .position(|entry| entry.is_some_and(|(owner, _)| owner == prototype))
                    .unwrap_or(WAYS - 1);
                entries[stale] = Some((prototype, shape));
                entries[..=stale].rotate_right(1);
                self.observe_pending_cleanup(poisoned, object)
            }
            Err(error) => {
                let cleanup = self
                    .heap
                    .release_shape(shape)
                    .inspect_err(|_| poisoned.set(true))?;
                self.apply_cleanup(cleanup)
                    .inspect_err(|_| poisoned.set(true))?;
                Err(error.into())
            }
        }
    }

    /// Allocation creates no temporary owners, so it releases nothing. It is
    /// still the operation boundary that observes cleanup already pending in
    /// the zero queue: the published object stays owned by the caller's
    /// result, and a failed drain quarantines before any further cleanup.
    #[inline]
    fn observe_pending_cleanup(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
    ) -> Result<ObjectId, RuntimeError> {
        if !self.heap.has_pending_zero_cleanup() {
            return Ok(object);
        }
        let cleanup = self
            .heap
            .drain_zero_queue()
            .inspect_err(|_| poisoned.set(true))?;
        self.apply_cleanup(cleanup)
            .inspect_err(|_| poisoned.set(true))?;
        Ok(object)
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::api::runtime::Runtime;
    use crate::engine::value::Value;

    #[test]
    fn empty_receivers_follow_prototypes_beyond_the_cached_ways() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            function F() {}
            const protos = [];
            for (let i = 0; i < 9; i++) protos.push({tag: i});
            for (let round = 0; round < 3; round++) {
                for (let i = 0; i < protos.length; i++) {
                    F.prototype = protos[i];
                    const o = new F();
                    if (Object.getPrototypeOf(o) !== protos[i] || o.tag !== i) return 1;
                    if (Object.keys(o).length !== 0) return 2;
                    o.own = round;
                    if (!Object.hasOwn(o, 'own') || Object.hasOwn(protos[i], 'own')) return 3;
                }
            }
            const a = {}, b = {};
            a.x = 1;
            if (Object.hasOwn(b, 'x') || Object.keys(b).length !== 0) return 4;
            Object.preventExtensions(b);
            const c = {};
            c.y = 2;
            return Object.isExtensible(c) && c.y === 2 ? 0 : 5;
        })()"#
                )
                .unwrap(),
            Value::Int(0)
        );
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn collected_or_mutated_empty_shapes_are_not_reused() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        drop(
            context
                .eval("globalThis.P = {}; globalThis.F = function () {}; F.prototype = P;")
                .unwrap(),
        );
        // Learn P's empty shape, then let every object using it die.
        drop(context.eval("new F(); new F();").unwrap());
        runtime.run_gc().unwrap();
        let shape_of = |runtime: &Runtime, value: &crate::engine::value::JsValue| {
            let crate::engine::value::JsValue::Object(id) = value else {
                panic!("object")
            };
            let state = runtime.0.state.borrow();
            let shape = state.heap.object(*id).unwrap().shape;
            let layout = state.heap.shape(shape).unwrap();
            (shape, layout.entries().len(), layout.is_dictionary())
        };
        let first = runtime
            .into_jsvalue(context.eval("new F()").unwrap())
            .unwrap();
        let (shape, entries, dictionary) = shape_of(&runtime, &first);
        assert_eq!((entries, dictionary), (0, false));
        // A dictionary conversion of a uniquely held empty shape changes it
        // in place; the next receiver must not adopt it.
        drop(
            context
                .eval("globalThis.d = new F(); d.a = 1; delete d.a; Object.keys(d).length")
                .unwrap(),
        );
        let second = runtime
            .into_jsvalue(context.eval("new F()").unwrap())
            .unwrap();
        let (second_shape, entries, dictionary) = shape_of(&runtime, &second);
        assert_eq!((entries, dictionary), (0, false));
        assert_eq!(second_shape, shape);
        runtime.release_jsvalue(first).unwrap();
        runtime.release_jsvalue(second).unwrap();
        drop(context.eval("d = null").unwrap());
        runtime.run_gc().unwrap();
        assert_eq!(
            context.eval("Object.keys(new F()).length").unwrap(),
            Value::Int(0)
        );
        assert!(!runtime.0.poisoned.get());
    }
}

//! The edge check of [`Heap::verify`]: after a collection, no live record,
//! side table or root names a freed record.
//!
//! It reads every record's fields directly instead of reusing the marking
//! code, so an edge the marker misses is reported here rather than missed
//! twice. Weak edges obey the same rule: a collection removes a dead key
//! from a weak collection, sets a dead `WeakRef` target to `undefined` and
//! empties a dead registry cell, so every edge is empty, a primitive or a
//! live record.

use std::fmt;

use super::Heap;
use super::arena::{Arena, Record};
use super::object::{InternalData, InternalTag, NONE, ObjectKind, SavedFrame};
use crate::value::{Kind, ObjectId, Value};

/// A heap value as the panic messages name it: its kind and index.
struct Named(Value);

impl fmt::Display for Named {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        match value.kind() {
            Kind::Object => write!(f, "object {}", value.as_object().unwrap().0),
            Kind::String => write!(f, "string {}", value.as_string().unwrap().0),
            Kind::Symbol => write!(f, "symbol {}", value.as_symbol().unwrap().0),
            Kind::BigInt => write!(f, "bigint {}", value.as_bigint().unwrap().0),
            kind => write!(f, "{kind:?}"),
        }
    }
}

/// Panics unless `value` is empty, a primitive or a live record.
#[track_caller]
fn value(heap: &Heap, value: Value, edge: fmt::Arguments<'_>) {
    if !heap.is_live(value) {
        panic!("gc verify: {} ({edge}) is freed", Named(value));
    }
}

/// Panics unless `index` is live in `arena`.
#[track_caller]
fn record<T: Record>(arena: &Arena<T>, name: &str, index: u32, edge: fmt::Arguments<'_>) {
    if !arena.is_live(index) {
        panic!("gc verify: {name} {index} ({edge}) is freed");
    }
}

impl Heap {
    /// Checks every edge of every live record, side table and root.
    pub(super) fn verify_edges(&self) {
        self.verify_objects();
        self.verify_payloads();
        self.verify_code();
        self.verify_shapes();
        self.verify_strings();
        self.verify_tables();
        self.verify_roots();
    }

    fn verify_objects(&self) {
        for index in self.objects.live_indices() {
            let header = self.objects.get(index);
            record(
                &self.shapes,
                "shape",
                header.shape.0,
                format_args!("shape of object {index}"),
            );
            for (slot, &v) in self.values.slice(header.slots()).iter().enumerate() {
                value(self, v, format_args!("slot {slot} of object {index}"));
            }
            for (element, &v) in self.values.slice(header.elements()).iter().enumerate() {
                value(self, v, format_args!("element {element} of object {index}"));
            }
            match header.kind {
                ObjectKind::Scope | ObjectKind::Arguments if header.payload != NONE => {
                    value(
                        self,
                        Value::object(ObjectId(header.payload)),
                        format_args!("scope payload of object {index}"),
                    );
                }
                kind if kind.has_internal() => record(
                    &self.payloads.internals,
                    "internal",
                    header.payload,
                    format_args!("payload of object {index}"),
                ),
                ObjectKind::Function => record(
                    &self.payloads.functions,
                    "function",
                    header.payload,
                    format_args!("payload of object {index}"),
                ),
                ObjectKind::RegExp if header.payload != NONE => assert!(
                    self.regexps.is_live(header.payload),
                    "gc verify: regular-expression program {} (payload of object {index}) \
                     is freed",
                    header.payload
                ),
                _ => {}
            }
        }
    }

    fn verify_payloads(&self) {
        for index in self.payloads.internals.live_indices() {
            let internal = self.payloads.internals.get(index);
            for (slot, &v) in internal.slots.iter().enumerate() {
                value(self, v, format_args!("slot {slot} of internal {index}"));
            }
            match &internal.data {
                InternalData::Exports(keys) => {
                    for key in keys.iter().filter_map(|key| key.heap_value()) {
                        value(self, key, format_args!("export key of internal {index}"));
                    }
                }
                InternalData::Module(data) => {
                    for module in data.links() {
                        self.verify_module(module, format_args!("link of module internal {index}"));
                    }
                }
                InternalData::Frame(frame)
                    if matches!(
                        frame.state,
                        crate::vm::suspend::YIELD | crate::vm::suspend::AWAIT
                    ) =>
                {
                    self.verify_saved_frame(index, internal.slots[0], frame);
                }
                _ => {}
            }
        }
        for index in self.payloads.functions.live_indices() {
            let function = self.payloads.functions.get(index);
            for (link, what) in [
                (function.scope, "scope"),
                (function.home_object, "home object"),
                (function.fields, "field initializer"),
            ] {
                if link != NONE {
                    value(
                        self,
                        Value::object(ObjectId(link)),
                        format_args!("{what} of function {index}"),
                    );
                }
            }
            if function.bytecode != NONE {
                record(
                    &self.payloads.bytecode,
                    "bytecode",
                    function.bytecode,
                    format_args!("bytecode of function {index}"),
                );
            }
        }
        for index in self.payloads.bytecode.live_indices() {
            let bytecode = self.payloads.bytecode.get(index);
            for (constant, &v) in self.values.slice(bytecode.constants).iter().enumerate() {
                value(
                    self,
                    v,
                    format_args!("constant {constant} of bytecode {index}"),
                );
            }
            let children = &self.values.slice(bytecode.children)[..bytecode.child_count as usize];
            for (position, child) in children.iter().enumerate() {
                let child = child.as_int32().unwrap_or_else(|| {
                    panic!("gc verify: child {position} of bytecode {index} is not a record index")
                });
                record(
                    &self.payloads.bytecode,
                    "bytecode",
                    child as u32,
                    format_args!("child {position} of bytecode {index}"),
                );
            }
        }
    }

    /// A module record a module, side table or root names.
    #[track_caller]
    fn verify_module(&self, module: ObjectId, edge: fmt::Arguments<'_>) {
        value(self, Value::object(module), edge);
        let header = self.objects.get(module.0);
        assert!(
            header.kind.has_internal()
                && self.payloads.internals.get(header.payload).tag == InternalTag::Module,
            "gc verify: object {} ({edge}) is not a module record",
            module.0
        );
    }

    /// A suspended activation resumes the bytecode of its function, in that
    /// bytecode's unit.
    fn verify_saved_frame(&self, internal: u32, function: Value, frame: &SavedFrame) {
        let Some(function) = function.as_object() else {
            panic!("gc verify: suspended frame of internal {internal} has no function");
        };
        self.verify_activation(
            function,
            frame.bytecode,
            frame.unit,
            format_args!("suspended frame of internal {internal}"),
        );
    }

    /// An activation of `function` runs `bytecode`, in `unit`: the
    /// function's own.
    #[track_caller]
    fn verify_activation(
        &self,
        function: ObjectId,
        bytecode: u32,
        unit: u32,
        frame: fmt::Arguments<'_>,
    ) {
        value(
            self,
            Value::object(function),
            format_args!("function of {frame}"),
        );
        let header = self.objects.get(function.0);
        assert!(
            header.kind == ObjectKind::Function,
            "gc verify: object {} (function of {frame}) is a {:?}",
            function.0,
            header.kind
        );
        let own = self.payloads.functions.get(header.payload).bytecode;
        assert!(
            own == bytecode,
            "gc verify: {frame} runs bytecode {bytecode}, its function {} has {own}",
            function.0
        );
        record(
            &self.payloads.bytecode,
            "bytecode",
            bytecode,
            format_args!("bytecode of {frame}"),
        );
        let own = self.payloads.bytecode.get(bytecode).unit;
        assert!(
            own == unit,
            "gc verify: {frame} runs unit {unit:#x}, its bytecode {bytecode} has {own:#x}"
        );
    }

    fn verify_code(&self) {
        for (id, unit) in self.code.units() {
            for &atom in unit.atoms.iter() {
                value(
                    self,
                    Value::string(atom),
                    format_args!("atom of dynamic unit {:#x}", id.index()),
                );
            }
        }
        for &index in self.code.pinned() {
            let id = crate::bytecode::CodeUnitId(index | crate::bytecode::DYNAMIC);
            assert!(
                self.code.lookup(id).is_some(),
                "gc verify: pinned dynamic unit {index} is freed"
            );
        }
        for (index, program) in self.regexps.records() {
            value(
                self,
                Value::string(program.source),
                format_args!("source of regular-expression program {index}"),
            );
            value(
                self,
                Value::string(program.flags),
                format_args!("flags of regular-expression program {index}"),
            );
        }
        for (key, index) in self.regexps.interned() {
            assert!(
                self.regexps.is_live(index)
                    && self.regexps.get(index).source == key.source
                    && self.regexps.get(index).flags == key.flags,
                "gc verify: the regular-expression index names program {index} for \
                 ({}, {}), which is freed or compiled from another pattern",
                key.source.0,
                key.flags.0
            );
        }
    }

    fn verify_shapes(&self) {
        for index in self.shapes.live_indices() {
            let shape = self.shapes.get(index);
            if let Some(parent) = shape.parent {
                record(
                    &self.shapes,
                    "shape",
                    parent.0,
                    format_args!("parent of shape {index}"),
                );
            }
            if shape.count > 0
                && let Some(key) = shape.key.heap_value()
            {
                value(self, key, format_args!("key of shape {index}"));
            }
            if let Some(prototype) = shape.prototype {
                value(
                    self,
                    Value::object(prototype),
                    format_args!("prototype of shape {index}"),
                );
            }
            if shape.validity.0 != NONE {
                record(
                    &self.cells,
                    "cell",
                    shape.validity.0,
                    format_args!("validity cell of shape {index}"),
                );
            }
            if let Some(dictionary) = &shape.dictionary {
                for key in dictionary
                    .entries()
                    .filter_map(|entry| entry.key.heap_value())
                {
                    value(self, key, format_args!("dictionary key of shape {index}"));
                }
            }
            for child in shape.transitions.children() {
                record(
                    &self.shapes,
                    "shape",
                    child.0,
                    format_args!("transition of shape {index}"),
                );
            }
        }
    }

    fn verify_strings(&self) {
        for index in self.strings.live_indices() {
            let flat = self.strings.get(index).flat().is_some();
            match self.ropes.get(&index) {
                Some(_) if flat => panic!("gc verify: flattened rope {index} keeps its halves"),
                None if !flat => panic!("gc verify: rope {index} has no halves"),
                Some(&(left, right)) => {
                    value(
                        self,
                        Value::string(left),
                        format_args!("left half of rope {index}"),
                    );
                    value(
                        self,
                        Value::string(right),
                        format_args!("right half of rope {index}"),
                    );
                }
                None => {}
            }
        }
        for &index in self.ropes.keys() {
            record(
                &self.strings,
                "string",
                index,
                format_args!("a rope with halves"),
            );
        }
        for index in self.symbols.live_indices() {
            let description = self.symbols.get(index).description;
            if description != NONE {
                record(
                    &self.strings,
                    "string",
                    description,
                    format_args!("description of symbol {index}"),
                );
            }
        }
    }

    /// The weak side tables, which a collection prunes.
    fn verify_tables(&self) {
        for &index in &self.holey {
            value(
                self,
                Value::object(ObjectId(index)),
                format_args!("in the holey set"),
            );
        }
        for (object, info) in &self.prototypes {
            value(
                self,
                Value::object(*object),
                format_args!("a prototype with an info"),
            );
            for &dependent in &info.dependents {
                value(
                    self,
                    Value::object(dependent),
                    format_args!("dependent of prototype {}", object.0),
                );
            }
            if let Some(cell) = info.cell {
                record(
                    &self.cells,
                    "cell",
                    cell.0,
                    format_args!("validity cell of prototype {}", object.0),
                );
            }
        }
        for (&(prototype, kind), shape) in &self.shape_roots {
            if prototype != NONE {
                value(
                    self,
                    Value::object(ObjectId(prototype)),
                    format_args!("prototype of the {kind:?} shape root"),
                );
            }
            record(
                &self.shapes,
                "shape",
                shape.0,
                format_args!("{kind:?} shape root of prototype {prototype}"),
            );
        }
        if let Some((_, shape)) = self.last_root {
            record(
                &self.shapes,
                "shape",
                shape.0,
                format_args!("the last shape root"),
            );
        }
        if let Some((error, _)) = &self.vm.unpositioned {
            value(
                self,
                Value::object(*error),
                format_args!("the unpositioned error"),
            );
        }
        for id in self.atoms.ids() {
            record(
                &self.strings,
                "string",
                id.0,
                format_args!("in the atom table"),
            );
            assert!(
                self.strings.get(id.0).is_atom,
                "gc verify: string {} is in the atom table but is no atom",
                id.0
            );
        }
    }

    /// The roots: what the marker starts from.
    fn verify_roots(&self) {
        for (position, &v) in self.roots.stack.values().iter().enumerate() {
            value(self, v, format_args!("root {position}"));
        }
        for v in self.roots.handles.values() {
            value(self, v, format_args!("a handle"));
        }
        for (slot, &v) in self.roots.fixed.iter().enumerate() {
            value(self, v, format_args!("fixed root {slot}"));
        }
        let mut job = Vec::new();
        for (position, queued) in self.jobs.iter().enumerate() {
            job.clear();
            queued.trace(&mut job);
            for &v in &job {
                value(self, v, format_args!("job {position}"));
            }
        }
        for (list, values) in [("kept", &self.kept), ("rejection", &self.rejections)] {
            for (position, &v) in values.iter().enumerate() {
                value(self, v, format_args!("{list} {position}"));
            }
        }
        for (position, &module) in self.modules.iter().enumerate() {
            let Some(module) = module.as_object() else {
                panic!("gc verify: module {position} is not an object");
            };
            self.verify_module(module, format_args!("module {position}"));
        }
        let top = self.vm.top as usize;
        for (register, &v) in self.vm.registers[..top].iter().enumerate() {
            value(self, v, format_args!("register {register}"));
        }
        for (depth, frame) in self.vm.frames.iter().enumerate() {
            self.verify_activation(
                frame.function,
                frame.bytecode.index(),
                frame.unit.index(),
                format_args!("frame {depth}"),
            );
        }
        for (mode, atoms) in [("pinned", &self.pinned_atoms), ("held", &self.held_atoms)] {
            for &id in atoms {
                record(&self.strings, "string", id.0, format_args!("{mode} atom"));
                let atom = self.strings.get(id.0);
                assert!(
                    atom.is_atom && atom.flat().is_some(),
                    "gc verify: {mode} string {} is not a flat atom",
                    id.0
                );
            }
        }
        for shape in self
            .closure_shapes
            .iter()
            .chain(&self.pending_closure_shapes)
            .chain(&self.arguments_shapes)
            .flatten()
        {
            record(
                &self.shapes,
                "shape",
                shape.0,
                format_args!("a cached closure or arguments shape"),
            );
        }
    }
}

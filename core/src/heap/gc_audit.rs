//! Independent typed heap graph, enabled only by tests and gc-stress.
//!
//! Enumerates records, not the collector's traversal. In particular it never
//! calls mark_from_roots, drain, mark_shapes, mark_bytecode, or Job::trace.
//! This is executable evidence for the representation adapter, not a deductive
//! proof that every mutator satisfies the rooting rule.

use std::collections::{BTreeMap, BTreeSet};

use super::super::Heap;
use super::super::object::{InternalData, InternalTag, NONE, ObjectKind};
use crate::bytecode::{CodeUnitId, DYNAMIC};
use crate::shape::STATIC_CELL;
use crate::value::{Kind, Value};
use crate::vm::ic::Record;
use crate::vm::jobs::Job;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Object(u32),
    String(u32),
    Symbol(u32),
    BigInt(u32),
    Shape(u32),
    Cell(u32),
    Function(u32),
    Bytecode(u32),
    Internal(u32),
    Code(u32),
    RegExp(u32),
}

fn node(value: Value) -> Option<Node> {
    match value.kind() {
        Kind::Object => Some(Node::Object(value.as_object().unwrap().index())),
        Kind::String => Some(Node::String(value.as_string().unwrap().index())),
        Kind::Symbol => Some(Node::Symbol(value.as_symbol().unwrap().index())),
        Kind::BigInt => Some(Node::BigInt(value.as_bigint().unwrap().index())),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct Snapshot {
    edges: BTreeMap<Node, Vec<Node>>,
    roots: BTreeSet<Node>,
    ephemerons: Vec<(Node, Node, Node)>,
    reachable: BTreeSet<Node>,
}

impl Snapshot {
    fn edge(&mut self, from: Node, to: Node) {
        self.edges.entry(from).or_default().push(to);
    }

    fn values(&mut self, from: Node, values: &[Value]) {
        for to in values.iter().copied().filter_map(node) {
            self.edge(from, to);
        }
    }

    fn root_values(&mut self, values: impl IntoIterator<Item = Value>) {
        self.roots.extend(values.into_iter().filter_map(node));
    }

    pub(super) fn capture(h: &Heap) -> Self {
        let mut s = Self::default();
        macro_rules! records {
            ($arena:expr, $kind:ident) => {
                for i in $arena.live_indices() {
                    s.edges.insert(Node::$kind(i), Vec::new());
                }
            };
        }
        records!(h.objects, Object);
        records!(h.strings, String);
        records!(h.symbols, Symbol);
        records!(h.bigints, BigInt);
        records!(h.shapes, Shape);
        records!(h.cells, Cell);
        records!(h.payloads.functions, Function);
        records!(h.payloads.bytecode, Bytecode);
        records!(h.payloads.internals, Internal);
        for (id, unit) in h.code.units() {
            let from = Node::Code(id.0 & !DYNAMIC);
            s.edges.insert(from, Vec::new());
            for atom in &unit.atoms {
                s.edge(from, Node::String(atom.index()));
            }
        }
        for (i, record, _) in h.regexps.audit_records() {
            let from = Node::RegExp(i);
            s.edges.insert(
                from,
                vec![
                    Node::String(record.source.index()),
                    Node::String(record.flags.index()),
                ],
            );
        }
        for i in h.objects.live_indices() {
            let from = Node::Object(i);
            let header = h.objects.get(i);
            s.edge(from, Node::Shape(header.shape.index()));
            s.values(from, h.values.slice(header.slots()));
            s.values(from, h.values.slice(header.elements()));
            match header.kind {
                ObjectKind::Function => s.edge(from, Node::Function(header.payload)),
                ObjectKind::Scope | ObjectKind::Arguments => {
                    if header.payload != NONE {
                        s.edge(from, Node::Object(header.payload));
                    }
                }
                ObjectKind::RegExp => {
                    if header.payload != NONE {
                        s.edge(from, Node::RegExp(header.payload));
                    }
                }
                ObjectKind::StringWrapper
                | ObjectKind::NumberWrapper
                | ObjectKind::BooleanWrapper
                | ObjectKind::SymbolWrapper
                | ObjectKind::BigIntWrapper
                | ObjectKind::Date
                | ObjectKind::BoundFunction
                | ObjectKind::Proxy
                | ObjectKind::TypedArray
                | ObjectKind::ModuleNamespace
                | ObjectKind::Builtin => {
                    s.edge(from, Node::Internal(header.payload));
                }
                ObjectKind::Ordinary
                | ObjectKind::Array
                | ObjectKind::GlobalObject
                | ObjectKind::ForInIterator
                | ObjectKind::Native => {}
                ObjectKind::Free => panic!("gc graph: live Free object {i}"),
            }
        }
        for i in h.payloads.functions.live_indices() {
            let from = Node::Function(i);
            let f = h.payloads.functions.get(i);
            for object in [f.scope, f.home_object, f.fields] {
                if object != NONE {
                    s.edge(from, Node::Object(object));
                }
            }
            if f.bytecode != NONE {
                s.edge(from, Node::Bytecode(f.bytecode));
            }
        }
        for i in h.payloads.bytecode.live_indices() {
            let from = Node::Bytecode(i);
            let code = h.payloads.bytecode.get(i);
            let unit = CodeUnitId(code.unit);
            if unit.is_dynamic() {
                s.edge(from, Node::Code(unit.0 & !DYNAMIC));
            }
            s.values(from, h.values.slice(code.constants));
            for child in &h.values.slice(code.children)[..code.child_count as usize] {
                s.edge(
                    from,
                    Node::Bytecode(child.as_int32().expect("bytecode child") as u32),
                );
            }
        }
        for i in h.payloads.internals.live_indices() {
            let from = Node::Internal(i);
            let internal = h.payloads.internals.get(i);
            match internal.tag {
                InternalTag::WeakMap => {
                    for pair in internal.slots.as_chunks::<2>().0 {
                        if pair[0].is_empty() {
                            continue;
                        }
                        let key = node(pair[0]).expect("weak map key is a heap reference");
                        if let Some(value) = node(pair[1]) {
                            s.ephemerons.push((from, key, value));
                        }
                    }
                }
                InternalTag::WeakSet | InternalTag::WeakRef => {}
                InternalTag::FinalizationRegistry => {
                    s.values(from, &internal.slots[..1]);
                    for cell in internal.slots[1..].as_chunks::<3>().0 {
                        s.values(from, &cell[1..2]);
                    }
                }
                // Exhaustive: adding a new internal kind requires a trace policy.
                InternalTag::None
                | InternalTag::ArrayIterator
                | InternalTag::StringIterator
                | InternalTag::MapIterator
                | InternalTag::SetIterator
                | InternalTag::RegExpStringIterator
                | InternalTag::IteratorHelper
                | InternalTag::WrapForValidIterator
                | InternalTag::AsyncFromSyncIterator
                | InternalTag::Promise
                | InternalTag::PromiseReaction
                | InternalTag::Map
                | InternalTag::Set
                | InternalTag::Generator
                | InternalTag::AsyncGenerator
                | InternalTag::AsyncFunction
                | InternalTag::ArrayBuffer
                | InternalTag::SharedArrayBuffer
                | InternalTag::TypedArray
                | InternalTag::DataView
                | InternalTag::ModuleNamespace
                | InternalTag::NativeData
                | InternalTag::HtmlDda
                | InternalTag::RawJson
                | InternalTag::Module
                | InternalTag::Iterator
                | InternalTag::HostData => s.values(from, &internal.slots),
            }
            if let InternalData::Exports(keys) = &internal.data {
                for key in keys {
                    if let Some(value) = key.heap_value() {
                        s.values(from, &[value]);
                    }
                }
            }
        }
        for i in h.shapes.live_indices() {
            let from = Node::Shape(i);
            let shape = h.shapes.get(i);
            if let Some(parent) = shape.parent {
                s.edge(from, Node::Shape(parent.index()));
            }
            if let Some(proto) = shape.prototype {
                s.edge(from, Node::Object(proto.index()));
            }
            if shape.validity.index() != NONE {
                s.edge(from, Node::Cell(shape.validity.index()));
            }
            if shape.count > 0
                && let Some(key) = shape.key.heap_value()
            {
                s.values(from, &[key]);
            }
            if let Some(dictionary) = &shape.dictionary {
                for entry in dictionary.entries() {
                    if let Some(key) = entry.key.heap_value() {
                        s.values(from, &[key]);
                    }
                }
            }
        }
        for i in h.strings.live_indices() {
            if h.strings.get(i).flat().is_none()
                && let Some(&(left, right)) = h.ropes.get(&i)
            {
                s.edge(Node::String(i), Node::String(left.index()));
                s.edge(Node::String(i), Node::String(right.index()));
            }
        }
        for i in h.symbols.live_indices() {
            let description = h.symbols.get(i).description;
            if description != NONE {
                s.edge(Node::Symbol(i), Node::String(description));
            }
        }
        for (object, info) in &h.prototypes {
            if h.objects.is_live(object.index())
                && let Some(cell) = info.cell
            {
                s.edge(Node::Object(object.index()), Node::Cell(cell.index()));
            }
        }

        s.root_values(h.roots.stack.values().iter().copied());
        s.root_values(h.roots.handles.values());
        s.root_values(h.roots.fixed.iter().copied());
        s.root_values(h.vm.registers[..h.vm.top as usize].iter().copied());
        for frame in &h.vm.frames {
            s.roots.insert(Node::Object(frame.function.index()));
        }
        for atom in h.pinned_atoms.iter().chain(&h.held_atoms) {
            s.roots.insert(Node::String(atom.index()));
        }
        for index in h.code.pinned() {
            s.roots.insert(Node::Code(*index));
        }
        for shape in h
            .closure_shapes
            .iter()
            .chain(&h.pending_closure_shapes)
            .chain(&h.arguments_shapes)
            .flatten()
        {
            s.roots.insert(Node::Shape(shape.index()));
        }
        s.roots.insert(Node::Cell(STATIC_CELL.index()));
        s.root_values(
            h.kept
                .iter()
                .chain(&h.rejections)
                .chain(&h.modules)
                .copied(),
        );
        for job in &h.jobs {
            s.root_values(job_values(job));
        }
        s.solve();
        s
    }

    // Independent saturation: strong BFS and repeated scans of ALL ephemerons.
    // Deliberately no key-indexed waiting table or discovered-map cursor.
    fn solve(&mut self) {
        let mut work: Vec<_> = self.roots.iter().copied().collect();
        loop {
            while let Some(n) = work.pop() {
                let edges = self
                    .edges
                    .get(&n)
                    .unwrap_or_else(|| panic!("gc graph: dangling {n:?}"));
                if self.reachable.insert(n) {
                    work.extend_from_slice(edges);
                }
            }
            for &(map, key, value) in &self.ephemerons {
                if self.reachable.contains(&map)
                    && self.reachable.contains(&key)
                    && !self.reachable.contains(&value)
                {
                    work.push(value);
                }
            }
            if work.is_empty() {
                break;
            }
        }
    }

    pub(super) fn check_marks(&self, h: &Heap) {
        let regex_marks: BTreeMap<_, _> = h
            .regexps
            .audit_records()
            .map(|(i, _, mark)| (i, mark))
            .collect();
        for &n in self.edges.keys() {
            let marked = match n {
                Node::Object(i) => h.objects.is_marked(i),
                Node::String(i) => h.strings.is_marked(i),
                Node::Symbol(i) => h.symbols.is_marked(i),
                Node::BigInt(i) => h.bigints.is_marked(i),
                Node::Shape(i) => h.shapes.is_marked(i),
                Node::Cell(i) => h.cells.is_marked(i),
                Node::Function(i) => h.payloads.functions.is_marked(i),
                Node::Bytecode(i) => h.payloads.bytecode.is_marked(i),
                Node::Internal(i) => h.payloads.internals.is_marked(i),
                Node::Code(i) => h.code.is_marked(i),
                Node::RegExp(i) => regex_marks[&i],
            };
            assert_eq!(
                marked,
                self.reachable.contains(&n),
                "gc graph: wrong mark for {n:?}"
            );
        }
        assert!(
            h.gc.ephemerons.is_empty(),
            "gc graph: waiters survive marking"
        );
        assert!(
            h.gc.mark_stack.is_empty()
                && h.gc.shape_stack.is_empty()
                && h.gc.bytecode_stack.is_empty(),
            "gc graph: work survives marking"
        );
    }

    pub(super) fn check_retained_references(&self, h: &Heap) {
        for job in &h.jobs {
            for n in job_values(job).into_iter().filter_map(node) {
                assert!(
                    self.reachable.contains(&n),
                    "gc graph: job names dead {n:?}"
                );
            }
        }
        for record in h.ics.named_records() {
            let n = match record {
                Record::Shape(i) => Node::Shape(i),
                Record::Object(i) => Node::Object(i),
                Record::Cell(i) => Node::Cell(i),
                Record::Atom(i) => Node::String(i),
            };
            assert!(
                self.reachable.contains(&n),
                "gc graph: cache names dead {n:?}"
            );
        }
    }

    pub(super) fn check_live(&self, h: &Heap) {
        let regex_live: BTreeSet<_> = h.regexps.audit_records().map(|(i, _, _)| i).collect();
        for &n in self.edges.keys() {
            let live = match n {
                Node::Object(i) => h.objects.is_live(i),
                Node::String(i) => h.strings.is_live(i),
                Node::Symbol(i) => h.symbols.is_live(i),
                Node::BigInt(i) => h.bigints.is_live(i),
                Node::Shape(i) => h.shapes.is_live(i),
                Node::Cell(i) => h.cells.is_live(i),
                Node::Function(i) => h.payloads.functions.is_live(i),
                Node::Bytecode(i) => h.payloads.bytecode.is_live(i),
                Node::Internal(i) => h.payloads.internals.is_live(i),
                Node::Code(i) => h.code.lookup(CodeUnitId(i | DYNAMIC)).is_some(),
                Node::RegExp(i) => regex_live.contains(&i),
            };
            assert_eq!(
                live,
                self.reachable.contains(&n),
                "gc graph: wrong survivor {n:?}"
            );
        }
    }
}

fn job_values(job: &Job) -> Vec<Value> {
    match job {
        Job::Reaction {
            record, argument, ..
        } => vec![Value::object(*record), *argument],
        Job::Thenable {
            promise,
            thenable,
            then,
        } => vec![Value::object(*promise), *thenable, *then],
        Job::Call {
            function,
            this,
            args,
        } => {
            let mut values = vec![*function, *this];
            values.extend_from_slice(args);
            values
        }
    }
}

//! Phase 3 M1: the `GetField` cache (own, prototype and string-receiver
//! entries), its invalidation and its pruning at sweep. Every test checks the
//! value read as well as the entry, and passes under `--features gc-stress`.

use oxide_core::bytecode::builder::UnitLayout;
use oxide_core::bytecode::{CodeBuilder, Opcode, flags};
use oxide_core::heap::gc::HeapLimits;
use oxide_core::vm::ic::{IcEntry, LENGTH_TAG, STRING_RECEIVER_TAG};
use oxide_core::{
    Handle, Intrinsic, ObjectId, ObjectKind, PropertyFlags, PropertyKey, Runtime, Value,
};

/// A runtime and a held function `(o) => o[key]` read through one
/// `GetField` site.
struct Reader {
    runtime: Runtime,
    function: Handle,
    site: usize,
}

impl Reader {
    fn new(name: &str) -> Reader {
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        let key = key(&mut runtime, name);
        let mut builder = CodeBuilder::new();
        builder.ab_key(Opcode::GetField, 2, 1, key);
        builder.a(Opcode::Return, 2);
        let unit = builder.finish(UnitLayout {
            register_count: 3,
            parameter_count: 1,
            flags: flags::STRICT,
            ..UnitLayout::default()
        });
        let unit = runtime.publish(unit).unwrap();
        let function = runtime.instantiate(unit, &[&[]]).unwrap();
        let function = runtime.hold(Value::object(function));
        let site = runtime.code().get(unit).ic_base as usize;
        Reader {
            runtime,
            function,
            site,
        }
    }

    fn read(&mut self, receiver: Value) -> Value {
        let function = self.runtime.held(self.function);
        self.runtime
            .call(function, Value::UNDEFINED, &[receiver])
            .unwrap()
    }

    fn entry(&self) -> IcEntry {
        self.runtime.heap().ics().entry(self.site)
    }

    /// The filled entries of the site, monomorphic or in its polymorphic row.
    fn entries(&self) -> Vec<IcEntry> {
        let ics = self.runtime.heap().ics();
        match ics.poly_row(self.site) {
            Some(row) => row.iter().copied().filter(IcEntry::is_filled).collect(),
            None => Some(ics.entry(self.site))
                .filter(IcEntry::is_filled)
                .into_iter()
                .collect(),
        }
    }

    /// The entry filled for `object`'s current shape.
    fn entry_for(&self, object: ObjectId) -> IcEntry {
        let shape = self.shape(object);
        let entries = self.entries();
        *entries
            .iter()
            .find(|entry| entry.w0 == shape)
            .unwrap_or_else(|| panic!("no entry for shape {shape} in {entries:?}"))
    }

    /// A held ordinary object inheriting from `prototype`.
    fn object(&mut self, prototype: Option<ObjectId>) -> ObjectId {
        let heap = self.runtime.heap_mut();
        let object = heap
            .alloc_object(ObjectKind::Ordinary, prototype, 4)
            .unwrap();
        heap.handles().create(Value::object(object));
        object
    }

    fn add(&mut self, object: ObjectId, name: &str, value: i32) {
        let key = key(&mut self.runtime, name);
        self.runtime
            .heap_mut()
            .add_property(object, key, PropertyFlags::DEFAULT, Value::int32(value))
            .unwrap();
    }

    fn shape(&self, object: ObjectId) -> u32 {
        self.runtime.heap().shape_of(object).index()
    }
}

fn key(runtime: &mut Runtime, name: &str) -> PropertyKey {
    runtime.heap_mut().property_key(name).unwrap()
}

#[test]
fn an_own_read_fills_and_then_hits() {
    let mut reader = Reader::new("x");
    let object = reader.object(None);
    reader.add(object, "x", 1);
    assert!(reader.entry().is_empty());
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    let filled = reader.entry();
    assert_eq!(filled.w0, reader.shape(object));
    assert_eq!(filled.w1, u32::MAX, "an own hit has no holder");
    // A hit reads the slot live and leaves the entry alone.
    let x = key(&mut reader.runtime, "x");
    let (slot, _) = reader.runtime.heap().lookup_own(object, x).unwrap();
    reader
        .runtime
        .heap_mut()
        .set_slot(object, slot, Value::int32(5));
    assert_eq!(reader.read(Value::object(object)), Value::int32(5));
    assert_eq!(reader.entry(), filled);
}

#[test]
fn a_shape_change_on_the_receiver_misses_and_refills() {
    let mut reader = Reader::new("x");
    let object = reader.object(None);
    reader.add(object, "x", 1);
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    let before = reader.entry().w0;
    reader.add(object, "y", 2);
    assert_ne!(reader.shape(object), before);
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    assert_eq!(reader.entry_for(object).w1, u32::MAX);
    // An object of another layout with the same key reads its own slot.
    let other = reader.object(None);
    reader.add(other, "z", 3);
    reader.add(other, "x", 4);
    assert_eq!(reader.read(Value::object(other)), Value::int32(4));
}

/// `object -> middle -> top`, with `x` on `top`.
fn chain(reader: &mut Reader) -> (ObjectId, ObjectId, ObjectId) {
    let top = reader.object(None);
    reader.add(top, "x", 1);
    let middle = reader.object(Some(top));
    reader.add(middle, "m", 0);
    let object = reader.object(Some(middle));
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    let entry = reader.entry_for(object);
    assert_eq!(entry.w1, top.index());
    assert!(reader.runtime.heap().cell_is_valid(cell(entry)));
    (object, middle, top)
}

fn cell(entry: IcEntry) -> oxide_core::ValidityCellId {
    entry.cell()
}

#[test]
fn a_prototype_read_hits_through_the_cell() {
    let mut reader = Reader::new("x");
    let (object, _, top) = chain(&mut reader);
    let filled = reader.entry_for(object);
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    assert_eq!(reader.entry_for(object), filled);
    // The holder's slot is read live.
    let x = key(&mut reader.runtime, "x");
    let (slot, _) = reader.runtime.heap().lookup_own(top, x).unwrap();
    reader
        .runtime
        .heap_mut()
        .set_slot(top, slot, Value::int32(7));
    assert_eq!(reader.read(Value::object(object)), Value::int32(7));
}

#[test]
fn adding_to_any_prototype_invalidates() {
    let mut reader = Reader::new("x");
    let (object, middle, _) = chain(&mut reader);
    let old = reader.entry_for(object);
    // A shadowing property on the middle prototype.
    reader.add(middle, "x", 2);
    assert!(!reader.runtime.heap().cell_is_valid(cell(old)));
    assert_eq!(reader.read(Value::object(object)), Value::int32(2));
    assert_eq!(reader.entry_for(object).w1, middle.index());
    // An unrelated property on the top prototype.
    let (object, _, top) = chain(&mut reader);
    reader.add(top, "unrelated", 3);
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    assert!(
        reader
            .runtime
            .heap()
            .cell_is_valid(cell(reader.entry_for(object)))
    );
}

#[test]
fn deleting_from_a_prototype_invalidates() {
    let mut reader = Reader::new("x");
    let (object, _, top) = chain(&mut reader);
    let x = key(&mut reader.runtime, "x");
    reader.runtime.heap_mut().delete_property(top, x).unwrap();
    assert_eq!(reader.read(Value::object(object)), Value::UNDEFINED);
}

#[test]
fn reflagging_a_prototype_property_invalidates() {
    let mut reader = Reader::new("x");
    let (object, middle, _) = chain(&mut reader);
    let old = reader.entry_for(object);
    let m = key(&mut reader.runtime, "m");
    reader
        .runtime
        .heap_mut()
        .set_property_flags(middle, m, PropertyFlags::data(false, false, false))
        .unwrap();
    assert!(!reader.runtime.heap().cell_is_valid(cell(old)));
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    assert!(
        reader
            .runtime
            .heap()
            .cell_is_valid(cell(reader.entry_for(object)))
    );
}

#[test]
fn set_prototype_on_the_chain_invalidates() {
    let mut reader = Reader::new("x");
    let (object, middle, _) = chain(&mut reader);
    let other = reader.object(None);
    reader.add(other, "x", 9);
    reader
        .runtime
        .heap_mut()
        .set_prototype(middle, Some(other))
        .unwrap();
    assert_eq!(reader.read(Value::object(object)), Value::int32(9));
    assert_eq!(reader.entry_for(object).w1, other.index());
    // And on the receiver itself, which changes its shape.
    let (object, _, _) = chain(&mut reader);
    reader
        .runtime
        .heap_mut()
        .set_prototype(object, Some(other))
        .unwrap();
    assert_eq!(reader.read(Value::object(object)), Value::int32(9));
}

#[test]
fn dictionary_receivers_and_misses_stay_uncached() {
    let mut reader = Reader::new("x");
    let object = reader.object(None);
    reader.add(object, "x", 1);
    reader.add(object, "y", 2);
    let y = key(&mut reader.runtime, "y");
    reader
        .runtime
        .heap_mut()
        .delete_property(object, y)
        .unwrap();
    assert!(reader.runtime.heap().is_dictionary(object));
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    assert!(reader.entry().is_empty());
    let empty = reader.object(None);
    assert_eq!(reader.read(Value::object(empty)), Value::UNDEFINED);
    assert!(reader.entry().is_empty());
    // The global object is a dictionary from creation.
    let global = reader.runtime.global_object();
    reader.add(global, "x", 3);
    assert_eq!(reader.read(Value::object(global)), Value::int32(3));
    assert!(reader.entry().is_empty());
}

#[test]
fn array_length_is_not_cached_as_a_slot() {
    let mut reader = Reader::new("length");
    let array = reader.runtime.new_array(4).unwrap();
    reader
        .runtime
        .heap_mut()
        .handles()
        .create(Value::object(array));
    reader
        .runtime
        .heap_mut()
        .push_element(array, Value::int32(1))
        .unwrap();
    assert_eq!(reader.read(Value::object(array)), Value::int32(1));
    reader
        .runtime
        .heap_mut()
        .push_element(array, Value::int32(2))
        .unwrap();
    assert_eq!(reader.read(Value::object(array)), Value::int32(2));
    // An ordinary object's own `length` is a slot like any other.
    let object = reader.object(None);
    reader.add(object, "length", 7);
    assert_eq!(reader.read(Value::object(object)), Value::int32(7));
    assert_eq!(reader.read(Value::object(array)), Value::int32(2));
}

#[test]
fn string_receivers_cache_their_prototype_chain() {
    let mut reader = Reader::new("method");
    let string_prototype = reader.runtime.intrinsic(Intrinsic::StringPrototype);
    let object_prototype = reader.runtime.intrinsic(Intrinsic::ObjectPrototype);
    reader.add(string_prototype, "method", 1);
    let text = reader.runtime.heap_mut().intern("abc").unwrap();
    let string = Value::string(text);
    assert_eq!(reader.read(string), Value::int32(1));
    let entry = reader.entry();
    assert_eq!(entry.w0, STRING_RECEIVER_TAG);
    assert_eq!(entry.w1, string_prototype.index());
    assert_eq!(reader.read(string), Value::int32(1));
    assert_eq!(reader.entry(), entry);
    // A change to `%String.prototype%` clears the cell.
    reader.add(string_prototype, "other", 2);
    assert!(!reader.runtime.heap().cell_is_valid(cell(entry)));
    assert_eq!(reader.read(string), Value::int32(1));
    assert!(reader.runtime.heap().cell_is_valid(cell(reader.entry())));
    // A method found on `%Object.prototype%`, then shadowed.
    let mut reader = Reader::new("shared");
    let string_prototype = reader.runtime.intrinsic(Intrinsic::StringPrototype);
    reader.add(object_prototype_of(&reader), "shared", 3);
    let text = reader.runtime.heap_mut().intern("abc").unwrap();
    assert_eq!(reader.read(Value::string(text)), Value::int32(3));
    assert_eq!(reader.entry().w1, object_prototype.index());
    reader.add(string_prototype, "shared", 4);
    assert_eq!(reader.read(Value::string(text)), Value::int32(4));
    // `length` on a string caches as the length entry (phase 3 M5).
    let mut reader = Reader::new("length");
    let text = reader.runtime.heap_mut().intern("abcd").unwrap();
    assert_eq!(reader.read(Value::string(text)), Value::int32(4));
    assert_eq!(reader.entry().w0, LENGTH_TAG);
    let other = reader.runtime.heap_mut().intern("xy").unwrap();
    assert_eq!(reader.read(Value::string(other)), Value::int32(2));
}

fn object_prototype_of(reader: &Reader) -> ObjectId {
    reader.runtime.intrinsic(Intrinsic::ObjectPrototype)
}

/// The recycled-id hazard of section 11: an entry filled for a shape that
/// dies must not hit for the shape that reuses its index.
#[test]
fn a_recycled_shape_does_not_hit_a_stale_entry() {
    let mut reader = Reader::new("x");
    let heap = reader.runtime.heap_mut();
    let mark = heap.roots().mark();
    let object = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.roots().push(Value::object(object));
    for (index, name) in ["a", "b", "x"].into_iter().enumerate() {
        let key = heap.property_key(name).unwrap();
        heap.add_property(
            object,
            key,
            PropertyFlags::DEFAULT,
            Value::int32(index as i32),
        )
        .unwrap();
    }
    let dead_shape = heap.shape_of(object).index();
    assert_eq!(reader.read(Value::object(object)), Value::int32(2));
    assert_eq!(reader.entry().w0, dead_shape);
    // Drop every object of that shape (and of its parents) and collect: the
    // entry is pruned before the shape's index is freed.
    reader.runtime.heap_mut().roots().truncate(mark);
    reader.runtime.heap_mut().collect();
    assert!(reader.entry().is_empty());
    // Allocate shapes until one reuses the dead index (immediately without
    // stress; the stress collector quarantines freed indices for one
    // collection), then read through it.
    let mut reused = false;
    for round in 0..64 {
        let heap = reader.runtime.heap_mut();
        let object = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
        heap.handles().create(Value::object(object));
        let name = format!("k{round}");
        let key = heap.property_key(&name).unwrap();
        heap.add_property(object, key, PropertyFlags::DEFAULT, Value::int32(-1))
            .unwrap();
        let shape = heap.shape_of(object).index();
        assert_eq!(reader.read(Value::object(object)), Value::UNDEFINED);
        reused |= shape == dead_shape;
        let heap = reader.runtime.heap_mut();
        let key = heap.property_key("x").unwrap();
        heap.add_property(object, key, PropertyFlags::DEFAULT, Value::int32(round))
            .unwrap();
        let shape = heap.shape_of(object).index();
        assert_eq!(reader.read(Value::object(object)), Value::int32(round));
        reused |= shape == dead_shape;
        if reused {
            break;
        }
    }
    if !cfg!(feature = "gc-stress") {
        assert!(reused, "the dead shape's index was not reused");
    }
}

/// Phase 3 M5: an array's `length` is a length entry that reads each
/// array's current length; an ordinary object's `length` stays a slot.
#[test]
fn array_length_reads_through_the_length_entry() {
    let mut reader = Reader::new("length");
    let array = reader.runtime.new_array(4).unwrap();
    reader
        .runtime
        .heap_mut()
        .handles()
        .create(Value::object(array));
    assert_eq!(reader.read(Value::object(array)), Value::int32(0));
    assert_eq!(reader.entry().w0, LENGTH_TAG);
    reader
        .runtime
        .heap_mut()
        .push_element(array, Value::int32(1))
        .unwrap();
    assert_eq!(reader.read(Value::object(array)), Value::int32(1));
    let object = reader.object(None);
    reader.add(object, "length", 9);
    assert_eq!(reader.read(Value::object(object)), Value::int32(9));
    assert_eq!(reader.read(Value::object(array)), Value::int32(1));
}

/// Phase 3 M5: a prototype in dictionary mode (more than 64 properties, or
/// a deletion) is a cacheable holder; its changes clear the cell.
#[test]
fn a_dictionary_prototype_is_a_cacheable_holder() {
    let mut reader = Reader::new("x");
    let top = reader.object(None);
    for index in 0..70 {
        reader.add(top, &format!("p{index}"), index);
    }
    reader.add(top, "x", 1);
    assert!(reader.runtime.heap().is_dictionary(top));
    let object = reader.object(Some(top));
    assert_eq!(reader.read(Value::object(object)), Value::int32(1));
    let entry = reader.entry_for(object);
    assert_eq!(entry.w1, top.index());
    // The slot is read live, and a change to the dictionary clears the cell.
    let x = key(&mut reader.runtime, "x");
    let (slot, _) = reader.runtime.heap().lookup_own(top, x).unwrap();
    reader
        .runtime
        .heap_mut()
        .set_slot(top, slot, Value::int32(2));
    assert_eq!(reader.read(Value::object(object)), Value::int32(2));
    reader.runtime.heap_mut().delete_property(top, x).unwrap();
    assert!(!reader.runtime.heap().cell_is_valid(entry.cell()));
    assert_eq!(reader.read(Value::object(object)), Value::UNDEFINED);
    reader.add(top, "x", 3);
    assert_eq!(reader.read(Value::object(object)), Value::int32(3));
}

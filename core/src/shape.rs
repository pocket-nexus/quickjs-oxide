//! Shapes: the transition tree, dictionary mode and prototype validity cells.
//!
//! A transition shape describes one property added on top of its parent; the
//! tree has one root per `(prototype, kind)`. Lookup walks `parent` for eight
//! properties or fewer and otherwise uses a table built on first lookup.
//! Transition shapes are immutable and shared; reconfiguring a property or
//! changing the prototype replays the object's keys from another root, so the
//! slots keep their positions.
//!
//! Dictionary mode gives an object an unshared shape with its own hash table
//! and insertion-ordered entry list. It is entered on deletion, past
//! [`DICTIONARY_THRESHOLD`] properties, and always for the global object.
//!
//! Every object used as a prototype has a `PrototypeInfo`: a lazily created
//! validity cell and the list of prototype objects that inherit from it. Any
//! change to a prototype object clears its cell and, through the dependents
//! lists, every cell downstream. A shape caches its prototype's cell; the
//! phase 3 prototype IC records `(holder, slot, cell)` and tests one boolean.

use std::cell::OnceCell;
use std::fmt;

use crate::heap::arena::Record;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::hash::FxHashMap;
use crate::heap::object::{NONE, ObjectKind, flags};
use crate::heap::{Heap, HeapError, Need};
use crate::value::{ObjectId, ShapeId, ValidityCellId, Value};

/// Past this many properties a transition shape builds a lookup table.
pub const TABLE_THRESHOLD: u32 = 8;
/// Adding a property beyond this count moves the object to dictionary mode.
pub const DICTIONARY_THRESHOLD: u32 = 64;
/// The always-valid cell of shapes without a prototype.
pub const STATIC_CELL: ValidityCellId = ValidityCellId(0);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyFlags(u8);

impl PropertyFlags {
    pub const WRITABLE: u8 = 1 << 0;
    pub const ENUMERABLE: u8 = 1 << 1;
    pub const CONFIGURABLE: u8 = 1 << 2;
    pub const ACCESSOR: u8 = 1 << 3;
    /// Writable, enumerable, configurable data property.
    pub const DEFAULT: PropertyFlags = PropertyFlags::data(true, true, true);

    pub const fn data(writable: bool, enumerable: bool, configurable: bool) -> Self {
        PropertyFlags(
            (writable as u8 * Self::WRITABLE)
                | (enumerable as u8 * Self::ENUMERABLE)
                | (configurable as u8 * Self::CONFIGURABLE),
        )
    }

    pub const fn accessor(enumerable: bool, configurable: bool) -> Self {
        PropertyFlags(
            Self::ACCESSOR
                | (enumerable as u8 * Self::ENUMERABLE)
                | (configurable as u8 * Self::CONFIGURABLE),
        )
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

impl fmt::Debug for PropertyFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flag = |bit, name| if self.has(bit) { name } else { "-" };
        write!(
            f,
            "{}{}{}{}",
            flag(Self::ACCESSOR, "a"),
            flag(Self::WRITABLE, "w"),
            flag(Self::ENUMERABLE, "e"),
            flag(Self::CONFIGURABLE, "c")
        )
    }
}

/// A shape's outgoing transitions: a vector for up to four entries, a map
/// beyond. Entries are weak: the collector drops those whose child died.
#[derive(Default)]
pub enum Transitions {
    #[default]
    None,
    Small(Vec<(PropertyKey, PropertyFlags, ShapeId)>),
    Map(Box<FxHashMap<(PropertyKey, PropertyFlags), ShapeId>>),
}

const SMALL_TRANSITIONS: usize = 4;

impl Transitions {
    #[inline]
    pub fn find(&self, key: PropertyKey, flags: PropertyFlags) -> Option<ShapeId> {
        match self {
            Transitions::None => None,
            Transitions::Small(entries) => entries
                .iter()
                .find(|(entry_key, entry_flags, _)| *entry_key == key && *entry_flags == flags)
                .map(|entry| entry.2),
            Transitions::Map(map) => map.get(&(key, flags)).copied(),
        }
    }

    fn insert(&mut self, key: PropertyKey, flags: PropertyFlags, child: ShapeId) {
        match self {
            Transitions::None => *self = Transitions::Small(vec![(key, flags, child)]),
            Transitions::Small(entries) if entries.len() < SMALL_TRANSITIONS => {
                entries.push((key, flags, child))
            }
            Transitions::Small(entries) => {
                let mut map: FxHashMap<_, _> =
                    entries.drain(..).map(|(k, f, c)| ((k, f), c)).collect();
                map.insert((key, flags), child);
                *self = Transitions::Map(Box::new(map));
            }
            Transitions::Map(map) => {
                map.insert((key, flags), child);
            }
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(ShapeId) -> bool) {
        match self {
            Transitions::None => {}
            Transitions::Small(entries) => entries.retain(|entry| keep(entry.2)),
            Transitions::Map(map) => map.retain(|_, child| keep(*child)),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Transitions::None => 0,
            Transitions::Small(entries) => entries.len(),
            Transitions::Map(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DictEntry {
    pub key: PropertyKey,
    pub flags: PropertyFlags,
    /// The value slot, or [`NONE`] for a deleted entry.
    pub slot: u32,
}

/// The unshared table of a dictionary-mode object.
#[derive(Default)]
pub struct Dictionary {
    map: FxHashMap<PropertyKey, u32>,
    entries: Vec<DictEntry>,
    free_slots: Vec<u32>,
    next_slot: u32,
}

impl Dictionary {
    fn from_entries(entries: impl IntoIterator<Item = (PropertyKey, PropertyFlags, u32)>) -> Self {
        let mut dictionary = Dictionary::default();
        for (key, flags, slot) in entries {
            dictionary.map.insert(key, dictionary.entries.len() as u32);
            dictionary.entries.push(DictEntry { key, flags, slot });
            dictionary.next_slot = dictionary.next_slot.max(slot + 1);
        }
        dictionary
    }

    #[inline]
    pub fn find(&self, key: PropertyKey) -> Option<(u32, PropertyFlags)> {
        let entry = &self.entries[*self.map.get(&key)? as usize];
        Some((entry.slot, entry.flags))
    }

    /// The slot the next insertion will use.
    fn next_slot(&self) -> u32 {
        self.free_slots.last().copied().unwrap_or(self.next_slot)
    }

    fn insert(&mut self, key: PropertyKey, flags: PropertyFlags) -> u32 {
        let slot = match self.free_slots.pop() {
            Some(slot) => slot,
            None => {
                self.next_slot += 1;
                self.next_slot - 1
            }
        };
        self.map.insert(key, self.entries.len() as u32);
        self.entries.push(DictEntry { key, flags, slot });
        slot
    }

    fn remove(&mut self, key: PropertyKey) -> Option<u32> {
        let index = self.map.remove(&key)?;
        let entry = &mut self.entries[index as usize];
        let slot = entry.slot;
        entry.slot = NONE;
        self.free_slots.push(slot);
        if self.entries.len() > 16 && self.entries.len() > 2 * self.map.len() {
            self.entries.retain(|entry| entry.slot != NONE);
            for (index, entry) in self.entries.iter().enumerate() {
                self.map.insert(entry.key, index as u32);
            }
        }
        Some(slot)
    }

    fn set_flags(&mut self, key: PropertyKey, flags: PropertyFlags) -> bool {
        match self.map.get(&key) {
            Some(&index) => {
                self.entries[index as usize].flags = flags;
                true
            }
            None => false,
        }
    }

    /// Live entries in insertion order.
    pub fn entries(&self) -> impl Iterator<Item = &DictEntry> {
        self.entries.iter().filter(|entry| entry.slot != NONE)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

pub struct Shape {
    /// Transition-tree parent; `None` for roots and dictionary shapes.
    pub parent: Option<ShapeId>,
    /// The property this node adds (meaningless when `count == 0`).
    pub key: PropertyKey,
    pub flags: PropertyFlags,
    pub slot: u32,
    /// Properties up to and including this node.
    pub count: u32,
    pub prototype: Option<ObjectId>,
    pub kind: ObjectKind,
    pub transitions: Transitions,
    /// Key → (slot, flags), built on the first lookup past
    /// [`TABLE_THRESHOLD`] properties.
    pub table: OnceCell<Box<FxHashMap<PropertyKey, (u32, PropertyFlags)>>>,
    /// The prototype's validity cell as last seen, or [`NONE`].
    pub validity: ValidityCellId,
    pub dictionary: Option<Box<Dictionary>>,
}

impl Shape {
    fn root(prototype: Option<ObjectId>, kind: ObjectKind) -> Self {
        Shape {
            parent: None,
            key: PropertyKey::index(0).unwrap(),
            flags: PropertyFlags::DEFAULT,
            slot: NONE,
            count: 0,
            prototype,
            kind,
            transitions: Transitions::None,
            table: OnceCell::new(),
            validity: ValidityCellId(NONE),
            dictionary: None,
        }
    }

    pub fn is_dictionary(&self) -> bool {
        self.dictionary.is_some()
    }

    /// Whether the lookup table has been built.
    pub fn has_table(&self) -> bool {
        self.table.get().is_some()
    }
}

impl Record for Shape {
    fn vacant() -> Self {
        Shape::root(None, ObjectKind::Free)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ValidityCell {
    pub valid: bool,
}

impl Record for ValidityCell {
    fn vacant() -> Self {
        ValidityCell { valid: false }
    }
}

/// Per-prototype bookkeeping. The table holding it is weak in its key and in
/// `dependents`; the collector prunes dead objects after marking.
#[derive(Default, Debug)]
pub struct PrototypeInfo {
    pub cell: Option<ValidityCellId>,
    /// Prototype objects whose `[[Prototype]]` is this object.
    pub dependents: Vec<ObjectId>,
}

impl Heap {
    // ---- shape construction (raw: callers have passed a safepoint) ----

    #[inline]
    pub(crate) fn root_shape(&mut self, prototype: Option<ObjectId>, kind: ObjectKind) -> ShapeId {
        let key = (prototype.map_or(NONE, |p| p.0), kind);
        if let Some((cached, shape)) = self.last_root
            && cached == key
        {
            return shape;
        }
        self.root_shape_slow(prototype, kind, key)
    }

    #[inline(never)]
    fn root_shape_slow(
        &mut self,
        prototype: Option<ObjectId>,
        kind: ObjectKind,
        key: (u32, ObjectKind),
    ) -> ShapeId {
        if let Some(&shape) = self.shape_roots.get(&key) {
            self.last_root = Some((key, shape));
            return shape;
        }
        if let Some(prototype) = prototype {
            self.ensure_prototype(prototype);
        }
        let shape = ShapeId(self.shapes.alloc(Shape::root(prototype, kind)));
        self.shape_roots.insert(key, shape);
        self.last_root = Some((key, shape));
        shape
    }

    pub(crate) fn dictionary_shape(
        &mut self,
        prototype: Option<ObjectId>,
        kind: ObjectKind,
        dictionary: Dictionary,
    ) -> ShapeId {
        if let Some(prototype) = prototype {
            self.ensure_prototype(prototype);
        }
        let mut shape = Shape::root(prototype, kind);
        shape.dictionary = Some(Box::new(dictionary));
        ShapeId(self.shapes.alloc(shape))
    }

    fn transition(&mut self, shape: ShapeId, key: PropertyKey, flags: PropertyFlags) -> ShapeId {
        let parent = self.shapes.get(shape.0);
        if let Some(child) = parent.transitions.find(key, flags) {
            return child;
        }
        let mut child = Shape::root(parent.prototype, parent.kind);
        child.parent = Some(shape);
        child.key = key;
        child.flags = flags;
        child.slot = parent.count;
        child.count = parent.count + 1;
        let child = ShapeId(self.shapes.alloc(child));
        self.shapes
            .get_mut(shape.0)
            .transitions
            .insert(key, flags, child);
        child
    }

    fn build_table(&self, shape: ShapeId) -> FxHashMap<PropertyKey, (u32, PropertyFlags)> {
        let mut current = self.shapes.get(shape.0);
        let mut table = FxHashMap::default();
        table.reserve(current.count as usize);
        while current.count > 0 {
            table.insert(current.key, (current.slot, current.flags));
            current = self
                .shapes
                .get(current.parent.expect("non-root shape has a parent").0);
        }
        table
    }

    /// Looks `key` up in one shape.
    #[inline]
    pub fn shape_lookup(&self, shape: ShapeId, key: PropertyKey) -> Option<(u32, PropertyFlags)> {
        let record = self.shapes.get(shape.0);
        if let Some(dictionary) = &record.dictionary {
            return dictionary.find(key);
        }
        if record.count > TABLE_THRESHOLD {
            return record
                .table
                .get_or_init(|| Box::new(self.build_table(shape)))
                .get(&key)
                .copied();
        }
        let mut current = record;
        while current.count > 0 {
            if current.key == key {
                return Some((current.slot, current.flags));
            }
            current = self
                .shapes
                .get(current.parent.expect("non-root shape has a parent").0);
        }
        None
    }

    /// `(key, flags, slot)` of every property in insertion order.
    pub fn shape_entries(&self, shape: ShapeId) -> Vec<(PropertyKey, PropertyFlags, u32)> {
        let record = self.shapes.get(shape.0);
        if let Some(dictionary) = &record.dictionary {
            return dictionary
                .entries()
                .map(|entry| (entry.key, entry.flags, entry.slot))
                .collect();
        }
        let mut entries = Vec::with_capacity(record.count as usize);
        let mut current = record;
        while current.count > 0 {
            entries.push((current.key, current.flags, current.slot));
            current = self
                .shapes
                .get(current.parent.expect("non-root shape has a parent").0);
        }
        entries.reverse();
        entries
    }

    pub fn shape(&self, shape: ShapeId) -> &Shape {
        self.shapes.get(shape.0)
    }

    pub fn shape_of(&self, object: ObjectId) -> ShapeId {
        self.objects.get(object.0).shape
    }

    pub fn is_dictionary(&self, object: ObjectId) -> bool {
        self.objects.get(object.0).has(flags::DICTIONARY)
    }

    pub fn prototype_of(&self, object: ObjectId) -> Option<ObjectId> {
        self.shapes
            .get(self.objects.get(object.0).shape.0)
            .prototype
    }

    // ---- properties ----

    /// The slot and flags of an own named property.
    #[inline]
    pub fn lookup_own(&self, object: ObjectId, key: PropertyKey) -> Option<(u32, PropertyFlags)> {
        self.shape_lookup(self.objects.get(object.0).shape, key)
    }

    /// `values[start + slot]`: one header load, one value load.
    #[inline]
    #[track_caller]
    pub fn get_slot(&self, object: ObjectId, slot: u32) -> Value {
        let header = self.objects.get(object.0);
        debug_assert!(
            slot < header.slots().len,
            "slot {slot} outside the object's range"
        );
        self.values.get_at(header.slots_start + slot)
    }

    #[inline]
    #[track_caller]
    pub fn set_slot(&mut self, object: ObjectId, slot: u32, value: Value) {
        let header = self.objects.get(object.0);
        debug_assert!(
            slot < header.slots().len,
            "slot {slot} outside the object's range"
        );
        self.values.set_at(header.slots_start + slot, value);
    }

    /// The value of a named property, following the prototype chain.
    pub fn get_property(&self, object: ObjectId, key: PropertyKey) -> Option<Value> {
        let mut current = object;
        loop {
            if let Some((slot, _)) = self.lookup_own(current, key) {
                return Some(self.get_slot(current, slot));
            }
            current = self.prototype_of(current)?;
        }
    }

    /// Adds a new own property. `DuplicateKey` if it already exists.
    #[inline]
    pub fn add_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        flags: PropertyFlags,
        value: Value,
    ) -> Result<(), HeapError> {
        // Fast path: a known transition and a free slot. A transition for
        // `key` proves the current shape lacks it (shapes are immutable), so
        // no duplicate check, and nothing is allocated.
        let header = self.objects.get(object.0);
        if !header.has(flags::DICTIONARY) {
            let slots = header.slots();
            let shape = self.shapes.get(header.shape.0);
            let count = shape.count;
            if count < slots.len
                && let Some(next) = shape.transitions.find(key, flags)
            {
                let header = self.objects.get_mut(object.0);
                header.shape = next;
                let prototype = header.has(flags::USED_AS_PROTOTYPE);
                self.values.set_at(slots.start + count, value);
                if prototype {
                    self.invalidate(object);
                }
                return Ok(());
            }
        }
        self.add_property_slow(object, key, flags, value)
    }

    #[inline(never)]
    fn add_property_slow(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        flags: PropertyFlags,
        value: Value,
    ) -> Result<(), HeapError> {
        let header = *self.objects.get(object.0);
        if header.has(flags::DICTIONARY) {
            return self.dictionary_add(object, key, flags, value);
        }
        let shape = self.shapes.get(header.shape.0);
        let count = shape.count;
        let known = shape.transitions.find(key, flags);
        // A known transition proves the key is absent; otherwise check.
        if known.is_none() && self.shape_lookup(header.shape, key).is_some() {
            return Err(HeapError::DuplicateKey);
        }
        if count >= DICTIONARY_THRESHOLD {
            self.enter_dictionary(object, &[key.to_value(), value])?;
            return self.dictionary_add(object, key, flags, value);
        }
        let slots = header.slots();
        let grow = count >= slots.len;
        let collections = self.gc.stats.collections;
        if grow || known.is_none() {
            let new_len = if grow { (slots.len * 2).max(4) } else { 0 };
            let need = Need {
                shapes: 1,
                values: [new_len, 0],
                ..Need::default()
            };
            self.safepoint(&need, &[Value::object(object), key.to_value(), value])?;
        }
        // The object was rooted, so its shape and ranges are unchanged. A
        // known child is reused only if no collection ran: it is reachable
        // only through the weak transition and may have died.
        let next = match known {
            Some(child) if self.gc.stats.collections == collections => child,
            _ => self.transition(header.shape, key, flags),
        };
        let mut slots = slots;
        if grow {
            slots = self.values.grow(slots, (slots.len * 2).max(4));
        }
        let header = self.objects.get_mut(object.0);
        header.shape = next;
        header.set_slots(slots);
        let prototype = header.has(flags::USED_AS_PROTOTYPE);
        self.values.set(slots, count, value);
        if prototype {
            self.invalidate(object);
        }
        Ok(())
    }

    fn dictionary_add(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        flags: PropertyFlags,
        value: Value,
    ) -> Result<(), HeapError> {
        let header = *self.objects.get(object.0);
        let dictionary = self
            .shapes
            .get(header.shape.0)
            .dictionary
            .as_deref()
            .expect("dictionary shape");
        if dictionary.find(key).is_some() {
            return Err(HeapError::DuplicateKey);
        }
        let slot = dictionary.next_slot();
        let mut slots = header.slots();
        if slot >= slots.len {
            let new_len = (slots.len * 2).max(slot + 1).max(4);
            let need = Need {
                values: [new_len, 0],
                ..Need::default()
            };
            self.safepoint(&need, &[Value::object(object), key.to_value(), value])?;
            slots = self.values.grow(slots, new_len);
            self.objects.get_mut(object.0).set_slots(slots);
        }
        let dictionary = self
            .shapes
            .get_mut(header.shape.0)
            .dictionary
            .as_deref_mut()
            .expect("dictionary shape");
        let used = dictionary.insert(key, flags);
        debug_assert_eq!(used, slot);
        self.values.set(slots, slot, value);
        if header.has(flags::USED_AS_PROTOTYPE) {
            self.invalidate(object);
        }
        Ok(())
    }

    /// Moves `object` to an unshared dictionary shape. `keep` are extra values
    /// to root across the allocation.
    pub(crate) fn enter_dictionary(
        &mut self,
        object: ObjectId,
        keep: &[Value],
    ) -> Result<(), HeapError> {
        let mut roots = vec![Value::object(object)];
        roots.extend_from_slice(keep);
        self.safepoint(
            &Need {
                shapes: 1,
                ..Need::default()
            },
            &roots,
        )?;
        let header = *self.objects.get(object.0);
        let shape = self.shapes.get(header.shape.0);
        let (prototype, kind) = (shape.prototype, shape.kind);
        let dictionary = Dictionary::from_entries(self.shape_entries(header.shape));
        let shape = self.dictionary_shape(prototype, kind, dictionary);
        let header = self.objects.get_mut(object.0);
        header.shape = shape;
        header.flags |= flags::DICTIONARY;
        Ok(())
    }

    /// Deletes an own property; returns whether it existed. The object enters
    /// dictionary mode.
    pub fn delete_property(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
    ) -> Result<bool, HeapError> {
        if self.lookup_own(object, key).is_none() {
            return Ok(false);
        }
        if !self.is_dictionary(object) {
            self.enter_dictionary(object, &[])?;
        }
        let header = *self.objects.get(object.0);
        let dictionary = self
            .shapes
            .get_mut(header.shape.0)
            .dictionary
            .as_deref_mut()
            .expect("dictionary shape");
        let slot = dictionary.remove(key).expect("present key");
        self.values.set(header.slots(), slot, Value::EMPTY);
        if header.has(flags::USED_AS_PROTOTYPE) {
            self.invalidate(object);
        }
        Ok(true)
    }

    /// Changes the flags of an own property, keeping its slot. Returns whether
    /// the property exists.
    pub fn set_property_flags(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        flags: PropertyFlags,
    ) -> Result<bool, HeapError> {
        let Some((_, old)) = self.lookup_own(object, key) else {
            return Ok(false);
        };
        if old == flags {
            return Ok(true);
        }
        let header = *self.objects.get(object.0);
        if header.has(flags::DICTIONARY) {
            let dictionary = self
                .shapes
                .get_mut(header.shape.0)
                .dictionary
                .as_deref_mut()
                .expect("dictionary shape");
            dictionary.set_flags(key, flags);
        } else {
            let mut entries = self.shape_entries(header.shape);
            for entry in &mut entries {
                if entry.0 == key {
                    entry.1 = flags;
                }
            }
            let prototype = self.prototype_of(object);
            self.replay_shape(object, prototype, &entries)?;
        }
        if header.has(flags::USED_AS_PROTOTYPE) {
            self.invalidate(object);
        }
        Ok(true)
    }

    /// Gives a transition-tree object the shape reached from the root for
    /// `(prototype, kind)` by adding `entries` in order. Slots keep their
    /// positions.
    fn replay_shape(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
        entries: &[(PropertyKey, PropertyFlags, u32)],
    ) -> Result<(), HeapError> {
        let need = Need {
            shapes: entries.len() + 1,
            ..Need::default()
        };
        let proto_value = prototype.map_or(Value::UNDEFINED, Value::object);
        self.safepoint(&need, &[Value::object(object), proto_value])?;
        let kind = self.shapes.get(self.objects.get(object.0).shape.0).kind;
        let mut shape = self.root_shape(prototype, kind);
        for &(key, flags, slot) in entries {
            debug_assert_eq!(slot, self.shapes.get(shape.0).count);
            shape = self.transition(shape, key, flags);
        }
        self.objects.get_mut(object.0).shape = shape;
        Ok(())
    }

    /// Sets `[[Prototype]]`. `PrototypeCycle` if `prototype` inherits from
    /// `object`.
    pub fn set_prototype(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
    ) -> Result<(), HeapError> {
        let old = self.prototype_of(object);
        if old == prototype {
            return Ok(());
        }
        let mut walk = prototype;
        while let Some(current) = walk {
            if current == object {
                return Err(HeapError::PrototypeCycle);
            }
            walk = self.prototype_of(current);
        }
        let header = *self.objects.get(object.0);
        if header.has(flags::DICTIONARY) {
            if let Some(prototype) = prototype {
                self.ensure_prototype(prototype);
            }
            let shape = self.shapes.get_mut(header.shape.0);
            shape.prototype = prototype;
            shape.validity = ValidityCellId(NONE);
        } else {
            let entries = self.shape_entries(header.shape);
            self.replay_shape(object, prototype, &entries)?;
        }
        if header.has(flags::USED_AS_PROTOTYPE) {
            if let Some(old) = old
                && let Some(info) = self.prototypes.get_mut(&old)
            {
                info.dependents.retain(|&dependent| dependent != object);
            }
            if let Some(new) = prototype {
                self.prototypes
                    .entry(new)
                    .or_default()
                    .dependents
                    .push(object);
            }
            self.invalidate(object);
        }
        Ok(())
    }

    /// Own keys in `[[OwnPropertyKeys]]` order: array indices ascending
    /// (elements and named), then strings, then symbols, each in insertion
    /// order. Private names are hidden.
    pub fn own_keys(&self, object: ObjectId) -> Vec<PropertyKey> {
        let header = self.objects.get(object.0);
        let mut indices = Vec::new();
        let elements = header.elements();
        let element_count = if header.kind == ObjectKind::Array {
            header.payload.min(elements.len)
        } else {
            elements.len
        };
        for index in 0..element_count {
            if !self.values.get(elements, index).is_empty()
                && let Some(key) = PropertyKey::index(index)
            {
                indices.push((index, key));
            }
        }
        let mut strings = Vec::new();
        let mut symbols = Vec::new();
        for (key, _, _) in self.shape_entries(header.shape) {
            match key.kind() {
                KeyKind::Index(index) => indices.push((index, key)),
                KeyKind::String(id) => match self.strings.get(id.0).array_index {
                    NONE => strings.push(key),
                    index => indices.push((index, key)),
                },
                KeyKind::Symbol(id) => {
                    if !self.symbols.get(id.0).private {
                        symbols.push(key);
                    }
                }
            }
        }
        indices.sort_by_key(|&(index, _)| index);
        indices
            .into_iter()
            .map(|(_, key)| key)
            .chain(strings)
            .chain(symbols)
            .collect()
    }

    // ---- prototypes and validity cells ----

    /// Flags `prototype` as used as a prototype and registers it with its own
    /// prototype's dependents, up the chain.
    pub(crate) fn ensure_prototype(&mut self, prototype: ObjectId) {
        let header = self.objects.get_mut(prototype.0);
        if header.has(flags::USED_AS_PROTOTYPE) {
            return;
        }
        header.flags |= flags::USED_AS_PROTOTYPE;
        self.prototypes.entry(prototype).or_default();
        if let Some(parent) = self.prototype_of(prototype) {
            self.ensure_prototype(parent);
            self.prototypes
                .entry(parent)
                .or_default()
                .dependents
                .push(prototype);
        }
    }

    /// Clears the validity cell of `object` and of every prototype object that
    /// inherits from it.
    pub(crate) fn invalidate(&mut self, object: ObjectId) {
        let mut work = vec![object];
        while let Some(current) = work.pop() {
            if let Some(info) = self.prototypes.get_mut(&current) {
                if let Some(cell) = info.cell.take() {
                    self.cells.get_mut(cell.0).valid = false;
                }
                work.extend_from_slice(&info.dependents);
            }
        }
    }

    /// The cell guarding `object`'s prototype chain: valid until any object on
    /// that chain changes. Allocates a fresh cell after an invalidation.
    pub fn validity_cell(&mut self, object: ObjectId) -> Result<ValidityCellId, HeapError> {
        let shape_id = self.objects.get(object.0).shape;
        let shape = self.shapes.get(shape_id.0);
        let Some(prototype) = shape.prototype else {
            return Ok(STATIC_CELL);
        };
        if shape.validity.0 != NONE && self.cells.get(shape.validity.0).valid {
            return Ok(shape.validity);
        }
        let cell = self.chain_cell_keeping(prototype, Value::object(object))?;
        // The object was rooted, so its shape is unchanged.
        self.shapes.get_mut(shape_id.0).validity = cell;
        Ok(cell)
    }

    /// The cell guarding the chain that starts at `prototype`: valid until
    /// `prototype` or any object above it changes. It is the cell
    /// [`Heap::validity_cell`] gives an object whose prototype is
    /// `prototype`; the string-receiver cache (phase 3, section 2.2) uses it
    /// for `%String.prototype%`, which no object inherits from.
    pub fn chain_cell(&mut self, prototype: ObjectId) -> Result<ValidityCellId, HeapError> {
        self.chain_cell_keeping(prototype, Value::UNDEFINED)
    }

    fn chain_cell_keeping(
        &mut self,
        prototype: ObjectId,
        keep: Value,
    ) -> Result<ValidityCellId, HeapError> {
        self.ensure_prototype(prototype);
        if let Some(cell) = self.prototypes.get(&prototype).and_then(|info| info.cell) {
            return Ok(cell);
        }
        self.safepoint(
            &Need {
                cells: 1,
                ..Need::default()
            },
            &[Value::object(prototype), keep],
        )?;
        let cell = ValidityCellId(self.cells.alloc(ValidityCell { valid: true }));
        self.prototypes.entry(prototype).or_default().cell = Some(cell);
        Ok(cell)
    }

    #[inline]
    pub fn cell_is_valid(&self, cell: ValidityCellId) -> bool {
        self.cells.get(cell.0).valid
    }
}

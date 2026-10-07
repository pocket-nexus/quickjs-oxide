//! Per-executable static-key location facts. Entries own no object, atom, or value.
//! A hit reads today's parallel data slot, never a value retained by the cache.
use std::cell::Cell;

use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx, AtomTable};
#[cfg(test)]
use crate::engine::code::bytecode::Instruction;
use crate::engine::heap::{ContextId, Heap, ObjectId, ObjectKind, PropertySlot, RawValue, ShapeId};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Location {
    domain: u64,
    realm: ContextId,
    shape: ShapeId,
    revision: u64,
    // Only prototype hits depend on other objects' layouts.
    prototype_epoch: u64,
    depth: u32,
    slot: u32,
    numeric_key: bool,
}

pub(crate) enum CacheSelection<'a> {
    Data(&'a RawValue),
    Accessor(Option<ObjectId>),
    CompleteAbsent,
    Unresolved,
}

enum OrdinarySelection<'a> {
    Data {
        raw: &'a RawValue,
        depth: u32,
        slot: u32,
    },
    Accessor {
        getter: Option<ObjectId>,
        depth: u32,
        slot: u32,
    },
    CompleteAbsent,
    Unresolved,
}

enum Located<'a> {
    Data(Location, &'a RawValue),
    Accessor(Location, Option<ObjectId>),
    CompleteAbsent,
    Unresolved,
}

/// Whole-cache view used by the miss path and tests. The hit path reads
/// `Kind` and individual entries instead of copying this value.
#[derive(Clone, Copy, Debug, Default)]
enum State {
    #[default]
    Cold,
    Monomorphic(Location),
    Accessor(Location),
    Polymorphic(Locations),
    Megamorphic(u16),
}

#[derive(Clone, Copy, Debug)]
struct Locations {
    entries: [Location; 4],
    len: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Cold,
    Monomorphic,
    Accessor,
    Polymorphic(u8),
    Megamorphic(u16),
}

const ENTRIES: usize = 4;

/// Four guarded locations cover small polymorphic sites. Unsupported/overflow sites
/// periodically retry specialization, without retaining object or value owners.
/// `kind` says which entries are live: one for monomorphic and accessor
/// sites, the first `len` for polymorphic ones.
#[derive(Debug, Default)]
pub(crate) struct PropertyReadCache {
    kind: Cell<Kind>,
    entries: [Cell<Option<Location>>; ENTRIES],
    backoff: Cell<u16>,
    hits: Cell<u8>,
}

impl PropertyReadCache {
    #[cfg(test)]
    fn with_state(state: State) -> Self {
        let cache = Self::default();
        cache.set_state(state);
        cache
    }

    fn entry(&self, index: usize) -> Location {
        self.entries[index]
            .get()
            .expect("the cache kind covers only filled entries")
    }

    fn state(&self) -> State {
        match self.kind.get() {
            Kind::Cold => State::Cold,
            Kind::Monomorphic => State::Monomorphic(self.entry(0)),
            Kind::Accessor => State::Accessor(self.entry(0)),
            Kind::Polymorphic(len) => {
                let first = self.entry(0);
                let mut entries = [first; ENTRIES];
                for (index, entry) in entries.iter_mut().enumerate().take(len as usize) {
                    *entry = self.entry(index);
                }
                State::Polymorphic(Locations {
                    entries,
                    len: len as usize,
                })
            }
            Kind::Megamorphic(left) => State::Megamorphic(left),
        }
    }

    fn set_state(&self, state: State) {
        let kind = match state {
            State::Cold => Kind::Cold,
            State::Monomorphic(location) => {
                self.entries[0].set(Some(location));
                Kind::Monomorphic
            }
            State::Accessor(location) => {
                self.entries[0].set(Some(location));
                Kind::Accessor
            }
            State::Polymorphic(locations) => {
                for (cell, location) in self.entries.iter().zip(&locations.entries[..locations.len])
                {
                    cell.set(Some(*location));
                }
                Kind::Polymorphic(locations.len as u8)
            }
            State::Megamorphic(left) => Kind::Megamorphic(left),
        };
        self.kind.set(kind);
    }

    /// The caller holds the heap borrow until it has retained/copied the value.
    /// No raw borrowed handle escapes that boundary. Not forced inline: a copy
    /// in each caller grew the hot code past the instruction cache (DeltaBlue
    /// L1i misses +8%, cycles +6% for the same instruction count).
    ///
    /// The receiver is loaded once; each live entry is compared by shape
    /// first and only a matching entry checks the remaining guards.
    pub(crate) fn read<'a>(
        &self,
        heap: &'a Heap,
        domain: u64,
        realm: ContextId,
        receiver: ObjectId,
    ) -> Option<&'a RawValue> {
        let len = match self.kind.get() {
            // Straight-line path: most sites, and nearly all reads in
            // Richards and EarleyBoyer, are monomorphic.
            Kind::Monomorphic => {
                let location = self.entry(0);
                let object = heap.object_fast(receiver);
                if location.shape == object.shape
                    && let Some(value) =
                        Self::read_matched(location, heap, object, domain, realm, receiver)
                {
                    event(if location.depth == 0 {
                        "property_ic.hit.monomorphic"
                    } else {
                        "property_ic.hit.monomorphic_prototype"
                    });
                    self.hit();
                    return Some(value);
                }
                event("property_ic.guard_miss.monomorphic");
                return None;
            }
            Kind::Polymorphic(len) => len as usize,
            Kind::Cold => {
                event("property_ic.cold");
                return None;
            }
            Kind::Accessor => {
                event("property_ic.accessor_site");
                return None;
            }
            Kind::Megamorphic(left) => {
                event("property_ic.megamorphic_skip");
                if left <= 1 {
                    self.kind.set(Kind::Cold);
                    event("property_ic.revive");
                } else {
                    self.kind.set(Kind::Megamorphic(left - 1));
                }
                return None;
            }
        };
        let object = heap.object_fast(receiver);
        for index in 0..len {
            let location = self.entry(index);
            if location.shape != object.shape {
                continue;
            }
            let Some(value) = Self::read_matched(location, heap, object, domain, realm, receiver)
            else {
                continue;
            };
            event(match (index, location.depth) {
                (0, 0) => "property_ic.hit.polymorphic_first",
                (_, 0) => "property_ic.hit.polymorphic_later",
                (0, _) => "property_ic.hit.polymorphic_first_prototype",
                _ => "property_ic.hit.polymorphic_later_prototype",
            });
            // A hit never reorders entries: sites that alternate between
            // shapes would otherwise rewrite the entries on every read.
            self.hit();
            return Some(value);
        }
        event("property_ic.guard_miss.polymorphic");
        None
    }

    fn cool_down(&self) {
        let delay = self.backoff.get().max(16);
        self.kind.set(Kind::Megamorphic(delay));
        self.backoff.set((delay * 2).min(256));
        self.hits.set(0);
    }

    #[inline]
    fn hit(&self) {
        let previous = self.hits.get();
        if previous < 16 {
            let hits = previous + 1;
            if hits == 16 {
                self.backoff.set(16);
            }
            self.hits.set(hits);
        }
    }

    /// Remaining guards of an entry whose shape equals the receiver's.
    #[inline(always)]
    fn read_matched<'a>(
        location: Location,
        heap: &'a Heap,
        object: &crate::engine::heap::ObjectData,
        domain: u64,
        realm: ContextId,
        receiver: ObjectId,
    ) -> Option<&'a RawValue> {
        if location.domain != domain || location.realm != realm {
            return None;
        }
        if !ordinary_receiver(object, location.numeric_key) {
            return None;
        }
        let shape = heap.shape_fast(object.shape);
        if shape.layout_revision() != location.revision {
            return None;
        }
        let mut holder = receiver;
        if location.depth != 0 {
            if heap.property_layout_epoch() != location.prototype_epoch {
                return None;
            }
            for _ in 0..location.depth {
                let data = heap.object_fast(holder);
                holder = heap.shape_fast(data.shape).prototype()?;
            }
        }
        match heap.object_fast(holder).slots.get(location.slot as usize)? {
            PropertySlot::Data(value) => Some(value),
            // VarRef/AutoInit can share data-shaped storage; never treat them
            // as immutable data, even if an internal slot writer changed kind.
            _ => None,
        }
    }

    #[inline]
    fn read_accessor_location(
        location: Location,
        heap: &Heap,
        domain: u64,
        realm: ContextId,
        receiver: ObjectId,
    ) -> Option<Option<ObjectId>> {
        if location.domain != domain || location.realm != realm {
            return None;
        }
        let object = heap.object_fast(receiver);
        if !ordinary_receiver(object, location.numeric_key) || object.shape != location.shape {
            return None;
        }
        if heap.shape_fast(object.shape).layout_revision() != location.revision {
            return None;
        }
        let mut holder = receiver;
        if location.depth != 0 {
            if heap.property_layout_epoch() != location.prototype_epoch {
                return None;
            }
            for _ in 0..location.depth {
                let data = heap.object_fast(holder);
                holder = heap.shape_fast(data.shape).prototype()?;
            }
        }
        match heap.object_fast(holder).slots.get(location.slot as usize)? {
            PropertySlot::Accessor { get, .. } => Some(get.option()),
            _ => None,
        }
    }

    /// Called once on a miss, before the canonical read. This is observational:
    /// it neither roots a value nor invokes an accessor/exotic operation.
    pub(crate) fn miss(
        &self,
        heap: &Heap,
        atoms: &AtomTable,
        domain: u64,
        realm: ContextId,
        receiver: Option<ObjectId>,
        atom: Atom,
    ) {
        let _ = self.miss_selected(heap, atoms, domain, realm, receiver, atom);
    }

    /// Select data, an accessor, or complete absence during one ordinary
    /// traversal. Adapt a data/accessor location when the site is eligible;
    /// cooldown still returns today's result without installing metadata.
    /// The caller must own a borrowed data/getter result before ending its
    /// heap borrow. Exotic and lazy storage stays with the object driver.
    pub(crate) fn miss_selected<'a>(
        &self,
        heap: &'a Heap,
        atoms: &AtomTable,
        domain: u64,
        realm: ContextId,
        receiver: Option<ObjectId>,
        atom: Atom,
    ) -> CacheSelection<'a> {
        let state = self.state();
        if let (State::Accessor(location), Some(receiver)) = (state, receiver)
            && let Some(getter) =
                Self::read_accessor_location(location, heap, domain, realm, receiver)
        {
            return CacheSelection::Accessor(getter);
        }
        if matches!(state, State::Megamorphic(_)) {
            // Cooldown prevents installing another location, but it does not
            // prevent selecting today's ordinary data/getter/absence. The
            // caller consumes that borrowed result under this same heap
            // borrow, avoiding a second canonical lookup on data reads.
            return match receiver.map(|r| select_cooldown(heap, atoms, r, atom)) {
                Some(OrdinarySelection::Data { raw, .. }) => CacheSelection::Data(raw),
                Some(OrdinarySelection::Accessor { getter, .. }) => {
                    CacheSelection::Accessor(getter)
                }
                Some(OrdinarySelection::CompleteAbsent) => CacheSelection::CompleteAbsent,
                _ => CacheSelection::Unresolved,
            };
        }
        let (location, raw) = match receiver.map(|r| locate(heap, atoms, domain, realm, r, atom)) {
            Some(Located::Data(location, raw)) => (location, raw),
            Some(Located::Accessor(location, getter)) => {
                self.set_state(State::Accessor(location));
                event("property_ic.miss");
                return CacheSelection::Accessor(getter);
            }
            found => {
                self.cool_down();
                event("property_ic.megamorphic");
                return if matches!(found, Some(Located::CompleteAbsent)) {
                    CacheSelection::CompleteAbsent
                } else {
                    CacheSelection::Unresolved
                };
            }
        };
        // A revision change of the same shape replaces stale knowledge instead
        // of spending another polymorphic slot on an unreachable old revision.
        let same_key = |old: Location| {
            old.domain == location.domain
                && old.realm == location.realm
                && old.shape == location.shape
        };
        let next = match state {
            State::Cold => State::Monomorphic(location),
            State::Accessor(_) => State::Monomorphic(location),
            State::Monomorphic(old) if same_key(old) => State::Monomorphic(location),
            State::Monomorphic(old) => State::Polymorphic(Locations {
                entries: [location, old, old, old],
                len: 2,
            }),
            State::Polymorphic(mut locations) => {
                if let Some(index) = locations.entries[..locations.len]
                    .iter()
                    .position(|old| same_key(*old))
                {
                    locations.entries[index] = location;
                    locations.entries[..=index].rotate_right(1);
                    State::Polymorphic(locations)
                } else if locations.len < locations.entries.len() {
                    locations.entries[locations.len] = location;
                    locations.len += 1;
                    locations.entries[..locations.len].rotate_right(1);
                    State::Polymorphic(locations)
                } else {
                    self.cool_down();
                    event("property_ic.megamorphic");
                    return CacheSelection::Data(raw);
                }
            }
            State::Megamorphic(_) => unreachable!("cooldown handled before location selection"),
        };
        self.set_state(next);
        event("property_ic.miss");
        CacheSelection::Data(raw)
    }
}

impl crate::engine::heap::runtime::RuntimeState {
    /// Read today's trap value through a cached location under the caller's
    /// current State access. A cold location still trains the same cache.
    pub(crate) fn proxy_trap_read_in_state(
        &mut self,
        domain_id: u64,
        trap: usize,
        realm: ContextId,
        handler: ObjectId,
        atom: Atom,
    ) -> Result<Option<crate::engine::value::JsValue>, RuntimeError> {
        let cache = &self.proxy_trap_reads[trap];
        let raw = match cache.read(&self.heap, domain_id, realm, handler) {
            Some(raw) => {
                if matches!(
                    raw,
                    RawValue::Private(_) | RawValue::Uninitialized | RawValue::Exception
                ) {
                    return Ok(None);
                }
                raw.clone()
            }
            None => {
                cache.miss(
                    &self.heap,
                    &self.atoms,
                    domain_id,
                    realm,
                    Some(handler),
                    atom,
                );
                return Ok(None);
            }
        };
        self.retain_raw_root(raw.clone())?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("proxy_trap_read.hit");
        Ok(Some(
            crate::engine::value::JsValue::from_raw(raw)
                .expect("cached trap excludes internal sentinels"),
        ))
    }
}

fn ordinary_receiver(data: &crate::engine::heap::ObjectData, numeric: bool) -> bool {
    match data.kind {
        ObjectKind::Proxy | ObjectKind::ModuleNamespace => false,
        // Indexed exotics may intercept keys before ordinary shape lookup.
        ObjectKind::Array
        | ObjectKind::Arguments
        | ObjectKind::Primitive
        | ObjectKind::TypedArray => !numeric,
        ObjectKind::Ordinary
        | ObjectKind::Iterator
        | ObjectKind::ArrayIterator
        | ObjectKind::ForInIterator
        | ObjectKind::Date
        | ObjectKind::RegExp
        | ObjectKind::RegExpStringIterator
        | ObjectKind::Map
        | ObjectKind::MapIterator
        | ObjectKind::Set
        | ObjectKind::SetIterator
        | ObjectKind::WeakMap
        | ObjectKind::WeakSet
        | ObjectKind::WeakRef
        | ObjectKind::FinalizationRegistry
        | ObjectKind::GlobalObject
        | ObjectKind::Error
        | ObjectKind::StringIterator
        | ObjectKind::IteratorHelper
        | ObjectKind::IteratorWrap
        | ObjectKind::AsyncFromSyncIterator
        | ObjectKind::IteratorConcat
        | ObjectKind::ArrayBuffer
        | ObjectKind::SharedArrayBuffer
        | ObjectKind::DataView
        | ObjectKind::NativeFunction
        | ObjectKind::BoundFunction
        | ObjectKind::BytecodeFunction
        | ObjectKind::Generator
        | ObjectKind::AsyncGenerator
        | ObjectKind::AsyncFunctionState
        | ObjectKind::Promise => true,
    }
}

fn locate<'a>(
    heap: &'a Heap,
    atoms: &AtomTable,
    domain: u64,
    realm: ContextId,
    receiver: ObjectId,
    atom: Atom,
) -> Located<'a> {
    let Some(initial) = heap.object(receiver).ok() else {
        return Located::Unresolved;
    };
    let Some(initial_shape) = heap.shape(initial.shape).ok() else {
        return Located::Unresolved;
    };
    let revision = initial_shape.layout_revision();
    let epoch = heap.property_layout_epoch();
    if revision == u64::MAX || epoch == u64::MAX {
        return Located::Unresolved;
    }
    let Some(numeric) = numeric_key(atoms, atom) else {
        return Located::Unresolved;
    };
    let location = |depth, slot| Location {
        domain,
        realm,
        shape: initial.shape,
        revision,
        prototype_epoch: epoch,
        depth,
        slot,
        numeric_key: numeric,
    };
    match select_ordinary(heap, atoms, receiver, atom, Some(numeric)) {
        OrdinarySelection::Data { raw, depth, slot } => Located::Data(location(depth, slot), raw),
        OrdinarySelection::Accessor {
            getter,
            depth,
            slot,
        } => Located::Accessor(location(depth, slot), getter),
        OrdinarySelection::CompleteAbsent => Located::CompleteAbsent,
        OrdinarySelection::Unresolved => Located::Unresolved,
    }
}

/// Cooldown consumes today's selection without constructing future cache facts.
/// Keep the same saturation admission as location-producing selection.
fn select_cooldown<'a>(
    heap: &'a Heap,
    atoms: &AtomTable,
    receiver: ObjectId,
    atom: Atom,
) -> OrdinarySelection<'a> {
    let Some(initial) = heap.object(receiver).ok() else {
        return OrdinarySelection::Unresolved;
    };
    let Some(shape) = heap.shape(initial.shape).ok() else {
        return OrdinarySelection::Unresolved;
    };
    if shape.layout_revision() == u64::MAX || heap.property_layout_epoch() == u64::MAX {
        return OrdinarySelection::Unresolved;
    }
    // Preserve rejection of null, stale, and foreign table-backed atoms even
    // when an ordinary receiver does not need numeric spelling inspection.
    if atoms.property_key_kind(atom).is_err() {
        return OrdinarySelection::Unresolved;
    }
    select_ordinary(heap, atoms, receiver, atom, None)
}

fn numeric_key(atoms: &AtomTable, atom: Atom) -> Option<bool> {
    let array_index = atoms.array_index(atom).ok()?;
    let key_kind = atoms.property_key_kind(atom).ok()?;
    Some(
        array_index.is_some()
            || (key_kind == crate::engine::atom::PropertyKeyKind::String && {
                // Conservative, allocation-free superset of CanonicalNumericIndexString.
                let spelling = atoms.to_js_string(atom).ok()?;
                let first = spelling.utf16_units().next();
                matches!(first, Some(43 | 45 | 46 | 48..=57))
                    || spelling.utf16_units().eq("NaN".encode_utf16())
                    || spelling.utf16_units().eq("Infinity".encode_utf16())
            }),
    )
}

/// Both adaptive and cooldown reads use this one ordinary traversal. Numeric
/// interception matters only at indexed exotic objects; a cooldown ordinary
/// chain need not inspect the immutable key's spelling at all. Borrowed results
/// remain valid only under the caller's heap borrow.
fn select_ordinary<'a>(
    heap: &'a Heap,
    atoms: &AtomTable,
    receiver: ObjectId,
    atom: Atom,
    mut numeric: Option<bool>,
) -> OrdinarySelection<'a> {
    let mut holder = receiver;
    let mut depth = 0u32;
    loop {
        let Some(data) = heap.object(holder).ok() else {
            return OrdinarySelection::Unresolved;
        };
        match data.kind {
            ObjectKind::Proxy | ObjectKind::ModuleNamespace => {
                return OrdinarySelection::Unresolved;
            }
            ObjectKind::Array
            | ObjectKind::Arguments
            | ObjectKind::Primitive
            | ObjectKind::TypedArray => {
                let value = match numeric {
                    Some(value) => value,
                    None => {
                        let Some(value) = numeric_key(atoms, atom) else {
                            return OrdinarySelection::Unresolved;
                        };
                        numeric = Some(value);
                        value
                    }
                };
                if value {
                    return OrdinarySelection::Unresolved;
                }
            }
            ObjectKind::Ordinary
            | ObjectKind::Iterator
            | ObjectKind::ArrayIterator
            | ObjectKind::ForInIterator
            | ObjectKind::Date
            | ObjectKind::RegExp
            | ObjectKind::RegExpStringIterator
            | ObjectKind::Map
            | ObjectKind::MapIterator
            | ObjectKind::Set
            | ObjectKind::SetIterator
            | ObjectKind::WeakMap
            | ObjectKind::WeakSet
            | ObjectKind::WeakRef
            | ObjectKind::FinalizationRegistry
            | ObjectKind::GlobalObject
            | ObjectKind::Error
            | ObjectKind::StringIterator
            | ObjectKind::IteratorHelper
            | ObjectKind::IteratorWrap
            | ObjectKind::AsyncFromSyncIterator
            | ObjectKind::IteratorConcat
            | ObjectKind::ArrayBuffer
            | ObjectKind::SharedArrayBuffer
            | ObjectKind::DataView
            | ObjectKind::NativeFunction
            | ObjectKind::BoundFunction
            | ObjectKind::BytecodeFunction
            | ObjectKind::Generator
            | ObjectKind::AsyncGenerator
            | ObjectKind::AsyncFunctionState
            | ObjectKind::Promise => {}
        }
        let Some(shape) = heap.shape(data.shape).ok() else {
            return OrdinarySelection::Unresolved;
        };
        if let Some(slot) = shape.find(AtomIdx::from_raw(atom.raw())) {
            return match data.slots.get(slot as usize) {
                Some(PropertySlot::Data(raw)) => OrdinarySelection::Data { raw, depth, slot },
                Some(PropertySlot::Accessor { get, .. }) => OrdinarySelection::Accessor {
                    getter: get.option(),
                    depth,
                    slot,
                },
                _ => OrdinarySelection::Unresolved,
            };
        }
        let Some(prototype) = shape.prototype() else {
            return OrdinarySelection::CompleteAbsent;
        };
        holder = prototype;
        let Some(next_depth) = depth.checked_add(1) else {
            return OrdinarySelection::Unresolved;
        };
        depth = next_depth;
    }
}

/// Per-executable site caches addressed by execution PC. Each PC has a
/// one-byte offset within its 64-PC block (or `NO_SITE`), and block ranks
/// give the first site index of each block, so a lookup is two loads and an
/// add (x86-64 baseline has no `popcnt` for a bitmap rank).
#[derive(Debug)]
pub(crate) struct SiteCacheTable<T> {
    site_offsets: Box<[u8]>,
    block_ranks: Box<[u32]>,
    sites: Box<[T]>,
}

const NO_SITE: u8 = u8::MAX;

pub(crate) type PropertyReadCacheTable = SiteCacheTable<PropertyReadCache>;

impl<T: Default> SiteCacheTable<T> {
    fn from_site_pcs(pc_len: usize, site_pcs: impl IntoIterator<Item = usize>) -> Self {
        let mut offsets = vec![NO_SITE; pc_len];
        for pc in site_pcs {
            offsets[pc] = 0;
        }
        let mut ranks = Vec::with_capacity(pc_len.div_ceil(64));
        let mut count = 0u32;
        for block in offsets.chunks_mut(64) {
            ranks.push(count);
            let mut within = 0u8;
            for offset in block.iter_mut().filter(|offset| **offset != NO_SITE) {
                *offset = within;
                within += 1;
            }
            count = count
                .checked_add(u32::from(within))
                .expect("bytecode site count fits u32");
        }
        Self {
            site_offsets: offsets.into_boxed_slice(),
            block_ranks: ranks.into_boxed_slice(),
            sites: (0..count).map(|_| T::default()).collect(),
        }
    }

    fn new_exec_sites(
        code: &crate::engine::code::exec::ExecCode,
        is_site: impl Fn(crate::engine::code::exec_opcode::Opcode) -> bool,
    ) -> Self {
        Self::from_site_pcs(
            code.word_len(),
            (0..code.instruction_len())
                .filter(|&source_pc| code.opcode_at_source(source_pc).is_some_and(&is_site))
                .map(|source_pc| {
                    code.exec_pc(source_pc as u32).expect("verified source PC") as usize
                }),
        )
    }

    #[inline]
    fn site_index(&self, pc: usize) -> Option<usize> {
        let offset = *self.site_offsets.get(pc)?;
        if offset == NO_SITE {
            return None;
        }
        Some(self.block_ranks[pc / 64] as usize + usize::from(offset))
    }

    #[inline]
    pub(crate) fn site(&self, pc: usize) -> Option<&T> {
        self.sites.get(self.site_index(pc)?)
    }

    /// PC blocks: one per 64 execution PCs.
    pub(crate) fn pc_words(&self) -> usize {
        self.block_ranks.len()
    }
}

impl SiteCacheTable<PropertyReadCache> {
    pub(crate) fn new_exec(code: &crate::engine::code::exec::ExecCode) -> Self {
        use crate::engine::code::exec_opcode::Opcode;
        Self::new_exec_sites(code, |opcode| {
            matches!(
                opcode,
                Opcode::GetField
                    | Opcode::GetField2
                    | Opcode::GetFieldCached
                    | Opcode::GetField2Cached
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn new(code: &[Instruction]) -> Self {
        Self::from_site_pcs(
            code.len(),
            code.iter().enumerate().filter_map(|(pc, instruction)| {
                matches!(
                    instruction,
                    Instruction::GetField(_) | Instruction::GetField2(_)
                )
                .then_some(pc)
            }),
        )
    }
}

impl SiteCacheTable<super::append_ic::PropertyAppendCache> {
    pub(crate) fn new_exec(code: &crate::engine::code::exec::ExecCode) -> Self {
        use crate::engine::code::exec_opcode::Opcode;
        Self::new_exec_sites(code, |opcode| {
            matches!(opcode, Opcode::PutField | Opcode::DefineField)
        })
    }
}

#[inline]
fn event(name: &'static str) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(name);
    #[cfg(not(feature = "profiling"))]
    let _ = name;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::runtime::Runtime;
    #[cfg(test)]
    use crate::engine::value::Value;

    fn object(value: Value) -> crate::engine::object::ObjectRef {
        let Value::Object(value) = value else {
            panic!("expected object")
        };
        value
    }
    fn install(
        cache: &PropertyReadCache,
        runtime: &Runtime,
        realm: ContextId,
        object: &crate::engine::object::ObjectRef,
        atom: Atom,
    ) {
        let state = runtime.0.state.borrow();
        cache.miss(
            &state.heap,
            &state.atoms,
            runtime.domain_id(),
            realm,
            Some(object.object_id()),
            atom,
        );
    }
    fn number(
        cache: &PropertyReadCache,
        runtime: &Runtime,
        realm: ContextId,
        object: &crate::engine::object::ObjectRef,
    ) -> Option<f64> {
        let state = runtime.0.state.borrow();
        cache
            .read(&state.heap, runtime.domain_id(), realm, object.object_id())
            .and_then(|value| match value {
                RawValue::Int(n) => Some(f64::from(*n)),
                RawValue::Float(n) => Some(*n),
                _ => None,
            })
    }
    #[test]
    fn cooldown_selection_keeps_exotic_and_callback_boundaries() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for (source, name, expected) in [
            ("({x:17})", "x", "data"),
            ("Object.create({x:17})", "x", "data"),
            ("({get x(){throw 1}})", "x", "accessor"),
            ("({})", "missing", "absent"),
            ("Object.create([17])", "0", "unresolved"),
            ("Object.create(Object.assign([], {x:17}))", "x", "data"),
            ("new Proxy({}, {get(){throw 1}})", "x", "unresolved"),
            ("new Uint8Array(1)", "-0", "unresolved"),
            ("new Uint8Array(1)", "NaN", "unresolved"),
        ] {
            let receiver = object(context.eval(source).unwrap());
            let key = runtime.intern_property_key(name).unwrap();
            let cache = PropertyReadCache::with_state(State::Megamorphic(7));
            let state = runtime.0.state.borrow();
            let selected = cache.miss_selected(
                &state.heap,
                &state.atoms,
                runtime.domain_id(),
                context.realm_id(),
                Some(receiver.object_id()),
                key.atom(),
            );
            let actual = match selected {
                CacheSelection::Data(_) => "data",
                CacheSelection::Accessor(_) => "accessor",
                CacheSelection::CompleteAbsent => "absent",
                CacheSelection::Unresolved => "unresolved",
            };
            assert_eq!(actual, expected, "{source}[{name}]");
            assert!(matches!(cache.state(), State::Megamorphic(7)));
        }
    }

    #[test]
    fn cooldown_rejects_foreign_and_null_atoms() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let receiver = object(context.eval("({x:17})").unwrap());
        let foreign_key = foreign.intern_property_key("x").unwrap();
        let state = runtime.0.state.borrow();
        for atom in [foreign_key.atom(), Atom::NULL] {
            assert!(matches!(
                select_cooldown(&state.heap, &state.atoms, receiver.object_id(), atom),
                OrdinarySelection::Unresolved
            ));
        }
    }

    #[test]
    fn sparse_site_rank_crosses_words_and_omits_writes() {
        let mut code = vec![Instruction::Nop; 130];
        code[0] = Instruction::GetField(0);
        code[63] = Instruction::PutField(1);
        code[64] = Instruction::GetField2(2);
        code[65] = Instruction::PutField(3);
        code[129] = Instruction::GetField(4);
        let table = PropertyReadCacheTable::new(&code);
        for (rank, pc) in [0, 64, 129].into_iter().enumerate() {
            assert_eq!(table.site_index(pc), Some(rank));
        }
        for pc in [1, 62, 63, 65, 66, 128, 130, usize::MAX] {
            assert_eq!(table.site_index(pc), None);
        }
        assert!(table.site(0).is_some());
        assert!(table.site(63).is_none());
        assert_eq!(table.site_offsets.len(), 130);
        assert_eq!(table.block_ranks.len(), 3);
        assert!(PropertyReadCacheTable::new(&[]).site(0).is_none());
    }

    #[test]
    fn unstable_sites_back_off_with_a_bounded_retry_delay() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let key = runtime.intern_property_key("x").unwrap();
        let receiver = runtime.new_object(None).unwrap();
        let cache = PropertyReadCache::default();
        let state = runtime.0.state.borrow();
        for delay in [16, 32, 64, 128, 256, 256] {
            cache.miss(
                &state.heap,
                &state.atoms,
                runtime.domain_id(),
                context.realm_id(),
                None,
                key.atom(),
            );
            assert!(matches!(cache.state(), State::Megamorphic(left) if left == delay));
            for _ in 0..delay {
                assert!(
                    cache
                        .read(
                            &state.heap,
                            runtime.domain_id(),
                            context.realm_id(),
                            receiver.object_id()
                        )
                        .is_none()
                );
            }
            assert!(matches!(cache.state(), State::Cold));
        }
    }

    #[test]
    fn four_shapes_alternate_and_fifth_shape_eventually_revives() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let first = object(context.eval("({x:1})").unwrap());
        let second = object(context.eval("({y:0,x:2})").unwrap());
        let third = object(context.eval("({z:0,y:0,x:3})").unwrap());
        let key = runtime.intern_property_key("x").unwrap();
        let realm = context.realm_id();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, realm, &first, key.atom());
        install(&cache, &runtime, realm, &second, key.atom());
        assert!(matches!(cache.state(), State::Polymorphic(_)));
        for _ in 0..8 {
            assert_eq!(number(&cache, &runtime, realm, &first), Some(1.0));
            assert_eq!(number(&cache, &runtime, realm, &second), Some(2.0));
        }
        let fourth = object(context.eval("({w:0,z:0,y:0,x:4})").unwrap());
        let fifth = object(context.eval("({v:0,w:0,z:0,y:0,x:5})").unwrap());
        install(&cache, &runtime, realm, &third, key.atom());
        install(&cache, &runtime, realm, &fourth, key.atom());
        for _ in 0..8 {
            assert_eq!(number(&cache, &runtime, realm, &first), Some(1.0));
            assert_eq!(number(&cache, &runtime, realm, &second), Some(2.0));
            assert_eq!(number(&cache, &runtime, realm, &third), Some(3.0));
            assert_eq!(number(&cache, &runtime, realm, &fourth), Some(4.0));
        }
        install(&cache, &runtime, realm, &fifth, key.atom());
        assert!(matches!(cache.state(), State::Megamorphic(16)));
        for _ in 0..16 {
            assert_eq!(number(&cache, &runtime, realm, &first), None);
        }
        assert!(matches!(cache.state(), State::Cold));
        install(&cache, &runtime, realm, &third, key.atom());
        assert_eq!(number(&cache, &runtime, realm, &third), Some(3.0));
    }

    #[test]
    fn exotic_named_storage_is_cached_but_typed_numeric_keys_are_not() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let key = runtime.intern_property_key("x").unwrap();
        for expression in ["new Map()", "new Date()", "new Uint8Array(2)"] {
            let receiver = object(
                context
                    .eval(&format!("var exotic={expression}; exotic.x=7; exotic"))
                    .unwrap(),
            );
            let cache = PropertyReadCache::default();
            install(&cache, &runtime, context.realm_id(), &receiver, key.atom());
            assert_eq!(
                number(&cache, &runtime, context.realm_id(), &receiver),
                Some(7.0)
            );
        }
        let typed = object(context.eval("new Uint8Array(2)").unwrap());
        for spelling in ["0", "-0", "NaN", "Infinity", "1.5"] {
            let key = runtime.intern_property_key(spelling).unwrap();
            let cache = PropertyReadCache::default();
            install(&cache, &runtime, context.realm_id(), &typed, key.atom());
            assert!(matches!(cache.state(), State::Megamorphic(_)));
        }
    }

    #[test]
    fn cached_location_reads_replaced_value_and_unsupported_miss_cools_down() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let obj = object(context.eval("var o = {x:1}; o").unwrap());
        let key = runtime.intern_property_key("x").unwrap();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(1.0)
        );
        drop(context.eval("o.x=9").unwrap());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(9.0)
        );
        drop(context.eval("delete o.x").unwrap());
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        drop(context.eval("o.x=11").unwrap());
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert!(matches!(cache.state(), State::Megamorphic(_)));
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
    }
    #[test]
    fn attributes_and_prototype_replacement_invalidate_before_accessor_execution() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let key = runtime.intern_property_key("x").unwrap();
        for mutation in [
            "Object.defineProperty(o,'x',{get(){throw 7}})",
            "Object.defineProperty(o,'x',{writable:false})",
            "Object.setPrototypeOf(o,{x:6})",
        ] {
            let obj = object(context.eval("var p={x:3}; var o={x:1}; o").unwrap());
            let cache = PropertyReadCache::default();
            install(&cache, &runtime, context.realm_id(), &obj, key.atom());
            drop(context.eval(mutation).unwrap());
            assert_eq!(
                number(&cache, &runtime, context.realm_id(), &obj),
                None,
                "{mutation}"
            );
        }
    }
    #[test]
    fn prototype_holder_mutation_and_shadowing_invalidate_without_caching_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let key = runtime.intern_property_key("x").unwrap();
        for mutation in [
            "delete p.x",
            "m.x=4",
            "Object.setPrototypeOf(m,{x:5})",
            "Object.defineProperty(p,'x',{get(){throw 7}})",
        ] {
            let obj = object(
                context
                    .eval("var p={x:3}; var m=Object.create(p); var o=Object.create(m); o")
                    .unwrap(),
            );
            let cache = PropertyReadCache::default();
            install(&cache, &runtime, context.realm_id(), &obj, key.atom());
            assert_eq!(
                number(&cache, &runtime, context.realm_id(), &obj),
                Some(3.0)
            );
            drop(context.eval("p.x=8").unwrap());
            assert_eq!(
                number(&cache, &runtime, context.realm_id(), &obj),
                Some(8.0)
            );
            drop(context.eval(mutation).unwrap());
            assert_eq!(
                number(&cache, &runtime, context.realm_id(), &obj),
                None,
                "{mutation}"
            );
        }
    }
    #[test]
    fn dictionary_slot_swap_and_new_key_invalidate() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let obj = object(
            context
                .eval("var o={x:1}; for(var i=0;i<100;i++)o['p'+i]=i; delete o.p0; o")
                .unwrap(),
        );
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .shape(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object(obj.object_id())
                        .unwrap()
                        .shape
                )
                .unwrap()
                .is_dictionary()
        );
        let key = runtime.intern_property_key("x").unwrap();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(1.0)
        );
        drop(context.eval("delete o.p1").unwrap());
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        drop(context.eval("o.more=6").unwrap());
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
    }
    #[test]
    fn realm_and_runtime_identity_never_alias_and_proxy_is_not_admitted() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let other = context.runtime().new_context().expect("create context");
        let obj = object(context.eval("({x:1})").unwrap());
        let key = runtime.intern_property_key("x").unwrap();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert_eq!(number(&cache, &runtime, other.realm_id(), &obj), None);
        let state = runtime.0.state.borrow();
        assert!(
            cache
                .read(
                    &state.heap,
                    runtime.domain_id() + 1,
                    context.realm_id(),
                    obj.object_id()
                )
                .is_none()
        );
        drop(state);
        let proxy = object(context.eval("new Proxy({x:2},{get(){throw 9}})").unwrap());
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &proxy, key.atom());
        assert_eq!(number(&cache, &runtime, context.realm_id(), &proxy), None);
    }
    #[test]
    fn named_array_cache_survives_value_write_and_invalidates_holey_materialization() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let obj = object(context.eval("var o=[1,2,3]; o.x=4; o").unwrap());
        let key = runtime.intern_property_key("x").unwrap();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(4.0)
        );
        drop(context.eval("o.x=5; o[0]=8").unwrap());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(5.0)
        );
        drop(context.eval("delete o[1]").unwrap());
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
    }
    #[test]
    fn entering_dictionary_storage_invalidates_an_existing_own_fact() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let obj = object(context.eval("var o={x:1}; o").unwrap());
        let key = runtime.intern_property_key("x").unwrap();
        let cache = PropertyReadCache::default();
        install(&cache, &runtime, context.realm_id(), &obj, key.atom());
        assert_eq!(
            number(&cache, &runtime, context.realm_id(), &obj),
            Some(1.0)
        );
        drop(
            context
                .eval("for(var i=0;i<100;i++) o['p'+i]=i; delete o.p0")
                .unwrap(),
        );
        assert_eq!(number(&cache, &runtime, context.realm_id(), &obj), None);
    }
    #[test]
    fn prototype_attribute_changes_invalidate_epoch_with_unchanged_receiver_layout() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let key = runtime.intern_property_key("x").unwrap();
        for dictionary in [false, true] {
            for mutation in [
                "Object.defineProperty(p,'x',{writable:false})",
                "Object.defineProperty(p,'x',{enumerable:false})",
                "Object.defineProperty(p,'x',{get(){calls++; return 17}})",
                "Object.defineProperty(m,'x',{get(){calls++; return 19},configurable:true})",
            ] {
                let receiver = object(context.eval("var calls=0; var p={x:3}; var m=Object.create(p); var o=Object.create(m); o").unwrap());
                if dictionary {
                    let holder = object(context.eval("p").unwrap());
                    runtime
                        .0
                        .state
                        .borrow_mut()
                        .ensure_dictionary_layout(holder.object_id())
                        .unwrap();
                }
                let cache = PropertyReadCache::default();
                install(&cache, &runtime, context.realm_id(), &receiver, key.atom());
                assert_eq!(
                    number(&cache, &runtime, context.realm_id(), &receiver),
                    Some(3.0)
                );
                let (shape, revision, epoch) = {
                    let state = runtime.0.state.borrow();
                    let shape = state.heap.object(receiver.object_id()).unwrap().shape;
                    (
                        shape,
                        state.heap.shape(shape).unwrap().layout_revision(),
                        state.heap.property_layout_epoch(),
                    )
                };
                drop(context.eval(mutation).unwrap());
                let state = runtime.0.state.borrow();
                assert_eq!(
                    state.heap.object(receiver.object_id()).unwrap().shape,
                    shape
                );
                assert_eq!(state.heap.shape(shape).unwrap().layout_revision(), revision);
                assert!(
                    state.heap.property_layout_epoch() > epoch,
                    "dictionary={dictionary}: {mutation}"
                );
                drop(state);
                assert_eq!(
                    number(&cache, &runtime, context.realm_id(), &receiver),
                    None
                );
                assert_eq!(context.eval("calls").unwrap(), Value::Int(0));
            }
        }
    }
}

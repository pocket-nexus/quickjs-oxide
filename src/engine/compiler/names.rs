//! Interned identifier and synthetic names shared by the parser and IR.
//!
//! The parser is the only writer: it interns every authored or synthetic name
//! once, and resolution/lowering read text back through `name`. Content keys
//! keep the default SipHash because compiler input is untrusted.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::engine::value::{JsString, JsStringError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::engine::compiler) struct NameId(u32);

const RECENT_NAMES: usize = 256;

#[derive(Debug)]
pub(in crate::engine::compiler) struct NameTable {
    names: Vec<Rc<str>>,
    by_name: HashMap<Rc<str>, NameId>,
    ascii_names: [Option<NameId>; 128],
    recent_names: [Cell<Option<NameId>>; RECENT_NAMES],
    js_strings: Vec<Option<JsString>>,
}

impl NameTable {
    pub(in crate::engine::compiler) fn new() -> Self {
        Self {
            names: Vec::new(),
            by_name: HashMap::new(),
            ascii_names: [None; 128],
            recent_names: [const { Cell::new(None) }; RECENT_NAMES],
            js_strings: Vec::new(),
        }
    }

    pub(in crate::engine::compiler) fn intern(&mut self, name: &str) -> NameId {
        if let Some(id) = self.lookup(name) {
            return id;
        }
        let id =
            NameId(u32::try_from(self.names.len()).expect("interned name count must fit in u32"));
        let owned: Rc<str> = name.into();
        self.names.push(owned.clone());
        self.by_name.insert(owned, id);
        self.js_strings.push(None);
        if let [byte @ 0..=127] = name.as_bytes() {
            self.ascii_names[*byte as usize] = Some(id);
        } else {
            self.recent_names[recent_name_slot(name)].set(Some(id));
        }
        id
    }

    pub(in crate::engine::compiler) fn name(&self, id: NameId) -> &str {
        &self.names[id.0 as usize]
    }

    #[inline]
    pub(in crate::engine::compiler) fn lookup(&self, name: &str) -> Option<NameId> {
        if let [byte @ 0..=127] = name.as_bytes() {
            return self.ascii_names[*byte as usize];
        }
        // A bounded, exact-match hint avoids rehashing repeated spellings.
        // Collisions only miss this one slot; the authoritative map still
        // uses randomized SipHash for arbitrary untrusted identifiers.
        let slot = &self.recent_names[recent_name_slot(name)];
        if let Some(id) = slot.get() {
            if self.name(id) == name {
                return Some(id);
            }
        }
        let id = self.by_name.get(name).copied();
        if id.is_some() {
            slot.set(id);
        }
        id
    }

    /// Reuse one `JsString` per interned name instead of re-allocating it at
    /// every constant lookup.
    pub(in crate::engine::compiler) fn js_string(
        &mut self,
        id: NameId,
    ) -> Result<JsString, JsStringError> {
        if let Some(cached) = &self.js_strings[id.0 as usize] {
            return Ok(cached.clone());
        }
        let string = JsString::try_from_utf8(&self.names[id.0 as usize])?;
        self.js_strings[id.0 as usize] = Some(string.clone());
        Ok(string)
    }
}

fn recent_name_slot(name: &str) -> usize {
    let bytes = name.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    ((usize::from(bytes[0]) * 33)
        ^ (usize::from(bytes[bytes.len() / 2]) * 9)
        ^ (usize::from(bytes[bytes.len() - 1]) * 17)
        ^ bytes.len())
        & (RECENT_NAMES - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_name_collisions_preserve_ids_and_missing_lookups() {
        let mut names = NameTable::new();
        // More distinct names than cache slots, including equal-length names
        // with the same sampled bytes, force evictions and exact comparisons.
        let text: Vec<_> = (0..1024).map(|i| format!("prefix_{i:04}_suffix")).collect();
        let ids: Vec<_> = text.iter().map(|name| names.intern(name)).collect();
        for _ in 0..2 {
            for (name, &id) in text.iter().zip(&ids).rev() {
                assert_eq!(names.intern(name), id);
                assert_eq!(names.lookup(name), Some(id));
                assert_eq!(names.name(id), name);
            }
        }
        assert_eq!(names.lookup("prefix_9999_suffix"), None);
        assert_eq!(names.names.len(), text.len());
        for name in ["", "a", "λ", "名称", "#private"] {
            let id = names.intern(name);
            assert_eq!(names.lookup(name), Some(id));
            assert_eq!(names.intern(name), id);
        }
    }
}

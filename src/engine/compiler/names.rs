//! Interned identifier and synthetic names shared by the parser and IR.
//!
//! The parser is the only writer: it interns every authored or synthetic name
//! once, and resolution/lowering read text back through `name`. Content keys
//! keep the default SipHash because compiler input is untrusted.

use std::collections::HashMap;

use crate::engine::value::{JsString, JsStringError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::engine::compiler) struct NameId(u32);

#[derive(Debug)]
pub(in crate::engine::compiler) struct NameTable {
    names: Vec<Box<str>>,
    by_name: HashMap<Box<str>, NameId>,
    js_strings: HashMap<NameId, JsString>,
}

impl NameTable {
    pub(in crate::engine::compiler) fn new() -> Self {
        Self {
            names: Vec::new(),
            by_name: HashMap::new(),
            js_strings: HashMap::new(),
        }
    }

    pub(in crate::engine::compiler) fn intern(&mut self, name: &str) -> NameId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id =
            NameId(u32::try_from(self.names.len()).expect("interned name count must fit in u32"));
        let owned: Box<str> = name.into();
        self.names.push(owned.clone());
        self.by_name.insert(owned, id);
        id
    }

    pub(in crate::engine::compiler) fn name(&self, id: NameId) -> &str {
        &self.names[id.0 as usize]
    }

    pub(in crate::engine::compiler) fn lookup(&self, name: &str) -> Option<NameId> {
        self.by_name.get(name).copied()
    }

    /// Reuse one `JsString` per interned name instead of re-allocating it at
    /// every constant lookup.
    pub(in crate::engine::compiler) fn js_string(
        &mut self,
        id: NameId,
    ) -> Result<JsString, JsStringError> {
        if let Some(cached) = self.js_strings.get(&id) {
            return Ok(cached.clone());
        }
        let string = JsString::try_from_utf8(&self.names[id.0 as usize])?;
        self.js_strings.insert(id, string.clone());
        Ok(string)
    }
}

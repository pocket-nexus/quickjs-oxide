//! Runtime-local permanent atoms for production static property spellings.
use super::{Atom, AtomError, AtomTable};
#[derive(Clone, Copy)]
#[allow(dead_code)] // Some spellings belong to optional execution features.
pub(crate) enum PinnedAtom {
    Literal0,                 //
    Literal1,                 // 0
    Literal2,                 // 1
    Literal3,                 // 2
    Literal4,                 // 2047
    Literal5,                 // 5
    This,                     // <this>
    Atomics,                  // Atomics
    Json,                     // JSON
    Math,                     // Math
    Object,                   // Object
    Reflect,                  // Reflect
    UnsupportedType,          // [unsupported type]
    Json5Value,               // __json5Value
    Proto,                    // __proto__
    Answer,                   // answer
    Apply,                    // apply
    Assign,                   // assign
    BatchFirst,               // batch_first
    BatchRealm,               // batch_realm
    Buffer,                   // buffer
    Callee,                   // callee
    Cause,                    // cause
    Construct,                // construct
    Constructor,              // constructor
    Count,                    // count
    Create,                   // create
    DeleteProperty,           // deleteProperty
    Done,                     // done
    Entries,                  // entries
    Exec,                     // exec
    Flags,                    // flags
    FromEntries,              // fromEntries
    Get,                      // get
    GetPrototypeOf,           // getPrototypeOf
    Global,                   // global
    GlobalThis,               // globalThis
    Groups,                   // groups
    Has,                      // has
    HasOwn,                   // hasOwn
    Index,                    // index
    Input,                    // input
    Is,                       // is
    Key,                      // key
    Keys,                     // keys
    LastIndex,                // lastIndex
    Length,                   // length
    Load,                     // load
    MaxByteLength,            // maxByteLength
    Message,                  // message
    Name,                     // name
    Next,                     // next
    Prototype,                // prototype
    QueuedAtTeardown,         // queued_at_teardown
    Raw,                      // raw
    RawJSON,                  // rawJSON
    Resolve,                  // resolve
    Return,                   // return
    Set,                      // set
    Source,                   // source
    Stack,                    // stack
    Then,                     // then
    Throw,                    // throw
    ToISOString,              // toISOString
    ToJSON,                   // toJSON
    ToLocaleString,           // toLocaleString
    ToString,                 // toString
    ToUTCString,              // toUTCString
    Unicode,                  // unicode
    Value,                    // value
    ValueOf,                  // valueOf
    Values,                   // values
    With,                     // with
    X,                        // x
    OwnKeys,                  // ownKeys
    GetOwnPropertyDescriptor, // getOwnPropertyDescriptor
    DefineProperty,           // defineProperty
    SetPrototypeOf,           // setPrototypeOf
    IsExtensible,             // isExtensible
    PreventExtensions,        // preventExtensions
}
pub(crate) struct PinnedAtoms([Atom; 80]);
impl PinnedAtoms {
    pub(crate) fn new(atoms: &mut AtomTable) -> Result<Self, AtomError> {
        Ok(Self([
            atoms.intern_static("")?,
            atoms.intern_static("0")?,
            atoms.intern_static("1")?,
            atoms.intern_static("2")?,
            atoms.intern_static("2047")?,
            atoms.intern_static("5")?,
            atoms.intern_static("<this>")?,
            atoms.intern_static("Atomics")?,
            atoms.intern_static("JSON")?,
            atoms.intern_static("Math")?,
            atoms.intern_static("Object")?,
            atoms.intern_static("Reflect")?,
            atoms.intern_static("[unsupported type]")?,
            atoms.intern_static("__json5Value")?,
            atoms.intern_static("__proto__")?,
            atoms.intern_static("answer")?,
            atoms.intern_static("apply")?,
            atoms.intern_static("assign")?,
            atoms.intern_static("batch_first")?,
            atoms.intern_static("batch_realm")?,
            atoms.intern_static("buffer")?,
            atoms.intern_static("callee")?,
            atoms.intern_static("cause")?,
            atoms.intern_static("construct")?,
            atoms.intern_static("constructor")?,
            atoms.intern_static("count")?,
            atoms.intern_static("create")?,
            atoms.intern_static("deleteProperty")?,
            atoms.intern_static("done")?,
            atoms.intern_static("entries")?,
            atoms.intern_static("exec")?,
            atoms.intern_static("flags")?,
            atoms.intern_static("fromEntries")?,
            atoms.intern_static("get")?,
            atoms.intern_static("getPrototypeOf")?,
            atoms.intern_static("global")?,
            atoms.intern_static("globalThis")?,
            atoms.intern_static("groups")?,
            atoms.intern_static("has")?,
            atoms.intern_static("hasOwn")?,
            atoms.intern_static("index")?,
            atoms.intern_static("input")?,
            atoms.intern_static("is")?,
            atoms.intern_static("key")?,
            atoms.intern_static("keys")?,
            atoms.intern_static("lastIndex")?,
            atoms.intern_static("length")?,
            atoms.intern_static("load")?,
            atoms.intern_static("maxByteLength")?,
            atoms.intern_static("message")?,
            atoms.intern_static("name")?,
            atoms.intern_static("next")?,
            atoms.intern_static("prototype")?,
            atoms.intern_static("queued_at_teardown")?,
            atoms.intern_static("raw")?,
            atoms.intern_static("rawJSON")?,
            atoms.intern_static("resolve")?,
            atoms.intern_static("return")?,
            atoms.intern_static("set")?,
            atoms.intern_static("source")?,
            atoms.intern_static("stack")?,
            atoms.intern_static("then")?,
            atoms.intern_static("throw")?,
            atoms.intern_static("toISOString")?,
            atoms.intern_static("toJSON")?,
            atoms.intern_static("toLocaleString")?,
            atoms.intern_static("toString")?,
            atoms.intern_static("toUTCString")?,
            atoms.intern_static("unicode")?,
            atoms.intern_static("value")?,
            atoms.intern_static("valueOf")?,
            atoms.intern_static("values")?,
            atoms.intern_static("with")?,
            atoms.intern_static("x")?,
            atoms.intern_static("ownKeys")?,
            atoms.intern_static("getOwnPropertyDescriptor")?,
            atoms.intern_static("defineProperty")?,
            atoms.intern_static("setPrototypeOf")?,
            atoms.intern_static("isExtensible")?,
            atoms.intern_static("preventExtensions")?,
        ]))
    }
    pub(crate) fn get(&self, key: PinnedAtom) -> Atom {
        self.0[key as usize]
    }
}

/// Number of ECMAScript Proxy internal methods, one cache slot per trap.
pub(crate) const PROXY_METHOD_COUNT: usize = 13;

impl PinnedAtom {
    /// Return the pinned trap atom and its stable cache index. The two values
    /// come from one match so the closed selector cannot diverge from the
    /// `proxy_trap_reads` array layout.
    pub(crate) fn proxy_method(name: &str) -> (Self, usize) {
        match name {
            "get" => (Self::Get, 0),
            "set" => (Self::Set, 1),
            "has" => (Self::Has, 2),
            "apply" => (Self::Apply, 3),
            "construct" => (Self::Construct, 4),
            "deleteProperty" => (Self::DeleteProperty, 5),
            "getPrototypeOf" => (Self::GetPrototypeOf, 6),
            "setPrototypeOf" => (Self::SetPrototypeOf, 7),
            "getOwnPropertyDescriptor" => (Self::GetOwnPropertyDescriptor, 8),
            "defineProperty" => (Self::DefineProperty, 9),
            "ownKeys" => (Self::OwnKeys, 10),
            "isExtensible" => (Self::IsExtensible, 11),
            "preventExtensions" => (Self::PreventExtensions, 12),
            _ => unreachable!("proxy method name is a closed internal selector"),
        }
    }
}

#[cfg(test)]
mod proxy_method_tests {
    use super::*;

    #[test]
    fn trap_selector_covers_every_cache_slot_exactly_once() {
        let names = [
            "get",
            "set",
            "has",
            "apply",
            "construct",
            "deleteProperty",
            "getPrototypeOf",
            "setPrototypeOf",
            "getOwnPropertyDescriptor",
            "defineProperty",
            "ownKeys",
            "isExtensible",
            "preventExtensions",
        ];
        assert_eq!(names.len(), PROXY_METHOD_COUNT);
        let mut seen = [false; PROXY_METHOD_COUNT];
        for name in names {
            let (_, index) = PinnedAtom::proxy_method(name);
            assert!(index < PROXY_METHOD_COUNT);
            assert!(!seen[index], "duplicate trap index for {name}");
            seen[index] = true;
        }
        assert!(seen.into_iter().all(|slot| slot));
    }
}

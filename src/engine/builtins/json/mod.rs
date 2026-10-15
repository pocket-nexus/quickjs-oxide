//! Pinned QuickJS `JSON` intrinsic algorithms.
//!
//! The global object table is installed in the exact `js_json_funcs` order so
//! later stringify and Raw JSON slices do not have to mutate observable own-
//! key order.  Strict parsing and reviver internalization live in separate
//! modules because their allocation and abrupt-completion boundaries are
//! independently observable.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::ObjectId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::object::builtin_properties::NativeBuiltinProperty;
use std::cell::Cell;

use crate::engine::builtins::native::{JsonNativeKind, NativeFunctionId};
use crate::engine::heap::{AutoInitProperty, ContextId, PropertySlot};
use crate::engine::object::ObjectRef;
use crate::engine::object::shape::PropertyFlags;
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

mod parse;
mod raw;
mod reviver;
mod stringify;

#[cfg(test)]
mod tests;

impl Runtime {
    /// Install QuickJS's lazy global `JSON` object after `%RegExp%`.
    pub(crate) fn initialize_json_intrinsic(
        &self,
        realm: ContextId,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Json)?;
        self.store_property_slot(
            global_object,
            &key,
            PropertyFlags::data(true, false, true),
            PropertySlot::auto_init(AutoInitProperty::Json { realm }),
        )
    }

    pub(crate) fn call_json_native(
        &self,
        realm: ContextId,
        kind: JsonNativeKind,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "JSON method did not receive a generic invocation",
            ));
        };
        invocation.release(self)?;
        match kind {
            JsonNativeKind::IsRawJson => self.call_json_is_raw_json(arguments),
            JsonNativeKind::Parse => self.call_json_parse(realm, arguments),
            JsonNativeKind::RawJson => self.call_json_raw_json(realm, arguments),
            JsonNativeKind::Stringify => self.call_json_stringify(realm, arguments),
        }
    }
}

pub(crate) use raw::RawResume as JsonRawResume;

pub(crate) use reviver::{ParseResume as JsonParseResume, ParseStep as JsonParseStep};

pub(crate) use stringify::{
    StringifyResume as JsonStringifyResume, StringifyStep as JsonStringifyStep,
};

impl RuntimeState {
    /// Materialize the complete pinned `js_json_funcs` property table.
    ///
    /// Stringify and Raw JSON keep honest typed frontiers until their bounded
    /// milestones land, but their callable identities are reserved now so the
    /// final object graph and own-key order need no migration.
    pub(crate) fn instantiate_json_intrinsic(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
    ) -> Result<ObjectId, RuntimeError> {
        use JsonNativeKind as K;
        const fn m(kind: JsonNativeKind, name: &'static str, length: u8) -> NativeBuiltinProperty {
            NativeBuiltinProperty::new(NativeFunctionId::Json(kind), name, length, length)
        }
        const METHODS: &[NativeBuiltinProperty] = &[
            m(K::IsRawJson, "isRawJSON", 1),
            m(K::Parse, "parse", 2),
            m(K::RawJson, "rawJSON", 1),
            m(K::Stringify, "stringify", 3),
        ];
        self.instantiate_namespace_object(poisoned, realm, METHODS, "JSON", &[])
    }
}

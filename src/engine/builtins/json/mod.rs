//! Pinned QuickJS `JSON` intrinsic algorithms.
//!
//! The global object table is installed in the exact `js_json_funcs` order so
//! later stringify and Raw JSON slices do not have to mutate observable own-
//! key order.  Strict parsing and reviver internalization live in separate
//! modules because their allocation and abrupt-completion boundaries are
//! independently observable.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{ObjectId, RawValue};
use std::cell::Cell;

use crate::engine::builtins::native::{JsonNativeKind, NativeFunctionId};
use crate::engine::heap::{AutoInitProperty, ContextId, PropertySlot};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{ObjectRef, WellKnownSymbol};
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::value::{JsString, JsValue};
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
        let json = self.new_ordinary_object_in_realm(poisoned, realm)?;
        let mut result_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(json));
        let (state, result_owner) = result_owner.parts();
        for (kind, name, length) in [
            (JsonNativeKind::IsRawJson, "isRawJSON", 1),
            (JsonNativeKind::Parse, "parse", 2),
            (JsonNativeKind::RawJson, "rawJSON", 1),
            (JsonNativeKind::Stringify, "stringify", 3),
        ] {
            state.define_native_builtin_auto_init(
                poisoned,
                json,
                realm,
                NativeFunctionId::Json(kind),
                name,
                length,
                length,
            )?;
        }

        let to_string_tag = state.well_known_symbols[&WellKnownSymbol::ToStringTag];
        {
            let string = state.heap.allocate_string(JsString::from_static("JSON"))?;
            let mut producer = OwnedValueGuard::new(state, poisoned, JsValue::String(string));
            let (state, producer) = producer.parts();
            if !state.define_raw_property_with_poison(
                poisoned,
                json,
                to_string_tag,
                &crate::engine::object::property::PropertyDescriptor {
                    value: Some(RawValue::String(string)),
                    writable: Some(false),
                    enumerable: Some(false),
                    configurable: Some(true),
                    ..crate::engine::object::property::PropertyDescriptor::new()
                },
            )? {
                return Err(RuntimeError::Invariant(
                    "JSON toStringTag definition was rejected",
                ));
            }
            state.release_owned_jsvalue(
                poisoned,
                producer.take().expect("intrinsic tag producer"),
            )?;
        }
        let JsValue::Object(object) = result_owner.take().expect("intrinsic factory result") else {
            unreachable!("intrinsic factory allocated an object")
        };
        Ok(object)
    }
}

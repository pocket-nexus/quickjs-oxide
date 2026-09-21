//! Object-literal method publication.
//!
//! QuickJS keeps this operation separate from ordinary field definition
//! because it must infer the closure name and choose a data or accessor
//! descriptor without exposing either decision to JavaScript code.

pub(crate) mod element;

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::atom::{AtomSpelling, PropertyKeyKind};
use crate::engine::code::bytecode::DefineMethodKind;
#[cfg(test)]
use crate::engine::heap::ContextId;
#[cfg(test)]
use crate::engine::object::operations::PropertyDefineOutcome;

use crate::engine::object::{
    AccessorValue, DescriptorField, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
};
#[cfg(test)]
use crate::engine::value::Value;
use crate::engine::value::{JsString, JsValue};

impl Runtime {
    #[cfg(test)]
    pub(crate) fn define_object_literal_method(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
        function: Value,
        kind: DefineMethodKind,
        enumerable: bool,
    ) -> Result<PropertyDefineOutcome, RuntimeError> {
        let function = self.into_jsvalue(function)?;
        let descriptor =
            self.prepare_object_literal_method(object, key, &function, kind, enumerable);
        self.release_jsvalue(function)?;
        self.define_owned_property_in_realm(Some(realm), object, key, &descriptor?)
    }

    /// Naming and HomeObject publication use context-free own storage and do
    /// not invoke Proxy traps. The final exotic definition may still coerce a
    /// data value (Array length or TypedArray index) and is a separate request.
    pub(crate) fn prepare_object_literal_method(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        function: &JsValue,
        kind: DefineMethodKind,
        enumerable: bool,
    ) -> Result<OwnedPropertyDescriptor, RuntimeError> {
        let callable = self.callable_from_jsvalue(function)?;
        let name = self.object_literal_method_name(key, kind)?;
        self.define_object_name_for_object(callable.as_object(), &name)?;
        self.install_object_literal_home_object(&callable, object)?;

        let mut descriptor = OwnedPropertyDescriptor::new(self);
        descriptor.enumerable = DescriptorField::Present(enumerable);
        descriptor.configurable = DescriptorField::Present(true);
        match kind {
            DefineMethodKind::Method => {
                descriptor.value = DescriptorField::Present(self.dup_jsvalue(function)?);
                descriptor.writable = DescriptorField::Present(true);
            }
            DefineMethodKind::Getter => {
                descriptor.get = DescriptorField::Present(AccessorValue::Callable(callable));
            }
            DefineMethodKind::Setter => {
                descriptor.set = DescriptorField::Present(AccessorValue::Callable(callable));
            }
        }

        Ok(descriptor)
    }

    fn object_literal_method_name(
        &self,
        key: &PropertyKey,
        kind: DefineMethodKind,
    ) -> Result<JsString, RuntimeError> {
        self.validate_object_literal_key(key)?;
        let key_name = {
            let state = self.0.state.borrow();
            match state.atoms.property_key_kind(key.atom())? {
                PropertyKeyKind::String => state.atoms.to_js_string(key.atom())?,
                PropertyKeyKind::Symbol => match state.atoms.resolve(key.atom())?.spelling {
                    // A Symbol without a description gives methods the empty
                    // name; an explicitly empty description remains `[]`.
                    AtomSpelling::NoDescription => JsString::from_static(""),
                    AtomSpelling::Text(description) => JsString::from_static("[")
                        .try_concat(description)?
                        .try_concat(&JsString::from_static("]"))?,
                    AtomSpelling::Integer(_) => {
                        return Err(RuntimeError::Invariant(
                            "symbol property key had an integer spelling",
                        ));
                    }
                },
                PropertyKeyKind::Private => {
                    return Err(RuntimeError::Invariant(
                        "object literal method used a private property key",
                    ));
                }
            }
        };

        let prefix = match kind {
            DefineMethodKind::Method => return Ok(key_name),
            DefineMethodKind::Getter => "get ",
            DefineMethodKind::Setter => "set ",
        };
        Ok(JsString::from_static(prefix).try_concat(&key_name)?)
    }

    fn validate_object_literal_key(&self, key: &PropertyKey) -> Result<(), RuntimeError> {
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object literal method key"));
        }
        Ok(())
    }
}

//! `%RegExp%` construction and derived allocation.
//!
//! The ordering follows pinned QuickJS `js_regexp_constructor`, not a
//! rearranged specification sketch.  In particular `IsRegExp` runs first,
//! pattern conversion precedes the derived `.prototype` lookup, and flags are
//! converted only after the branded object has been allocated.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::AtomIdx;

use crate::engine::heap::{
    ContextId, ObjectData, ObjectPayload, PropertySlot, RawValue, RegExpObjectData, RegExpRealmData,
};
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use crate::engine::object::{ObjectRef, PropertyKey, WellKnownSymbol};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue};
use crate::engine::vm::call::{
    ConstructorPrototypeSource, ConstructorRef, NativeArguments, NativeInvocation,
    prototype::{ProtoSourceStep, finish as finish_source},
};
use crate::engine::vm::{Completion, ToPrimitiveHint};
use crate::regexp::CompiledRegExp;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct GenuineRegExp {
    pub(crate) pattern: JsString,
    pub(crate) program: Rc<CompiledRegExp>,
}

impl Runtime {
    /// Pinned QuickJS `JS_SpeciesConstructor` specialized with this native
    /// method's defining-realm retained `%RegExp%` constructor as the default.
    pub(crate) fn regexp_species_constructor(
        &self,
        realm: ContextId,
        regexp: &ObjectRef,
    ) -> Result<NativeConversion<ConstructorRef>, RuntimeError> {
        let mut step = super::species::RegExpSpeciesStep::start(self, realm, regexp.clone())?;
        loop {
            step = match step {
                super::species::RegExpSpeciesStep::Complete(result) => return Ok(result),
                super::species::RegExpSpeciesStep::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(
                    self,
                    self.internal_get_jsvalue(
                        realm,
                        &object,
                        &key,
                        JsValue::Object(object.clone().into_handle()),
                    )?,
                )?,
            };
        }
    }

    pub(crate) fn call_regexp_constructor(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        self.dispatch_borrowed_invocation(invocation, |invocation| {
            finish_constructor(
                self,
                realm,
                RegExpConstructorStep::start(self, realm, invocation, arguments)?,
            )
        })
    }

    pub(crate) fn compile_regexp_program(
        pattern: &JsString,
        flags: &JsString,
    ) -> Result<Rc<CompiledRegExp>, RuntimeError> {
        match crate::regexp::compile(pattern, flags) {
            Ok(program) => Ok(Rc::new(program)),
            Err(error) => {
                let kind = crate::regexp::javascript_compile_error_kind(&error);
                let message = crate::regexp::javascript_compile_error_message(&error).to_owned();
                Err(RuntimeError::Engine(Error::new(kind, message)))
            }
        }
    }

    pub(crate) fn call_regexp_species(
        &self,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Getter { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp species did not receive a getter invocation",
            ));
        };
        Ok(Completion::Return(self.dup_jsvalue(this_value)?))
    }

    pub(crate) fn genuine_regexp_jsvalue(
        &self,
        value: &JsValue,
    ) -> Result<Option<GenuineRegExp>, RuntimeError> {
        let JsValue::Object(object) = value else {
            return Ok(None);
        };
        let state = self.0.state.borrow();
        Ok(match &state.heap.object(*object)?.payload {
            ObjectPayload::RegExp(RegExpObjectData::Compiled { pattern, program }) => {
                Some(GenuineRegExp {
                    pattern: pattern.clone(),
                    program: program.clone(),
                })
            }
            ObjectPayload::RegExp(RegExpObjectData::Uninitialized) => {
                return Err(RuntimeError::Invariant(
                    "observable RegExp object was not initialized",
                ));
            }
            ObjectPayload::Ordinary
            | ObjectPayload::Proxy(_)
            | ObjectPayload::RawJson
            | ObjectPayload::Promise(_)
            | ObjectPayload::Array { .. }
            | ObjectPayload::Arguments { .. }
            | ObjectPayload::ArrayIterator { .. }
            | ObjectPayload::IteratorHelper(_)
            | ObjectPayload::IteratorWrap(_)
            | ObjectPayload::AsyncFromSyncIterator(_)
            | ObjectPayload::IteratorConcat(_)
            | ObjectPayload::Map { .. }
            | ObjectPayload::MapIterator { .. }
            | ObjectPayload::Set { .. }
            | ObjectPayload::WeakMap { .. }
            | ObjectPayload::WeakSet { .. }
            | ObjectPayload::WeakRef { .. }
            | ObjectPayload::FinalizationRegistry(_)
            | ObjectPayload::SetIterator { .. }
            | ObjectPayload::ForInIterator(_)
            | ObjectPayload::Primitive(_)
            | ObjectPayload::Date(_)
            | ObjectPayload::ArrayBuffer(_)
            | ObjectPayload::SharedArrayBuffer(_)
            | ObjectPayload::DataView(_)
            | ObjectPayload::TypedArray(_)
            | ObjectPayload::GlobalObject { .. }
            | ObjectPayload::Error
            | ObjectPayload::StringIterator { .. }
            | ObjectPayload::RegExpStringIterator { .. }
            | ObjectPayload::NativeFunction { .. }
            | ObjectPayload::BoundFunction { .. }
            | ObjectPayload::BytecodeFunction { .. }
            | ObjectPayload::AsyncFunctionState(_)
            | ObjectPayload::Generator { .. }
            | ObjectPayload::AsyncGenerator(_) => None,
        })
    }

    fn new_uninitialized_regexp(&self, prototype: &ObjectRef) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("RegExp prototype"));
        }
        let last_index =
            self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::LastIndex)?;
        let entries = [ShapeEntry {
            atom: AtomIdx::from_raw(last_index.atom().raw()),
            flags: PropertyFlags::data(true, false, false),
        }];
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &entries)?;
        let object = match state.heap.allocate_object(ObjectData::regexp(
            shape,
            vec![PropertySlot::Data(RawValue::Int(0))],
        )) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    fn publish_regexp(
        &self,
        object: &ObjectRef,
        pattern: JsString,
        program: Rc<CompiledRegExp>,
    ) -> Result<(), RuntimeError> {
        let previous = self.0.state.borrow_mut().heap.replace_regexp_data(
            object.object_id(),
            RegExpObjectData::Compiled { pattern, program },
        )?;
        if !matches!(previous, RegExpObjectData::Uninitialized) {
            return Err(RuntimeError::Invariant(
                "fresh RegExp object already had compiled data",
            ));
        }
        Ok(())
    }

    pub(crate) fn regexp_realm_data(
        &self,
        realm: ContextId,
    ) -> Result<RegExpRealmData, RuntimeError> {
        self.0
            .state
            .borrow()
            .heap
            .context(realm)?
            .regexp
            .as_ref()
            .copied()
            .ok_or(RuntimeError::Invariant("realm has no RegExp intrinsic"))
    }

    /// QuickJS `OP_regexp`: instantiate one already-compiled literal using
    /// the bytecode realm's canonical RegExp shape. This path intentionally
    /// performs no `Get` on the global constructor or its mutable `prototype`
    /// property and therefore cannot invoke user code.
    pub(crate) fn new_compiled_regexp_literal(
        &self,
        realm: ContextId,
        pattern: JsString,
        program: Rc<CompiledRegExp>,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        let shape = self.regexp_realm_data(realm)?.object_shape;
        let object =
            self.0
                .state
                .borrow_mut()
                .heap
                .allocate_object(ObjectData::compiled_regexp(
                    shape,
                    vec![PropertySlot::Data(RawValue::Int(0))],
                    pattern,
                    program,
                ))?;
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }
}

pub(crate) enum RegExpConstructorStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpConstructorResume,
    },
    Primitive {
        value: JsValue,
        resume: RegExpConstructorResume,
    },
    Prototype {
        new_target: JsValue,
        resume: RegExpConstructorResume,
    },
}
pub(crate) struct RegExpConstructorResume(Box<RegExpConstructorResumeState>);
impl std::ops::Deref for RegExpConstructorResume {
    type Target = RegExpConstructorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RegExpConstructorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<RegExpConstructorResume>() <= 8);
pub(crate) struct RegExpConstructorResumeState {
    runtime: Runtime,
    realm: ContextId,
    new_target: JsValue,
    pattern: JsValue,
    flags: JsValue,
    source: JsValue,
    conversion_flags: JsValue,
    reply: JsValue,
    is_regexp: bool,
    phase: RegExpConstructorPhase,
}
enum RegExpConstructorPhase {
    Match,
    Identity(ObjectRef),
    Source,
    SourceFlags,
    Pattern,
    Prototype(RegExpPublication),
    Flags {
        object: ObjectRef,
        pattern: JsString,
    },
}
enum RegExpPublication {
    Copy(GenuineRegExp),
    Compile { pattern: JsString },
}
impl Drop for RegExpConstructorResumeState {
    fn drop(&mut self) {
        for value in [
            &mut self.new_target,
            &mut self.pattern,
            &mut self.flags,
            &mut self.source,
            &mut self.conversion_flags,
            &mut self.reply,
        ] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
impl RegExpConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp constructor did not receive constructor-or-function invocation",
            ));
        };
        let mut resume = RegExpConstructorResume(Box::new(RegExpConstructorResumeState {
            runtime: runtime.clone(),
            realm,
            new_target: JsValue::Undefined,
            pattern: JsValue::Undefined,
            flags: JsValue::Undefined,
            source: JsValue::Undefined,
            conversion_flags: JsValue::Undefined,
            reply: JsValue::Undefined,
            is_regexp: false,
            phase: RegExpConstructorPhase::Match,
        }));
        resume.0.new_target = runtime.dup_jsvalue(new_target)?;
        resume.0.pattern = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("RegExp constructor pattern argv was not padded"),
        )?)?;
        resume.0.flags = runtime.dup_jsvalue(arguments.readable.get(1).ok_or(
            RuntimeError::Invariant("RegExp constructor flags argv was not padded"),
        )?)?;
        if let JsValue::Object(id) = &resume.pattern {
            Ok(Self::Read {
                object: ObjectRef::from_borrowed_handle(runtime.clone(), *id)?,
                key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Match)),
                resume,
            })
        } else {
            resume.checked(runtime, false)
        }
    }
}
impl RegExpConstructorResume {
    fn take_reply(&mut self) -> JsValue {
        std::mem::replace(&mut self.0.reply, JsValue::Undefined)
    }
    fn checked(
        mut self,
        runtime: &Runtime,
        is_regexp: bool,
    ) -> Result<RegExpConstructorStep, RuntimeError> {
        self.0.is_regexp = is_regexp;
        if matches!(self.0.new_target, JsValue::Undefined) {
            let active = runtime.active_function()?;
            self.0.new_target = JsValue::Object(active.clone().into_handle());
            if is_regexp && matches!(self.0.flags, JsValue::Undefined) {
                let JsValue::Object(id) = &self.0.pattern else {
                    return Err(RuntimeError::Invariant(
                        "IsRegExp accepted a primitive pattern",
                    ));
                };
                let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
                let key = runtime
                    .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Constructor)?;
                self.0.phase = RegExpConstructorPhase::Identity(active);
                return Ok(RegExpConstructorStep::Read {
                    object,
                    key,
                    resume: self,
                });
            }
        }
        self.prepare(runtime)
    }
    fn prepare(mut self, runtime: &Runtime) -> Result<RegExpConstructorStep, RuntimeError> {
        let genuine = runtime.genuine_regexp_jsvalue(&self.0.pattern)?;
        if matches!(self.0.flags, JsValue::Undefined)
            && let Some(genuine) = genuine.as_ref()
        {
            return self.lookup(runtime, RegExpPublication::Copy(genuine.clone()));
        }
        if let Some(genuine) = genuine {
            self.0.conversion_flags = runtime.dup_jsvalue(&self.0.flags)?;
            // The compiled payload already is text; no observable conversion is needed.
            self.lookup(
                runtime,
                RegExpPublication::Compile {
                    pattern: genuine.pattern.linearize(),
                },
            )
        } else if self.0.is_regexp {
            let JsValue::Object(id) = &self.0.pattern else {
                return Err(RuntimeError::Invariant(
                    "IsRegExp accepted a primitive pattern",
                ));
            };
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
            let key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Source)?;
            self.0.phase = RegExpConstructorPhase::Source;
            Ok(RegExpConstructorStep::Read {
                object,
                key,
                resume: self,
            })
        } else {
            self.0.source = runtime.dup_jsvalue(&self.0.pattern)?;
            self.0.conversion_flags = runtime.dup_jsvalue(&self.0.flags)?;
            self.pattern_value(runtime)
        }
    }
    fn pattern_value(mut self, runtime: &Runtime) -> Result<RegExpConstructorStep, RuntimeError> {
        if matches!(self.0.source, JsValue::Undefined) {
            self.lookup(
                runtime,
                RegExpPublication::Compile {
                    pattern: JsString::from_static(""),
                },
            )
        } else {
            self.0.phase = RegExpConstructorPhase::Pattern;
            let value = std::mem::replace(&mut self.0.source, JsValue::Undefined);
            Ok(RegExpConstructorStep::Primitive {
                value,
                resume: self,
            })
        }
    }
    fn lookup(
        mut self,
        runtime: &Runtime,
        publication: RegExpPublication,
    ) -> Result<RegExpConstructorStep, RuntimeError> {
        let new_target = runtime.dup_jsvalue(&self.0.new_target)?;
        self.0.phase = RegExpConstructorPhase::Prototype(publication);
        Ok(RegExpConstructorStep::Prototype {
            new_target,
            resume: self,
        })
    }
    fn publish(
        runtime: &Runtime,
        object: ObjectRef,
        pattern: JsString,
        flags: JsString,
    ) -> Result<RegExpConstructorStep, RuntimeError> {
        let program = Runtime::compile_regexp_program(&pattern, &flags)?;
        runtime.publish_regexp(&object, pattern, program)?;
        Ok(RegExpConstructorStep::Complete(Completion::Return(
            JsValue::Object(object.into_handle()),
        )))
    }
    pub(crate) fn prototype(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<RegExpConstructorStep, RuntimeError> {
        let prototype = match result {
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(value)) => value,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                ObjectRef::from_borrowed_handle(
                    runtime.clone(),
                    runtime.regexp_realm_data(realm)?.prototype,
                )?
            }
            NativeConversion::Throw(value) => {
                return Ok(RegExpConstructorStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
        };
        let object = runtime.new_uninitialized_regexp(&prototype)?;
        let RegExpConstructorPhase::Prototype(publication) =
            std::mem::replace(&mut self.0.phase, RegExpConstructorPhase::Match)
        else {
            return Err(RuntimeError::Invariant(
                "RegExp constructor received an unexpected prototype reply",
            ));
        };
        match publication {
            RegExpPublication::Copy(genuine) => {
                runtime.publish_regexp(&object, genuine.pattern, genuine.program)?;
                Ok(RegExpConstructorStep::Complete(Completion::Return(
                    JsValue::Object(object.into_handle()),
                )))
            }
            RegExpPublication::Compile { pattern } => {
                if matches!(self.0.conversion_flags, JsValue::Undefined) {
                    Self::publish(runtime, object, pattern, JsString::from_static(""))
                } else {
                    self.0.phase = RegExpConstructorPhase::Flags { object, pattern };
                    let value = std::mem::replace(&mut self.0.conversion_flags, JsValue::Undefined);
                    Ok(RegExpConstructorStep::Primitive {
                        value,
                        resume: self,
                    })
                }
            }
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpConstructorStep, RuntimeError> {
        match result {
            Completion::Return(value) => self.0.reply = value,
            Completion::Throw(value) => {
                return Ok(RegExpConstructorStep::Complete(Completion::Throw(value)));
            }
        }
        match std::mem::replace(&mut self.0.phase, RegExpConstructorPhase::Match) {
            RegExpConstructorPhase::Match => {
                let JsValue::Object(id) = &self.0.pattern else {
                    return Err(RuntimeError::Invariant(
                        "RegExp match check lost its object",
                    ));
                };
                let is_regexp = if matches!(self.0.reply, JsValue::Undefined) {
                    runtime.native_object_has_regexp_brand(&ObjectRef::from_borrowed_handle(
                        runtime.clone(),
                        *id,
                    )?)
                } else {
                    runtime.value_to_boolean_jsvalue(&self.0.reply)
                };
                runtime.release_jsvalue(self.take_reply())?;
                self.checked(runtime, is_regexp?)
            }
            RegExpConstructorPhase::Identity(active) => {
                let same =
                    matches!(&self.0.reply, JsValue::Object(id) if *id == active.object_id());
                runtime.release_jsvalue(self.take_reply())?;
                if same {
                    Ok(RegExpConstructorStep::Complete(Completion::Return(
                        std::mem::replace(&mut self.0.pattern, JsValue::Undefined),
                    )))
                } else {
                    self.prepare(runtime)
                }
            }
            RegExpConstructorPhase::Source => {
                self.0.source = self.take_reply();
                if matches!(self.0.flags, JsValue::Undefined) {
                    let JsValue::Object(id) = &self.0.pattern else {
                        return Err(RuntimeError::Invariant(
                            "RegExp source lookup lost its object",
                        ));
                    };
                    let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
                    let key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Flags)?;
                    self.0.phase = RegExpConstructorPhase::SourceFlags;
                    Ok(RegExpConstructorStep::Read {
                        object,
                        key,
                        resume: self,
                    })
                } else {
                    self.0.conversion_flags = runtime.dup_jsvalue(&self.0.flags)?;
                    self.pattern_value(runtime)
                }
            }
            RegExpConstructorPhase::SourceFlags => {
                self.0.conversion_flags = self.take_reply();
                self.pattern_value(runtime)
            }
            RegExpConstructorPhase::Pattern => {
                if matches!(self.0.reply, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp pattern conversion returned an object",
                    ));
                }
                let result = runtime.string_from_primitive_jsvalue(self.0.realm, &self.0.reply);
                runtime.release_jsvalue(self.take_reply())?;
                let pattern = match result? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpConstructorStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                self.lookup(runtime, RegExpPublication::Compile { pattern })
            }
            RegExpConstructorPhase::Flags { object, pattern } => {
                if matches!(self.0.reply, JsValue::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp flags conversion returned an object",
                    ));
                }
                let result = runtime.string_from_primitive_jsvalue(self.0.realm, &self.0.reply);
                runtime.release_jsvalue(self.take_reply())?;
                let flags = match result? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpConstructorStep::Complete(Completion::Throw(
                            runtime.into_jsvalue(value)?,
                        )));
                    }
                };
                Self::publish(runtime, object, pattern, flags)
            }
            RegExpConstructorPhase::Prototype(_) => Err(RuntimeError::Invariant(
                "RegExp prototype request received an untyped reply",
            )),
        }
    }
}
fn finish_constructor(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            RegExpConstructorStep::Complete(result) => return Ok(result),
            RegExpConstructorStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get_jsvalue(
                    realm,
                    &object,
                    &key,
                    JsValue::Object(object.clone().into_handle()),
                )?,
            )?,
            RegExpConstructorStep::Primitive { value, resume } => {
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::String)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            RegExpConstructorStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                finish_source(
                    runtime,
                    realm,
                    ProtoSourceStep::start(runtime, realm, new_target)?,
                )?,
            )?,
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<RegExpConstructorStep>() <= 64);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::Value;
    #[test]
    fn unpublished_regexp_is_rooted_during_flags_conversion_and_reclaimed_on_abandonment() {
        let runtime = Runtime::new();
        let weak = Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let new_target = context.eval("(function(){})").unwrap();
        let flags = runtime.new_object(None).unwrap();
        let flags_id = flags.object_id();
        let invocation = NativeInvocation::Construct {
            new_target: runtime.into_jsvalue(new_target).unwrap(),
        };
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![
                runtime
                    .into_jsvalue(Value::String(JsString::from_static("a")))
                    .unwrap(),
                runtime.into_jsvalue(Value::Object(flags)).unwrap(),
            ],
        };
        let RegExpConstructorStep::Primitive { value, resume } =
            RegExpConstructorStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
        else {
            panic!("expected pattern conversion")
        };
        runtime.release_jsvalue(value).unwrap();
        {
            let NativeInvocation::Construct { new_target } = invocation else {
                unreachable!()
            };
            runtime.release_jsvalue(new_target).unwrap();
            for value in arguments.readable {
                runtime.release_jsvalue(value).unwrap();
            }
        }
        let RegExpConstructorStep::Prototype { new_target, resume } = resume
            .resume(
                &runtime,
                Completion::Return(
                    runtime
                        .into_jsvalue(Value::String(JsString::from_static("a")))
                        .unwrap(),
                ),
            )
            .unwrap()
        else {
            panic!("expected prototype request")
        };
        runtime.release_jsvalue(new_target).unwrap();
        let prototype = runtime.new_object(None).unwrap();
        let prototype_id = prototype.object_id();
        let RegExpConstructorStep::Primitive { value, resume } = resume
            .prototype(
                &runtime,
                NativeConversion::Value(ConstructorPrototypeSource::Explicit(prototype)),
            )
            .unwrap()
        else {
            panic!("expected flags conversion")
        };
        runtime.release_jsvalue(value).unwrap();
        let RegExpConstructorPhase::Flags { object, .. } = &resume.phase else {
            panic!("expected unpublished result")
        };
        let object_id = object.object_id();
        runtime.run_gc().unwrap();
        for id in [object_id, prototype_id, flags_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        assert!(matches!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(object_id)
                .unwrap()
                .payload,
            ObjectPayload::RegExp(RegExpObjectData::Uninitialized)
        ));
        drop(resume);
        runtime.run_gc().unwrap();
        for id in [object_id, prototype_id, flags_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

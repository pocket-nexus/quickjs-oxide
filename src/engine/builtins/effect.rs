//! Effects a resumable builtin asks its driver to perform, and their replies.
//!
//! A builtin that may call back into JavaScript returns an [`EffectStep`]. Its
//! resume state implements [`Continuation`]: it parks one [`Effect`] in
//! [`Continuation::pending`], and receives the effect's [`Reply`] in
//! [`Continuation::reply`]. [`finish`] drives a step synchronously; the VM
//! converts the same step into its own scheduler step. See
//! `docs/native-effects.md`.

use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        array::species,
        iterator::step::{CloseStep, NextStep, finish_close, finish_next},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{
        CallableRef, ObjectRef, OwnedPropertyDescriptor, PropertyKey,
        operations::{InternalDefineResult, InternalSetResult},
    },
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{ConstructNewTarget, ConstructorRef},
    },
};

pub(crate) enum Effect {
    /// `[[Get]]` on an object, with the object as receiver.
    Read {
        object: ObjectRef,
        key: PropertyKey,
    },
    ReadValue {
        receiver: JsValue,
        key: PropertyKey,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
    },
    /// ArraySpeciesCreate(source, length).
    Species {
        source: ObjectRef,
        length: u64,
    },
    Number {
        value: JsValue,
    },
    Call {
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
    },
    Construct {
        target: ConstructorRef,
        arguments: Vec<JsValue>,
    },
    ParseIterator {
        result: Completion,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        /// Boxed: the descriptor would double the size of every effect.
        descriptor: Box<OwnedPropertyDescriptor>,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
    },
}

impl Effect {
    /// Release the value handles of an effect that will not be performed.
    /// Object and key handles release themselves on drop.
    pub(crate) fn release(self, runtime: &Runtime) {
        let release_all = |values: Vec<JsValue>| {
            for value in values {
                let _ = runtime.release_jsvalue(value);
            }
        };
        match self {
            Self::ReadValue {
                receiver: value, ..
            }
            | Self::Number { value }
            | Self::Set { value, .. }
            | Self::ParseIterator {
                result: Completion::Return(value) | Completion::Throw(value),
            } => {
                let _ = runtime.release_jsvalue(value);
            }
            Self::Call {
                receiver,
                arguments,
                ..
            } => {
                let _ = runtime.release_jsvalue(receiver);
                release_all(arguments);
            }
            Self::Construct { arguments, .. } => release_all(arguments),
            Self::Read { .. } | Self::Has { .. } | Self::Species { .. } | Self::Define { .. } => {}
        }
    }
}

// One cache line: effects are parked in and moved out of the boxed builtin state.
const _: () = assert!(std::mem::size_of::<Effect>() <= 64);

pub(crate) enum Reply {
    /// Reply to the reads, [`Effect::Call`], [`Effect::Construct`] and [`Effect::Species`].
    Completion(Completion),
    Boolean(NativeConversion<bool>),
    Number(NativeConversion<f64>),
    Parsed(ObjectIteratorStep),
    Defined(NativeConversion<InternalDefineResult>),
    Set {
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    },
}

pub(crate) trait Continuation {
    /// The effect this continuation is waiting on, parked until a driver takes it.
    fn pending(&mut self) -> &mut Option<Effect>;
    fn reply(self: Box<Self>, runtime: &Runtime, reply: Reply) -> Result<EffectStep, RuntimeError>;

    /// Move the parked effect out so the driver owns it while performing it.
    fn take_pending(&mut self) -> Result<Effect, RuntimeError> {
        self.pending()
            .take()
            .ok_or(RuntimeError::Invariant("effect step has no pending effect"))
    }
}

/// Park `effect` on `continuation` and hand both to the driver.
pub(crate) fn request<C: Continuation + 'static>(
    effect: Effect,
    mut continuation: Box<C>,
) -> Result<EffectStep, RuntimeError> {
    *continuation.pending() = Some(effect);
    Ok(EffectStep::Effect(continuation))
}

pub(crate) enum EffectStep {
    Complete(Completion),
    Effect(Box<dyn Continuation>),
    /// Close the iterator, then complete with `completion`.
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}

/// Perform effects synchronously until the builtin completes.
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: EffectStep,
) -> Result<Completion, RuntimeError> {
    loop {
        let mut continuation = match step {
            EffectStep::Complete(completion) => return Ok(completion),
            EffectStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
            EffectStep::Effect(continuation) => continuation,
        };
        let reply = perform(runtime, realm, continuation.take_pending()?)?;
        step = continuation.reply(runtime, reply)?;
    }
}

fn perform(runtime: &Runtime, realm: ContextId, effect: Effect) -> Result<Reply, RuntimeError> {
    Ok(match effect {
        Effect::Read { object, key } => {
            Reply::Completion(runtime.get_property_in_realm(realm, &object, &key)?)
        }
        Effect::Has { object, key } => {
            Reply::Boolean(runtime.internal_has_property(realm, &object, &key)?)
        }
        Effect::Species { source, length } => Reply::Completion(species::finish(
            runtime,
            realm,
            species::SpeciesStep::start(runtime, realm, &source, length)?,
        )?),
        Effect::ReadValue { receiver, key } => {
            Reply::Completion(runtime.get_value_property_in_realm_jsvalue(realm, receiver, &key)?)
        }
        Effect::Number { value } => Reply::Number(runtime.native_to_number_jsvalue(realm, value)?),
        Effect::Call {
            callable,
            receiver,
            arguments,
        } => {
            Reply::Completion(runtime.call_internal_jsvalue(realm, &callable, receiver, arguments)?)
        }
        Effect::Construct { target, arguments } => {
            let new_target = target.try_clone()?;
            Reply::Completion(runtime.construct_internal_jsvalue(
                realm,
                &target,
                ConstructNewTarget::Validated(new_target),
                arguments,
            )?)
        }
        Effect::ParseIterator { result } => Reply::Parsed(finish_next(
            runtime,
            realm,
            NextStep::parse_result(runtime, realm, result)?,
        )?),
        Effect::Define {
            object,
            key,
            descriptor,
        } => Reply::Defined(runtime.internal_define_owned_property(
            realm,
            &object,
            &key,
            *descriptor,
        )?),
        Effect::Set { object, key, value } => {
            let receiver = JsValue::Object(object.try_clone()?.into_handle());
            let result = runtime.internal_set_jsvalue(realm, &object, &key, value, receiver)?;
            Reply::Set { key, result }
        }
    })
}

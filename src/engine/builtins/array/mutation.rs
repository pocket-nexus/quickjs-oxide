//! Array endpoint mutations retain their copy cursor across observable property operations.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayPopKind, ArrayPushKind},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum MutationKind {
    Push(ArrayPushKind),
    Pop(ArrayPopKind),
}
impl MutationKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypePush(kind) => Some(Self::Push(kind)),
            NativeFunctionId::ArrayPrototypePop(kind) => Some(Self::Pop(kind)),
            _ => None,
        }
    }
}
pub(crate) enum MutationStep {
    Complete(Completion),
    #[cfg(feature = "stack-vm")]
    PreparedRead {
        read: crate::engine::object::OrdinaryRead,
        key: PropertyKey,
        resume: MutationResume,
    },
    #[cfg(feature = "stack-vm")]
    PreparedSet {
        step: Box<crate::engine::object::SetStep>,
        key: PropertyKey,
        resume: MutationResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
    Number {
        value: Value,
        resume: MutationResume,
    },
    Copy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: MutationResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: MutationResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
}
enum Phase {
    Length,
    Number,
    Result,
    Copy,
    Write,
    DeleteLast,
    LengthWrite,
}
pub(crate) struct MutationResume {
    realm: ContextId,
    kind: MutationKind,
    object: ObjectRef,
    arguments: Vec<Value>,
    // Push never uses the Pop result slot. A single immediate argument lives
    // there until completion, avoiding a Vec allocation without moving roots.
    inline_argument: bool,
    phase: Phase,
    length: u64,
    new_length: u64,
    cursor: u64,
    result: Value,
}
// This enum carries only the selected effect. The source, arguments and result
// remain in one MutationResume until a real callback requires owned transport.
enum MutationAction {
    Complete(Completion),
    Read(PropertyKey),
    Number(Value),
    Copy {
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
    },
    Set {
        key: PropertyKey,
        value: Value,
    },
    Delete(PropertyKey),
}

impl MutationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array mutation requires generic invocation",
            ));
        };
        let values = &arguments.readable[..arguments.actual_arg_count];
        let inline = match (kind, values) {
            (
                MutationKind::Push(_),
                [
                    value @ (Value::Undefined
                    | Value::Null
                    | Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)),
                ],
            ) => Some(value.clone()),
            _ => None,
        };
        let values = if inline.is_some() {
            #[cfg(all(feature = "profiling", feature = "stack-vm"))]
            crate::engine::api::profiling::record_owned_execution_event(
                "array_mutation_inline_argument",
            );
            Vec::new()
        } else {
            values.to_vec()
        };
        Self::start_arguments(runtime, realm, kind, this_value.clone(), values, inline)
    }
    pub(crate) fn start_values(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        receiver: Value,
        arguments: Vec<Value>,
    ) -> Result<Self, RuntimeError> {
        Self::start_arguments(runtime, realm, kind, receiver, arguments, None)
    }
    fn start_arguments(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        receiver: Value,
        arguments: Vec<Value>,
        inline: Option<Value>,
    ) -> Result<Self, RuntimeError> {
        let object = match runtime.native_to_object(realm, receiver)? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let action = MutationAction::Read(runtime.intern_property_key("length")?);
        MutationResume {
            realm,
            kind,
            object,
            arguments,
            inline_argument: inline.is_some(),
            phase: Phase::Length,
            length: 0,
            new_length: 0,
            cursor: 0,
            result: inline.unwrap_or(Value::Undefined),
        }
        .drive(runtime, action)
    }
}
impl MutationResume {
    fn argument_count(&self) -> usize {
        if self.inline_argument {
            1
        } else {
            self.arguments.len()
        }
    }
    fn argument(&self, index: usize) -> Option<&Value> {
        if self.inline_argument {
            (index == 0).then_some(&self.result)
        } else {
            self.arguments.get(index)
        }
    }
    fn resume_once(
        &mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<MutationAction, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(MutationAction::Number(value))
            }
            Phase::Result => {
                self.result = value;
                self.copy_next(runtime)
            }
            Phase::Copy => self.copied(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array mutation received unexpected value reply",
            )),
        }
    }
    fn number_once(
        &mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<MutationAction, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array mutation number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(number) => Runtime::length_from_number(number),
            NativeConversion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        match self.kind {
            MutationKind::Push(_) => {
                self.new_length = self.length.saturating_add(self.argument_count() as u64);
                if self.new_length > (1_u64 << 53) - 1 {
                    return Ok(MutationAction::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "Array loo long",
                        )?,
                    )));
                }
                self.copy_next(runtime)
            }
            MutationKind::Pop(kind) => {
                self.new_length = self.length.saturating_sub(1);
                if self.length == 0 {
                    return self.write_length(runtime);
                }
                let index = if kind == ArrayPopKind::Shift {
                    0
                } else {
                    self.new_length
                };
                self.phase = Phase::Result;
                Ok(MutationAction::Read(runtime.property_key_for_index(index)?))
            }
        }
    }
    fn copy_next(&mut self, runtime: &Runtime) -> Result<MutationAction, RuntimeError> {
        let (to, from, count, backwards) = match self.kind {
            MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => {
                (self.argument_count() as u64, 0, self.length, true)
            }
            MutationKind::Pop(ArrayPopKind::Shift) => (0, 1, self.new_length, false),
            _ => return self.copied(runtime),
        };
        self.phase = Phase::Copy;
        Ok(MutationAction::Copy {
            to,
            from,
            count,
            backwards,
        })
    }
    fn copied(&mut self, runtime: &Runtime) -> Result<MutationAction, RuntimeError> {
        self.cursor = 0;
        match self.kind {
            MutationKind::Push(_) => self.write_next(runtime),
            MutationKind::Pop(_) => {
                self.phase = Phase::DeleteLast;
                Ok(MutationAction::Delete(
                    runtime.property_key_for_index(self.new_length)?,
                ))
            }
        }
    }
    fn write_next(&mut self, runtime: &Runtime) -> Result<MutationAction, RuntimeError> {
        if let Some(value) = self.argument(self.cursor as usize).cloned() {
            let from = match self.kind {
                MutationKind::Push(ArrayPushKind::Unshift) if self.argument_count() != 0 => 0,
                _ => self.length,
            };
            self.phase = Phase::Write;
            return Ok(MutationAction::Set {
                key: runtime.property_key_for_index(from + self.cursor)?,
                value,
            });
        }
        let redundant = matches!(self.kind, MutationKind::Push(ArrayPushKind::Push))
            && self.new_length <= u64::from(u32::MAX)
            && matches!(runtime.array_length_state_if_genuine(&self.object)?, Some((length, true)) if u64::from(length) == self.new_length);
        if redundant {
            self.complete()
        } else {
            self.write_length(runtime)
        }
    }
    fn write_length(&mut self, runtime: &Runtime) -> Result<MutationAction, RuntimeError> {
        self.phase = Phase::LengthWrite;
        Ok(MutationAction::Set {
            key: runtime.intern_property_key("length")?,
            value: Value::number(self.new_length as f64),
        })
    }
    fn complete(&mut self) -> Result<MutationAction, RuntimeError> {
        Ok(MutationAction::Complete(Completion::Return(
            match self.kind {
                MutationKind::Push(_) => Value::number(self.new_length as f64),
                MutationKind::Pop(_) => std::mem::replace(&mut self.result, Value::Undefined),
            },
        )))
    }
    fn boolean_once(
        &mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<MutationAction, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(MutationAction::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::DeleteLast => {
                if !value {
                    return Ok(MutationAction::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.write_length(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array mutation boolean phase mismatch",
            )),
        }
    }
    fn set_once(
        &mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<MutationAction, RuntimeError> {
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(MutationAction::Complete(Completion::Throw(value)));
        }
        match self.phase {
            Phase::Write => {
                self.cursor += 1;
                self.write_next(runtime)
            }
            Phase::LengthWrite => self.complete(),
            _ => Err(RuntimeError::Invariant("Array mutation set phase mismatch")),
        }
    }
}
impl MutationResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<MutationStep, RuntimeError> {
        let action = self.resume_once(runtime, reply)?;
        self.drive(runtime, action)
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<MutationStep, RuntimeError> {
        let action = self.number_once(runtime, reply)?;
        self.drive(runtime, action)
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<MutationStep, RuntimeError> {
        let action = self.boolean_once(runtime, reply)?;
        self.drive(runtime, action)
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<MutationStep, RuntimeError> {
        let action = self.set_once(runtime, key, reply)?;
        self.drive(runtime, action)
    }
    fn drive(
        mut self,
        runtime: &Runtime,
        mut action: MutationAction,
    ) -> Result<MutationStep, RuntimeError> {
        loop {
            #[cfg(feature = "stack-vm")]
            {
                use crate::engine::object::{OrdinaryRead, SetStep};
                use crate::engine::value::conversion::number::NumberStep;
                action = match action {
                    MutationAction::Read(key) => {
                        let receiver = Value::Object(self.object.clone());
                        match runtime.prepare_ordinary_read_borrowed(
                            &self.object,
                            &key,
                            &receiver,
                        )? {
                            OrdinaryRead::Complete(value) => self.resume_once(
                                runtime,
                                Completion::Return(value.unwrap_or(Value::Undefined)),
                            )?,
                            read => {
                                return Ok(MutationStep::PreparedRead {
                                    read,
                                    key,
                                    resume: self,
                                });
                            }
                        }
                    }
                    MutationAction::Number(value) if !matches!(value, Value::Object(_)) => {
                        let NumberStep::Complete(reply) =
                            NumberStep::start(runtime, self.realm, value)?
                        else {
                            return Err(RuntimeError::Invariant(
                                "primitive mutation number suspended",
                            ));
                        };
                        self.number_once(runtime, reply)?
                    }
                    MutationAction::Set { key, value } => {
                        let mut pending = None;
                        let selected = SetStep::start_receiver_into(
                            runtime,
                            self.realm,
                            &key,
                            value,
                            Value::Object(self.object.clone()),
                            |step| pending = Some(step),
                        )?;
                        let step = match selected {
                            Some(action) => SetStep::Complete(action),
                            None => pending
                                .ok_or(RuntimeError::Invariant(
                                    "mutation Set lost selected effect",
                                ))?
                                .advance_without_callback(runtime)?,
                        };
                        match step {
                            SetStep::Complete(action)
                                if !matches!(
                                    action,
                                    crate::engine::object::operations::PropertySetAction::Call { .. }
                                ) =>
                            {
                                self.set_once(runtime, key, local_set_result(action)?)?
                            }
                            step => {
                                return Ok(MutationStep::PreparedSet {
                                    step: Box::new(step),
                                    key,
                                    resume: self,
                                });
                            }
                        }
                    }
                    MutationAction::Delete(key) if !runtime.is_proxy_object(&self.object)? => {
                        let reply =
                            runtime.internal_delete_property(self.realm, &self.object, &key)?;
                        self.boolean_once(runtime, reply)?
                    }
                    action => return Ok(self.wait(action)),
                };
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "array_mutation_local_stage",
                );
            }
            #[cfg(not(feature = "stack-vm"))]
            return Ok(self.wait(action));
        }
    }
    fn wait(self, action: MutationAction) -> MutationStep {
        match action {
            MutationAction::Complete(result) => MutationStep::Complete(result),
            MutationAction::Read(key) => MutationStep::Read {
                object: self.object.clone(),
                key,
                resume: self,
            },
            MutationAction::Number(value) => MutationStep::Number {
                value,
                resume: self,
            },
            MutationAction::Copy {
                to,
                from,
                count,
                backwards,
            } => MutationStep::Copy {
                object: self.object.clone(),
                to,
                from,
                count,
                backwards,
                resume: self,
            },
            MutationAction::Set { key, value } => MutationStep::Set {
                object: self.object.clone(),
                key,
                value,
                resume: self,
            },
            MutationAction::Delete(key) => MutationStep::Delete {
                object: self.object.clone(),
                key,
                resume: self,
            },
        }
    }
}

#[cfg(feature = "stack-vm")]
fn local_set_result(
    action: crate::engine::object::operations::PropertySetAction,
) -> Result<NativeConversion<InternalSetResult>, RuntimeError> {
    use crate::engine::object::operations::PropertySetAction;
    Ok(match action {
        PropertySetAction::Complete => NativeConversion::Value(InternalSetResult::Accepted),
        PropertySetAction::Rejected(reason) => {
            NativeConversion::Value(InternalSetResult::Rejected(reason))
        }
        PropertySetAction::RejectedProxyTrap => {
            NativeConversion::Value(InternalSetResult::RejectedProxyTrap)
        }
        PropertySetAction::Throw(value) => NativeConversion::Throw(value),
        PropertySetAction::Call { .. } => {
            return Err(RuntimeError::Invariant(
                "mutation completed before setter returned",
            ));
        }
    })
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: MutationStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            MutationStep::Complete(result) => return Ok(result),
            #[cfg(feature = "stack-vm")]
            MutationStep::PreparedRead { read, key, resume } => {
                let reply = match runtime.finish_prepared_read(realm, &key, read)? {
                    NativeConversion::Value(value) => {
                        Completion::Return(value.unwrap_or(Value::Undefined))
                    }
                    NativeConversion::Throw(value) => Completion::Throw(value),
                };
                resume.resume(runtime, reply)?
            }
            #[cfg(feature = "stack-vm")]
            MutationStep::PreparedSet { step, key, resume } => {
                use crate::engine::object::{SetStep, operations::PropertySetAction};
                let mut step = *step;
                let result = loop {
                    match step {
                        SetStep::Complete(PropertySetAction::Call {
                            setter,
                            receiver,
                            argument,
                        }) => {
                            break match runtime.call_internal(
                                realm,
                                &setter,
                                receiver,
                                &[argument],
                            )? {
                                Completion::Return(_) => {
                                    NativeConversion::Value(InternalSetResult::Accepted)
                                }
                                Completion::Throw(value) => NativeConversion::Throw(value),
                            };
                        }
                        SetStep::Complete(action) => break local_set_result(action)?,
                        request => step = request.finish_sync(runtime)?,
                    }
                };
                resume.set(runtime, key, result)?
            }
            MutationStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            MutationStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            MutationStep::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => resume.resume(
                runtime,
                super::copy::finish(
                    runtime,
                    realm,
                    super::copy::CopyStep::start(
                        runtime, realm, object, to, from, count, backwards,
                    )?,
                )?,
            )?,
            MutationStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
            }
            MutationStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_mutation_keeps_selected_setter_and_proxy_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let trace='', stored;
            const proto={set 0(value){trace+='s';stored=value;}};
            const target=Object.create(proto);target.length=0;
            Object.defineProperty(target,'length',{get(){trace+='g';return 0;},set(value){trace+='l'+value;},configurable:true});
            if(Array.prototype.push.call(target,7)!==1 || stored!==7 || trace!=='gsl1')return false;
            trace='';const data={length:0};
            const proxy=new Proxy(data,{get(o,k,r){if(k==='length')trace+='g';return Reflect.get(o,k,r);},set(o,k,v,r){trace+='s'+k;return Reflect.set(o,k,v,r);}});
            if(Array.prototype.push.call(proxy,8)!==1 || trace!=='gs0slength' || data[0]!==8)return false;
            trace=''; const pop=Object.create({get 1(){trace+='r';return 9;}});
            Object.defineProperty(pop,'length',{get(){trace+='g';return 2;},set(v){trace+='l'+v;}});
            return Array.prototype.pop.call(pop)===9 && trace==='grl1';
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn local_mutation_keeps_partial_effects_on_rejected_length_or_delete() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            const a=[1,2];Object.defineProperty(a,'1',{configurable:false});
            let rejected=false;try{a.pop();}catch(e){rejected=e instanceof TypeError;}
            if(!rejected || a.length!==2 || a[1]!==2)return false;
            const target={length:0};Object.defineProperty(target,'length',{writable:false});
            rejected=false;try{Array.prototype.push.call(target,3);}catch(e){rejected=e instanceof TypeError;}
            if(!rejected || target[0]!==3 || target.length!==0)return false;
            const b=[1];Object.defineProperty(b,'length',{writable:false});
            rejected=false;try{b.push(4);}catch(e){rejected=e instanceof TypeError;}
            return rejected && b.length===1 && !(1 in b);
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn push_inline_argument_uses_actual_count_and_retains_immediate_payload() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        #[cfg(all(feature = "profiling", feature = "stack-vm"))]
        let profile = crate::engine::api::profiling::CostProfile::start();
        for value in [
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(-0.0),
        ] {
            let array = runtime.new_array(context.realm).unwrap();
            let invocation = NativeInvocation::Call {
                this_value: Value::Object(array.clone()),
            };
            let arguments = NativeArguments {
                actual_arg_count: 1,
                readable: vec![value.clone(), Value::Undefined],
            };
            let step = MutationStep::start(
                &runtime,
                context.realm,
                MutationKind::Push(ArrayPushKind::Push),
                &invocation,
                &arguments,
            )
            .unwrap();
            let result = finish(&runtime, context.realm, step).unwrap();
            assert!(matches!(result, Completion::Return(Value::Int(1))));
            let key = runtime.property_key_for_index(0).unwrap();
            let Completion::Return(actual) = runtime
                .get_property_in_realm(context.realm, &array, &key)
                .unwrap()
            else {
                panic!("element read threw")
            };
            assert!(actual.same_quickjs_representation(&value));
        }
        let array = runtime.new_array(context.realm).unwrap();
        let invocation = NativeInvocation::Call {
            this_value: Value::Object(array),
        };
        let arguments = NativeArguments {
            actual_arg_count: 0,
            readable: vec![Value::Undefined],
        };
        let step = MutationStep::start(
            &runtime,
            context.realm,
            MutationKind::Push(ArrayPushKind::Push),
            &invocation,
            &arguments,
        )
        .unwrap();
        assert!(matches!(
            finish(&runtime, context.realm, step).unwrap(),
            Completion::Return(Value::Int(0))
        ));
        #[cfg(all(feature = "profiling", feature = "stack-vm"))]
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("array_mutation_inline_argument")
                .copied()
                .unwrap_or(0),
            5
        );
    }

    #[test]
    fn push_inline_argument_preserves_observable_mutation_steps() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let value = context.eval(r#"(function () {
            var trace = '', stored;
            var target = {
                get length() { trace += 'g'; return { valueOf: function () { trace += 'n'; return 0; } }; },
                set length(v) { trace += 'l' + v; },
                set 0(v) { trace += 's'; stored = v; }
            };
            if (Array.prototype.push.call(target, -0) !== 1 ||
                trace !== 'gnsl1' || 1 / stored !== -Infinity) return false;
            var a = [2, 3];
            if (a.unshift(1) !== 3 || a.join(',') !== '1,2,3') return false;
            var object = {}, b = [];
            if (b.push(object) !== 1 || b[0] !== object) return false;
            if (b.push(4, 5) !== 3 || b[1] !== 4 || b[2] !== 5) return false;
            var frozen = Object.freeze([]), threw = false;
            try { frozen.push(1); } catch (e) { threw = e instanceof TypeError; }
            return threw && frozen.length === 0;
        })()"#).unwrap();
        assert!(matches!(value, Value::Bool(true)));
    }
}

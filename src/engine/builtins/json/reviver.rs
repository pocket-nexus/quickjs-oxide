//! QuickJS-shaped `JSON.parse` builtin and reviver internalization.

use super::parse::JsonParseRecord;
use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::PropertyKeyKind;
use crate::engine::heap::ContextId;
use std::rc::Rc;

use crate::engine::object::operations::{InternalDefineResult, PropertyDefineOutcome};
use crate::engine::object::{
    CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, Value};
use crate::engine::vm::Completion;
use crate::engine::vm::call::NativeArguments;

const MAX_JSON_REVIVER_DEPTH: usize = 128;

impl Runtime {
    pub(crate) fn call_json_parse(
        &self,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        finish(self, realm, ParseStep::start(self, realm, arguments)?)
    }

    fn define_json_reviver_property(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> Result<PropertyDefineOutcome, RuntimeError> {
        let descriptor = OrdinaryPropertyDescriptor {
            value: DescriptorField::Present(value),
            writable: DescriptorField::Present(true),
            enumerable: DescriptorField::Present(true),
            configurable: DescriptorField::Present(true),
            ..OrdinaryPropertyDescriptor::new()
        };
        Ok(
            match self.internal_define_own_property(realm, object, key, &descriptor)? {
                NativeConversion::Value(InternalDefineResult::Defined) => {
                    PropertyDefineOutcome::Defined(true)
                }
                NativeConversion::Value(
                    InternalDefineResult::RejectedOrdinary(_)
                    | InternalDefineResult::RejectedProxyTrap,
                ) => PropertyDefineOutcome::Defined(false),
                NativeConversion::Throw(value) => PropertyDefineOutcome::Throw(value),
            },
        )
    }
}

pub(crate) enum ParseStep {
    Complete(Completion),
    String {
        value: Value,
        resume: ParseResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ParseResume,
    },
    Number {
        value: Value,
        resume: ParseResume,
    },
    Keys {
        object: ObjectRef,
        resume: ParseResume,
    },
    Enumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: ParseResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: ParseResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: ParseResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: ParseResume,
    },
}
pub(crate) struct ParseResume {
    state: Box<State>,
    phase: Phase,
}
enum Phase {
    Source(Value),
    Read,
    Length,
    Number,
    Keys,
    Enumerable {
        keys: std::vec::IntoIter<PropertyKey>,
        selected: Vec<PropertyKey>,
        key: PropertyKey,
    },
    Revived,
    Applied,
}
struct State {
    realm: ContextId,
    source: JsString,
    reviver: Option<CallableRef>,
    frames: Vec<Node>,
    _record: Option<Rc<JsonParseRecord>>,
}
struct Node {
    holder: ObjectRef,
    key: PropertyKey,
    record: Option<Rc<JsonParseRecord>>,
    value: Value,
    context: Option<ObjectRef>,
    children: Children,
}
enum Children {
    None,
    Array { index: u32, length: u32 },
    Object(std::vec::IntoIter<PropertyKey>),
}
impl ParseStep {
    pub(crate) fn start(
        _runtime: &Runtime,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::String {
            value: arguments.readable[0].clone(),
            resume: ParseResume {
                state: Box::new(State {
                    realm,
                    source: JsString::from_static(""),
                    reviver: None,
                    frames: Vec::new(),
                    _record: None,
                }),
                phase: Phase::Source(arguments.readable[1].clone()),
            },
        })
    }
}
impl State {
    fn top(&mut self) -> Result<&mut Node, RuntimeError> {
        self.frames
            .last_mut()
            .ok_or(RuntimeError::Invariant("JSON reviver lost its node"))
    }
    fn enter(
        mut self: Box<Self>,
        runtime: &Runtime,
        holder: ObjectRef,
        key: PropertyKey,
        record: Option<Rc<JsonParseRecord>>,
    ) -> Result<ParseStep, RuntimeError> {
        if self.frames.len() > MAX_JSON_REVIVER_DEPTH {
            return Ok(ParseStep::Complete(Completion::Throw(
                runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Internal,
                    "stack overflow",
                )?,
            )));
        }
        if self.frames.try_reserve(1).is_err() {
            return Ok(ParseStep::Complete(Completion::Throw(
                runtime.new_native_error(self.realm, NativeErrorKind::Internal, "out of memory")?,
            )));
        }
        self.frames.push(Node {
            holder: holder.clone(),
            key: key.clone(),
            record,
            value: Value::Undefined,
            context: None,
            children: Children::None,
        });
        Ok(ParseStep::Read {
            object: holder,
            key,
            resume: ParseResume {
                state: self,
                phase: Phase::Read,
            },
        })
    }
    fn next(mut self: Box<Self>, runtime: &Runtime) -> Result<ParseStep, RuntimeError> {
        let realm = self.realm;
        let node = self.top()?;
        let child = match &mut node.children {
            Children::None => None,
            Children::Array { index, length } if *index < *length => {
                let key = runtime.intern_property_key(&index.to_string())?;
                let record = node
                    .record
                    .as_mut()
                    .and_then(|record| record.array_child(*index as usize));
                *index += 1;
                Some((key, record))
            }
            Children::Array { .. } => None,
            Children::Object(keys) => keys.next().map(|key| {
                let record = node
                    .record
                    .as_mut()
                    .and_then(|record| record.object_child(&key));
                (key, record)
            }),
        };
        if let Some((key, record)) = child {
            let Value::Object(object) = &node.value else {
                return Err(RuntimeError::Invariant(
                    "JSON reviver child holder is not an object",
                ));
            };
            let object = object.clone();
            return self.enter(runtime, object, key, record);
        }
        let receiver = Value::Object(node.holder.clone());
        let name = Value::String(
            runtime
                .0
                .state
                .borrow()
                .atoms
                .to_js_string(node.key.atom())?,
        );
        let context = node
            .context
            .clone()
            .ok_or(RuntimeError::Invariant("JSON reviver lost its context"))?;
        let mut arguments = Vec::new();
        if arguments.try_reserve_exact(3).is_err() {
            return Ok(ParseStep::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Internal, "out of memory")?,
            )));
        }
        arguments.push(name);
        arguments.push(node.value.clone());
        arguments.push(Value::Object(context));
        let callable = self
            .reviver
            .clone()
            .ok_or(RuntimeError::Invariant("JSON reviver lost its callback"))?;
        Ok(ParseStep::Call {
            callable,
            receiver,
            arguments,
            resume: ParseResume {
                state: self,
                phase: Phase::Revived,
            },
        })
    }
    fn enumerate(
        mut self: Box<Self>,
        runtime: &Runtime,
        mut keys: std::vec::IntoIter<PropertyKey>,
        selected: Vec<PropertyKey>,
    ) -> Result<ParseStep, RuntimeError> {
        for key in keys.by_ref() {
            if runtime
                .0
                .state
                .borrow()
                .atoms
                .property_key_kind(key.atom())?
                != PropertyKeyKind::String
            {
                continue;
            }
            let Value::Object(object) = &self.top()?.value else {
                return Err(RuntimeError::Invariant(
                    "JSON reviver key holder is not an object",
                ));
            };
            return Ok(ParseStep::Enumerable {
                object: object.clone(),
                key: key.clone(),
                resume: ParseResume {
                    state: self,
                    phase: Phase::Enumerable {
                        keys,
                        selected,
                        key,
                    },
                },
            });
        }
        self.top()?.children = Children::Object(selected.into_iter());
        self.next(runtime)
    }
}
impl ParseResume {
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        reply: NativeConversion<JsString>,
    ) -> Result<ParseStep, RuntimeError> {
        let source = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ParseStep::Complete(Completion::Throw(value)));
            }
        };
        let Phase::Source(reviver) = self.phase else {
            return Err(RuntimeError::Invariant(
                "JSON parse unexpected string reply",
            ));
        };
        let mut state = self.state;
        state.source = source;
        state.reviver = match reviver {
            Value::Object(object) => runtime.as_callable(&object)?,
            _ => None,
        };
        // Holder allocation precedes parsing, as in the pinned implementation.
        let root = state
            .reviver
            .as_ref()
            .map(|_| runtime.new_ordinary_object_in_realm(state.realm))
            .transpose()?;
        let (parsed, record) =
            match runtime.parse_json_text(state.realm, &state.source, state.reviver.is_some())? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(value) => {
                    return Ok(ParseStep::Complete(Completion::Throw(value)));
                }
            };
        let Some(root) = root else {
            return Ok(ParseStep::Complete(Completion::Return(parsed)));
        };
        let key = runtime.intern_property_key("")?;
        match runtime.define_json_reviver_property(state.realm, &root, &key, parsed)? {
            PropertyDefineOutcome::Defined(true) => {}
            PropertyDefineOutcome::Defined(false) => {
                return Err(RuntimeError::Invariant(
                    "fresh JSON reviver root definition was rejected",
                ));
            }
            PropertyDefineOutcome::Throw(value) => {
                return Ok(ParseStep::Complete(Completion::Throw(value)));
            }
        }
        let record = record.map(Rc::new);
        // Keep the whole original parse graph alive until the root callback
        // returns, while child requests own stable immutable record handles.
        state._record = record.clone();
        state.enter(runtime, root, key, record)
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ParseStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(ParseStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Read => {
                let realm = self.state.realm;
                let node = self.state.top()?;
                node.record = node.record.take().filter(|record| record.matches(&value));
                node.value = value.clone();
                node.context = Some(runtime.new_ordinary_object_in_realm(realm)?);
                if let Value::Object(object) = value {
                    let array =
                        match runtime.internal_is_array(realm, &Value::Object(object.clone()))? {
                            NativeConversion::Value(value) => value,
                            NativeConversion::Throw(value) => {
                                return Ok(ParseStep::Complete(Completion::Throw(value)));
                            }
                        };
                    if array {
                        Ok(ParseStep::Read {
                            object,
                            key: runtime.intern_property_key("length")?,
                            resume: Self {
                                state: self.state,
                                phase: Phase::Length,
                            },
                        })
                    } else {
                        Ok(ParseStep::Keys {
                            object,
                            resume: Self {
                                state: self.state,
                                phase: Phase::Keys,
                            },
                        })
                    }
                } else {
                    if let Some((start, end)) = node
                        .record
                        .as_ref()
                        .and_then(|record| record.primitive_span())
                    {
                        let context = node.context.clone().unwrap();
                        let source = Value::String(self.state.source.sub_string(start, end));
                        let key = runtime.intern_property_key("source")?;
                        match runtime.define_json_reviver_property(realm, &context, &key, source)? {
                            PropertyDefineOutcome::Defined(true) => {}
                            PropertyDefineOutcome::Defined(false) => {
                                return Err(RuntimeError::Invariant(
                                    "fresh JSON reviver source definition was rejected",
                                ));
                            }
                            PropertyDefineOutcome::Throw(value) => {
                                return Ok(ParseStep::Complete(Completion::Throw(value)));
                            }
                        }
                    }
                    self.state.next(runtime)
                }
            }
            Phase::Length => Ok(ParseStep::Number {
                value,
                resume: Self {
                    state: self.state,
                    phase: Phase::Number,
                },
            }),
            Phase::Revived => {
                let node = self
                    .state
                    .frames
                    .pop()
                    .ok_or(RuntimeError::Invariant("JSON reviver reply lost node"))?;
                if self.state.frames.is_empty() {
                    return Ok(ParseStep::Complete(Completion::Return(value)));
                }
                let resume = Self {
                    state: self.state,
                    phase: Phase::Applied,
                };
                if matches!(value, Value::Undefined) {
                    Ok(ParseStep::Delete {
                        object: node.holder,
                        key: node.key,
                        resume,
                    })
                } else {
                    Ok(ParseStep::Define {
                        object: node.holder,
                        key: node.key,
                        descriptor: OrdinaryPropertyDescriptor {
                            value: DescriptorField::Present(value),
                            writable: DescriptorField::Present(true),
                            enumerable: DescriptorField::Present(true),
                            configurable: DescriptorField::Present(true),
                            ..OrdinaryPropertyDescriptor::new()
                        },
                        resume,
                    })
                }
            }
            _ => Err(RuntimeError::Invariant(
                "JSON reviver unexpected value reply",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<ParseStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "JSON reviver unexpected number reply",
            ));
        }
        let number = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ParseStep::Complete(Completion::Throw(value)));
            }
        };
        self.state.top()?.children = Children::Array {
            index: 0,
            length: Runtime::to_uint32_number(number),
        };
        self.state.next(runtime)
    }
    pub(crate) fn keys(
        self,
        runtime: &Runtime,
        reply: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<ParseStep, RuntimeError> {
        if !matches!(self.phase, Phase::Keys) {
            return Err(RuntimeError::Invariant(
                "JSON reviver unexpected keys reply",
            ));
        }
        match reply {
            NativeConversion::Value(keys) => {
                self.state.enumerate(runtime, keys.into_iter(), Vec::new())
            }
            NativeConversion::Throw(value) => Ok(ParseStep::Complete(Completion::Throw(value))),
        }
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<ParseStep, RuntimeError> {
        let value = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ParseStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Enumerable {
                keys,
                mut selected,
                key,
            } => {
                if value {
                    if selected.try_reserve(1).is_err() {
                        return Ok(ParseStep::Complete(Completion::Throw(
                            runtime.new_native_error(
                                self.state.realm,
                                NativeErrorKind::Internal,
                                "out of memory",
                            )?,
                        )));
                    }
                    selected.push(key);
                }
                self.state.enumerate(runtime, keys, selected)
            }
            Phase::Applied => self.state.next(runtime),
            _ => Err(RuntimeError::Invariant(
                "JSON reviver unexpected boolean reply",
            )),
        }
    }
}
fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ParseStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ParseStep::Complete(result) => return Ok(result),
            ParseStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            ParseStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            ParseStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ParseStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            ParseStep::Enumerable {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_snapshot_own_property_is_enumerable(realm, &object, &key)?,
            )?,
            ParseStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
            ParseStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
            ParseStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.boolean(
                runtime,
                match runtime.internal_define_own_property(realm, &object, &key, &descriptor)? {
                    NativeConversion::Value(result) => {
                        NativeConversion::Value(matches!(result, InternalDefineResult::Defined))
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                },
            )?,
        };
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    fn until_call(runtime: &Runtime, realm: ContextId, mut step: ParseStep) -> ParseStep {
        loop {
            step = match step {
                step @ ParseStep::Call { .. } => return step,
                ParseStep::Read {
                    object,
                    key,
                    resume,
                } => resume
                    .resume(
                        runtime,
                        runtime.get_property_in_realm(realm, &object, &key).unwrap(),
                    )
                    .unwrap(),
                ParseStep::Keys { object, resume } => resume
                    .keys(
                        runtime,
                        runtime.internal_own_property_keys(realm, &object).unwrap(),
                    )
                    .unwrap(),
                ParseStep::Enumerable {
                    object,
                    key,
                    resume,
                } => resume
                    .boolean(
                        runtime,
                        runtime
                            .internal_snapshot_own_property_is_enumerable(realm, &object, &key)
                            .unwrap(),
                    )
                    .unwrap(),
                ParseStep::Delete {
                    object,
                    key,
                    resume,
                } => resume
                    .boolean(
                        runtime,
                        runtime
                            .internal_delete_property(realm, &object, &key)
                            .unwrap(),
                    )
                    .unwrap(),
                _ => panic!("unexpected reviver test step"),
            };
        }
    }
    #[test]
    fn parse_record_keeps_deleted_prior_children_until_callback_abandonment() {
        let runtime = Runtime::new();
        let weak = Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let callback = context.eval("(function(k,v){return v})").unwrap();
        let Value::Object(callback_object) = &callback else {
            panic!("expected callback");
        };
        let callback_id = callback_object.object_id();
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![
                Value::String(JsString::from_static("{\"a\":{},\"b\":{}}")),
                callback,
            ],
        };
        let ParseStep::String {
            value: Value::String(source),
            resume,
        } = ParseStep::start(&runtime, context.realm, &arguments).unwrap()
        else {
            panic!("expected source conversion");
        };
        drop(arguments);
        let step = until_call(
            &runtime,
            context.realm,
            resume
                .string(&runtime, NativeConversion::Value(source))
                .unwrap(),
        );
        let ParseStep::Call {
            arguments,
            resume,
            callable,
            receiver,
        } = step
        else {
            panic!("expected a callback");
        };
        drop(callable);
        drop(receiver);
        assert_eq!(arguments[0].to_js_string().unwrap().to_utf8_lossy(), "a");
        let Value::Object(first) = &arguments[1] else {
            panic!("expected first child");
        };
        let first_id = first.object_id();
        drop(arguments);
        let step = until_call(
            &runtime,
            context.realm,
            resume
                .resume(&runtime, Completion::Return(Value::Undefined))
                .unwrap(),
        );
        let ParseStep::Call {
            arguments, resume, ..
        } = &step
        else {
            panic!("expected b callback");
        };
        assert_eq!(arguments[0].to_js_string().unwrap().to_utf8_lossy(), "b");
        let Value::Object(second) = &arguments[1] else {
            panic!("expected second child");
        };
        let second_id = second.object_id();
        let context_id = resume
            .state
            .frames
            .last()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .object_id();
        let holder_id = resume.state.frames[0].holder.object_id();
        let Value::Object(root) = &resume.state.frames[0].value else {
            panic!("expected parsed root");
        };
        let root_id = root.object_id();
        let key = runtime.intern_property_key("a").unwrap();
        assert!(matches!(
            runtime
                .get_property_in_realm(context.realm, root, &key)
                .unwrap(),
            Completion::Return(Value::Undefined)
        ));
        drop(key);
        runtime.run_gc().unwrap();
        let ids = [
            first_id,
            second_id,
            root_id,
            holder_id,
            context_id,
            callback_id,
        ];
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(step);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(
                runtime.0.state.borrow().heap.object(id).is_err(),
                "abandoned reviver retained {id:?}"
            );
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

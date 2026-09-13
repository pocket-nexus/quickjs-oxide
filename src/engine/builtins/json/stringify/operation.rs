//! Owned normalization and traversal of JSON.stringify. Both VM consumers
//! execute the same phases; no getter, conversion or callback is replayed.
use super::{
    JsonSerializeTask as Task, JsonStringifier as State, JsonStringifyFailure, JsonStringifyResult,
    JsonWrapperKind,
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    atom::PropertyKeyKind,
    heap::ContextId,
    object::{CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey},
    value::{JsString, JsStringBuilder, Value, conversion::NativeConversion},
    vm::{Completion, call::NativeArguments},
};

pub(crate) enum StringifyStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: StringifyResume,
    },
    String {
        value: Value,
        resume: StringifyResume,
    },
    Number {
        value: Value,
        resume: StringifyResume,
    },
    Keys {
        object: ObjectRef,
        resume: StringifyResume,
    },
    Enumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringifyResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: StringifyResume,
    },
}
pub(crate) struct StringifyResume {
    state: Box<State>,
    phase: Phase,
}
struct List {
    object: ObjectRef,
    index: u64,
    length: u64,
    items: Vec<JsString>,
}
struct Check {
    holder: ObjectRef,
    value: Value,
    key: JsString,
    destination: Destination,
}
enum Destination {
    Root,
    Array(Task),
    Object(Task),
}
struct ObjectStart {
    object: ObjectRef,
    indent: JsString,
    next_indent: JsString,
}
enum Phase {
    ListLength(ObjectRef),
    ListNumber(ObjectRef),
    ListItem(List),
    ListString(List),
    GapNumber,
    GapString,
    ReadCheck {
        holder: ObjectRef,
        key: JsString,
        destination: Destination,
    },
    ToJson(Check),
    ToJsonCall(Check),
    Replacer(Check),
    WrapperString,
    WrapperNumber(JsString),
    Raw,
    ArrayLength(ObjectStart),
    ArrayNumber(ObjectStart),
    ObjectKeys(ObjectStart),
    Enumerable {
        start: ObjectStart,
        remaining: std::vec::IntoIter<PropertyKey>,
        keys: Vec<JsString>,
        key: PropertyKey,
    },
}
fn result(value: JsonStringifyResult<StringifyStep>) -> Result<StringifyStep, RuntimeError> {
    match value {
        Ok(step) => Ok(step),
        Err(JsonStringifyFailure::Throw(value)) => {
            Ok(StringifyStep::Complete(Completion::Throw(value)))
        }
        Err(JsonStringifyFailure::Runtime(error)) => Err(error),
    }
}
fn converted<T>(reply: NativeConversion<T>) -> JsonStringifyResult<T> {
    match reply {
        NativeConversion::Value(value) => Ok(value),
        NativeConversion::Throw(value) => Err(JsonStringifyFailure::Throw(value)),
    }
}
fn returned(reply: Completion) -> JsonStringifyResult<Value> {
    match reply {
        Completion::Return(value) => Ok(value),
        Completion::Throw(value) => Err(JsonStringifyFailure::Throw(value)),
    }
}
impl StringifyStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        result((|| {
            runtime.0.state.borrow().heap.context(realm)?;
            let replacer_value = &arguments.readable[1];
            let replacer = match replacer_value {
                Value::Object(object) => runtime.as_callable(object)?,
                _ => None,
            };
            let state = Box::new(State {
                realm,
                replacer,
                property_list: None,
                gap: JsString::from_static(""),
                to_json_key: None,
                stack: Vec::new(),
                output: JsStringBuilder::new(0),
                tasks: Vec::new(),
                root: arguments.readable[0].clone(),
                space: arguments.readable[2].clone(),
            });
            if state.replacer.is_none()
                && let Value::Object(object) = replacer_value
                && converted(runtime.internal_is_array(realm, replacer_value)?)?
            {
                return state.read(
                    runtime,
                    object.clone(),
                    "length",
                    Phase::ListLength(object.clone()),
                );
            }
            state.gap(runtime)
        })())
    }
}
impl State {
    fn read(
        self: Box<Self>,
        runtime: &Runtime,
        object: ObjectRef,
        key: &str,
        phase: Phase,
    ) -> JsonStringifyResult<StringifyStep> {
        Ok(StringifyStep::Read {
            receiver: Value::Object(object),
            key: runtime.intern_property_key(key)?,
            resume: StringifyResume { state: self, phase },
        })
    }
    fn list(
        mut self: Box<Self>,
        runtime: &Runtime,
        list: List,
    ) -> JsonStringifyResult<StringifyStep> {
        if list.index == list.length {
            self.property_list = Some(list.items);
            return self.gap(runtime);
        }
        let key = runtime.intern_property_key(&list.index.to_string())?;
        Ok(StringifyStep::Read {
            receiver: Value::Object(list.object.clone()),
            key,
            resume: StringifyResume {
                state: self,
                phase: Phase::ListItem(list),
            },
        })
    }
    fn gap(self: Box<Self>, runtime: &Runtime) -> JsonStringifyResult<StringifyStep> {
        match &self.space {
            Value::Object(object) => match runtime.json_wrapper_kind(object)? {
                JsonWrapperKind::String => {
                    return Ok(StringifyStep::String {
                        value: self.space.clone(),
                        resume: StringifyResume {
                            state: self,
                            phase: Phase::GapString,
                        },
                    });
                }
                JsonWrapperKind::Number => {
                    return Ok(StringifyStep::Number {
                        value: self.space.clone(),
                        resume: StringifyResume {
                            state: self,
                            phase: Phase::GapNumber,
                        },
                    });
                }
                _ => {}
            },
            Value::String(value) => {
                let gap = value.sub_string(0, value.len().min(10));
                return self.root(runtime, gap);
            }
            Value::Int(value) => {
                let number = f64::from(*value);
                return self.number_gap(runtime, number);
            }
            Value::Float(number) => {
                let number = *number;
                return self.number_gap(runtime, number);
            }
            _ => {}
        }
        self.root(runtime, JsString::from_static(""))
    }
    fn number_gap(
        self: Box<Self>,
        runtime: &Runtime,
        number: f64,
    ) -> JsonStringifyResult<StringifyStep> {
        let count = if number.is_nan() || number <= 0.0 {
            0
        } else if number >= 10.0 {
            10
        } else {
            number.trunc() as usize
        };
        self.root(runtime, JsString::try_from_utf8(&" ".repeat(count))?)
    }
    fn root(
        mut self: Box<Self>,
        runtime: &Runtime,
        gap: JsString,
    ) -> JsonStringifyResult<StringifyStep> {
        self.gap = gap;
        let holder = runtime.new_ordinary_object_in_realm(self.realm)?;
        let key = runtime.intern_property_key("")?;
        if !runtime.define_own_property(
            &holder,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(self.root.clone()),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(true),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "fresh JSON.stringify root definition was rejected",
            )
            .into());
        }
        self.to_json_key = Some(runtime.intern_property_key("toJSON")?);
        self.output = JsStringBuilder::new(256);
        let value = std::mem::replace(&mut self.root, Value::Undefined);
        self.check(
            runtime,
            Check {
                holder,
                value,
                key: JsString::from_static(""),
                destination: Destination::Root,
            },
        )
    }
    fn check(
        self: Box<Self>,
        runtime: &Runtime,
        check: Check,
    ) -> JsonStringifyResult<StringifyStep> {
        if matches!(check.value, Value::Object(_) | Value::BigInt(_)) {
            Ok(StringifyStep::Read {
                receiver: check.value.clone(),
                key: self
                    .to_json_key
                    .clone()
                    .ok_or(RuntimeError::Invariant("JSON stringify lost toJSON key"))?,
                resume: StringifyResume {
                    state: self,
                    phase: Phase::ToJson(check),
                },
            })
        } else {
            self.replacer(runtime, check)
        }
    }
    fn replacer(
        self: Box<Self>,
        runtime: &Runtime,
        check: Check,
    ) -> JsonStringifyResult<StringifyStep> {
        if let Some(callable) = &self.replacer {
            let mut arguments = Vec::new();
            if arguments.try_reserve_exact(2).is_err() {
                return Err(JsonStringifyFailure::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Internal,
                    "out of memory",
                )?));
            }
            arguments.push(Value::String(check.key.clone()));
            arguments.push(check.value.clone());
            return Ok(StringifyStep::Call {
                callable: callable.clone(),
                receiver: Value::Object(check.holder.clone()),
                arguments,
                resume: StringifyResume {
                    state: self,
                    phase: Phase::Replacer(check),
                },
            });
        }
        self.checked(runtime, check)
    }
    fn checked(
        mut self: Box<Self>,
        runtime: &Runtime,
        check: Check,
    ) -> JsonStringifyResult<StringifyStep> {
        let accepted = match &check.value {
            Value::Undefined | Value::Symbol(_) => false,
            Value::Object(object) => runtime.as_callable(object)?.is_none(),
            _ => true,
        };
        match check.destination {
            Destination::Root => {
                if !accepted {
                    return Ok(StringifyStep::Complete(Completion::Return(
                        Value::Undefined,
                    )));
                }
                self.tasks.push(Task::Value {
                    value: check.value,
                    indent: JsString::from_static(""),
                });
            }
            Destination::Array(task) => {
                let Task::ArrayElement { next_indent, .. } = &task else {
                    return Err(
                        RuntimeError::Invariant("JSON array continuation lost its task").into(),
                    );
                };
                let indent = next_indent.clone();
                self.tasks.push(task);
                if accepted {
                    self.tasks.push(Task::Value {
                        value: check.value,
                        indent,
                    });
                } else {
                    self.output.push_utf8("null")?;
                }
            }
            Destination::Object(mut task) => {
                let Task::ObjectProperty {
                    has_content,
                    next_indent,
                    ..
                } = &mut task
                else {
                    return Err(
                        RuntimeError::Invariant("JSON object continuation lost its task").into(),
                    );
                };
                let indent = next_indent.clone();
                if accepted {
                    if *has_content {
                        self.output.push_utf8(",")?;
                    }
                    self.append_separator(&indent)?;
                    self.append_quoted(&check.key)?;
                    self.output.push_utf8(":")?;
                    if !self.gap.is_empty() {
                        self.output.push_utf8(" ")?;
                    }
                    *has_content = true;
                }
                self.tasks.push(task);
                if accepted {
                    self.tasks.push(Task::Value {
                        value: check.value,
                        indent,
                    });
                }
            }
        }
        self.advance(runtime)
    }
    fn advance(mut self: Box<Self>, runtime: &Runtime) -> JsonStringifyResult<StringifyStep> {
        while let Some(task) = self.tasks.pop() {
            match task {
                Task::Value { mut value, indent } => loop {
                    match value {
                        Value::String(value) => {
                            self.append_quoted(&value)?;
                            break;
                        }
                        Value::Int(value) => {
                            self.output.push_utf8(&value.to_string())?;
                            break;
                        }
                        Value::Float(value) if value.is_finite() => {
                            self.output
                                .push_js_string(&Value::Float(value).to_js_string()?)?;
                            break;
                        }
                        Value::Float(_) | Value::Null => {
                            self.output.push_utf8("null")?;
                            break;
                        }
                        Value::Bool(value) => {
                            self.output
                                .push_utf8(if value { "true" } else { "false" })?;
                            break;
                        }
                        Value::BigInt(_) => {
                            return Err(JsonStringifyFailure::Throw(runtime.new_native_error(
                                self.realm,
                                NativeErrorKind::Type,
                                "Do not know how to serialize a BigInt",
                            )?));
                        }
                        Value::Object(object) => {
                            if runtime.is_raw_json_object(&object)? {
                                return self.read(runtime, object, "rawJSON", Phase::Raw);
                            }
                            match runtime.json_wrapper_kind(&object)? {
                                JsonWrapperKind::String => {
                                    return Ok(StringifyStep::String {
                                        value: Value::Object(object),
                                        resume: StringifyResume {
                                            state: self,
                                            phase: Phase::WrapperString,
                                        },
                                    });
                                }
                                JsonWrapperKind::Number => {
                                    return Ok(StringifyStep::Number {
                                        value: Value::Object(object),
                                        resume: StringifyResume {
                                            state: self,
                                            phase: Phase::WrapperNumber(indent),
                                        },
                                    });
                                }
                                JsonWrapperKind::Boolean(boolean) => value = Value::Bool(boolean),
                                JsonWrapperKind::BigInt(bigint) => value = Value::BigInt(bigint),
                                JsonWrapperKind::Other => {
                                    return self.begin(runtime, object, indent);
                                }
                            }
                        }
                        Value::Undefined | Value::Symbol(_) => break,
                    }
                },
                Task::ArrayElement {
                    array,
                    index,
                    length,
                    indent,
                    next_indent,
                } => {
                    if index == length {
                        if length > 0 && !self.gap.is_empty() {
                            self.output.push_utf8("\n")?;
                            self.output.push_js_string(&indent)?;
                        }
                        self.output.push_utf8("]")?;
                        self.pop_object(&array);
                        continue;
                    }
                    if index > 0 {
                        self.output.push_utf8(",")?;
                    }
                    self.append_separator(&next_indent)?;
                    let name = index.to_string();
                    let key = runtime.intern_property_key(&name)?;
                    let destination = Destination::Array(Task::ArrayElement {
                        array: array.clone(),
                        index: index + 1,
                        length,
                        indent,
                        next_indent,
                    });
                    return Ok(StringifyStep::Read {
                        receiver: Value::Object(array.clone()),
                        key,
                        resume: StringifyResume {
                            state: self,
                            phase: Phase::ReadCheck {
                                holder: array,
                                key: JsString::try_from_utf8(&name)?,
                                destination,
                            },
                        },
                    });
                }
                Task::ObjectProperty {
                    object,
                    keys,
                    index,
                    has_content,
                    indent,
                    next_indent,
                } => {
                    let Some(name) = keys.get(index).cloned() else {
                        if has_content && !self.gap.is_empty() {
                            self.output.push_utf8("\n")?;
                            self.output.push_js_string(&indent)?;
                        }
                        self.output.push_utf8("}")?;
                        self.pop_object(&object);
                        continue;
                    };
                    let key = runtime.intern_property_key_js_string(&name)?;
                    let destination = Destination::Object(Task::ObjectProperty {
                        object: object.clone(),
                        keys,
                        index: index + 1,
                        has_content,
                        indent,
                        next_indent,
                    });
                    return Ok(StringifyStep::Read {
                        receiver: Value::Object(object.clone()),
                        key,
                        resume: StringifyResume {
                            state: self,
                            phase: Phase::ReadCheck {
                                holder: object,
                                key: name,
                                destination,
                            },
                        },
                    });
                }
            }
        }
        Ok(StringifyStep::Complete(Completion::Return(Value::String(
            self.output.finish()?,
        ))))
    }
    fn begin(
        mut self: Box<Self>,
        runtime: &Runtime,
        object: ObjectRef,
        indent: JsString,
    ) -> JsonStringifyResult<StringifyStep> {
        if self.stack.iter().any(|ancestor| ancestor == &object) {
            return Err(JsonStringifyFailure::Throw(runtime.new_native_error(
                self.realm,
                NativeErrorKind::Type,
                "circular reference",
            )?));
        }
        let next_indent = indent.try_concat(&self.gap)?;
        self.stack.push(object.clone());
        let start = ObjectStart {
            object: object.clone(),
            indent,
            next_indent,
        };
        if converted(runtime.internal_is_array(self.realm, &Value::Object(object.clone()))?)? {
            return self.read(runtime, object, "length", Phase::ArrayLength(start));
        }
        if let Some(keys) = &self.property_list {
            let keys = keys.clone();
            return self.object_ready(runtime, start, keys);
        }
        Ok(StringifyStep::Keys {
            object,
            resume: StringifyResume {
                state: self,
                phase: Phase::ObjectKeys(start),
            },
        })
    }
    fn enumerate(
        self: Box<Self>,
        runtime: &Runtime,
        start: ObjectStart,
        mut remaining: std::vec::IntoIter<PropertyKey>,
        keys: Vec<JsString>,
    ) -> JsonStringifyResult<StringifyStep> {
        for key in remaining.by_ref() {
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
            return Ok(StringifyStep::Enumerable {
                object: start.object.clone(),
                key: key.clone(),
                resume: StringifyResume {
                    state: self,
                    phase: Phase::Enumerable {
                        start,
                        remaining,
                        keys,
                        key,
                    },
                },
            });
        }
        self.object_ready(runtime, start, keys)
    }
    fn object_ready(
        mut self: Box<Self>,
        runtime: &Runtime,
        start: ObjectStart,
        keys: Vec<JsString>,
    ) -> JsonStringifyResult<StringifyStep> {
        self.output.push_utf8("{")?;
        self.tasks.push(Task::ObjectProperty {
            object: start.object,
            keys,
            index: 0,
            has_content: false,
            indent: start.indent,
            next_indent: start.next_indent,
        });
        self.advance(runtime)
    }
}
impl StringifyResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<StringifyStep, RuntimeError> {
        result(self.value(runtime, reply))
    }
    fn value(self, runtime: &Runtime, reply: Completion) -> JsonStringifyResult<StringifyStep> {
        let value = returned(reply)?;
        let state = self.state;
        match self.phase {
            Phase::ListLength(object) => Ok(StringifyStep::Number {
                value,
                resume: Self {
                    state,
                    phase: Phase::ListNumber(object),
                },
            }),
            Phase::ListItem(mut list) => {
                let string = match &value {
                    Value::String(_) | Value::Int(_) | Value::Float(_) => true,
                    Value::Object(object) => matches!(
                        runtime.json_wrapper_kind(object)?,
                        JsonWrapperKind::String | JsonWrapperKind::Number
                    ),
                    _ => false,
                };
                if string {
                    Ok(StringifyStep::String {
                        value,
                        resume: Self {
                            state,
                            phase: Phase::ListString(list),
                        },
                    })
                } else {
                    list.index += 1;
                    state.list(runtime, list)
                }
            }
            Phase::ReadCheck {
                holder,
                key,
                destination,
            } => state.check(
                runtime,
                Check {
                    holder,
                    value,
                    key,
                    destination,
                },
            ),
            Phase::ToJson(check) => {
                if let Value::Object(object) = value
                    && let Some(callable) = runtime.as_callable(&object)?
                {
                    let mut arguments = Vec::new();
                    if arguments.try_reserve_exact(1).is_err() {
                        return Err(JsonStringifyFailure::Throw(runtime.new_native_error(
                            state.realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?));
                    }
                    arguments.push(Value::String(check.key.clone()));
                    Ok(StringifyStep::Call {
                        callable,
                        receiver: check.value.clone(),
                        arguments,
                        resume: Self {
                            state,
                            phase: Phase::ToJsonCall(check),
                        },
                    })
                } else {
                    state.replacer(runtime, check)
                }
            }
            Phase::ToJsonCall(mut check) => {
                check.value = value;
                state.replacer(runtime, check)
            }
            Phase::Replacer(mut check) => {
                check.value = value;
                state.checked(runtime, check)
            }
            Phase::Raw => {
                let Value::String(source) = value else {
                    return Err(RuntimeError::Invariant(
                        "Raw JSON branded object lost its source string",
                    )
                    .into());
                };
                let mut state = state;
                state.output.push_js_string(&source)?;
                state.advance(runtime)
            }
            Phase::ArrayLength(start) => Ok(StringifyStep::Number {
                value,
                resume: Self {
                    state,
                    phase: Phase::ArrayNumber(start),
                },
            }),
            _ => Err(RuntimeError::Invariant("JSON stringify unexpected value reply").into()),
        }
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        reply: NativeConversion<JsString>,
    ) -> Result<StringifyStep, RuntimeError> {
        result((|| {
            let value = converted(reply)?;
            match self.phase {
                Phase::ListString(mut list) => {
                    if !list.items.iter().any(|item| item == &value) {
                        list.items.push(value);
                    }
                    list.index += 1;
                    self.state.list(runtime, list)
                }
                Phase::GapString => {
                    let gap = value.sub_string(0, value.len().min(10));
                    self.state.root(runtime, gap)
                }
                Phase::WrapperString => {
                    let mut state = self.state;
                    state.append_quoted(&value)?;
                    state.advance(runtime)
                }
                _ => Err(RuntimeError::Invariant("JSON stringify unexpected string reply").into()),
            }
        })())
    }
    pub(crate) fn number(
        self,
        runtime: &Runtime,
        reply: NativeConversion<f64>,
    ) -> Result<StringifyStep, RuntimeError> {
        result((|| {
            let value = converted(reply)?;
            let mut state = self.state;
            match self.phase {
                Phase::ListNumber(object) => state.list(
                    runtime,
                    List {
                        object,
                        index: 0,
                        length: Runtime::length_from_number(value),
                        items: Vec::new(),
                    },
                ),
                Phase::GapNumber => state.number_gap(runtime, value),
                Phase::WrapperNumber(indent) => {
                    state.tasks.push(Task::Value {
                        value: Value::number(value),
                        indent,
                    });
                    state.advance(runtime)
                }
                Phase::ArrayNumber(start) => {
                    state.output.push_utf8("[")?;
                    state.tasks.push(Task::ArrayElement {
                        array: start.object,
                        index: 0,
                        length: Runtime::length_from_number(value),
                        indent: start.indent,
                        next_indent: start.next_indent,
                    });
                    state.advance(runtime)
                }
                _ => Err(RuntimeError::Invariant("JSON stringify unexpected number reply").into()),
            }
        })())
    }
    pub(crate) fn keys(
        self,
        runtime: &Runtime,
        reply: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<StringifyStep, RuntimeError> {
        result((|| {
            let keys = converted(reply)?;
            let Phase::ObjectKeys(start) = self.phase else {
                return Err(RuntimeError::Invariant("JSON stringify unexpected keys reply").into());
            };
            self.state
                .enumerate(runtime, start, keys.into_iter(), Vec::new())
        })())
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<StringifyStep, RuntimeError> {
        result((|| {
            let value = converted(reply)?;
            let Phase::Enumerable {
                start,
                remaining,
                mut keys,
                key,
            } = self.phase
            else {
                return Err(
                    RuntimeError::Invariant("JSON stringify unexpected boolean reply").into(),
                );
            };
            if value {
                keys.push(runtime.0.state.borrow().atoms.to_js_string(key.atom())?);
            }
            self.state.enumerate(runtime, start, remaining, keys)
        })())
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringifyStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringifyStep::Complete(result) => return Ok(result),
            StringifyStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            StringifyStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            StringifyStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            StringifyStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            StringifyStep::Enumerable {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_own_property_is_enumerable(realm, &object, &key)?,
            )?,
            StringifyStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &arguments)?,
            )?,
        };
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    fn pending_to_json_keeps_ancestors_detached_value_and_replacer_until_abandonment() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let root = context.eval("({x:{}})").unwrap();
        let replacer = context.eval("(function(k,v){return v})").unwrap();
        let to_json = context.eval("(function(){return 1})").unwrap();
        let Value::Object(replacer_object) = &replacer else {
            panic!("expected replacer");
        };
        let replacer_id = replacer_object.object_id();
        let Value::Object(to_json_object) = &to_json else {
            panic!("expected toJSON callback");
        };
        let to_json_id = to_json_object.object_id();
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![root, replacer, Value::Undefined],
        };
        let StringifyStep::Read { resume, .. } =
            StringifyStep::start(&runtime, context.realm, &arguments).unwrap()
        else {
            panic!("expected root toJSON lookup");
        };
        drop(arguments);
        let StringifyStep::Call {
            arguments, resume, ..
        } = resume
            .resume(&runtime, Completion::Return(Value::Undefined))
            .unwrap()
        else {
            panic!("expected root replacer");
        };
        let root = arguments[1].clone();
        drop(arguments);
        let StringifyStep::Keys { object, resume } =
            resume.resume(&runtime, Completion::Return(root)).unwrap()
        else {
            panic!("expected object keys");
        };
        let keys = runtime
            .internal_own_property_keys(context.realm, &object)
            .unwrap();
        let root_id = object.object_id();
        drop(object);
        let StringifyStep::Enumerable { resume, .. } = resume.keys(&runtime, keys).unwrap() else {
            panic!("expected enumerable check");
        };
        let StringifyStep::Read {
            receiver: Value::Object(root),
            key,
            resume,
        } = resume
            .boolean(&runtime, NativeConversion::Value(true))
            .unwrap()
        else {
            panic!("expected property read");
        };
        let child = runtime
            .get_property_in_realm(context.realm, &root, &key)
            .unwrap();
        let Completion::Return(Value::Object(child_object)) = &child else {
            panic!("expected child");
        };
        let child_id = child_object.object_id();
        runtime
            .internal_delete_property(context.realm, &root, &key)
            .unwrap();
        drop(root);
        drop(key);
        let StringifyStep::Read { resume, .. } = resume.resume(&runtime, child).unwrap() else {
            panic!("expected child toJSON lookup");
        };
        let step = resume
            .resume(&runtime, Completion::Return(to_json))
            .unwrap();
        assert!(matches!(step, StringifyStep::Call { .. }));
        runtime.run_gc().unwrap();
        let ids = [root_id, child_id, replacer_id, to_json_id];
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        }
        drop(step);
        runtime.run_gc().unwrap();
        for id in ids {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

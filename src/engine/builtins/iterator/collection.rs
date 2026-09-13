//! Collection construction shares iterator acquisition and exact entry close lifetimes.
use super::{
    ObjectIteratorStep,
    step::{CloseStep, NextStep, finish_close, finish_next},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        native::{
            MapNativeKind, NativeFunctionId, SetNativeKind, WeakMapNativeKind, WeakSetNativeKind,
        },
        weak_collection::WeakCollectionKind,
    },
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{ConstructorPrototypeSource, NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum CollectionKind {
    Map,
    Set,
    WeakMap,
    WeakSet,
}
impl CollectionKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::Map(MapNativeKind::Constructor) => Some(Self::Map),
            NativeFunctionId::Set(SetNativeKind::Constructor) => Some(Self::Set),
            NativeFunctionId::WeakMap(WeakMapNativeKind::Constructor) => Some(Self::WeakMap),
            NativeFunctionId::WeakSet(WeakSetNativeKind::Constructor) => Some(Self::WeakSet),
            _ => None,
        }
    }
    fn pairs(self) -> bool {
        matches!(self, Self::Map | Self::WeakMap)
    }
}
pub(crate) enum CollectionStep {
    Complete(Completion),
    Prototype {
        new_target: Value,
        resume: CollectionResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: CollectionResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: CollectionResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: CollectionResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
        resume: CollectionResume,
    },
}
pub(crate) struct CollectionResume {
    realm: ContextId,
    kind: CollectionKind,
    collection: Option<ObjectRef>,
    iterable: Option<Value>,
    iterator: Option<ObjectRef>,
    next: Value,
    adder: Option<CallableRef>,
    phase: Phase,
    closing: bool,
}
enum Phase {
    Prototype,
    Adder,
    Method,
    Iterator,
    NextMethod,
    Next,
    Key(ObjectRef),
    Value { item: ObjectRef, key: Value },
    Add(Option<ObjectRef>),
}
impl CollectionStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: CollectionKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "collection constructor did not receive a constructor invocation",
            ));
        };
        let _ = runtime;
        Ok(Self::Prototype {
            new_target: new_target.clone(),
            resume: CollectionResume {
                realm,
                kind,
                collection: None,
                iterable: if arguments.actual_arg_count == 0 {
                    None
                } else {
                    Some(
                        arguments
                            .readable
                            .first()
                            .cloned()
                            .ok_or(RuntimeError::Invariant(
                                "collection iterable argv was not padded",
                            ))?,
                    )
                },
                iterator: None,
                next: Value::Undefined,
                adder: None,
                phase: Phase::Prototype,
                closing: false,
            },
        })
    }
}
impl CollectionResume {
    fn collection(&self) -> Result<ObjectRef, RuntimeError> {
        self.collection
            .clone()
            .ok_or(RuntimeError::Invariant("collection result missing"))
    }
    fn iterator(&self) -> Result<ObjectRef, RuntimeError> {
        self.iterator
            .clone()
            .ok_or(RuntimeError::Invariant("collection iterator missing"))
    }
    fn abrupt(mut self, value: Value) -> Result<CollectionStep, RuntimeError> {
        if matches!(
            self.phase,
            Phase::Key(_) | Phase::Value { .. } | Phase::Add(_)
        ) {
            // WeakMap explicitly releases the yielded pair and key before
            // close. Strong Map retains that pair until its ordinary exit.
            if !matches!(self.kind, CollectionKind::Map) {
                self.phase = Phase::Next;
            }
            self.closing = true;
            return Ok(CollectionStep::Close {
                iterator: self.iterator()?,
                completion: Completion::Throw(value),
                resume: self,
            });
        }
        Ok(CollectionStep::Complete(Completion::Throw(value)))
    }
    fn next_step(mut self) -> Result<CollectionStep, RuntimeError> {
        self.phase = Phase::Next;
        Ok(CollectionStep::Next {
            iterator: self.iterator()?,
            method: self.next.clone(),
            resume: self,
        })
    }
    pub(crate) fn prototype(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<CollectionStep, RuntimeError> {
        if !matches!(self.phase, Phase::Prototype) {
            return Err(RuntimeError::Invariant(
                "collection prototype phase mismatch",
            ));
        }
        let prototype = match reply {
            NativeConversion::Throw(value) => {
                return Ok(CollectionStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(prototype)) => prototype,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                let prototype = match self.kind {
                    CollectionKind::Map => runtime.map_realm_data(realm)?.prototype,
                    CollectionKind::Set => runtime.set_realm_data(realm)?.prototype,
                    CollectionKind::WeakMap => {
                        runtime.weak_collection_prototype(realm, WeakCollectionKind::Map)?
                    }
                    CollectionKind::WeakSet => {
                        runtime.weak_collection_prototype(realm, WeakCollectionKind::Set)?
                    }
                };
                ObjectRef::from_borrowed_handle(runtime.clone(), prototype)?
            }
        };
        let collection = match self.kind {
            CollectionKind::Map => runtime.new_map_object(&prototype)?,
            CollectionKind::Set => runtime.new_set_object(&prototype)?,
            CollectionKind::WeakMap => {
                runtime.new_weak_collection_object(&prototype, WeakCollectionKind::Map)?
            }
            CollectionKind::WeakSet => {
                runtime.new_weak_collection_object(&prototype, WeakCollectionKind::Set)?
            }
        };
        self.collection = Some(collection.clone());
        if self
            .iterable
            .as_ref()
            .is_none_or(|value| matches!(value, Value::Null | Value::Undefined))
        {
            return Ok(CollectionStep::Complete(Completion::Return(Value::Object(
                collection,
            ))));
        }
        self.phase = Phase::Adder;
        Ok(CollectionStep::Read {
            receiver: Value::Object(collection),
            key: runtime.intern_property_key(if self.kind.pairs() { "set" } else { "add" })?,
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CollectionStep, RuntimeError> {
        if self.closing {
            return Ok(CollectionStep::Complete(reply));
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return self.abrupt(value),
        };
        match std::mem::replace(&mut self.phase, Phase::Next) {
            Phase::Adder => {
                let callback = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callback) = callback else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "set/add is not a function",
                    )?;
                    return self.abrupt(error);
                };
                self.adder = Some(callback);
                self.phase = Phase::Method;
                Ok(CollectionStep::Read {
                    receiver: self
                        .iterable
                        .clone()
                        .ok_or(RuntimeError::Invariant("collection iterable missing"))?,
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
                    resume: self,
                })
            }
            Phase::Method => {
                let callback = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callback else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    return self.abrupt(error);
                };
                self.phase = Phase::Iterator;
                Ok(CollectionStep::Call {
                    callable,
                    receiver: self
                        .iterable
                        .take()
                        .ok_or(RuntimeError::Invariant("collection iterable missing"))?,
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Phase::Iterator => {
                let Value::Object(iterator) = value else {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.abrupt(error);
                };
                self.iterator = Some(iterator.clone());
                self.phase = Phase::NextMethod;
                Ok(CollectionStep::Read {
                    receiver: Value::Object(iterator),
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                self.next = value;
                self.next_step()
            }
            Phase::Key(item) => {
                self.phase = Phase::Value {
                    item: item.clone(),
                    key: value,
                };
                Ok(CollectionStep::Read {
                    receiver: Value::Object(item),
                    key: runtime.intern_property_key("1")?,
                    resume: self,
                })
            }
            Phase::Value { item, key } => {
                self.phase = Phase::Add(if matches!(self.kind, CollectionKind::Map) {
                    Some(item)
                } else {
                    None
                });
                self.add(vec![key, value])
            }
            Phase::Add(entry) => {
                drop(entry);
                self.next_step()
            }
            _ => Err(RuntimeError::Invariant(
                "collection completion phase mismatch",
            )),
        }
    }
    fn add(self, arguments: Vec<Value>) -> Result<CollectionStep, RuntimeError> {
        Ok(CollectionStep::Call {
            callable: self
                .adder
                .clone()
                .ok_or(RuntimeError::Invariant("collection adder missing"))?,
            receiver: Value::Object(self.collection()?),
            arguments,
            resume: self,
        })
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<CollectionStep, RuntimeError> {
        if !matches!(self.phase, Phase::Next) {
            return Err(RuntimeError::Invariant("collection next phase mismatch"));
        }
        let item = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(CollectionStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Done => {
                return Ok(CollectionStep::Complete(Completion::Return(Value::Object(
                    self.collection()?,
                ))));
            }
            ObjectIteratorStep::Yield(value) => value,
        };
        if !self.kind.pairs() {
            self.phase = Phase::Add(None);
            return self.add(vec![item]);
        }
        let Value::Object(item) = item else {
            let error =
                runtime.new_native_error(self.realm, NativeErrorKind::Type, "not an object")?;
            drop(item);
            self.phase = Phase::Add(None);
            return self.abrupt(error);
        };
        self.phase = Phase::Key(item.clone());
        Ok(CollectionStep::Read {
            receiver: Value::Object(item),
            key: runtime.intern_property_key("0")?,
            resume: self,
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CollectionStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CollectionStep::Complete(result) => return Ok(result),
            CollectionStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                runtime.constructor_prototype_source(realm, &new_target)?,
            )?,
            CollectionStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            CollectionStep::Call {
                callable,
                receiver,
                arguments,
                resume,
            } => {
                let result = runtime.call_internal(realm, &callable, receiver, &arguments)?;
                drop(arguments);
                resume.resume(runtime, result)?
            }
            CollectionStep::Next {
                iterator,
                method,
                resume,
            } => resume.next(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::start(runtime, realm, iterator, method)?,
                )?,
            )?,
            CollectionStep::Close {
                iterator,
                completion,
                resume,
            } => resume.resume(
                runtime,
                finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                )?,
            )?,
        };
    }
}

#[cfg(all(test, feature = "stack-vm", feature = "profiling"))]
mod owned_tests {
    use super::*;
    use crate::engine::api::profiling::CostProfile;

    #[test]
    fn iterator_and_collection_callbacks_stay_on_owned_driver() {
        for source in [
            "Object.fromEntries({ [Symbol.iterator]() { let n=0; return { next() { return n++ ? {done:true} : {value:['x',42],done:false}; } }; } }).x",
            "Object.groupBy([21,21], x => 'x').x.reduce((a,b) => a+b)",
            "Map.groupBy([21,21], x => 0).get(0).reduce((a,b) => a+b)",
            "Iterator.from([41]).map(x=>x+1).next().value",
            "Iterator.from([41,42]).filter(x=>x>41).next().value",
            "Iterator.from([22]).flatMap(x=>[x,20]).reduce((a,b)=>a+b)",
            "Iterator.from([42]).take(1).next().value",
            "Iterator.from([20,21]).map(x=>x+1).filter(x=>x>21).flatMap(x=>[x,20]).take(3).reduce((a,b)=>a+b)",
            "Iterator.concat([20],[22]).toArray().reduce((a,b)=>a+b)",
            "Array.from({[Symbol.iterator](){let n=0;return {next(){return n++?{done:true}:{done:false,value:21}}}}},x=>x*2)[0]",
            "new Map([[{},21]]).getOrInsertComputed('x', key=>42)",
            "(function(){let n=0;new Map([[1,20],[2,22]]).forEach(v=>{n+=v});return n})()",
            "(function(){let n=0;new Set([20,22]).forEach(v=>{n+=v});return n})()",
            "new WeakMap().getOrInsertComputed({}, key=>42)",
            "new Set([20]).union({get size(){return 1},has(x){return x===22},keys(){return [22][Symbol.iterator]()}}).values().reduce((a,b)=>a+b)",
            "new Set([20,22]).intersection({get size(){return 3},has(x){return true},keys(){return [20,22,23][Symbol.iterator]()}}).values().reduce((a,b)=>a+b)",
            "(function(){let closed=0;let source={next(){return {done:false,value:42}},return(){closed++;return {done:true}}};let found=Iterator.from(source).some(x=>x===42);return found&&closed===1?42:0})()",
            "(function(){let n=0;let matcher={exec(){return n++?null:{get 0(){return {toString(){return ''}}},answer:42}},get lastIndex(){return {valueOf(){return 0}}},set lastIndex(value){}};let source={flags:'g',lastIndex:0,constructor:{[Symbol.species]:function(){return matcher}}};return RegExp.prototype[Symbol.matchAll].call(source,'x').next().value.answer})()",
            "(function(){let n=0;let matcher={exec(){return n++?null:{0:'x',answer:42}}};let source={flags:'g',lastIndex:0,constructor:{[Symbol.species]:function(){return matcher}}};return Array.from(RegExp.prototype[Symbol.matchAll].call(source,'x'))[0].answer})()",
            "(function(){let n=0;let matcher={exec(){return n++?null:{0:'x',answer:42}}};let source={flags:'g',lastIndex:0,constructor:{[Symbol.species]:function(){return matcher}}};return Iterator.from(RegExp.prototype[Symbol.matchAll].call(source,'x')).toArray()[0].answer})()",
            "String.fromCharCode({valueOf(){return 42}}).charCodeAt(0)",
            "String.fromCodePoint({valueOf(){return 42}}).codePointAt(0)",
            "+String.raw({get raw(){return {get length(){return {valueOf(){return 2}}},get 0(){return {toString(){return '4'}}},get 1(){return {toString(){return '2'}}}}}})",
            "(function(){let view=new Int32Array(1);Atomics.store(view,{valueOf(){return 0}},{valueOf(){return 42}});return Atomics.load(view,0)})()",
            "(function(){let view=new Int32Array(1);view[0]=20;Atomics.compareExchange(view,{valueOf(){return 0}},{valueOf(){return 20}},{valueOf(){return 42}});return view[0]})()",
            "(function(){let n=0;let view=new Int32Array(1);Atomics.notify(view,0,{valueOf(){n=42;return 1}});return n})()",
            "(function(){let n=0;let view=new Int32Array(new SharedArrayBuffer(4));try{Atomics.wait(view,0,{valueOf(){n+=20;return 0}},{valueOf(){n+=22;return 0}})}catch(e){}return n})()",
            "Atomics.isLockFree({valueOf(){return 4}})?42:0",
            "(function(){class C{};try{C()}catch(e){return e instanceof TypeError?42:0}return 0})()",
            "Math.sumPrecise({[Symbol.iterator](){let n=0;return {next(){return n++?{done:true}:{done:false,value:42}}}}})",
            "(function(){let target={...{get x(){return 42}}};let {x,...rest}=target;return x})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let profile = CostProfile::start();
            if source.contains("get 0(){") {
                profile.capture_disassembly();
            }
            let value = context.eval(source).unwrap_or_else(|error| {
                let exception = context.take_exception().unwrap();
                let message = match &exception {
                    Some(Value::Object(object)) => context
                        .get_property(object, &runtime.intern_property_key("message").unwrap())
                        .unwrap(),
                    _ => Value::Undefined,
                };
                panic!("{source}: {error:?}: {message:?}")
            });
            let costs = profile.snapshot();
            assert_eq!(value, Value::Int(42), "{source}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_sync_call_bridges, 0, "{source}: {costs:?}");
        }
    }
}

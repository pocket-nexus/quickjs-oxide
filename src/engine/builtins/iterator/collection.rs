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
    value::{JsValue, Value, conversion::NativeConversion},
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
    Prototype { resume: CollectionResume },
    Read { resume: CollectionResume },
    Call { resume: CollectionResume },
    Next { resume: CollectionResume },
    Close { resume: CollectionResume },
}
pub(crate) struct CollectionResume(Box<CollectionResumeState>);
impl std::ops::Deref for CollectionResume {
    type Target = CollectionResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for CollectionResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<CollectionResume>() <= 8);
pub(crate) struct CollectionResumeState {
    runtime: Runtime,
    pending_effect: CollectionStepPending,
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
impl Drop for CollectionResumeState {
    /// Release the internal edges the pending effect still owns when the
    /// request is abandoned. Consumption goes through `Option::take`, so a
    /// drained field is `None` here; releases are defer-safe and nothrow.
    fn drop(&mut self) {
        for value in [
            self.pending_effect.prototype_new_target.take(),
            self.pending_effect.read_receiver.take(),
            self.pending_effect.call_receiver.take(),
            self.pending_effect.next_method.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
    }
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
        Ok({
            let __pending_field_new_target = runtime.dup_jsvalue(new_target)?;
            let __pending_field_resume = CollectionResume(Box::new(CollectionResumeState {
                runtime: runtime.clone(),
                pending_effect: CollectionStepPending::default(),
                realm,
                kind,
                collection: None,
                iterable: if arguments.actual_arg_count == 0 {
                    None
                } else {
                    Some(runtime.root_value(arguments.readable.first().ok_or(
                        RuntimeError::Invariant("collection iterable argv was not padded"),
                    )?)?)
                },
                iterator: None,
                next: Value::Undefined,
                adder: None,
                phase: Phase::Prototype,
                closing: false,
            }));
            Self::request_prototype(__pending_field_new_target, __pending_field_resume)
        })
    }
}
impl CollectionResume {
    fn collection(&self) -> Result<ObjectRef, RuntimeError> {
        self.0
            .collection
            .clone()
            .ok_or(RuntimeError::Invariant("collection result missing"))
    }
    fn iterator(&self) -> Result<ObjectRef, RuntimeError> {
        self.0
            .iterator
            .clone()
            .ok_or(RuntimeError::Invariant("collection iterator missing"))
    }
    fn abrupt(mut self, runtime: &Runtime, value: Value) -> Result<CollectionStep, RuntimeError> {
        if matches!(
            self.0.phase,
            Phase::Key(_) | Phase::Value { .. } | Phase::Add(_)
        ) {
            // WeakMap explicitly releases the yielded pair and key before
            // close. Strong Map retains that pair until its ordinary exit.
            if !matches!(self.0.kind, CollectionKind::Map) {
                self.0.phase = Phase::Next;
            }
            self.0.closing = true;
            return Ok({
                let __pending_field_iterator = self.iterator()?;
                let __pending_field_completion = Completion::Throw(runtime.into_jsvalue(value)?);
                let __pending_field_resume = self;
                CollectionStep::request_close(
                    __pending_field_iterator,
                    __pending_field_completion,
                    __pending_field_resume,
                )
            });
        }
        Ok(CollectionStep::Complete(Completion::Throw(
            runtime.into_jsvalue(value)?,
        )))
    }
    fn next_step(mut self, runtime: &Runtime) -> Result<CollectionStep, RuntimeError> {
        self.0.phase = Phase::Next;
        Ok({
            let __pending_field_iterator = self.iterator()?;
            let __pending_field_method = runtime.into_jsvalue(self.0.next.clone())?;
            let __pending_field_resume = self;
            CollectionStep::request_next(
                __pending_field_iterator,
                __pending_field_method,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn prototype(
        mut self,
        runtime: &Runtime,
        reply: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<CollectionStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Prototype) {
            return Err(RuntimeError::Invariant(
                "collection prototype phase mismatch",
            ));
        }
        let prototype = match reply {
            NativeConversion::Throw(value) => {
                return Ok(CollectionStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
            }
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(prototype)) => prototype,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                let prototype = match self.0.kind {
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
        let collection = match self.0.kind {
            CollectionKind::Map => runtime.new_map_object(&prototype)?,
            CollectionKind::Set => runtime.new_set_object(&prototype)?,
            CollectionKind::WeakMap => {
                runtime.new_weak_collection_object(&prototype, WeakCollectionKind::Map)?
            }
            CollectionKind::WeakSet => {
                runtime.new_weak_collection_object(&prototype, WeakCollectionKind::Set)?
            }
        };
        self.0.collection = Some(collection.clone());
        if self
            .0
            .iterable
            .as_ref()
            .is_none_or(|value| matches!(value, Value::Null | Value::Undefined))
        {
            return Ok(CollectionStep::Complete(Completion::Return(
                runtime.into_jsvalue(Value::Object(collection))?,
            )));
        }
        self.0.phase = Phase::Adder;
        Ok({
            let __pending_field_receiver = runtime.into_jsvalue(Value::Object(collection))?;
            let __pending_field_key =
                runtime.intern_property_key(if self.0.kind.pairs() { "set" } else { "add" })?;
            let __pending_field_resume = self;
            CollectionStep::request_read(
                __pending_field_receiver,
                __pending_field_key,
                __pending_field_resume,
            )
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CollectionStep, RuntimeError> {
        if self.0.closing {
            return Ok(CollectionStep::Complete(reply));
        }
        let value = match reply {
            Completion::Return(value) => runtime.root_and_release_jsvalue(value)?,
            Completion::Throw(value) => {
                return self.abrupt(runtime, runtime.root_and_release_jsvalue(value)?);
            }
        };
        match std::mem::replace(&mut self.0.phase, Phase::Next) {
            Phase::Adder => {
                let callback = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callback) = callback else {
                    let error = runtime.new_native_error(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "set/add is not a function",
                    )?;
                    return self.abrupt(runtime, error);
                };
                self.0.adder = Some(callback);
                self.0.phase = Phase::Method;
                Ok({
                    let __pending_field_receiver = runtime.into_jsvalue(
                        self.0
                            .iterable
                            .clone()
                            .ok_or(RuntimeError::Invariant("collection iterable missing"))?,
                    )?;
                    let __pending_field_key =
                        PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator));
                    let __pending_field_resume = self;
                    CollectionStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Method => {
                let callback = match value {
                    Value::Object(ref object) => runtime.as_callable(object)?,
                    _ => None,
                };
                let Some(callable) = callback else {
                    let error = runtime.new_native_error(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    return self.abrupt(runtime, error);
                };
                self.0.phase = Phase::Iterator;
                Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver = runtime.into_jsvalue(
                        self.0
                            .iterable
                            .take()
                            .ok_or(RuntimeError::Invariant("collection iterable missing"))?,
                    )?;
                    let __pending_field_arguments = Vec::new();
                    let __pending_field_resume = self;
                    CollectionStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Iterator => {
                let Value::Object(iterator) = value else {
                    let error = runtime.new_native_error(
                        self.0.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?;
                    return self.abrupt(runtime, error);
                };
                self.0.iterator = Some(iterator.clone());
                self.0.phase = Phase::NextMethod;
                Ok({
                    let __pending_field_receiver = runtime.into_jsvalue(Value::Object(iterator))?;
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
                    let __pending_field_resume = self;
                    CollectionStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            Phase::NextMethod => {
                self.0.next = value;
                self.next_step(runtime)
            }
            Phase::Key(item) => {
                self.0.phase = Phase::Value {
                    item: item.clone(),
                    key: value,
                };
                Ok({
                    let __pending_field_receiver = runtime.into_jsvalue(Value::Object(item))?;
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal2)?;
                    let __pending_field_resume = self;
                    CollectionStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Value { item, key } => {
                self.0.phase = Phase::Add(if matches!(self.0.kind, CollectionKind::Map) {
                    Some(item)
                } else {
                    None
                });
                self.add(
                    runtime,
                    vec![runtime.into_jsvalue(key)?, runtime.into_jsvalue(value)?],
                )
            }
            Phase::Add(entry) => {
                drop(entry);
                self.next_step(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "collection completion phase mismatch",
            )),
        }
    }
    fn add(
        self,
        runtime: &Runtime,
        arguments: Vec<JsValue>,
    ) -> Result<CollectionStep, RuntimeError> {
        let __pending_field_callable = self
            .0
            .adder
            .clone()
            .ok_or(RuntimeError::Invariant("collection adder missing"))?;
        let __pending_field_receiver = runtime.into_jsvalue(Value::Object(self.collection()?))?;
        Ok(CollectionStep::request_call(
            __pending_field_callable,
            __pending_field_receiver,
            arguments,
            self,
        ))
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<CollectionStep, RuntimeError> {
        if !matches!(self.0.phase, Phase::Next) {
            return Err(RuntimeError::Invariant("collection next phase mismatch"));
        }
        let item = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(CollectionStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Done => {
                return Ok(CollectionStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::Object(self.collection()?))?,
                )));
            }
            ObjectIteratorStep::Yield(value) => runtime.root_and_release_jsvalue(value)?,
        };
        if !self.0.kind.pairs() {
            self.0.phase = Phase::Add(None);
            return self.add(runtime, vec![runtime.into_jsvalue(item)?]);
        }
        let Value::Object(item) = item else {
            let error =
                runtime.new_native_error(self.0.realm, NativeErrorKind::Type, "not an object")?;
            drop(item);
            self.0.phase = Phase::Add(None);
            return self.abrupt(runtime, error);
        };
        self.0.phase = Phase::Key(item.clone());
        Ok({
            let __pending_field_receiver = runtime.into_jsvalue(Value::Object(item))?;
            let __pending_field_key =
                runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Literal1)?;
            let __pending_field_resume = self;
            CollectionStep::request_read(
                __pending_field_receiver,
                __pending_field_key,
                __pending_field_resume,
            )
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
            CollectionStep::Prototype { mut resume } => {
                let new_target =
                    runtime.root_and_release_jsvalue(resume.take_prototype_new_target())?;
                resume.prototype(
                    runtime,
                    runtime.constructor_prototype_source(realm, &new_target)?,
                )?
            }
            CollectionStep::Read { mut resume } => {
                let receiver = runtime.root_and_release_jsvalue(resume.take_read_receiver())?;
                let key = resume.take_read_key();
                resume.resume(
                    runtime,
                    runtime.get_value_property_in_realm(realm, receiver, &key)?,
                )?
            }
            CollectionStep::Call { mut resume } => {
                let callable = resume.take_call_callable();
                let receiver = runtime.root_and_release_jsvalue(resume.take_call_receiver())?;
                let arguments = resume
                    .take_call_arguments()
                    .into_iter()
                    .map(|value| runtime.root_and_release_jsvalue(value))
                    .collect::<Result<Vec<_>, _>>()?;
                {
                    let result = runtime.call_internal(realm, &callable, receiver, &arguments)?;
                    drop(arguments);
                    resume.resume(runtime, result)?
                }
            }
            CollectionStep::Next { mut resume } => {
                let iterator = resume.take_next_iterator();
                let method = runtime.root_and_release_jsvalue(resume.take_next_method())?;
                resume.next(
                    runtime,
                    finish_next(
                        runtime,
                        realm,
                        NextStep::start(runtime, realm, iterator, method)?,
                    )?,
                )?
            }
            CollectionStep::Close { mut resume } => {
                let iterator = resume.take_close_iterator();
                let completion = resume.take_close_completion();
                resume.resume(
                    runtime,
                    finish_close(
                        runtime,
                        realm,
                        CloseStep::start(runtime, realm, iterator, completion)?,
                    )?,
                )?
            }
        };
    }
}

#[cfg(all(test, feature = "profiling"))]
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
                        .get_property(
                            object,
                            &runtime
                                .pinned_property_key(
                                    crate::engine::atom::pinned::PinnedAtom::Message,
                                )
                                .unwrap(),
                        )
                        .unwrap(),
                    _ => Value::Undefined,
                };
                panic!("{source}: {error:?}: {message:?}")
            });
            let _costs = profile.snapshot();
            assert_eq!(value, Value::Int(42), "{source}");
        }
    }
}

#[derive(Default)]
struct CollectionStepPending {
    prototype_new_target: Option<JsValue>,
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
    close_iterator: Option<ObjectRef>,
    close_completion: Option<Completion>,
}
impl CollectionStep {
    pub(crate) fn request_prototype(new_target: JsValue, mut resume: CollectionResume) -> Self {
        resume.0.pending_effect.prototype_new_target = Some(new_target);
        Self::Prototype { resume }
    }
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: CollectionResume,
    ) -> Self {
        resume.0.pending_effect.read_receiver = Some(receiver);
        resume.0.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: CollectionResume,
    ) -> Self {
        resume.0.pending_effect.call_callable = Some(callable);
        resume.0.pending_effect.call_receiver = Some(receiver);
        resume.0.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: CollectionResume,
    ) -> Self {
        resume.0.pending_effect.next_iterator = Some(iterator);
        resume.0.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
    pub(crate) fn request_close(
        iterator: ObjectRef,
        completion: Completion,
        mut resume: CollectionResume,
    ) -> Self {
        resume.0.pending_effect.close_iterator = Some(iterator);
        resume.0.pending_effect.close_completion = Some(completion);
        Self::Close { resume }
    }
}
impl CollectionResume {
    pub(crate) fn take_prototype_new_target(&mut self) -> JsValue {
        self.0
            .pending_effect
            .prototype_new_target
            .take()
            .expect("CollectionStep Prototype new_target")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .read_receiver
            .take()
            .expect("CollectionStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.0
            .pending_effect
            .read_key
            .take()
            .expect("CollectionStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.0
            .pending_effect
            .call_callable
            .take()
            .expect("CollectionStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.0
            .pending_effect
            .call_receiver
            .take()
            .expect("CollectionStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.0
            .pending_effect
            .call_arguments
            .take()
            .expect("CollectionStep Call arguments")
    }
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .next_iterator
            .take()
            .expect("CollectionStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.0
            .pending_effect
            .next_method
            .take()
            .expect("CollectionStep Next method")
    }
    pub(crate) fn take_close_iterator(&mut self) -> ObjectRef {
        self.0
            .pending_effect
            .close_iterator
            .take()
            .expect("CollectionStep Close iterator")
    }
    pub(crate) fn take_close_completion(&mut self) -> Completion {
        self.0
            .pending_effect
            .close_completion
            .take()
            .expect("CollectionStep Close completion")
    }
}
const _: () = assert!(std::mem::size_of::<CollectionStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<CollectionStep>() <= 64);

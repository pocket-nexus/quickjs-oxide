//! Finish no-callback slice stages before transporting a waiting domain state.
use super::*;
use crate::engine::object::OrdinaryRead;
use crate::engine::value::conversion::number::NumberStep;

impl SliceStep {
    pub(super) fn advance_local(
        mut self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Self, RuntimeError> {
        loop {
            self = match self {
                Self::Read { mut resume } => {
                    let (object, key) = resume.take_read();
                    let receiver = JsValue::Object(object.object_id());
                    match runtime.prepare_ordinary_read_selected(&object, &key, &receiver, None)? {
                        OrdinaryRead::Complete(value) => {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "array_slice_local_read",
                            );
                            resume.resume_once(
                                runtime,
                                Completion::Return(value.unwrap_or(JsValue::Undefined)),
                            )?
                        }
                        read => return Ok(Self::make_preparedread(read, key, resume)),
                    }
                }
                Self::Number { mut resume }
                    if !matches!(resume.0.pending.value.as_ref(), Some(JsValue::Object(_))) =>
                {
                    let (value,) = resume.take_number();
                    let NumberStep::Complete(result) =
                        NumberStep::start_jsvalue(runtime, realm, value)?
                    else {
                        return Err(RuntimeError::Invariant(
                            "primitive slice conversion suspended",
                        ));
                    };
                    resume.number_once(runtime, result)?
                }
                Self::Define { mut resume }
                    if direct_indexed_target(
                        runtime,
                        resume
                            .0
                            .pending
                            .object
                            .as_ref()
                            .expect("slice Define lost object"),
                        resume
                            .0
                            .pending
                            .key
                            .as_ref()
                            .expect("slice Define lost key"),
                    )? =>
                {
                    let (object, key, descriptor) = resume.take_define();
                    let result = define_local(runtime, realm, &object, &key, descriptor)?;
                    resume.defined_once(runtime, result)?
                }
                step => return Ok(step),
            };
        }
    }
}

pub(super) fn define_local(
    runtime: &Runtime,
    realm: ContextId,
    object: &ObjectRef,
    key: &PropertyKey,
    descriptor: OwnedPropertyDescriptor,
) -> Result<NativeConversion<InternalDefineResult>, RuntimeError> {
    let result = runtime.internal_define_owned_property(realm, object, key, descriptor)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("array_slice_local_define");
    Ok(result)
}

pub(super) fn direct_indexed_target(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
) -> Result<bool, RuntimeError> {
    if !object.belongs_to(runtime) || !key.atom().is_immediate_integer() {
        return Ok(false);
    }
    Ok(matches!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object(object.object_id())?
            .payload,
        crate::engine::heap::ObjectPayload::Ordinary
            | crate::engine::heap::ObjectPayload::Array { .. }
    ))
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn slice_local_progress_preserves_holes_getters_and_reentry() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let log='', source=[1,,3];
            let proto=Object.create(Array.prototype);
            Object.defineProperty(proto,'1',{get(){log+='g';source[2]=4;return 2;}});
            Object.setPrototypeOf(source,proto);
            let result=source.slice();
            let sparse=[1,,3].slice();
            let nested={length:1,get 0(){return [7,8].slice()[1];}};
            return log==='g' && result.join(',')==='1,2,4' && !(1 in sparse)
                && sparse.length===3 && Array.prototype.slice.call(nested)[0]===8;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn slice_local_progress_preserves_species_proxy_order_and_throws() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let source=[1,,3], trace=[], target;
            function Species(){target={};return new Proxy(target,{defineProperty(o,k,d){trace.push('d'+k);if(k==='0')source[2]=9;return Reflect.defineProperty(o,k,d);}});}
            source.constructor={[Symbol.species]:Species};
            let proxy=new Proxy(source,{has(o,k){trace.push('h'+k);return Reflect.has(o,k);},get(o,k,r){if(k==='0'||k==='1'||k==='2')trace.push('g'+k);return Reflect.get(o,k,r);}});
            let result=Array.prototype.slice.call(proxy);
            if(trace.join(',')!=='h0,g0,d0,h1,h2,g2,d2,dlength' || result[0]!==1 || result[2]!==9 || 1 in result)return false;
            let calls=0, caught=false;
            source.constructor={[Symbol.species]:function(){return new Proxy({}, {defineProperty(){calls++;return false;}});}};
            try{source.slice();}catch(e){caught=e instanceof TypeError;}
            let getterCalls=0, getterCaught=false;
            try{Array.prototype.slice.call({length:2,get 0(){getterCalls++;throw 42;}});}catch(e){getterCaught=e===42;}
            return caught && calls===1 && getterCaught && getterCalls===1;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn slice_prepared_has_keeps_selected_proxy_and_refreshes_following_read() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let calls='', source;
            let proto=new Proxy({}, {has(o,k){calls+='h';Object.setPrototypeOf(source,null);return true;},get(){calls+='g';return 9;}});
            source=Object.create(proto);source.length=1;
            let result=Array.prototype.slice.call(source);
            return calls==='h' && result.length===1 && 0 in result && result[0]===undefined;
        })()"#).unwrap(), Value::Bool(true));
    }

    #[test]
    fn slice_local_progress_keeps_splice_mutations_and_to_spliced_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let source=[1,2,3,4], removed=source.splice(1,2,8,9,10);
            let copied=source.toSpliced(1,2,7);
            return removed.join(',')==='2,3' && source.join(',')==='1,8,9,10,4'
                && copied.join(',')==='1,7,10,4';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    #[cfg(feature = "profiling")]
    fn slice_local_progress_reports_shared_read_has_define_hits() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context.eval("[1,2,3].slice().length").unwrap(),
            Value::Int(3)
        );
        let dense = profile.snapshot();
        assert_eq!(
            dense
                .owned_execution_events
                .get("array_slice_dense_batch")
                .copied()
                .unwrap_or(0),
            1
        );
        // The initial length Get precedes species selection and the dense
        // payload guard. Batching removes element reads, not that required Get.
        assert_eq!(
            dense
                .owned_execution_events
                .get("array_slice_local_read")
                .copied()
                .unwrap_or(0),
            1
        );
        for event in ["array_slice_local_has", "array_slice_local_define"] {
            assert_eq!(
                dense
                    .owned_execution_events
                    .get(event)
                    .copied()
                    .unwrap_or(0),
                0,
                "{event}"
            );
        }
        // Generic array-like storage cannot use the dense Array payload path.
        assert_eq!(
            context
                .eval("Array.prototype.slice.call({0:1,1:2,2:3,length:3}).length")
                .unwrap(),
            Value::Int(3)
        );
        let cost = profile.snapshot();
        for (event, expected) in [
            ("array_slice_local_read", 4), // length plus three present elements
            ("array_slice_local_has", 3),
            ("array_slice_local_define", 3),
        ] {
            let before = dense
                .owned_execution_events
                .get(event)
                .copied()
                .unwrap_or(0);
            let after = cost.owned_execution_events.get(event).copied().unwrap_or(0);
            assert_eq!(after - before, expected, "{event}");
        }
        assert_eq!(
            cost.owned_execution_events.get("array_slice_dense_batch"),
            dense.owned_execution_events.get("array_slice_dense_batch")
        );
    }
}

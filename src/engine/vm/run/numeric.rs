//! Primitive arithmetic completes without returning the resident run frame.
//! This is an allocation/release boundary, never a live RunSlots borrow.
use crate::engine::{
    api::{Error, runtime::Runtime},
    heap::ContextId,
    value::JsValue,
    vm::{
        exception::runtime_error_to_vm_error,
        numeric::operation::{NumericKind, primitive_output},
        stack::{FrameTransaction, RunSlots},
    },
};

#[inline]
pub(super) fn supported(slots: &RunSlots<'_>, kind: NumericKind) -> bool {
    // Check each operand with straight-line code. Iterating a range keeps the
    // peek Result in memory across the loop and drags its drop glue onto the
    // hot success path. Destructuring (instead of matching a temporary) moves
    // the Err variant out, so the Ok branch folds to a plain tag test.
    #[inline(always)]
    fn primitive_operand(slots: &RunSlots<'_>, offset: usize) -> bool {
        match slots.peek(offset) {
            Ok(value) => !matches!(value, JsValue::Object(_)),
            Err(error) => {
                drop(error);
                false
            }
        }
    }
    kind.primitive_arithmetic()
        && primitive_operand(slots, 0)
        && (kind.unary() || primitive_operand(slots, 1))
}

/// The caller published the exact arithmetic PC before entering this helper.
/// The output and possible exception stay here or in pending owner storage;
/// only a bool/engine-error crosses back into the resident dispatch frame.
///
/// Active-PC publication is deferred to the JavaScript error branch: a
/// successful primitive completion allocates only Rc/scalar storage and never
/// observes the stack, so the eager publication the caller previously performed
/// was pure bookkeeping.
#[inline(never)]
#[allow(
    clippy::too_many_arguments,
    reason = "Keep the cold numeric handoff explicit without widening the hot transaction."
)]
pub(super) fn complete(
    runtime: &Runtime,
    realm: ContextId,
    transaction: &mut FrameTransaction<'_>,
    kind: NumericKind,
    thrown: &mut Option<JsValue>,
    active_frame: super::super::frames::ActiveFrameToken,
    fault_pc: usize,
    cache_output: bool,
) -> Result<bool, Error> {
    let (left, right) = {
        let mut slots = transaction.canonical_slots("tos.spill.numeric");
        let right = slots.pop()?;
        if kind.unary() {
            (right, None)
        } else {
            (slots.pop()?, Some(right))
        }
    };
    // Parsing, BigInt allocation, Symbol release and error materialization all
    // occur after the input RunSlots has ended. Object coercion is never admitted.
    // The Err payload is handed to the outlined cold helper by reference so the
    // wide Error never travels through this frame's success path.
    let output = match primitive_output(runtime, kind, left, right) {
        Ok(output) => output,
        Err(error) => {
            return materialize_thrown(runtime, realm, error, thrown, active_frame, fault_pc);
        }
    };
    let mut value = Some(output.value);
    let mut previous = output.previous;
    #[cfg(not(any(test, oxide_owned_tos)))]
    let _ = cache_output;
    let committed = (|| {
        #[cfg(any(test, oxide_owned_tos))]
        if cache_output && previous.is_none() && transaction.cache_numeric_output(&mut value)? {
            debug_assert!(transaction.has_owned_numeric_output());
            return Ok(());
        }
        let mut slots = transaction.canonical_slots("tos.spill.numeric");
        if previous.is_some() {
            slots.push_pending(&mut previous)?;
        }
        slots.push_pending(&mut value)
    })();
    if let Err(error) = committed {
        transaction.canonicalize("tos.spill.numeric_error");
        // A postfix previous value may already be committed. Only the owners
        // still pending belong here; Option<JsValue> has no release-on-drop.
        // End the short slot borrow before releasing either pending edge.
        if let Some(value) = value {
            let _ = runtime.release_jsvalue(value);
        }
        if let Some(previous) = previous {
            let _ = runtime.release_jsvalue(previous);
        }
        return Err(error);
    }
    #[cfg(feature = "profiling")]
    {
        crate::engine::api::profiling::record_owned_execution_event(
            "numeric_completed_without_query",
        );
        crate::engine::api::profiling::record_owned_execution_event("numeric_completed_in_run");
        // Keep the existing same-JS-frame metric while distinguishing true
        // run residency with the new event above.
        crate::engine::api::profiling::record_owned_execution_event(
            "numeric_completed_in_same_frame",
        );
    }
    Ok(true)
}

/// Outlined JavaScript-error materialization for [`complete`]. Keeping the
/// engine-error passthrough, PC publication and native-error construction out
/// of the resident helper removes the wide Error temporaries and their drop
/// glue from the arithmetic success path.
#[cold]
#[inline(never)]
fn materialize_thrown(
    runtime: &Runtime,
    realm: ContextId,
    error: Error,
    thrown: &mut Option<JsValue>,
    active_frame: super::super::frames::ActiveFrameToken,
    fault_pc: usize,
) -> Result<bool, Error> {
    let Some(kind) =
        crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
    else {
        return Err(error);
    };
    // The JavaScript error is the only observation point for this PC.
    // The Symbol/BigInt pre-materialize gate already materialized the
    // frame for the cases that reach it; string-too-long may not have.
    if active_frame.is_materialized() {
        runtime
            .update_active_bytecode_pc(active_frame, crate::engine::vm::BytecodePc::new(fault_pc))
            .map_err(runtime_error_to_vm_error)?;
    }
    *thrown = Some(
        runtime
            .new_native_error_from_error_jsvalue(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    );
    Ok(false)
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::api::{Runtime, Value, profiling::CostProfile};

    #[test]
    fn numeric_non_number_loops_stay_in_one_run_transaction() {
        for (operand, body, resident) in [
            ("1.5", "r-=s", false),
            ("true", "r-=s", true),
            ("'12345.6'", "r-=s", true),
            ("'12345'", "r=s|0", true),
        ] {
            let mut authentication = None;
            for iterations in [4, 9] {
                let runtime = Runtime::new();
                let mut context = runtime.new_context();
                let profile = CostProfile::start();
                let result = context.eval(&format!("(function(){{var r=0,s={operand};for(var i=0;i<{iterations};i++){{{body};}}return r}})()")).unwrap();
                let expected = if body == "r=s|0" {
                    12345.0
                } else {
                    let number = match operand {
                        "1.5" => 1.5,
                        "true" => 1.0,
                        _ => 12345.6,
                    };
                    (0..iterations).fold(0.0, |r, _| r - number)
                };
                assert_eq!(result, Value::number(expected));
                let costs = profile.snapshot();
                let events = &costs.owned_execution_events;
                assert_eq!(
                    events.get("run_exit.Numeric").copied().unwrap_or(0),
                    0,
                    "{operand}: {events:?}"
                );
                assert_eq!(
                    events.get("numeric_completed_in_run").copied().unwrap_or(0),
                    if resident { iterations } else { 0 }
                );
                let current = events["slot_authentication"];
                if let Some(previous) = authentication {
                    assert_eq!(
                        current, previous,
                        "authentication must not grow with primitive iterations"
                    );
                }
                authentication = Some(current);
            }
        }
    }

    #[test]
    fn numeric_resident_operators_keep_errors_updates_and_object_callbacks() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "(()=>{let s='6';return s-'2'===4&&s*'2'===12&&s/'2'===3&&s%'4'===2&&s**'2'===36})()",
            "(()=>{let s='6';return (s<<'1')===12&&(s>>'1')===3&&(s>>>'1')===3&&(s&'3')===2&&(s|'3')===7&&(s^'3')===5})()",
            "(()=>{let s='2',a=s++,b=s--;return a===2&&b===3&&s===2&&(-'0')===0&&Object.is(-'0',-0)&&(~'0')===-1})()",
            "(()=>{let s=3n,a=s++,b=s--;return a===3n&&b===4n&&s===3n&&s*2n===6n})()",
            "(()=>{let log='';let a={valueOf(){log+='l';return 8}},b={valueOf(){log+='r';return 2}};return a-b===6&&log==='lr'})()",
            "(()=>{let log='';let a={valueOf(){log+='l';throw 9}},b={valueOf(){log+='r';return 2}};try{a-b}catch(e){return e===9&&log==='l'}return false})()",
            "(()=>{let n=0;try{Symbol()-1}catch(e){if(!(e instanceof TypeError))return false;n++}try{1n-1}catch(e){if(!(e instanceof TypeError))return false;n++}try{1n/0n}catch(e){if(!(e instanceof RangeError))return false;n++}return n===3&&('7'-true)===6})()",
            "(()=>{try{throw 7}catch(e){try{Symbol()-1}catch(inner){if(!(inner instanceof TypeError))return false}return e===7}})()",
            "(()=>{let s='  \u{00a0} 2 ';return s*3===6&&Number.isNaN('bad'-1)&&('Infinity'-1)===Infinity})()",
            "(()=>{let closed=0,it={[Symbol.iterator](){return this},next(){return {value:1,done:false}},return(){closed++;return {done:true}}};try{for(const x of it){Symbol()-x}}catch(e){return e instanceof TypeError&&closed===1}return false})()",
            "(()=>{let n=0;try{try{1n/0n}finally{n++}}catch(e){return e instanceof RangeError&&n===1}return false})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
        }
        let result = context
            .eval("(function numericSite(){\ntry {\nSymbol()-1;\n} catch(e) {return e.stack}\n})()")
            .unwrap();
        let Value::String(stack) = result else {
            panic!("stack")
        };
        assert!(stack.to_string().contains(":3:"), "{stack}");
    }
}

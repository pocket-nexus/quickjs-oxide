//! One numeric operand transfer shared by same-frame and outer-driver paths.
use crate::engine::api::{Error, runtime::Runtime};
use crate::engine::vm::{
    driver::CallStep,
    execution::RunningExecution,
    frame::FrameId,
    numeric::operation::{NumericKind, NumericStep},
};

pub(in crate::engine::vm) enum NumericProgress {
    Completed,
    Deferred(CallStep),
}
impl NumericProgress {
    pub(in crate::engine::vm) fn into_call_step(self) -> CallStep {
        match self {
            Self::Completed => CallStep::Entered,
            Self::Deferred(step) => step,
        }
    }
}

pub(in crate::engine::vm) fn complete(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: NumericKind,
) -> Result<NumericProgress, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window);
    // Preserve the existing unary/right-before-left pop order and error path.
    let (left, right) = if kind.unary() {
        (execution.slots.pop(&mut frame.window)?, None)
    } else {
        let right = execution.slots.pop(&mut frame.window)?;
        (execution.slots.pop(&mut frame.window)?, Some(right))
    };
    let result = match NumericStep::start(kind, left, right) {
        Ok(step) => {
            crate::engine::vm::proxy_get_driver::start_numeric(runtime, execution, id, step, depth)?
        }
        Err(error) => NumericProgress::Deferred(crate::engine::vm::property_driver::throw_error(
            runtime, realm, error,
        )?),
    };
    match result {
        NumericProgress::Deferred(CallStep::Bridge) => {
            Err(Error::internal("numeric operation attempted replay"))
        }
        result => Ok(result),
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn primitive_numeric_completion_keeps_bigint_operators_and_two_update_results() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let x=5n, old=x++, next=++x, removed=x--, last=--x;
            let a=6n, b=3n;
            let values=[a*b,a/b,a%b,a**b,a<<b,a>>b,a&b,a|b,a^b,~a,-a];
            let errors='';
            try{a/0n}catch(e){errors+=e instanceof RangeError?'z':'!'}
            try{a*2}catch(e){errors+=e instanceof TypeError?'m':'!'}
            try{a>>>b}catch(e){errors+=e instanceof TypeError?'u':'!'}
            try{Symbol()*2}catch(e){errors+=e instanceof TypeError?'s':'!'}
            finally{errors+='f'}
            return values.join(',')==='18,2,0,216,48,0,2,7,5,-7,-6'
                && old===5n && next===7n && removed===7n && last===5n && x===5n
                && errors==='zmusf';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("numeric_completed_in_same_frame")
                .copied()
                .unwrap_or(0)
                >= 10
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn object_numeric_fallback_keeps_conversion_order_throw_identity_and_finally() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let trace='', marker={}, left={valueOf(){trace+='l';return 6n}},
                right={valueOf(){trace+='r';return 3n}};
            if(left*right!==18n)throw 'product';
            try{({valueOf(){trace+='x';throw marker}})*right}catch(e){if(e===marker)trace+='t'}
            finally{trace+='f'}
            let target={valueOf(){trace+='p';return 8n}};
            let old=target++;
            const fixed={valueOf(){trace+='c';return 2n}};
            try{fixed++}catch(e){if(e instanceof TypeError)trace+='e'}
            return trace==='lrxtfpce' && old===8n && target===9n;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

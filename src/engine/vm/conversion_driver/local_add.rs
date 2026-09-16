//! Authenticated local addition without temporary operand roots.
use super::*;
use crate::engine::code::bytecode::Instruction;

pub(in crate::engine::vm) fn complete_local_add(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    next_operation: &mut u64,
) -> Result<PrimitiveCompletion, Error> {
    let frame = execution.frames.current_mut(id)?;
    let start = frame.fault_pc;
    let span = frame
        .executable
        .fusion
        .local_add_span(start)
        .ok_or_else(|| Error::internal("local addition lost authenticated span"))?;
    let [
        Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
        right,
        ..,
    ] = &frame.executable.code[start..]
    else {
        return Err(Error::internal("local addition span lost local reads"));
    };
    enum Right {
        Local(u16),
        Constant(Value),
    }
    let left = *left;
    let right = match right {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => Right::Local(*index),
        Instruction::PushConst(index) => {
            use crate::engine::heap::{BytecodeConstant, RawValue};
            let Some(BytecodeConstant::Value(RawValue::String(value))) =
                frame.executable.constant(*index)
            else {
                return Ok(PrimitiveCompletion::Declined);
            };
            Right::Constant(Value::String(value.clone()))
        }
        _ => return Err(Error::internal("local addition span lost RHS")),
    };
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let mut transaction = execution.slots.frame_transaction(&mut frame.cold.window)?;
    // All preflight remains non-mutating; checked/captured/TDZ fallbacks retain
    // the canonical first GetLocal PC and original operand stack.
    enum PreparedAdd {
        Exhausted(Value, Value),
        Result(Result<Value, Error>),
        Appended,
    }
    let consume = |left: &mut Value, right: &Value| {
        frame.fault_pc = start + 2;
        frame.resume_pc = frame.fault_pc;
        runtime
            .update_active_bytecode_pc(
                frame.active_frame,
                super::super::BytecodePc::new(frame.fault_pc),
            )
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_instruction(depth);
            crate::engine::api::profiling::record_owned_instruction(depth + 1);
        }
        let Some(next) = next_operation.checked_add(1) else {
            // Reconstruct canonical GetLocal operands only on this cold error.
            return Ok::<_, Error>(PreparedAdd::Exhausted(left.clone(), right.clone()));
        };
        *next_operation = next;
        if let Value::String(string) = left {
            let suffix = match right {
                Value::String(value) => std::borrow::Cow::Borrowed(value),
                value => match value.to_js_string() {
                    Ok(value) => std::borrow::Cow::Owned(value),
                    Err(error) => return Ok(PreparedAdd::Result(Err(error))),
                },
            };
            match string.try_concat_in_place(&suffix) {
                Ok(true) => return Ok(PreparedAdd::Appended),
                Err(error) => return Ok(PreparedAdd::Result(Err(error.into()))),
                Ok(false) => {
                    // Reuse the conversion already completed above even when
                    // a shared/rope lhs cannot append into its own buffer.
                    return Ok(PreparedAdd::Result(
                        string
                            .try_concat(&suffix)
                            .map(Value::String)
                            .map_err(Error::from),
                    ));
                }
            }
        }
        Ok(PreparedAdd::Result(
            super::super::numeric::add_primitives_ref(left, right),
        ))
    };
    let prepared = match right {
        Right::Local(right) => transaction.with_local_add_inputs(left, right, consume)?,
        Right::Constant(ref right) => transaction.with_local_add_constant(left, right, consume)?,
    };
    let Some(prepared) = prepared else {
        return Ok(PrimitiveCompletion::Declined);
    };
    let result = match prepared? {
        PreparedAdd::Exhausted(left, right) => {
            let mut slots = transaction.slots();
            slots.push(left)?;
            slots.push(right)?;
            return Err(Error::internal("conversion identity exhausted"));
        }
        PreparedAdd::Result(result) => Some(result),
        PreparedAdd::Appended => {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("local_add_in_place");
            None
        }
    };
    if let Some(result) = result {
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                let Some(kind) =
                    crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
                else {
                    return Err(error);
                };
                return Ok(PrimitiveCompletion::Throw(
                    runtime
                        .new_native_error_from_error(frame.executable.realm, kind, &error)
                        .map_err(runtime_error_to_vm_error)?,
                ));
            }
        };
        // Successful primitive addition proves the old local is neither Object
        // nor Symbol. Replacing it only releases scalar/Rc String/BigInt storage;
        // it cannot drain runtime roots, call JS or observe the active frame. Keep
        // the Add publication for the whole transaction and commit frame PCs once.
        let mut pending = Some(super::super::bindings::FrameBinding::Direct(value));
        let old = {
            let mut slots = transaction.slots();
            slots.replace_local_pending(left, &mut pending)
        };
        let old = match old {
            Ok(old) => old,
            Err(error) => {
                (frame.fault_pc, frame.resume_pc) = (start + 3, start + 3);
                // An invariant error still identifies the canonical store, and the
                // pending output is released only after its active PC is published.
                runtime
                    .update_active_bytecode_pc(
                        frame.active_frame,
                        super::super::BytecodePc::new(frame.fault_pc),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                return Err(error);
            }
        };
        drop(old);
    }
    (frame.fault_pc, frame.resume_pc) = (start + span - 1, start + span);
    #[cfg(feature = "profiling")]
    {
        // Canonical virtual stack depths even though operand copies disappear.
        for offset in 2..span {
            let observed = match offset {
                0 => depth,
                1 => depth + 1,
                2 => depth + 2,
                _ => depth + 1,
            };
            crate::engine::api::profiling::record_owned_instruction(observed);
        }
        crate::engine::api::profiling::record_owned_execution_event("local_add_borrowed_span");
    }
    Ok(PrimitiveCompletion::Completed)
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};

    #[cfg(feature = "profiling")]
    #[test]
    fn local_string_append_reaches_unique_storage_and_preserves_failure_binding() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval("(()=>{let r='start',s='xy';for(let i=0;i<100;i++)r+=s;return r==='start'+'xy'.repeat(100);})()").unwrap(), Value::Bool(true));
        assert_eq!(context.eval("(()=>{let r='start';for(let i=0;i<100;i++)r+='xy';return r==='start'+'xy'.repeat(100);})()").unwrap(), Value::Bool(true));
        let costs = profile.snapshot();
        assert!(
            costs
                .owned_execution_events
                .get("local_add_in_place")
                .copied()
                .unwrap_or(0)
                >= 198,
            "{costs:?}"
        );
        drop(profile);
        let callable = runtime.callable_from_value(context.eval("(function pcAppend(){\nlet r='start'.slice(0,4),s='xy';\ntry { r+=s; } catch(e) { return r==='star' && e.stack.includes(':3:'); } return false;})").unwrap()).unwrap();
        crate::engine::value::fail_next_concat_reservation_for_test();
        let completion = runtime
            .call_internal(context.realm, &callable, Value::Undefined, &[])
            .unwrap();
        assert!(
            matches!(
                completion,
                crate::engine::vm::Completion::Return(Value::Bool(true))
            ),
            "{completion:?}"
        );
    }

    #[test]
    fn local_add_identity_exhaustion_retains_canonical_operands_and_add_pc() {
        use crate::engine::vm::{
            bindings::FrameBinding,
            call::{BytecodeCallRequest, CallableExecution},
            execution::{ExecutionLimits, RunningExecution},
            frame::{ReturnOwner, ReturnTarget, ReturnValue},
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(
                context
                    .eval("(function(){let a='a',b='b';a=a+b;return a;})")
                    .unwrap(),
            )
            .unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode")
        };
        let a = context.eval("'a'").unwrap();
        let b = context.eval("'b'").unwrap();
        let mut execution = RunningExecution::new(
            &runtime,
            ExecutionLimits {
                frames: 8,
                slots: 128,
            },
        )
        .unwrap();
        let entry = BytecodeCallRequest {
            callable,
            receiver: Value::Undefined,
            new_target: Value::Undefined,
            arguments: vec![],
            bytecode,
            closure_slots,
            caller_realm: context.realm,
            return_to: ReturnTarget {
                value_use: ReturnValue::Push,
                owner: ReturnOwner::Root,
                tail: false,
                operation: None,
            },
        }
        .prepare(&runtime, &mut execution.call_storage)
        .unwrap();
        let id = crate::engine::vm::driver::push_frame(&mut execution, entry).unwrap();
        let start = {
            let frame = execution.frames.current_mut(id).unwrap();
            let start = (0..frame.executable.code.len())
                .find(|pc| frame.executable.fusion.local_add_span(*pc).is_some())
                .expect("full local span");
            let [
                super::Instruction::GetLocal(left) | super::Instruction::GetLocalCheck(left),
                super::Instruction::GetLocal(right) | super::Instruction::GetLocalCheck(right),
                ..,
            ] = &frame.executable.code[start..]
            else {
                panic!("span")
            };
            execution
                .slots
                .replace_local(&frame.window, *left, FrameBinding::Direct(a.clone()))
                .unwrap();
            execution
                .slots
                .replace_local(&frame.window, *right, FrameBinding::Direct(b.clone()))
                .unwrap();
            frame.fault_pc = start;
            frame.resume_pc = start;
            start
        };
        let mut identity = u64::MAX;
        let result = super::complete_local_add(&runtime, &mut execution, id, &mut identity);
        assert!(matches!(result,Err(ref e) if e.message()=="conversion identity exhausted"));
        assert_eq!(identity, u64::MAX);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.fault_pc, start + 2);
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(execution.slots.peek(&frame.window, 1).unwrap(), &a);
        assert_eq!(execution.slots.peek(&frame.window, 0).unwrap(), &b);
    }

    #[test]
    fn local_add_throw_reports_add_line_before_error_allocation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let result = context.eval("(function pcProbe(){\nlet a=1n,b=2;\ntry { a=a+b; } catch(e) { return e.stack; }\n})()").unwrap();
        let Value::String(stack) = result else {
            panic!("expected stack");
        };
        assert!(stack.to_string().contains(":3:"), "{stack}");
    }

    #[test]
    fn local_add_preserves_aliases_mixed_errors_and_captured_fallbacks() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            function strings(){let a='a',b='b';a=a+b;a=a+a;return a;}
            function big(){let a=17n,b=23n;a=a+b;a=a+a;return a;}
            function mixed(){let a=1n,b=2;try{a=a+b;}catch(e){return e instanceof TypeError && a===1n;}return false;}
            function symbol(){let a='x',b=Symbol();try{a=a+b;}catch(e){return e instanceof TypeError && a==='x';}return false;}
            function captured(){let a='a',b='b';const read=()=>a;a=a+b;return read();}
            function coercion(){let a='a',b={toString(){a='changed';return 'b';}};a=a+b;return a;}
            function tdz(){let a='x';try{a=a+b;let b='y';}catch(e){return e instanceof ReferenceError && a==='x';}return false;}
            return strings()==='abab' && big()===80n && mixed() && symbol()
                && captured()==='ab' && coercion()==='ab' && tdz();
        })()"#).unwrap(), Value::Bool(true));
    }
}

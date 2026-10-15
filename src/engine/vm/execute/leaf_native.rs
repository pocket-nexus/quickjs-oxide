//! Callback-free native leaves completed inside the ordinary segment.
//!
//! A selected native normally leaves the dispatch loop and runs as a native
//! activation. These targets cannot call JavaScript, throw or allocate when
//! every operand they read is a Number (and, for `charCodeAt`, the receiver
//! is a String primitive), so the call completes on the caller's frame. Any
//! other operand declines before an operand moves, and the call takes the
//! general path unchanged.
use super::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

/// Math reads at most this many Number arguments on the leaf path.
const MAX_ARGUMENTS: usize = 4;

/// Complete the current call when its callee is the selected native and the
/// operands allow a leaf. Returns `false` with every operand left in place.
#[inline(never)]
pub(super) fn try_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    arguments: u16,
    method: bool,
    fallthrough: FallthroughPc,
) -> Result<bool, Error> {
    let count = usize::from(arguments);
    if count > MAX_ARGUMENTS {
        return Ok(false);
    }
    let FrameTurn {
        transaction,
        fault_pc,
        resume_pc,
        selected_native,
        ..
    } = segment.frame();
    let Some(selected) = selected_native.as_ref() else {
        return Ok(false);
    };
    let target = selected.target();
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let result = cursor.with_slots(|slots| {
        let JsValue::Object(callee) = slots.peek(count)? else {
            return Ok(None);
        };
        if !selected.matches_in_domain(runtime.domain_id(), *callee) {
            return Ok(None);
        }
        let mut numbers = [0.0; MAX_ARGUMENTS];
        for (index, number) in numbers.iter_mut().enumerate().take(count) {
            *number = match slots.peek(count - 1 - index)? {
                JsValue::Int(value) => f64::from(*value),
                JsValue::Float(value) => *value,
                _ => return Ok(None),
            };
        }
        if target == NativeFunctionId::StringPrototypeCharCodeAt {
            if !method {
                return Ok(None);
            }
            let JsValue::String(receiver) = slots.peek(count + 1)? else {
                return Ok(None);
            };
            // ToIntegerOrInfinity(undefined) is 0, as is NaN.
            let index = if count == 0 { 0.0 } else { numbers[0] };
            let index = crate::engine::value::number::to_int32_sat(index);
            let string = state
                .heap
                .string(*receiver)
                .map_err(|error| runtime_error_to_vm_error(error.into()))?;
            return Ok(Some(
                usize::try_from(index)
                    .ok()
                    .and_then(|index| string.code_unit_at(index))
                    .map_or(JsValue::Float(f64::NAN), |unit| {
                        JsValue::Int(i32::from(unit))
                    }),
            ));
        }
        crate::engine::builtins::leaf_math_numbers(runtime, target, &numbers[..count])
            .map_err(runtime_error_to_vm_error)
    })?;
    let Some(result) = result else {
        return Ok(false);
    };
    // The leaf consumed the selection exactly as a native activation would.
    *selected_native = None;
    for _ in 0..count + 1 + usize::from(method) {
        let operand = cursor.move_owned()?;
        state
            .release_owned_jsvalue(&runtime.0.poisoned, operand)
            .map_err(runtime_error_to_vm_error)?;
    }
    cursor.commit_owned(state, result)?;
    cursor.advance(fallthrough.index());
    profile!(record_owned_execution_event(
        "native_leaf_completed_in_segment"
    ));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn leaf_natives_match_the_native_activation() {
        let (_runtime, mut context) = test_context();
        let source = r#"
            function run(f) { var out = []; for (var i = 0; i < 3; i++) out.push(f()); return out.join('/'); }
            [
                run(() => Math.abs(-3)), run(() => Math.abs(-3.5)), run(() => Math.abs()),
                run(() => Math.max()), run(() => Math.min()), run(() => Math.max(1, 2, 3, 4)),
                run(() => Math.max(1, 2, 3, 4, 5)), run(() => Object.is(Math.min(0, -0), -0)),
                run(() => Math.max(1, NaN)), run(() => Math.pow(2, 10)), run(() => Math.atan2(1)),
                run(() => Math.imul(3, 4)), run(() => Math.clz32(1)), run(() => Math.hypot(3, 4)),
                run(() => Math.hypot()), run(() => Object.is(Math.round(-0.4), -0)),
                run(() => 'abc'.charCodeAt(1)), run(() => 'abc'.charCodeAt()),
                run(() => 'abc'.charCodeAt(5)), run(() => 'abc'.charCodeAt(-1)),
                run(() => 'abc'.charCodeAt(1.7)), run(() => 'abc'.charCodeAt(NaN)),
                run(() => '\u{1F600}'.charCodeAt(1)),
            ].join()
        "#;
        assert_eq!(
            eval_string(&mut context, source),
            "3/3/3,3.5/3.5/3.5,NaN/NaN/NaN,-Infinity/-Infinity/-Infinity,\
             Infinity/Infinity/Infinity,4/4/4,5/5/5,true/true/true,NaN/NaN/NaN,\
             1024/1024/1024,NaN/NaN/NaN,12/12/12,31/31/31,5/5/5,0/0/0,true/true/true,\
             98/98/98,97/97/97,NaN/NaN/NaN,NaN/NaN/NaN,98/98/98,97/97/97,56832/56832/56832"
        );
    }

    #[test]
    fn non_leaf_operands_and_callees_take_the_general_path() {
        let (_runtime, mut context) = test_context();
        let source = r#"
            var log = [];
            var valued = { valueOf() { log.push('v'); return 9; } };
            var out = [Math.abs('5'), Math.max(valued, 1), 'abc'.charCodeAt('1'),
                new String('abc').charCodeAt(0), Math.abs(1n === 1n)];
            var abs = Math.abs; out.push(abs(-2));
            var charCodeAt = String.prototype.charCodeAt;
            try { charCodeAt(0); } catch (e) { out.push(e.name); }
            out.push(charCodeAt.call('xy', 1));
            var holder = { abs: Math.abs }; out.push(holder.abs(-8));
            Math.abs = function (x) { return 'patched' + x; }; out.push(Math.abs(-1));
            function tail(x) { 'use strict'; return Math.max(x, 2); } out.push(tail(1));
            out.join() + ':' + log.join()
        "#;
        assert_eq!(
            eval_string(&mut context, source),
            "5,9,98,97,1,2,TypeError,121,8,patched-1,2:v"
        );
    }

    #[test]
    fn leaf_natives_preserve_operand_ownership() {
        let (_runtime, mut context) = test_context();
        let source = r#"
            var s = 'x'.repeat(3) + 'y', total = 0;
            for (var i = 0; i < 2000; i++) {
                total += s.charCodeAt(i & 3) + Math.max(i, 7) + Math.abs(-i);
                if (i % 500 === 0) s = s + i;
            }
            String(total)
        "#;
        assert_eq!(eval_string(&mut context, source), "4238528");
    }
}

//! String + String completes in the segment through the general path's
//! state kernel. Either operand being any other value, a result over the
//! String length limit, or a pending deferred release (whose drain the
//! general path performs first), declines.
use super::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

#[inline(never)]
pub(super) fn try_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    fallthrough: FallthroughPc,
) -> Result<bool, Error> {
    if runtime.0.deferred_references.has_pending() {
        return Ok(false);
    }
    let FrameTurn {
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let strings = cursor.with_slots(|slots| {
        let (JsValue::String(prefix), JsValue::String(suffix)) = (slots.peek(1)?, slots.peek(0)?)
        else {
            return Ok(false);
        };
        // An over-long result is a catchable error, published by the general
        // path; only joins within the limit complete here.
        let length = |id| {
            state
                .heap
                .string(id)
                .map(crate::engine::value::JsString::len)
                .map_err(|error| Error::internal(error.to_string()))
        };
        Ok(length(*prefix)?.saturating_add(length(*suffix)?)
            <= crate::engine::value::JsString::MAX_LEN)
    })?;
    if !strings {
        return Ok(false);
    }
    let (JsValue::String(suffix), JsValue::String(prefix)) =
        (cursor.move_owned()?, cursor.move_owned()?)
    else {
        unreachable!("both operands were checked as Strings");
    };
    // The kernel consumes both edges, including on error.
    let result = crate::engine::vm::numeric::add_strings_with_state(
        state,
        &runtime.0.poisoned,
        prefix,
        suffix,
    )?;
    cursor.commit_owned(state, result)?;
    cursor.advance(fallthrough.index());
    profile!(record_owned_execution_event("string_add_in_segment"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn string_addition_in_the_segment_matches_the_general_path() {
        let (_runtime, mut context) = test_context();
        let source = r#"
            var s = '', t = 'a', u = '';
            for (var i = 0; i < 300; i++) { s += 'x'; t = t + t.charAt(i % 2); u = 'p' + u + ''; }
            var a = 'q', b = a; b += 'r';
            var sym; try { 'x' + Symbol(); } catch (e) { sym = e.name; }
            [s.length, t.length, t.slice(0, 4), u.length, a, b, JSON.stringify('' + ''),
                'n' + 1 + true + null + {} + [1, 2], sym, 'é' + '中',
                ('ab' + 'cd').length].join()
        "#;
        assert_eq!(
            eval_string(&mut context, source),
            "300,301,aaaa,300,q,qr,\"\",n1truenull[object Object]1,2,TypeError,\u{e9}\u{4e2d},4"
        );
    }
}

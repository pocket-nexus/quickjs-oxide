//! `for-of` over an Array iterator whose captured `next` is the intrinsic
//! `%ArrayIteratorPrototype%.next` yields dense elements on the caller's
//! frame. Exhaustion, holes, sparse or non-Array sources, other iterators and
//! a disabled or foreign iterator record keep the general ForOfNext path.
use super::FrameCursor;
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectPayload, RawValue};
use crate::engine::value::JsValue;
use crate::engine::vm::VmUnwindRegion;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn, copy_value_in_state};

#[inline(never)]
pub(super) fn try_next(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    offset: usize,
) -> Result<bool, Error> {
    if runtime.0.deferred_references.has_pending() {
        return Ok(false);
    }
    let FrameTurn {
        executable,
        owners,
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    let Some(VmUnwindRegion::Iterator {
        record_base,
        enabled: true,
        asynchronous: false,
    }) = owners
        .rare
        .get()
        .and_then(|rare| rare.regions.last().copied())
    else {
        return Ok(false);
    };
    if Some(transaction.operand_depth()) != record_base.checked_add(2 + offset) {
        return Ok(false);
    }
    let next = executable
        .exec
        .decode_published(*fault_pc as u32)
        .map_err(|_| Error::internal("ForOfNext has no instruction"))?
        .next_pc as usize;
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let internal =
        |error: crate::engine::api::runtime_error::RuntimeError| runtime_error_to_vm_error(error);
    let step = cursor.with_slots(|slots| {
        let (JsValue::Object(method), JsValue::Object(iterator)) =
            (slots.peek(offset)?, slots.peek(offset + 1)?)
        else {
            return Ok(None);
        };
        if !matches!(
            &state.heap.object(*method).map_err(|error| internal(error.into()))?.payload,
            ObjectPayload::NativeFunction { data, .. }
                if data.target == NativeFunctionId::ArrayIteratorNext
        ) {
            return Ok(None);
        }
        let iterator = *iterator;
        if !matches!(
            state
                .heap
                .object(iterator)
                .map_err(|error| internal(error.into()))?
                .payload,
            ObjectPayload::ArrayIterator { .. }
        ) {
            return Ok(None);
        }
        let (Some(source), index, kind) = state
            .heap
            .array_iterator_state(iterator)
            .map_err(|error| internal(error.into()))?
        else {
            return Ok(None);
        };
        let Some((raw, next_index)) =
            crate::engine::builtins::dense_next_in_state(state, source, index, kind)
                .map_err(internal)?
        else {
            return Ok(None);
        };
        let value = if kind == crate::engine::builtins::native::ArrayIteratorKind::Key {
            i32::try_from(index).map_or(JsValue::Float(f64::from(index)), JsValue::Int)
        } else {
            match raw {
                RawValue::Undefined
                | RawValue::Null
                | RawValue::Bool(_)
                | RawValue::Int(_)
                | RawValue::Float(_)
                | RawValue::ShortBigInt(_)
                | RawValue::String(_)
                | RawValue::Object(_) => JsValue::from_raw(raw)
                    .ok_or_else(|| Error::internal("dense element held a sentinel"))?,
                _ => return Ok(None),
            }
        };
        Ok(Some((iterator, value, next_index)))
    })?;
    let Some((iterator, value, next_index)) = step else {
        return Ok(false);
    };
    // The yielded element gets its own edge before the iterator advances.
    let value = copy_value_in_state(state, &value)?;
    state
        .heap
        .set_array_iterator_index(iterator, next_index)
        .map_err(|error| internal(error.into()))?;
    cursor.commit_owned(state, value)?;
    cursor.commit_owned(state, JsValue::Bool(false))?;
    cursor.advance(next);
    profile!(record_owned_execution_event("for_of_array_next_in_segment"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn array_for_of_in_the_segment_matches_the_iterator_protocol() {
        let (_runtime, mut context) = test_context();
        let source = r#"
var out = [];
function collect(iterable) { var r = []; for (const v of iterable) r.push(typeof v === 'object' && v !== null ? JSON.stringify(v) : String(v)); return r.join('|'); }
out.push(collect([1, 'a', {x: 1}, null, undefined, 2.5, true, 10n, [3]]));
var grow = [1, 2]; var seen = []; for (const v of grow) { seen.push(v); if (grow.length < 5) grow.push(v * 10); } out.push(seen.join());
var shrink = [1, 2, 3, 4]; seen = []; for (const v of shrink) { seen.push(v); shrink.length = 2; } out.push(seen.join());
var holes = [1, , 3]; Array.prototype[1] = 'proto'; out.push(collect(holes)); delete Array.prototype[1];
out.push(collect([5, 6].keys()), collect(['a', 'b'].entries()), collect('xy'), collect(new Set([7, 8])), collect(new Uint8Array([9, 10])));
var it = [1, 2, 3][Symbol.iterator](); it.next(); out.push(collect({ [Symbol.iterator]() { return it; } }));
var saved = Object.getPrototypeOf([][Symbol.iterator]()).next; var calls = 0;
Object.getPrototypeOf([][Symbol.iterator]()).next = function () { calls++; return saved.call(this); };
out.push(collect([1, 2]), calls); Object.getPrototypeOf([][Symbol.iterator]()).next = saved;
function early() { for (const v of [1, 2, 3]) { if (v === 2) return v; } } out.push(early());
var brk = 0; for (const v of [1, 2, 3]) { brk += v; if (v === 2) break; } out.push(brk);
try { for (const v of [1, 2]) { throw new Error('x' + v); } } catch (e) { out.push(e.message); }
var fns = []; for (const v of [1, 2, 3]) fns.push(() => v); out.push(fns.map(f => f()).join());
var big = []; for (var i = 0; i < 1000; i++) big.push({ i }); var sum = 0; for (const o of big) sum += o.i; out.push(sum);
var nested = 0; for (const a of [[1, 2], [3]]) for (const b of a) nested += b; out.push(nested);
out.join(';')
"#;
        assert_eq!(
            eval_string(&mut context, source),
            r#"1|a|{"x":1}|null|undefined|2.5|true|10|[3];1,2,10,20,100;1,2;1|proto|3;0|1;[0,"a"]|[1,"b"];x|y;7|8;9|10;2|3;1|2;3;2;3;x1;1,2,3;499500;6"#
        );
    }
}

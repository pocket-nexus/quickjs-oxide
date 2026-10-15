//! `new Array(...)` with the intrinsic Array as both callee and new.target
//! completes in the segment. Its prototype is then the callee realm's
//! %Array.prototype%, and no step of the constructor can call JavaScript:
//! no arguments make an empty Array, one integral Number in [0, 2^32) sets
//! the length of a fresh Array (as the constructor's length definition grows
//! it), and any other single value or several values become the elements.
//! A Number length that would throw a RangeError declines, as does every
//! other callee. Allocation follows the Array literal's publication and GC
//! checkpoint protocol.
use super::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectPayload, PropertySlot, RawValue};
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

enum Shape {
    Length(u32),
    Elements,
}

#[inline(never)]
pub(super) fn try_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    arguments: u16,
    fallthrough: FallthroughPc,
) -> Result<bool, Error> {
    let count = usize::from(arguments);
    if runtime.0.deferred_references.has_pending() {
        return Ok(false);
    }
    let selected = {
        let FrameTurn { transaction, .. } = segment.frame();
        let (JsValue::Object(callee), JsValue::Object(new_target)) =
            (transaction.peek(count + 1)?, transaction.peek(count)?)
        else {
            return Ok(false);
        };
        if callee != new_target {
            return Ok(false);
        }
        let realm = match &state
            .heap
            .object(*callee)
            .map_err(|error| runtime_error_to_vm_error(error.into()))?
            .payload
        {
            ObjectPayload::NativeFunction { data, .. }
                if data.target == NativeFunctionId::ArrayConstructor =>
            {
                data.realm
            }
            _ => None,
        };
        let Some(realm) = realm else {
            return Ok(false);
        };
        let shape = match (count, transaction.peek(0)?) {
            (0, _) => Shape::Length(0),
            (1, JsValue::Int(length)) => match u32::try_from(*length) {
                Ok(length) => Shape::Length(length),
                Err(_) => return Ok(false),
            },
            (1, JsValue::Float(length)) => {
                if length.fract() != 0.0 || !(0.0..=f64::from(u32::MAX)).contains(length) {
                    return Ok(false);
                }
                Shape::Length(*length as u32)
            }
            _ => Shape::Elements,
        };
        (realm, shape)
    };
    let (realm, shape) = selected;
    // As for an Array literal: publish the frames only when the checkpoint
    // below can collect; allocation errors materialize on the throw path.
    if runtime
        .0
        .gc_pressure
        .may_request_within(super::LITERAL_ALLOCATION_NODES)
    {
        segment.materialize_in_state(state)?;
    }
    {
        let FrameTurn {
            transaction,
            fault_pc,
            resume_pc,
            ..
        } = segment.frame();
        let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
        let poisoned = &runtime.0.poisoned;
        let array = match shape {
            Shape::Length(length) => {
                if count == 1 {
                    let _length = cursor.move_owned()?;
                }
                let array = state
                    .new_array(poisoned, realm)
                    .map_err(runtime_error_to_vm_error)?;
                if length > 0 {
                    let raw = i32::try_from(length)
                        .map_or(RawValue::Float(f64::from(length)), RawValue::Int);
                    // A fresh Array's first own slot is its writable length.
                    state
                        .replace_property_slot_with_poison(
                            poisoned,
                            array,
                            0,
                            PropertySlot::Data(raw),
                        )
                        .map_err(runtime_error_to_vm_error)?;
                }
                array
            }
            Shape::Elements => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(count)
                    .map_err(|_| Error::internal("array elements allocation failed"))?;
                for _ in 0..count {
                    values.push(cursor.move_owned()?);
                }
                values.reverse();
                state
                    .new_array_from_values_jsvalue(poisoned, realm, values)
                    .map_err(runtime_error_to_vm_error)?
            }
        };
        let mut array = Some(JsValue::Object(array));
        // new.target and the callee are the same intrinsic; release both edges.
        for _ in 0..2 {
            let operand = cursor.move_owned()?;
            if let Err(error) = state.release_owned_jsvalue(poisoned, operand) {
                if let Some(array) = array.take() {
                    let _ = state.release_owned_jsvalue(poisoned, array);
                }
                return Err(runtime_error_to_vm_error(error));
            }
        }
        cursor.commit_owned(state, array.take().expect("constructed Array"))?;
        cursor.advance(fallthrough.index());
    }
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .map_err(runtime_error_to_vm_error)?;
    profile!(record_owned_execution_event("array_construct_in_segment"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn array_construction_in_the_segment_matches_the_constructor() {
        let (_runtime, mut context) = test_context();
        let source = r#"
var out = [];
function t(name, f) { try { out.push(name + '=' + JSON.stringify(f())); } catch (e) { out.push(name + '!' + e.name + ':' + e.message); } }
function d(a) { return [a.length, Object.keys(a).join(), Object.getPrototypeOf(a) === Array.prototype, Array.isArray(a)]; }
t('empty', () => d(new Array()));
t('len', () => d(new Array(4)));
t('len0', () => d(new Array(0)));
t('lenFloat', () => d(new Array(3.0)));
t('frac', () => d(new Array(1.5)));
t('neg', () => d(new Array(-1)));
t('huge', () => d(new Array(2 ** 32)));
t('max', () => new Array(2 ** 32 - 1).length);
t('big31', () => new Array(2 ** 31).length);
t('one', () => d(new Array('3')));
t('oneObj', () => { var o = {}; var a = new Array(o); return [a.length, a[0] === o]; });
t('two', () => d(new Array(1, 2)));
t('three', () => { var a = new Array('a', null, undefined); return [a.length, a[0], a[1], a[2], 2 in a]; });
t('fill', () => { var a = new Array(3); a[0] = 1; a[1] = 2; a[2] = 3; a.push(4); return [a.join(), a.length]; });
t('holesThenPush', () => { var a = new Array(2); a.push(9); return [a.length, 0 in a, a[2]]; });
t('write', () => { var a = new Array(2); a[5] = 1; return [a.length, Object.keys(a).join()]; });
t('lengthShrink', () => { var a = new Array(5); a.length = 2; return a.length; });
t('desc', () => JSON.stringify(Object.getOwnPropertyDescriptor(new Array(3), 'length')));
t('subclass', () => { class A extends Array {} var a = new A(3); return [a.length, a instanceof A]; });
t('reflect', () => { var a = Reflect.construct(Array, [2], Object); return [Array.isArray(a), Object.getPrototypeOf(a) === Object.prototype, a.length]; });
t('call', () => d(Array(3)));
t('patched', () => { var saved = Array; Array = function () { return { fake: true }; }; try { return new Array(2); } finally { Array = saved; } });
t('loop', () => { var s = 0; for (var i = 0; i < 1000; i++) { var a = new Array(i & 7); s += a.length; } return s; });
t('nested', () => { var a = new Array(new Array(2)); return [a.length, a[0].length]; });
t('frozenProto', () => { var a = new Array(2); return Object.isExtensible(a); });
out.join(';')
"#;
        assert_eq!(
            eval_string(&mut context, source),
            r#"empty=[0,"",true,true];len=[4,"",true,true];len0=[0,"",true,true];lenFloat=[3,"",true,true];frac!RangeError:invalid array length;neg!RangeError:invalid array length;huge!RangeError:invalid array length;max=4294967295;big31=2147483648;one=[1,"0",true,true];oneObj=[1,true];two=[2,"0,1",true,true];three=[3,"a",null,null,true];fill=["1,2,3,4",4];holesThenPush=[3,false,9];write=[6,"5"];lengthShrink=2;desc="{\"value\":3,\"writable\":true,\"enumerable\":false,\"configurable\":false}";subclass=[3,true];reflect=[true,true,2];call=[3,"",true,true];patched={"fake":true};loop=3500;nested=[1,2];frozenProto=true"#
        );
    }
}

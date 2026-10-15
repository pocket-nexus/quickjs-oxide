//! Indexed reads of Arrays left the dispatch loop whenever the specialized
//! dense read missed (or the site was never specialized). A non-negative Int
//! key on an Array either names a dense element, read with its own edge, or
//! lies past the dense prefix; when no object from the Array up its chain has
//! that index and none is exotic for it, the read is `undefined` without
//! observable steps (the `new Array(n)` holes AWFY reads). Every other case
//! keeps the general element read.
use super::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectKind, ObjectPayload};
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn, copy_value_in_state};

#[inline(never)]
pub(super) fn try_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
    keep_receiver: bool,
    keep_key: bool,
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
    let internal =
        |error: crate::engine::api::runtime_error::RuntimeError| runtime_error_to_vm_error(error);
    let value = cursor.with_slots(|slots| {
        let (JsValue::Object(array), JsValue::Int(index)) = (slots.peek(1)?, slots.peek(0)?) else {
            return Ok(None);
        };
        let Ok(index) = u32::try_from(*index) else {
            return Ok(None);
        };
        let data = state
            .heap
            .object(*array)
            .map_err(|error| internal(error.into()))?;
        let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
            return Ok(None);
        };
        if data.kind != ObjectKind::Array {
            return Ok(None);
        }
        if let Some(raw) = dense.get(index as usize) {
            return Ok(JsValue::from_raw(raw.clone()));
        }
        let Some(atom) = Atom::from_immediate_integer(index) else {
            return Ok(None);
        };
        Ok(
            crate::engine::object::chain_lacks_property(state, atom, Some(*array))
                .map_err(internal)?
                .then_some(JsValue::Undefined),
        )
    })?;
    let Some(value) = value else {
        return Ok(false);
    };
    // The element gets its own edge before any operand is released.
    let value = copy_value_in_state(state, &value)?;
    if !keep_key {
        let _integer_key = cursor.move_owned()?;
        if !keep_receiver {
            let array = cursor.move_owned()?;
            state
                .release_owned_jsvalue(&runtime.0.poisoned, array)
                .map_err(internal)?;
        }
    }
    cursor.commit_owned(state, value)?;
    cursor.advance(fallthrough.index());
    profile!(record_owned_execution_event(
        "array_element_read_in_segment"
    ));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn array_element_reads_in_the_segment_match_the_general_read() {
        let (_runtime, mut context) = test_context();
        let source = r#"
var out = [];
function t(name, f) { try { out.push(name + '=' + JSON.stringify(f())); } catch (e) { out.push(name + '!' + e.name); } }
function readAll(a, n) { var r = ''; for (var i = 0; i < n; i++) r += (i ? ',' : '') + String(a[i]); return r; }
function viaField(o, n) { var r = ''; for (var i = 0; i < n; i++) r += (i ? ',' : '') + String(o.store[i]); return r; }
t('dense', () => readAll([1, 'a', null, { x: 1 }], 5));
t('holes', () => readAll(new Array(3), 4));
t('holes2', () => readAll([1, , 3], 3));
t('field', () => viaField({ store: new Array(2) }, 3));
t('protoIdx', () => { Array.prototype[1] = 'P'; var r = readAll(new Array(3), 3); delete Array.prototype[1]; return r; });
t('objProtoIdx', () => { Object.prototype[2] = 'O'; var r = readAll([0], 3); delete Object.prototype[2]; return r; });
t('getter', () => { var n = 0; Object.defineProperty(Array.prototype, 4, { get() { n++; return 'G'; }, configurable: true }); try { return [readAll([1, 2], 6), n]; } finally { delete Array.prototype[4]; } });
t('proxyProto', () => { var a = new Array(2); Object.setPrototypeOf(a, new Proxy([], { get(t, k) { return 'X' + String(k); } })); return readAll(a, 3); });
t('subclass', () => { class A extends Array {} var a = new A(); a.push(5); return readAll(a, 2); });
t('keys', () => { var a = [7, 8]; return [a[-1], a[1.5], a['1'], a[2 ** 32], a[NaN]]; });
t('args', () => (function () { return readAll(arguments, 3); })(4, 5));
t('typed', () => readAll(new Uint8Array([1, 2]), 3));
t('string', () => readAll('ab', 3));
t('sparse', () => { var a = []; a[100] = 1; return [a[0], a[100], a[50]]; });
t('nested', () => { var g = [[1, 2], new Array(2)]; var s = 0; for (var i = 0; i < 2; i++) for (var j = 0; j < 2; j++) s += g[i][j] === undefined ? 10 : g[i][j]; return s; });
t('setProto', () => { var a = new Array(2); Object.setPrototypeOf(a, { 1: 'own' }); return readAll(a, 2); });
t('big', () => { var a = new Array(1000), s = 0; for (var i = 0; i < 1000; i++) if (a[i] === undefined) s++; return s; });
out.join(';')
"#;
        assert_eq!(
            eval_string(&mut context, source),
            r#"dense="1,a,null,[object Object],undefined";holes="undefined,undefined,undefined,undefined";holes2="1,undefined,3";field="undefined,undefined,undefined";protoIdx="undefined,P,undefined";objProtoIdx="0,undefined,O";getter=["1,2,undefined,undefined,G,undefined",1];proxyProto="X0,X1,X2";subclass="5,undefined";keys=[null,null,8,null,null];args="4,5,undefined";typed="1,2,undefined";string="a,b,undefined";sparse=[null,1,null];nested=23;setProto="undefined,own";big=1000"#
        );
    }
}

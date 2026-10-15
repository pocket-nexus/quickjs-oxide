//! `instanceof` whose answer needs no callback completes in the segment
//! through the ready loop's kernel (`try_ordinary_instanceof_in_state`): an
//! ordinary target with the intrinsic or no @@hasInstance and an own data
//! `prototype`. A miss leaves both operands and the PC untouched.
use super::FrameCursor;
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

#[inline(never)]
pub(super) fn try_complete(
    runtime: &Runtime,
    state: &mut RuntimeState,
    segment: &mut FrameExecution<'_>,
) -> Result<bool, Error> {
    let intrinsic_budget = segment.can_push_with_continuations(0);
    let FrameTurn {
        executable,
        transaction,
        fault_pc,
        resume_pc,
        ..
    } = segment.frame();
    let next = executable
        .exec
        .decode_published(*fault_pc as u32)
        .map_err(|_| Error::internal("instanceof has no instruction"))?
        .next_pc as usize;
    let mut cursor = FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
    let found = cursor.with_slots(|slots| {
        let JsValue::Object(target) = slots.peek(0)? else {
            return Ok(None);
        };
        Ok(crate::engine::builtins::try_ordinary_instanceof_in_state(
            runtime,
            state,
            slots.peek(1)?,
            *target,
            intrinsic_budget,
        ))
    })?;
    let Some(found) = found else {
        return Ok(false);
    };
    let target = cursor.move_owned()?;
    let candidate = cursor.move_owned()?;
    cursor.commit_push(JsValue::Bool(found))?;
    cursor.advance(next);
    let poisoned = &runtime.0.poisoned;
    state
        .release_owned_jsvalue(poisoned, candidate)
        .map_err(runtime_error_to_vm_error)?;
    state
        .release_owned_jsvalue(poisoned, target)
        .map_err(runtime_error_to_vm_error)?;
    profile!(record_owned_execution_event("instanceof_in_segment"));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::test_support::{eval_string, test_context};

    #[test]
    fn instanceof_in_the_segment_matches_the_operator() {
        let (_runtime, mut context) = test_context();
        let source = r#"
var out = [];
function t(name, f) { try { out.push(name + '=' + JSON.stringify(f())); } catch (e) { out.push(name + '!' + e.name); } }
function run(f) { var r = []; for (var i = 0; i < 3; i++) r.push(f()); return r.join('/'); }
class A {} class B extends A {} function F() {} var o = new B(), f = new F();
t('basic', () => run(() => [o instanceof A, o instanceof B, f instanceof F, f instanceof A, o instanceof Object].join()));
t('prims', () => run(() => [1 instanceof Object, 'x' instanceof String, null instanceof Object, undefined instanceof F, 1n instanceof Object, Symbol() instanceof Object].join()));
t('notCallable', () => run(() => o instanceof {}));
t('hasInstance', () => { var C = { [Symbol.hasInstance](v) { return v === 1; } }; return run(() => [1 instanceof C, 2 instanceof C].join()); });
t('fnHasInstance', () => { function G() {} Object.defineProperty(G, Symbol.hasInstance, { value: () => true }); return run(() => ({} instanceof G)); });
t('bound', () => { var BB = B.bind(null); return run(() => o instanceof BB); });
t('proxy', () => { var P = new Proxy(A, {}); return run(() => o instanceof P); });
t('protoNonObj', () => { function H() {} H.prototype = 1; return run(() => ({} instanceof H)); });
t('protoGetter', () => { var n = 0; var obj = {}; var p = new Proxy({}, { getPrototypeOf() { n++; return A.prototype; } }); return [run(() => p instanceof A), n]; });
t('arrowNoProto', () => run(() => o instanceof (() => 1)));
t('arrays', () => run(() => [[] instanceof Array, [] instanceof Object, new Uint8Array(1) instanceof Uint8Array].join()));
t('nullProto', () => { var x = Object.create(null); return run(() => x instanceof Object); });
t('lazyProto', () => { function L() {} var l = Object.create(L.prototype); return run(() => l instanceof L); });
t('setProto', () => { var x = {}; var r = [x instanceof A]; Object.setPrototypeOf(x, B.prototype); r.push(x instanceof A); return r; });
t('loop', () => { var c = 0; for (var i = 0; i < 1000; i++) if ((i & 1 ? o : f) instanceof A) c++; return c; });
out.join(';')
"#;
        assert_eq!(
            eval_string(&mut context, source),
            r#"basic="true,true,true,false,true/true,true,true,false,true/true,true,true,false,true";prims="false,false,false,false,false,false/false,false,false,false,false,false/false,false,false,false,false,false";notCallable!TypeError;hasInstance="true,false/true,false/true,false";fnHasInstance="true/true/true";bound="true/true/true";proxy="true/true/true";protoNonObj!TypeError;protoGetter=["true/true/true",3];arrowNoProto!TypeError;arrays="true,true,true/true,true,true/true,true,true";nullProto="false/false/false";lazyProto="true/true/true";setProto=[false,true];loop=500"#
        );
    }
}

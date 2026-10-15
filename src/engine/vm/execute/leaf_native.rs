//! Callback-free native leaves completed inside the ordinary segment.
//!
//! A selected native normally leaves the dispatch loop and runs as a native
//! activation. Math functions, and `charCodeAt`, `charAt`, `at`, `substring`,
//! `substr` and `slice` on a String primitive receiver, cannot call
//! JavaScript or throw when every argument they read is a Number, so the call
//! completes on the caller's frame, as does `push` of one value onto a dense
//! Array with a writable length. The String results allocate through the
//! same heap entry as the native activation. Any other operand declines
//! before an operand moves, and the call takes the general path unchanged.
use super::{FallthroughPc, FrameCursor};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::builtins::native::{ArrayPushKind, NativeFunctionId};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::JsValue;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::stack::{FrameExecution, FrameTurn};

enum Leaf {
    Value(JsValue),
    String(crate::engine::value::JsString),
}

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
    if target == NativeFunctionId::ArrayPrototypePush(ArrayPushKind::Push) {
        if count != 1 || !method {
            return Ok(false);
        }
        let receiver = cursor.with_slots(|slots| {
            let (JsValue::Object(callee), JsValue::Object(receiver)) =
                (slots.peek(1)?, slots.peek(2)?)
            else {
                return Ok(None);
            };
            Ok(selected
                .matches_in_domain(runtime.domain_id(), *callee)
                .then_some(*receiver))
        })?;
        let Some(receiver) = receiver else {
            return Ok(false);
        };
        let mut input = cursor.move_owned()?;
        let length = match state.try_push_dense_value(receiver, &mut input) {
            Ok(Some(length)) => length,
            // Both outcomes leave `input` as it was; restore the operand.
            Ok(None) => {
                cursor.commit_push(input)?;
                return Ok(false);
            }
            Err(error) => {
                cursor.commit_push(input)?;
                return Err(runtime_error_to_vm_error(error));
            }
        };
        *selected_native = None;
        for _ in 0..2 {
            let operand = cursor.move_owned()?;
            state
                .release_owned_jsvalue(&runtime.0.poisoned, operand)
                .map_err(runtime_error_to_vm_error)?;
        }
        let length = i32::try_from(length).map_or(JsValue::Float(f64::from(length)), JsValue::Int);
        cursor.commit_owned(state, length)?;
        cursor.advance(fallthrough.index());
        profile!(record_owned_execution_event("native_leaf_push_in_segment"));
        return Ok(true);
    }
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
        if matches!(
            target,
            NativeFunctionId::StringPrototypeCharCodeAt
                | NativeFunctionId::StringPrototypeCharAt(_)
                | NativeFunctionId::StringPrototypeSubrange(_)
        ) {
            if !method {
                return Ok(None);
            }
            let JsValue::String(receiver) = slots.peek(count + 1)? else {
                return Ok(None);
            };
            let string = state
                .heap
                .string(*receiver)
                .map_err(|error| runtime_error_to_vm_error(error.into()))?;
            // An absent argument is `undefined`, whose Number is NaN.
            let first = if count == 0 { f64::NAN } else { numbers[0] };
            return Ok(Some(match target {
                NativeFunctionId::StringPrototypeCharAt(kind) => {
                    match crate::engine::builtins::primitive::text::char_at_result(
                        kind, string, first,
                    )
                    .map_err(runtime_error_to_vm_error)?
                    {
                        Some(unit) => Leaf::String(unit),
                        None => Leaf::Value(JsValue::Undefined),
                    }
                }
                NativeFunctionId::StringPrototypeSubrange(kind) => {
                    let end = (count >= 2).then_some(numbers[1]);
                    let (start, end) =
                        crate::engine::builtins::string_subrange_bounds(kind, string, first, end)
                            .map_err(runtime_error_to_vm_error)?;
                    Leaf::String(string.sub_string(start, end))
                }
                _ => Leaf::Value(
                    usize::try_from(crate::engine::value::number::to_int32_sat(first))
                        .ok()
                        .and_then(|index| string.code_unit_at(index))
                        .map_or(JsValue::Float(f64::NAN), |unit| {
                            JsValue::Int(i32::from(unit))
                        }),
                ),
            }));
        }
        Ok(
            crate::engine::builtins::leaf_math_numbers(runtime, target, &numbers[..count])
                .map_err(runtime_error_to_vm_error)?
                .map(Leaf::Value),
        )
    })?;
    let result = match result {
        None => return Ok(false),
        Some(Leaf::Value(value)) => value,
        Some(Leaf::String(string)) => JsValue::String(
            state
                .heap
                .allocate_string(string)
                .map_err(|error| runtime_error_to_vm_error(error.into()))?,
        ),
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
    fn string_leaves_match_the_native_activation() {
        let (_runtime, mut context) = test_context();
        // `.call` reaches the same native through Function.prototype.call,
        // which is not a leaf, so it is the activation's own answer.
        let source = r#"
            var s = 'héllo中\u{1F600}', mismatches = [], checked = 0;
            var cases = [
                ['charAt', [1]], ['charAt', []], ['charAt', [99]], ['charAt', [-1]],
                ['charAt', [2.9]], ['charAt', [NaN]], ['at', [-1]], ['at', [-99]], ['at', []],
                ['substring', [1, 3]], ['substring', [3, 1]], ['substring', []],
                ['substring', [-5, 99]], ['substring', [NaN, 2]], ['substr', [1, 2]],
                ['substr', [-3]], ['substr', [2, -1]], ['slice', [1, -1]], ['slice', [-2]],
                ['slice', [3, 1]], ['slice', [0, Infinity]], ['charCodeAt', [6]],
            ];
            for (var round = 0; round < 3; round++) {
                for (var [name, args] of cases) {
                    var expected = JSON.stringify(String.prototype[name].call(s, ...args));
                    var actual;
                    switch (args.length) {
                        case 0: actual = s[name](); break;
                        case 1: actual = s[name](args[0]); break;
                        default: actual = s[name](args[0], args[1]);
                    }
                    checked++;
                    if (JSON.stringify(actual) !== expected) mismatches.push(name + args);
                }
            }
            checked + ':' + mismatches.join() + ':' + ''.charAt(0).length + ':' + s.substring(2)
                .length + ':' + new String('ab').charAt(1)
        "#;
        assert_eq!(eval_string(&mut context, source), "66::0:6:b");
    }

    #[test]
    fn push_leaf_matches_the_native_activation() {
        let (_runtime, mut context) = test_context();
        let source = r#"
var out = [];
function t(name, f) { try { out.push(name + '=' + JSON.stringify(f())); } catch (e) { out.push(name + '!' + e.name); } }
t('basic', () => { var a = []; var r = []; for (var i = 0; i < 5; i++) r.push(a.push(i * 2)); return [a, r]; });
t('objects', () => { var a = []; var o = {}; a.push(o); a.push('s'); return [a.length, a[0] === o, a[1]]; });
t('holes', () => { var a = new Array(3); var n = a.push(9); return [n, a.length, 1 in a, a[3]]; });
t('holes2', () => { var a = [1, , 3]; return [a.push(4), a]; });
t('frozen', () => { var a = Object.freeze([1]); return a.push(2); });
t('nonext', () => { var a = [1]; Object.preventExtensions(a); return a.push(2); });
t('rolen', () => { var a = [1]; Object.defineProperty(a, 'length', { writable: false }); return a.push(2); });
t('setter', () => { var log = []; Object.defineProperty(Array.prototype, 3, { set(v) { log.push(v); }, configurable: true }); var a = [0, 1, 2]; var n = a.push(7); delete Array.prototype[3]; return [n, a.length, log, 3 in a]; });
t('arraylike', () => { var o = { length: 1 }; return [Array.prototype.push.call(o, 5), o]; });
t('multi', () => { var a = [1]; return [a.push(2, 3), a.push(), a]; });
t('typed', () => { var a = []; for (var i = 0; i < 3; i++) a.push(i + 0.5); return a; });
t('big', () => { var a = []; for (var i = 0; i < 2000; i++) a.push({ i }); var s = 0; for (var i = 0; i < a.length; i++) s += a[i].i; return [a.length, s]; });
t('subclass', () => { class A extends Array {} var a = new A(); a.push(1); return [a.length, a instanceof A]; });
t('patched', () => { var saved = Array.prototype.push; Array.prototype.push = function (v) { return 'p' + v; }; var r = [].push(1); Array.prototype.push = saved; return r; });
out.join(';')
"#;
        assert_eq!(
            eval_string(&mut context, source),
            r#"basic=[[0,2,4,6,8],[1,2,3,4,5]];objects=[2,true,"s"];holes=[4,4,false,9];holes2=[4,[1,null,3,4]];frozen!TypeError;nonext!TypeError;rolen!TypeError;setter=[4,4,[7],false];arraylike=[2,{"1":5,"length":2}];multi=[3,3,[1,2,3]];typed=[0.5,1.5,2.5];big=[2000,1999000];subclass=[1,true];patched="p1""#
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

use crate::engine::api::Runtime;
use crate::engine::value::Value;

#[test]
fn numeric_object_conversion_preserves_hints_order_and_abrupt_completion() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(context.eval(r#"
        (() => {
            function check(expression, hint, expected) {
                let events = [];
                const left = { [Symbol.toPrimitive](h) { events.push('L:' + h); return 6; } };
                const right = { [Symbol.toPrimitive](h) { events.push('R:' + h); return 2; } };
                if (expression(left, right) !== expected || events.join(',') !== 'L:' + hint + ',R:' + hint)
                    throw new Error('conversion order or hint');
                events = [];
                const marker = {};
                const throws = { [Symbol.toPrimitive](h) { events.push('L:' + h); throw marker; } };
                try { expression(throws, right); throw new Error('missing throw'); }
                catch (e) { if (e !== marker) throw e; }
                if (events.join(',') !== 'L:' + hint) throw new Error('right converted after left throw');
            }
            check((a, b) => a + b, 'default', 8);
            check((a, b) => a - b, 'number', 4);
            check((a, b) => a >>> b, 'number', 1);
            check((a, b) => a < b, 'number', false);
            check((a, b) => a > b, 'number', true);
            check((a, b) => a <= b, 'number', false);
            check((a, b) => a >= b, 'number', true);
            let hints = [];
            const object = { [Symbol.toPrimitive](h) { hints.push(h); return '6'; } };
            if (+object !== 6 || -object !== -6 || ~object !== -7) return false;
            let value = object;
            if (value++ !== 6 || value !== 7) return false;
            value = object;
            if (--value !== 5 || hints.join(',') !== 'number,number,number,number,number') return false;
            let ordinary = [];
            if (({ valueOf() { ordinary.push('valueOf'); return {}; },
                   toString() { ordinary.push('toString'); return '4'; } }) - 1 !== 3) return false;
            return ordinary.join(',') === 'valueOf,toString';
        })()
    "#).unwrap(), Value::Bool(true));
}

#[test]
fn numeric_preparation_keeps_string_bigint_and_number_semantics() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(context.eval(r#"
        (() => {
            if ('1' + 2 !== '12' || '10' < '2' !== true || 1n + 2n !== 3n) return false;
            if ('6' - true !== 5 || null * 3 !== 0 || !Number.isNaN(undefined - 1)) return false;
            if (2 + 0.5 !== 2.5 || !Object.is(-0 * 1, -0) || !Object.is(-0 + -0, -0)) return false;
            if (NaN < 1 || NaN >= 1 || Infinity + 1 !== Infinity) return false;
            let value = '6';
            if (value++ !== 6 || value !== 7) return false;
            value = 2147483647;
            if (++value !== 2147483648) return false;
            for (const operation of [() => 1n + 2, () => +1n, () => Symbol() - 1, () => 1n >>> 1n]) {
                try { operation(); return false; }
                catch (e) { if (!(e instanceof TypeError)) throw e; }
            }
            return true;
        })()
    "#).unwrap(), Value::Bool(true));
}

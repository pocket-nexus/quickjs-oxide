// for-of, spread, destructuring and the iterator protocol.
var total = 0;
for (var value of [1, 2, 3]) total += value;
for (const ch of "hé") total += ch.length;
print(total);

function* range(n) { for (let i = 0; i < n; i++) yield i; }
print([...range(4)].join(), Math.max(...range(5)), [0, ...[1, 2], ...range(2)].length);

var closed = [];
var iterable = {
  [Symbol.iterator]() {
    var i = 0;
    return {
      next() { return { value: i++, done: i > 5 }; },
      return() { closed.push("closed"); return {}; },
    };
  },
};
for (var x of iterable) { if (x === 2) break; }
try { for (var y of iterable) { throw new Error("boom"); } } catch (e) { print(e.message); }
print(closed.join());

var [a, b = 10, ...rest] = [1, undefined, 3, 4];
var { p, q: { r = "deep" } = {}, ...others } = { p: 1, s: 2, t: 3 };
print(a, b, rest.join(), p, r, JSON.stringify(others));
function params({ x, y = 2 }, [z, w] = [3, 4], ...more) { return [x, y, z, w, more.length].join(); }
print(params({ x: 1 }), params({ x: 1, y: 0 }, [5, 6], 7, 8));
var swap1 = 1, swap2 = 2;
[swap1, swap2] = [swap2, swap1];
print(swap1, swap2);
var { length } = "four";
print(length);
try { var { missing } = null; } catch (e) { print(e.constructor.name); }

var copy = { ...{ a: 1, b: 2 }, c: 3, ...null };
print(JSON.stringify(copy));

function* inner() { var got = yield 1; print("got", got); yield 2; return "inner done"; }
function* outer() { var result = yield* inner(); print(result); }
var it = outer();
print(JSON.stringify(it.next()), JSON.stringify(it.next("A")), JSON.stringify(it.next()));

var labels = [];
outer: for (var i of [1, 2, 3]) {
  for (var j of [1, 2, 3]) {
    if (j === 2) continue outer;
    if (i === 3) break outer;
    labels.push(i + "" + j);
  }
}
print(labels.join());

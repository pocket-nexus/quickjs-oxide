// mode: print
// The combinators' element functions and result lists (27.2.4.1–27.2.4.3).
function Custom(executor) {
    var state = { settled: [] };
    executor(function (value) { state.settled.push(["resolve", value]); },
             function (reason) { state.settled.push(["reject", reason]); });
    return state;
}
Custom.resolve = function (value) { return value; };
// case: allSettled element functions share one alreadyCalled record
var state = Promise.allSettled.call(Custom, [
    { then: function (onFulfilled, onRejected) { onFulfilled(1); onRejected(2); } }
]);
var values = state.settled[0][1];
print(state.settled.length, values.length, values[0].status, values[0].value);
// case: a late onRejected after fulfillment leaves the result unchanged
var late;
var state = Promise.allSettled.call(Custom, [
    { then: function (onFulfilled, onRejected) { onFulfilled(1); late = onRejected; } }
]);
var values = state.settled[0][1];
late(2);
print(state.settled.length, values[0].status, values[0].value);
// case: all resolves with a fresh array the elements no longer write
var callbacks = [];
var state = Promise.all.call(Custom, [
    { then: function (onFulfilled) { callbacks.push(onFulfilled); } },
    { then: function (onFulfilled) { callbacks.push(onFulfilled); } }
]);
callbacks[0]("a");
callbacks[1]("b");
var values = state.settled[0][1];
values.push("mutated");
callbacks[0]("late");
print(state.settled.length, Array.isArray(values), values.join(","));
// case: any rejects with an AggregateError over a fresh errors array
var state = Promise.any.call(Custom, [
    { then: function (onFulfilled, onRejected) { onRejected("x"); } }
]);
var error = state.settled[0][1];
print(state.settled[0][0], error instanceof AggregateError, error.errors.join(","),
    Object.getOwnPropertyDescriptor(error, "errors").enumerable);

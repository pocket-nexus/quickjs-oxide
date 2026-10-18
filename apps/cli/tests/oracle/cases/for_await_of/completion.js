// mode: print
// ForIn/OfBodyEvaluation (14.7.5.6): a done result ends the loop without
// reading its value or closing the iterator.
// case: a completed async iterator is neither read nor closed
var log = [];
var iterable = {};
iterable[Symbol.asyncIterator] = function () {
    var step = 0;
    return {
        next: function () {
            step++;
            return {
                get done() { log.push("done:" + step); return step > 1; },
                get value() { log.push("value:" + step); return step; }
            };
        },
        return: function () { log.push("return"); return {}; }
    };
};
(async function () {
    for await (var x of iterable) log.push("body:" + x);
    print(log.join(","));
})();

// mode: print
// case: brand failure through Function.prototype.call
Number.prototype.toFixed.call(Function, 0)
// case: toFixed range
(1).toFixed(-1)
// case: toExponential range
(1).toExponential(101)
// case: toPrecision range
(1).toPrecision(0)
// case: toString radix range
(1).toString(1)
// case: BigInt digits conversion
(1).toFixed(1n)
// case: predicate is not constructable
new Number.isNaN()
// case: parse alias is not constructable
new Number.parseInt()
// case: formatter is not constructable
new Number.prototype.toFixed()
// case: valueOf is not constructable
new Number.prototype.valueOf()

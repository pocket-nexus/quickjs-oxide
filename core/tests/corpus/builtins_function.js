// Function: Function.prototype (callable), call, apply, toString.
print(typeof Function.prototype, Function.prototype(), Function.prototype.call === (function () {}).call);
function add(a, b) { return a + b; }
print(add.call(null, 1, 2), add.apply(null, [3, 4]), typeof add.toString());
print((function () {}).constructor === Function, add instanceof Function, add instanceof Object);
try { Function.prototype.toString.call({}); } catch (e) { print(e.name); }

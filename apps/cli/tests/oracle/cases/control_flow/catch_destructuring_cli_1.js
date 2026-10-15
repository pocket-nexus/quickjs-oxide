// mode: print
// flags:
// flags: --strip-source
// flags: -s
// case: object catch conversion fault uses the throw site
(function catchObjectBinding(){
  try {
    throw null;
  } catch ({ value }) {
  }
})()
// case: computed catch key fault keeps its user function origin
(function catchComputedBinding(){
  var key = {};
  key[Symbol.toPrimitive] = function catchKey(){ throw new Error("computed"); };
  try {
    throw {};
  } catch ({ [key]: value }) {
  }
})()
// case: array catch default fault closes at its initializer origin
(function catchArrayBinding(){
  var iterable = {
    [Symbol.iterator]: function(){ return this; },
    next: function(){ return { value: undefined, done: false }; },
    return: function(){ return { done: true }; }
  };
  try {
    throw iterable;
  } catch ([value = (function catchDefault(){ null.value; })()]) {
  }
})()

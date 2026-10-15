// mode: print
// flags:
// flags: --strip-source
// flags: -s
// case: caught native fault rethrown without replacing its origin
(function outer(){
  try {
    (function inner(){ null.rethrowFault; })();
  } catch (error) {
    throw error;
  }
})()
// case: fault raised from a catch body
(function outer(){
  try {
    throw 1;
  } catch (error) {
    (function catchInner(){ null.catchFault; })();
  }
})()
// case: finally fault overrides the try fault
(function outer(){
  try {
    null.tryFault;
  } finally {
    null.finallyFault;
  }
})()
// case: caught Error object keeps its eager callee stack
(function outer(){
  try {
    (function inner(){ throw new Error('boom'); })();
  } catch (error) {
    throw error;
  }
})()

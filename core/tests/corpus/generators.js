// Generators: suspension, return, throw, finally.
function* counter() {
  try {
    var n = 0;
    while (true) {
      var reset = yield n++;
      if (reset) n = 0;
    }
  } finally {
    print("cleanup");
  }
}
var g = counter();
print(g.next().value, g.next().value, g.next(true).value, g.next().value);
print(JSON.stringify(g.return(9)), JSON.stringify(g.next()));

function* catcher() {
  while (true) {
    try { yield "waiting"; } catch (e) { print("caught", e); }
  }
}
var c = catcher();
c.next();
print(c.throw("x").value);

var o = {
  *items() { yield "a"; yield "b"; },
  [Symbol.iterator]: function* () { yield* this.items(); },
};
print([...o].join());

class Tree {
  constructor(value, left, right) { this.value = value; this.left = left; this.right = right; }
  *[Symbol.iterator]() {
    if (this.left) yield* this.left;
    yield this.value;
    if (this.right) yield* this.right;
  }
}
print([...new Tree(2, new Tree(1), new Tree(3))].join());

function* delegateThrow() {
  try { yield* catcher(); } finally { print("outer finally"); }
}
var dt = delegateThrow();
dt.next();
print(dt.throw("inner").value);
print(JSON.stringify(dt.return("done")));
var fresh = counter();
print(JSON.stringify(fresh.return(1)));
try { var bad = counter(); bad.next(); bad.throw(new Error("fail")); } catch (e) { print(e.message); }

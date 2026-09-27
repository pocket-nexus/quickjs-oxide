function makeCell(n) {
  let value = n;
  return function() { value += 1; return value; };
}
globalThis.keep = [];
for (let i = 0; i < 4096; i++) keep.push(makeCell(i));
let result = keep[0]() + keep[4095]();
globalThis.keep = null;
print(result);

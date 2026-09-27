function makeCell(n) {
  let value = n;
  return function() { value += 1; return value; };
}
globalThis.keep = [];
for (let i = 0; i < 4096; i++) keep.push(makeCell(i));
print(keep[0]() + keep[4095]());

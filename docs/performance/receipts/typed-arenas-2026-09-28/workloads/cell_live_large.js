function makeCell(n) {
  let value = n;
  return function() { value += 1; return value; };
}
globalThis.keep = [];
for (let i = 0; i < 32768; i++) keep.push(makeCell(i));
print(keep[0]() + keep[32767]());

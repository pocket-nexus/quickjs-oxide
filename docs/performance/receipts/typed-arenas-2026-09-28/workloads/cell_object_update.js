function makeCell(n) {
  let value = {n};
  return {
    set(next) { value = next; },
    read() { return value.n; }
  };
}
globalThis.keep = [];
for (let i = 0; i < 2048; i++) keep.push(makeCell(i));
for (let i = 0; i < 2048; i++) keep[i].set({n: i + 1});
print(keep[0].read() + keep[2047].read());

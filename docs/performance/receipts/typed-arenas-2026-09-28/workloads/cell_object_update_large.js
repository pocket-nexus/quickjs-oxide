function makeCell(n) {
  let value = {n};
  return {
    set(next) { value = next; },
    read() { return value.n; }
  };
}
globalThis.keep = [];
for (let i = 0; i < 16384; i++) keep.push(makeCell(i));
for (let i = 0; i < 16384; i++) keep[i].set({n: i + 1});
print(keep[0].read() + keep[16383].read());

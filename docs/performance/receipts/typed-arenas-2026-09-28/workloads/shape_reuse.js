globalThis.keep = [];
for (let i = 0; i < 4096; i++) {
  let object = {};
  object["key" + i] = i;
  keep.push(object);
}
let result = keep[4095].key4095;
globalThis.keep = null;
globalThis.keep = [];
for (let i = 0; i < 4096; i++) {
  let object = {};
  object["key" + i] = i;
  keep.push(object);
}
print(result + keep[4095].key4095);

globalThis.keep = [];
for (let i = 0; i < 4096; i++) {
  let object = {};
  object["key" + i] = i;
  keep.push(object);
}
print(keep[4095].key4095);

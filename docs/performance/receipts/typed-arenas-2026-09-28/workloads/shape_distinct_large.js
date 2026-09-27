globalThis.keep = [];
for (let i = 0; i < 32768; i++) {
  let object = {};
  object["key" + i] = i;
  keep.push(object);
}
print(keep[32767].key32767);

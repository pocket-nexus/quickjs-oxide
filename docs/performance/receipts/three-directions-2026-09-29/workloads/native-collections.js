// Object arguments and method receivers retain their identity across transfer.
function run(count) {
  var map = new Map(), key = {}, value = {}, sum = 0;
  for (var i = 0; i < count; i++) {
    map.set(key, value);
    if (map.get(key) !== value || !map.has(key)) throw new Error('identity');
    if (!map.delete(key)) throw new Error('delete');
    sum++;
  }
  return sum + ':' + map.size;
}
print(run(100000));

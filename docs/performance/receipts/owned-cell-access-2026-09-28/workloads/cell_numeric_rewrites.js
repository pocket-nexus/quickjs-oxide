function makeSetter() {
  let value = 0;
  return function(next) { value = next; return value; };
}
const setter = makeSetter();
let total = 0;
for (let i = 0; i < 262144; i++) total += setter(i);
print(total);

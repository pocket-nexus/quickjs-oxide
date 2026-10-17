// Async functions, await, async generators and job order.
var log = [];
async function f(x) {
  log.push("f start " + x);
  var y = await x;
  log.push("f resumed " + y);
  return y * 2;
}
f(1).then(function (v) { log.push("f result " + v); });
Promise.resolve().then(function () { log.push("then 1"); }).then(function () { log.push("then 2"); });
log.push("sync end");

async function thrower() { await null; throw new Error("async boom"); }
thrower().catch(function (e) { log.push("caught " + e.message); });

async function* agen() {
  yield 1;
  var got = yield await Promise.resolve(2);
  log.push("agen got " + got);
  return 3;
}
(async function () {
  var it = agen();
  log.push(JSON.stringify(await it.next()));
  log.push(JSON.stringify(await it.next()));
  log.push(JSON.stringify(await it.next("x")));
  var sum = 0;
  for await (var v of agen()) sum += v;
  for await (var w of [Promise.resolve(10), 20]) sum += w;
  log.push("sum " + sum);
  var arrow = async () => (await 5) + 1;
  log.push("arrow " + (await arrow()));
  class K { async m() { return await "method"; } static async s() { return "static"; } }
  log.push((await new K().m()) + " " + (await K.s()));
})().then(function () {
  print(log.join("\n"));
});

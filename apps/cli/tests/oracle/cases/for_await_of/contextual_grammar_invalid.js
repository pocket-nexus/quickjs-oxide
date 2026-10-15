// mode: completion-name
// case: for await(var x of xs){}
for await(var x of xs){}
// case: function f(xs){for await(var x of xs){}}
function f(xs){for await(var x of xs){}}
// case: function* f(xs){for await(var x of xs){}}
function* f(xs){for await(var x of xs){}}
// case: async function f(xs){for await(var x in xs){}}
async function f(xs){for await(var x in xs){}}
// case: async function f(xs){for aw\u0061it(var x of xs){}}
async function f(xs){for aw\u0061it(var x of xs){}}

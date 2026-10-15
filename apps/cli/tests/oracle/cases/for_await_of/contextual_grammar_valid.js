// mode: print
// case: async function f(xs){for await(var x of xs){}}
async function f(xs){for await(var x of xs){}}
// case: async function* f(xs){for await(var x of xs){yield x;}}
async function* f(xs){for await(var x of xs){yield x;}}
// case: ({async f(xs){for await(async of xs){}}})
({async f(xs){for await(async of xs){}}})
// case: class C { static async f(xs){for await(const x of xs){}} }
class C { static async f(xs){for await(const x of xs){}} }

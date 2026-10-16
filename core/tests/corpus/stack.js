function down(n) { return down(n + 1) + 1; }
try { down(0); } catch (e) { print(e.name, e.message, e instanceof Error); }
function deepButFine(n) { return n === 0 ? 'ok' : deepButFine(n - 1); }
print(deepButFine(5000));
var o = { valueOf: function () { return this + 1; } };
try { o + 1; } catch (e) { print(e.name, e.message); }

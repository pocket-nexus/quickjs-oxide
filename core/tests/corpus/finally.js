// `finally` runs on every exit path.
var log = '';
function normal() { try { log += 'a'; } finally { log += 'b'; } return log; }
print(normal());
log = '';
function returning() { try { log += 'r'; return 1; } finally { log += 'f'; } }
print(returning(), log);
log = '';
function throwing() {
    try { try { throw 'x'; } finally { log += 'inner'; } } catch (e) { log += ':' + e; }
    return log;
}
print(throwing());
log = '';
function override() { try { return 'try'; } finally { return 'finally'; } }
print(override());
function loops() {
    var out = '';
    for (var i = 0; i < 4; i++) {
        try {
            if (i === 1) continue;
            if (i === 3) break;
            out += i;
        } finally {
            out += 'f';
        }
    }
    return out;
}
print(loops());
function nested() {
    var out = '';
    try {
        try { out += '1'; throw new Error('e'); }
        catch (e) { out += '2'; throw e; }
        finally { out += '3'; }
    } catch (e) {
        out += '4' + e.message;
    } finally {
        out += '5';
    }
    return out;
}
print(nested());
function swallow() { for (;;) { try { throw 1; } finally { break; } } return 'swallowed'; }
print(swallow());
function value() { var x = 1; try { return x; } finally { x = 2; } }
print(value());

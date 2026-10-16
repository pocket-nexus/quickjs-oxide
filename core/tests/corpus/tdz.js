// Script-level `let` and `const` are global properties in phase 2 (no TDZ
// there yet); every binding here is function-scoped.
function program() {
    function before() { return x; }
    try { before(); } catch (e) { print(e.name + ': ' + e.message); }
    let x = 1;
    print(before());
    function local() { try { return y; } catch (e) { return e.name; } let y; }
    print(local());
    const c = 5;
    try { c = 6; } catch (e) { print(e.name + ': ' + e.message); }
    print(c);
    function captured() { var f = () => z; try { f(); } catch (e) { print('captured', e.name); } let z = 2; return f(); }
    print(captured());
    function selfRef() { try { let q = q + 1; } catch (e) { return e.name; } }
    print(selfRef());
    function constLocal() { const k = 1; try { k = 2; } catch (e) { return e.message; } }
    print(constLocal());
    function loopTdz() {
        var out = '';
        for (let i = 0; i < 2; i++) { try { out += inner; } catch (e) { out += 'T'; } let inner = i; out += inner; }
        return out;
    }
    print(loopTdz());
}
program();

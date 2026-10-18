// `#x in o` checks the class's brand for every private accessor (13.10.1,
// PrivateElementFind).
// case: setter-only getter-only and paired accessors test the brand
(function () {
    class C {
        set #setter(value) {}
        get #getter() { return 1; }
        get #both() { return 1; }
        set #both(value) {}
        static test(o) { return [#setter in o, #getter in o, #both in o].join(","); }
    }
    var impostor = { "[unsupported type]": 1, undefined: 1 };
    return C.test(new C()) + "|" + C.test({}) + "|" + C.test(impostor);
})()

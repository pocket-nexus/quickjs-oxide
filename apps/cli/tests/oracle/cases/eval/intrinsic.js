// mode: completion
// case: eval shell
(function () {
    function flags(descriptor) {
        return (descriptor.writable ? "1" : "0") +
               (descriptor.enumerable ? "1" : "0") +
               (descriptor.configurable ? "1" : "0");
    }
    function isConstructor(value) {
        try {
            Reflect.construct(function () {}, [], value);
            return true;
        } catch (_) {
            return false;
        }
    }

    var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "eval");
    var lengthDescriptor = Object.getOwnPropertyDescriptor(eval, "length");
    var nameDescriptor = Object.getOwnPropertyDescriptor(eval, "name");
    var observations = [
        "metadata=" + [
            typeof eval,
            eval.name,
            eval.length,
            Object.getPrototypeOf(eval) === Function.prototype,
            Object.prototype.hasOwnProperty.call(eval, "prototype"),
            eval.prototype === undefined,
            isConstructor(eval),
            Object.getOwnPropertyNames(eval).join(","),
            globalDescriptor.value === eval,
            flags(globalDescriptor),
            flags(lengthDescriptor),
            flags(nameDescriptor)
        ].join("|")
    ];

    var constructError = "none";
    try {
        new eval();
    } catch (error) {
        constructError = error.name;
    }
    observations.push("construct=" + constructError);

    var marker = {};
    var alias = eval;
    observations.push("calls=" + [
        eval(marker) === marker,
        (eval)(marker) === marker,
        ((eval))(marker) === marker,
        \u0065val(marker) === marker,
        (function (eval) { return eval(marker) === marker; })(eval),
        (0, eval)(marker) === marker,
        alias(marker) === marker,
        globalThis.eval(marker) === marker,
        eval.call(null, marker) === marker,
        eval.apply(null, [marker]) === marker,
        eval() === undefined
    ].join("|"));

    var coercions = 0;
    function poison() {
        coercions++;
        throw 99;
    }
    var ordinary = {};
    ordinary[Symbol.toPrimitive] = poison;
    ordinary.toString = poison;
    ordinary.valueOf = poison;
    var boxedString = new String("40 + 2");
    boxedString[Symbol.toPrimitive] = poison;
    boxedString.toString = poison;
    boxedString.valueOf = poison;
    var symbol = Symbol("eval source");
    observations.push("identity=" + [
        eval(ordinary) === ordinary,
        eval(boxedString) === boxedString,
        eval(symbol) === symbol,
        coercions
    ].join("|"));

    var held = eval;
    var deleted = delete globalThis.eval;
    var absent = typeof globalThis.eval === "undefined";
    var heldAfterDelete = held(ordinary) === ordinary;
    globalThis.eval = function replacement() { return 17; };
    var replacementVisible = globalThis.eval(ordinary) === 17;
    var heldAfterReplacement = held(ordinary) === ordinary;
    observations.push("mutation=" + [
        deleted,
        absent,
        heldAfterDelete,
        replacementVisible,
        heldAfterReplacement,
        coercions
    ].join("|"));

    return observations.join("\n");
})()
// case: global Proxy binding internal methods
(function () {
    var originalPrototype = Object.getPrototypeOf(globalThis);
    var observations = [];
    var log;
    var proxy;
    var target;
    var value;

    function restore() {
        Object.setPrototypeOf(globalThis, originalPrototype);
    }
    function thrownValue(thunk) {
        try {
            thunk();
            return "none";
        } catch (error) {
            if (error !== null && typeof error === "object")
                return error.name + ":" + error.message;
            return typeof error + ":" + String(error);
        }
    }

    log = [];
    proxy = new Proxy({ proxyEvalRead: 42 }, {
        get: function (target, key, receiver) {
            log.push("get:" + String(key));
            return Reflect.get(target, key, receiver);
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = eval("proxyEvalRead");
    restore();
    observations.push("read=" + value + ":" + log.join(","));

    log = [];
    proxy = new Proxy({ proxyFunctionRead: 84 }, {
        get: function (target, key, receiver) {
            log.push("get:" + String(key));
            return Reflect.get(target, key, receiver);
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = Function("return proxyFunctionRead")();
    restore();
    observations.push("functionRead=" + value + ":" + log.join(","));

    proxy = new Proxy({}, {});
    Object.setPrototypeOf(globalThis, proxy);
    value = eval("proxyEvalMissing");
    restore();
    observations.push("missing=" + String(value));

    proxy = new Proxy({}, {
        get: function () {
            throw 19;
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = thrownValue(function () {
        eval("proxyEvalThrowingRead");
    });
    restore();
    observations.push("getThrow=" + value);

    log = [];
    target = { proxyEvalWrite: 1 };
    proxy = new Proxy(target, {
        has: function (target, key) {
            log.push("has:" + String(key));
            return Reflect.has(target, key);
        },
        set: function (target, key, next, receiver) {
            log.push("set:" + String(key) + ":" + next);
            return Reflect.set(target, key, next, receiver);
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    eval("proxyEvalWrite = 43");
    value = Object.prototype.hasOwnProperty.call(globalThis, "proxyEvalWrite");
    delete globalThis.proxyEvalWrite;
    restore();
    observations.push("write=" + log.join(",") + ":" + value + ":" + target.proxyEvalWrite);

    proxy = new Proxy({ proxyEvalHasThrow: 1 }, {
        has: function () {
            throw 17;
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = thrownValue(function () {
        eval("proxyEvalHasThrow = 2");
    });
    delete globalThis.proxyEvalHasThrow;
    restore();
    observations.push("hasThrow=" + value);

    log = [];
    proxy = new Proxy({ proxyEvalDelete: 1 }, {
        has: function (target, key) {
            log.push("has:" + String(key));
            return Reflect.has(target, key);
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = eval("delete proxyEvalDelete");
    restore();
    observations.push("delete=" + value + ":" + log.join(","));

    target = {};
    Object.defineProperty(target, "proxyEvalReadonly", {
        value: 1,
        writable: false,
        configurable: true
    });
    proxy = new Proxy(target, {});
    Object.setPrototypeOf(globalThis, proxy);
    value = thrownValue(function () {
        eval("'use strict'; proxyEvalReadonly = 2");
    });
    restore();
    observations.push("fallbackReject=" + value);

    proxy = new Proxy({ proxyEvalTrapReject: 1 }, {
        has: function () {
            return true;
        },
        set: function () {
            return false;
        }
    });
    Object.setPrototypeOf(globalThis, proxy);
    value = thrownValue(function () {
        eval("'use strict'; proxyEvalTrapReject = 2");
    });
    restore();
    observations.push("trapReject=" + value);

    return observations.join("|");
})()
// case: direct and indirect eval syntax
(function () {
    globalThis.x = "G";
    var result = (function () {
        var x = "L";
        var original = eval;
        var alias = eval;
        var observations = [
            eval("x"),
            (eval)("x"),
            ((eval))("x"),
            \u0065val("x"),
            (function (eval) { return eval("x"); })(original),
            (0, eval)("x"),
            alias("x"),
            globalThis.eval("x"),
            eval.call(null, "x"),
            eval.apply(null, ["x"]),
            (true ? eval : eval)("x"),
            (eval = original)("x"),
            (function () {
                try { new eval("x"); return "none"; }
                catch (error) { return error.name; }
            })(),
            (function (eval) { return eval("x", 7); })(
                function replacement(source, extra) {
                    return "R:" + source + ":" + extra;
                }
            )
        ];
        globalThis.eval = function replacement(source, extra) {
            return "R:" + source + ":" + extra;
        };
        observations.push(eval("x", 8));
        globalThis.eval = original;
        observations.push(eval(...["x"]));
        observations.push(eval?.("x"));
        return observations.join("|");
    })();
    delete globalThis.x;
    return result;
})()
// case: primitive string directness
(function () {
    globalThis.x = "G";
    var result = (function () {
        var x = "L";
        var original = eval;
        var alias = eval;
        return [
            eval("x"),
            (eval)("x"),
            ((eval))("x"),
            \u0065val("x"),
            (function (eval) { return eval("x"); })(original),
            (0, eval)("x"),
            alias("x"),
            globalThis.eval("x"),
            eval.call(null, "x"),
            eval.apply(null, ["x"]),
            (true ? eval : eval)("x"),
            (eval = original)("x")
        ].join("|");
    })();
    delete globalThis.x;
    return result;
})()
// case: eval environment
(function () {
    var observations = [];
    function sloppy(argument) {
        var local = 2;
        {
            let block = 3;
            observations.push("direct=" +
                eval("[argument,local,block,this.tag,arguments[0]].join(',')"));
            eval("local=4; var added=5; let hidden=6");
        }
        observations.push("sloppy=" + [local, added, typeof hidden].join(","));
    }
    sloppy.call({ tag: "T" }, 1);

    function strict() {
        "use strict";
        var outer = 1;
        var within = eval(
            "outer=2; var onlyVar=3; let onlyLex=4; [onlyVar,onlyLex].join(',')"
        );
        observations.push(
            "strict=" + [within, outer, typeof onlyVar, typeof onlyLex].join("|")
        );
    }
    strict();

    function redeclaration() {
        let conflict = 1;
        globalThis.evalTouch = 0;
        try {
            eval("evalTouch=1; var conflict");
        } catch (error) {
            observations.push(
                "redeclare=" + [error.name, evalTouch, conflict].join(",")
            );
        }
        delete globalThis.evalTouch;
    }
    redeclaration();

    function C(argument) {
        observations.push(
            "special=" +
            eval("[new.target===C,arguments[0],this instanceof C].join(',')")
        );
    }
    new C(7);

    observations.push(
        "indirect=" +
        (0, eval)(
            "var indirectVar=8; let indirectLex=9; " +
            "[this===globalThis,indirectVar,typeof indirectLex].join(',')"
        )
    );
    observations.push(
        "indirectAfter=" +
        [globalThis.indirectVar, typeof indirectLex, delete globalThis.indirectVar].join(",")
    );
    observations.push(
        "indirectStrict=" +
        (0, eval)(
            "'use strict'; var strictVar=10; let strictLex=11; " +
            "[this===globalThis,strictVar,typeof strictLex].join(',')"
        )
    );
    observations.push(
        "indirectStrictAfter=" + [typeof strictVar, typeof strictLex].join(",")
    );
    return observations.join("\n");
})()
// case: var objects and source order
(function () {
    var observations = [];
    function sloppy(parameter) {
        var existing = 1;
        function compiledBeforeEval() {
            return [fresh, existing, parameter].join(",");
        }
        var completion = eval(
            "var fresh=40; existing=2; parameter=3; fresh+2"
        );
        observations.push(
            "sloppy=" + [
                completion,
                compiledBeforeEval(),
                existing,
                parameter,
                delete fresh,
                typeof fresh
            ].join("|")
        );
    }
    sloppy(9);

    function retained() {
        eval("var answer=42");
        return function () { return answer; };
    }
    observations.push("closure=" + retained()());

    function repeated() {
        eval("var x=1");
        eval("var x");
        return typeof x + "|" + String(x);
    }
    observations.push("repeat=" + repeated());

    function functionThenVar() {
        return eval("function x(){}; var x; typeof x");
    }
    function varThenFunction() {
        return eval("var x; function x(){}; typeof x");
    }
    observations.push(
        "order=" + functionThenVar() + "|" + varThenFunction()
    );
    return observations.join("\n");
})()
// case: strict and indirect eval declarations
(function () {
    var observations = [];
    function strictCaller() {
        "use strict";
        var value = eval(
            "var strictVar=40; function strictFn(){return strictVar+2} strictFn()"
        );
        return [value, typeof strictVar, typeof strictFn].join("|");
    }
    function strictSource() {
        var value = eval(
            "'use strict'; var strictVar=40; " +
            "function strictFn(){return strictVar+2} strictFn()"
        );
        return [value, typeof strictVar, typeof strictFn].join("|");
    }
    observations.push(
        "strict=" + strictCaller() + "|" + strictSource()
    );

    var indirect = (0, eval)(
        "var indirectVar=40; " +
        "function indirectFn(){return indirectVar+2} indirectFn()"
    );
    var varFlags = Object.getOwnPropertyDescriptor(globalThis, "indirectVar");
    var fnFlags = Object.getOwnPropertyDescriptor(globalThis, "indirectFn");
    observations.push(
        "indirect=" + [
            indirect,
            indirectFn(),
            varFlags.configurable,
            fnFlags.configurable,
            delete globalThis.indirectVar,
            delete globalThis.indirectFn
        ].join("|")
    );

    var indirectStrict = (0, eval)(
        "'use strict'; var strictVar=40; " +
        "function strictFn(){return strictVar+2} strictFn()"
    );
    observations.push(
        "indirectStrict=" + [
            indirectStrict,
            typeof strictVar,
            typeof strictFn
        ].join("|")
    );
    return observations.join("\n");
})()
// case: catch lexicals and implicit bindings
(function () {
    var observations = [];
    function caught() {
        try {
            throw 1;
        } catch (error) {
            eval("var error=42");
            return error;
        }
    }
    observations.push("catch=" + caught());

    function sourceCatch() {
        var sloppy = eval("try{throw 1}catch(e){var e=2;e}");
        var strict = eval(
            "'use strict';try{throw 1}catch(e){var e=2;e}"
        );
        return sloppy + "|" + strict;
    }
    observations.push("sourceCatch=" + sourceCatch());

    function outer() {
        let value = 1;
        var inner = (function () {
            eval("var value=2");
            return value;
        })();
        return inner + "|" + value;
    }
    observations.push("outer=" + outer());

    function conflict() {
        let value = 1;
        try {
            eval("var value=2");
            return "none";
        } catch (error) {
            return error.name + "|" + value;
        }
    }
    observations.push("conflict=" + conflict());

    function named(argument) {
        eval("var arguments=1; var named=2");
        return [
            arguments[0],
            named,
            eval("arguments"),
            eval("named"),
            eval("delete arguments"),
            eval("delete named"),
            arguments[0],
            typeof named
        ].join("|");
    }
    observations.push("special=" + named(7));
    return observations.join("\n");
})()
// case: private function expression names
(function () {
    var observations = [];

    var direct = function named() {
        eval("var named=42");
        return [typeof named, named === direct, eval("named")].join("|");
    };
    observations.push("direct=" + direct());

    var hoist = function named() {
        var inside = eval("function named(){return 43}; named()");
        return [inside, typeof named, named === hoist, eval("named()")].join("|");
    };
    observations.push("function=" + hoist());

    var nested = function named() {
        var inside = eval(
            "eval(\"var named=44\"); " +
            "[typeof named,named].join(\"|\")"
        );
        return [inside, typeof named, named === nested, eval("named")].join("|");
    };
    observations.push("nested=" + nested());

    var strictCaller = function named() {
        "use strict";
        var inside = eval("var named=45; named");
        return [
            inside,
            typeof named,
            named === strictCaller,
            eval("typeof named")
        ].join("|");
    };
    observations.push("strictCaller=" + strictCaller());

    var strictSource = function named() {
        var inside = eval("\"use strict\"; var named=46; named");
        return [
            inside,
            typeof named,
            named === strictSource,
            eval("named===strictSource")
        ].join("|");
    };
    observations.push("strictSource=" + strictSource());

    var parent = function named() {
        eval("var named=47");
        return function () {
            return [typeof named, named === parent].join("|");
        };
    };
    observations.push("plainChild=" + parent()());

    var childParent = function named() {
        return function () {
            eval("var named=48");
            return [
                typeof named,
                named === childParent,
                eval("named")
            ].join("|");
        };
    };
    observations.push("childOwn=" + childParent()());

    var sourceView = function named() {
        var inside = eval(
            "var named=49; " +
            "[typeof named,named===sourceView].join(\"|\")"
        );
        return [inside, typeof named, named === sourceView].join("|");
    };
    observations.push("sourceView=" + sourceView());

    var deleted = function named() {
        var result = eval("var named=50; delete named");
        return [result, eval("named===deleted"), named === deleted].join("|");
    };
    observations.push("delete=" + deleted());

    var assigned = function named() {
        var result = eval("named=51; named===assigned");
        return [result, named === assigned].join("|");
    };
    observations.push("assignment=" + assigned());

    var bareVar = function named() {
        eval("var named=52");
        var before = eval("named");
        eval("var named");
        return [
            before,
            eval("typeof named"),
            typeof named,
            named === bareVar
        ].join("|");
    };
    observations.push("bareVar=" + bareVar());

    var plainChildWrite = function named() {
        return function () {
            named = 8;
            return [typeof named, named === plainChildWrite].join("|");
        };
    };
    observations.push("plainChildWrite=" + plainChildWrite()());

    // QuickJS's ordinary direct-eval closure prepass intentionally loses the
    // FunctionName/is_const flags copied from an ancestor unscoped binding.
    // Freeze that implementation quirk in both sloppy and strict children.
    var childWrite = function named() {
        return function () {
            eval("0");
            named = 9;
            return [named, named === childWrite].join("|");
        };
    };
    observations.push("childWrite=" + childWrite()());

    var strictChildWrite = function named() {
        "use strict";
        return function () {
            eval("0");
            named = 10;
            return [named, named === strictChildWrite].join("|");
        };
    };
    observations.push("strictChildWrite=" + strictChildWrite()());

    var strictEvalWrite = function named() {
        "use strict";
        return function () {
            return eval(
                "named=11; " +
                "[named,named===strictEvalWrite].join(\"|\")"
            );
        };
    };
    observations.push("strictEvalWrite=" + strictEvalWrite()());

    // A child created inside eval uses Eval-root closure copying, which keeps
    // the FunctionName metadata and therefore remains immutable.
    var evalChild = function named() {
        return eval(
            "(function(){named=12;return " +
            "[typeof named,named===evalChild].join(\"|\")})"
        );
    };
    observations.push("evalChild=" + evalChild()());

    // The metadata loss belongs only to the ordinary function that contains
    // direct eval. A plain descendant resolves the original FunctionName and
    // may use a second, immutable relay through that same parent frame.
    var directThenPlain = function named() {
        return function () {
            eval("0");
            return function () {
                named = 13;
                return [
                    typeof named,
                    named === directThenPlain
                ].join("|");
            };
        };
    };
    observations.push("directThenPlain=" + directThenPlain()()());

    // A descendant that itself contains direct eval gets its own erased view.
    var directThenDirect = function named() {
        return function () {
            eval("0");
            return function () {
                eval("0");
                named = 14;
                return [named, named === directThenDirect].join("|");
            };
        };
    };
    observations.push("directThenDirect=" + directThenDirect()()());

    var doubleDirectThenPlain = function named() {
        return function () {
            eval("0");
            return function () {
                eval("0");
                return function () {
                    named = 15;
                    return [
                        typeof named,
                        named === doubleDirectThenPlain
                    ].join("|");
                };
            };
        };
    };
    observations.push(
        "doubleDirectThenPlain=" + doubleDirectThenPlain()()()()
    );

    // Synthetic Eval roots copy imported closure flags exactly. They neither
    // erase an imported FunctionName nor restore an already-erased Normal view.
    var evalRootFunctionName = function named() {
        return eval(
            "(function(){eval('0');named=16;return " +
            "[typeof named,named===evalRootFunctionName].join('|')})"
        );
    };
    observations.push("evalRootFunctionName=" + evalRootFunctionName()());

    var evalRootErasedNormal = function named() {
        return function () {
            eval("0");
            return eval(
                "(function(){eval('0');named=17;return " +
                "[named,named===evalRootErasedNormal].join('|')})"
            );
        };
    };
    observations.push("evalRootErasedNormal=" + evalRootErasedNormal()()());

    // QuickJS creates a child's direct-eval closure table before resolving
    // identifiers in its plain parent. That descendant can therefore claim
    // the parent's physical relay first and make the parent's later view Normal.
    var descendantSeedsParent = function named() {
        return function () {
            var seed = function () { eval("0"); };
            named = 18;
            return [
                typeof named,
                named === descendantSeedsParent
            ].join("|");
        };
    };
    observations.push("descendantSeedsParent=" + descendantSeedsParent()());

    var evalFirstThenPlain = function named() {
        return function () {
            var seed = function () { eval("0"); };
            var leaf = function () {
                named = 19;
                return named === evalFirstThenPlain;
            };
            var leafResult = leaf();
            named = 20;
            return [
                leafResult,
                named,
                named === evalFirstThenPlain
            ].join("|");
        };
    };
    observations.push("evalFirstThenPlain=" + evalFirstThenPlain()());

    // Reversing the two children lets the plain leaf claim the parent relay
    // with FunctionName metadata before the later eval prepass reaches it.
    var earlierPlainWins = function named() {
        return function () {
            var first = function () {
                named = 21;
                return named === earlierPlainWins;
            };
            var seed = function () { eval("0"); };
            var firstResult = first();
            named = 22;
            return [
                firstResult,
                typeof named,
                named === earlierPlainWins
            ].join("|");
        };
    };
    observations.push("earlierPlainWins=" + earlierPlainWins()());

    var strictDirectThenPlain = function named() {
        "use strict";
        return function () {
            eval("0");
            return function () {
                try {
                    named = 23;
                    return "none";
                } catch (error) {
                    return [
                        error.name,
                        typeof named,
                        named === strictDirectThenPlain
                    ].join("|");
                }
            };
        };
    };
    observations.push(
        "strictDirectThenPlain=" + strictDirectThenPlain()()()
    );

    return observations.join("\n");
})()
// case: declaration conflicts and global flags
var observations = [];
{
    let blockConflict = 1;
    try {
        eval(
            "function functionBeforeThrow(){}; " +
            "var createdBeforeThrow; var blockConflict"
        );
        observations.push("block=none");
    } catch (error) {
        observations.push(
            "block=" + [
                error.name,
                Object.prototype.hasOwnProperty.call(
                    globalThis,
                    "createdBeforeThrow"
                ),
                Object.prototype.hasOwnProperty.call(globalThis, "blockConflict"),
                Object.prototype.hasOwnProperty.call(
                    globalThis,
                    "functionBeforeThrow"
                ),
                String(globalThis.createdBeforeThrow),
                String(globalThis.blockConflict),
                typeof globalThis.functionBeforeThrow
            ].join("|")
        );
    }
}
delete globalThis.createdBeforeThrow;
delete globalThis.blockConflict;
delete globalThis.functionBeforeThrow;

let globalLocked = 1;
try {
    eval("var preflightMustNotExist; var globalLocked");
    observations.push("global=none");
} catch (error) {
    observations.push(
        "global=" + [
            error.name,
            Object.prototype.hasOwnProperty.call(
                globalThis,
                "preflightMustNotExist"
            ),
            Object.prototype.hasOwnProperty.call(globalThis, "globalLocked"),
            globalLocked
        ].join("|")
    );
}

var scriptVar = 1;
function scriptFn() {}
eval("var evalVar=1; function evalFn(){}");
observations.push(
    "flags=" + [
        Object.getOwnPropertyDescriptor(globalThis, "scriptVar").configurable,
        Object.getOwnPropertyDescriptor(globalThis, "scriptFn").configurable,
        Object.getOwnPropertyDescriptor(globalThis, "evalVar").configurable,
        Object.getOwnPropertyDescriptor(globalThis, "evalFn").configurable,
        delete globalThis.evalVar,
        delete globalThis.evalFn
    ].join("|")
);
observations.join("\n")
// case: labelled functions
(function () {
    var observations = [];
    function direct() {
        var lexicalClosure;
        eval(
            "label: function local(){return 42;} " +
            "lexicalClosure=local"
        );
        return [
            lexicalClosure === local,
            lexicalClosure(),
            local()
        ].join("|");
    }
    observations.push("direct=" + direct());

    globalThis.labelledClosure = undefined;
    Object.defineProperty(globalThis, "labelled", {
        configurable: true,
        get: function () { return 9; },
        set: function (value) { labelledClosure = value; }
    });
    var inside = (0, eval)(
        "label: function labelled(){return 42;} " +
        "[typeof labelled,labelled(),labelledClosure===labelled].join('|')"
    );
    observations.push(
        "global=" + [
            inside,
            typeof labelled,
            labelled,
            labelledClosure(),
            Object.getOwnPropertyDescriptor(globalThis, "labelled").configurable
        ].join("|")
    );
    delete globalThis.labelled;
    delete globalThis.labelledClosure;
    return observations.join("\n");
})()
// case: nested direct eval environment relay
(function () {
    var observations = [];
    observations.push("simple=" + eval("eval('40+2')"));
    observations.push(
        "triple=" + eval("let x=40;eval('eval(\"x+=2\")');x")
    );

    function dynamic() {
        eval("eval('var x=42')");
        return x;
    }
    observations.push("dynamic=" + dynamic());

    function offsetDynamic() {
        var authored = 1;
        return eval("let pad=0;pad;eval('var x=42;x')");
    }
    observations.push("offset=" + offsetDynamic());

    function strictOuter() {
        return eval("'use strict';var x=40;eval('x+=2');x");
    }
    function strictNonLeak() {
        return eval("'use strict';eval('var x=42');typeof x");
    }
    observations.push(
        "strict=" + strictOuter() + "|" + strictNonLeak()
    );

    function catchDirect() {
        try { throw 1; }
        catch (x) {
            var result = eval("eval('var x=2;x')");
            return [result, x].join("|");
        }
    }
    function catchChild() {
        try { throw 1; }
        catch (x) {
            var result = eval(
                "(function(){return eval('var x=2;x')})()"
            );
            return [result, x].join("|");
        }
    }
    observations.push(
        "catch=" + catchDirect() + "|" + catchChild()
    );

    function lexicalChild() {
        let x = 1;
        var result = eval(
            "(function(){return eval('var x=2;x')})()"
        );
        return [result, x].join("|");
    }
    function lexicalConflict() {
        let x = 1;
        try {
            eval("eval('var x=2;x')");
            return "none";
        } catch (error) {
            return error.name + "|" + x;
        }
    }
    observations.push(
        "lexical=" + lexicalChild() + "|" + lexicalConflict()
    );

    function relay() {
        let x = 42;
        return eval("(function(){return eval('x')})()");
    }
    function escapedCaller() {
        let x = 42;
        var closure = eval(
            "(function(){return function(){return eval('x')}})()"
        );
        return closure();
    }
    function escapedEval() {
        var closure = eval(
            "let x=42;(function(){return function(){return eval('x')}})()"
        );
        return closure();
    }
    function paddedChild() {
        return eval(
            "let pad=0;(function(){pad;return eval('var x=42;x')})()"
        );
    }
    observations.push(
        "relay=" + relay() + "|" + escapedCaller() + "|" + escapedEval()
            + "|" + paddedChild()
    );
    return observations.join("\n");
})()

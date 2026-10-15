// mode: print
function show(value) {
    if (value === undefined) return "undefined:undefined";
    if (value === null) return "object:null";
    if (typeof value === "symbol") return "symbol:<symbol>";
    if (typeof value === "object" || typeof value === "function")
        return typeof value + ":<object>";
    return typeof value + ":" + String(value);
}
function observe(thunk) {
    try {
        return show(thunk());
    } catch (error) {
        if (error === membershipSentinel) return "throw:sentinel";
        if (error !== null && typeof error === "object")
            return "throw:" + error.name + "|" + error.message;
        return "throw:" + typeof error + "|" + String(error);
    }
}
function identity(value) { return value; }

var membershipLog = "";
var membershipSentinel = {};
var membershipProto = {};
Object.defineProperty(membershipProto, "inherited", {
    value: 1, writable: true, enumerable: true, configurable: true
});
Object.defineProperty(membershipProto, "accessor", {
    get: function () { membershipLog += "G"; return 1; },
    enumerable: true, configurable: true
});
var membershipTarget = Object.create(membershipProto);
for (var membershipName of ["own", "1", "2", "true", "string"])
    Object.defineProperty(membershipTarget, membershipName, {
        value: 1, writable: true, enumerable: true, configurable: true
    });
var membershipSymbol = Symbol("membership");
membershipTarget[membershipSymbol] = 1;
var membershipKey = {
    [Symbol.toPrimitive]: function (hint) {
        membershipLog += "k(" + hint + ")";
        return "own";
    }
};
var membershipThrowKey = {
    [Symbol.toPrimitive]: function (hint) {
        membershipLog += "k(" + hint + ")";
        throw membershipSentinel;
    }
};
function membershipLeft() { membershipLog += "l"; return membershipKey; }
function membershipThrowLeft() { membershipLog += "l"; return membershipThrowKey; }
function membershipRight() { membershipLog += "r"; return membershipTarget; }
function membershipPrimitiveRight() { membershipLog += "r"; return 1; }

function MembershipCtor() {}
var membershipCtor = MembershipCtor;
var membershipInstance = new membershipCtor();
var membershipOther = {};
var membershipNeedle = {};
var membershipBound = membershipCtor.bind(null);
var membershipBoundInstance = new membershipBound();

function MembershipCustomCtor() {}
var membershipCustomCtor = MembershipCustomCtor;
Object.defineProperty(membershipCustomCtor, Symbol.hasInstance, {
    value: function (candidate) {
        membershipLog += this === membershipCustomCtor ? "ct" : "cw";
        return candidate === membershipNeedle;
    },
    configurable: true
});
var membershipCustomBound = membershipCustomCtor.bind(null);

var membershipInheritedHasInstanceProto = {};
Object.defineProperty(membershipInheritedHasInstanceProto, Symbol.hasInstance, {
    value: function (candidate) {
        membershipLog += this === membershipInheritedHasInstanceTarget ? "it" : "iw";
        return candidate === membershipNeedle;
    },
    configurable: true
});
var membershipInheritedHasInstanceTarget =
    Object.create(membershipInheritedHasInstanceProto);

var membershipAccessorMethod = function (candidate) {
    membershipLog += this === membershipAccessorTarget ? "ct" : "cw";
    return candidate === membershipNeedle;
};
var membershipAccessorTarget = {};
Object.defineProperty(membershipAccessorTarget, Symbol.hasInstance, {
    get: function () { membershipLog += "g"; return membershipAccessorMethod; },
    configurable: true
});
function membershipInstanceLeft() { membershipLog += "l"; return membershipNeedle; }
function membershipInstanceRight() { membershipLog += "r"; return membershipAccessorTarget; }

function MembershipNullHasInstance() {}
var membershipNullHasInstance = MembershipNullHasInstance;
Object.defineProperty(membershipNullHasInstance, Symbol.hasInstance, {
    value: null, configurable: true
});
var membershipNullInstance = new membershipNullHasInstance();

var membershipPrototypeMatch = {};
var membershipPrototypeCandidate = Object.create(membershipPrototypeMatch);
Object.defineProperty(Function.prototype, "prototype", {
    get: function () { membershipLog += "p"; return membershipPrototypeMatch; },
    configurable: true
});

var membershipPrototypeThrowTarget = Function.prototype.call;
Object.defineProperty(membershipPrototypeThrowTarget, "prototype", {
    get: function () { membershipLog += "P"; throw membershipSentinel; },
    configurable: true
});

var membershipInvalidTarget = {};
var membershipBadMethodTarget = {};
Object.defineProperty(membershipBadMethodTarget, Symbol.hasInstance, {
    value: 1, configurable: true
});
var membershipGetterThrowTarget = {};
Object.defineProperty(membershipGetterThrowTarget, Symbol.hasInstance, {
    get: function () { membershipLog += "g"; throw membershipSentinel; },
    configurable: true
});
var membershipMethodThrowTarget = {};
Object.defineProperty(membershipMethodThrowTarget, Symbol.hasInstance, {
    value: function () { membershipLog += "c"; throw membershipSentinel; },
    configurable: true
});
function MembershipBadPrototype() {}
var membershipBadPrototype = MembershipBadPrototype;
membershipBadPrototype.prototype = 1;

print("precedence=" + [
    show(1 + 1 in membershipTarget),
    show(0 | 1 in membershipTarget),
    show("own" in membershipTarget === true),
    show(false == "missing" in membershipTarget),
    show("own" in membershipTarget ? 7 : 9),
    show(membershipInstance instanceof membershipCtor === true),
    show(membershipInstance instanceof membershipCtor in membershipTarget),
    show(false && "x" in null),
    show(typeof "own" in membershipTarget)
].join("|"));

print("no-in-valid=" + [
    show((function () {
        for (var value = ("own" in membershipTarget); false; );
        return value;
    })()),
    show((function () {
        for (var value = true ? "own" in membershipTarget : false; false; );
        return value;
    })()),
    show((function () {
        for (var value = identity("own" in membershipTarget); false; );
        return value;
    })()),
    show((function () {
        for (var value = false ? false : ("own" in membershipTarget); false; );
        return value;
    })()),
    show((function () {
        for (var value = false; false; );
        return "own" in membershipTarget;
    })()),
    show((function () {
        for (var value = membershipInstance instanceof membershipCtor; false; );
        return value;
    })())
].join("|"));

print("in-basic=" + [
    show("own" in membershipTarget),
    show("inherited" in membershipTarget),
    show("missing" in membershipTarget),
    show(1 in membershipTarget),
    show(membershipSymbol in membershipTarget),
    show(Symbol.hasInstance in Function.prototype)
].join("|"));

membershipLog = "";
var membershipObservation = observe(function () {
    return "accessor" in membershipTarget;
});
print("in-accessor=" + membershipObservation + "|log:" + membershipLog);

membershipLog = "";
membershipObservation = observe(function () {
    return membershipLeft() in membershipRight();
});
print("in-order=" + membershipObservation + "|log:" + membershipLog);

membershipLog = "";
membershipObservation = observe(function () {
    return membershipLeft() in membershipPrimitiveRight();
});
print("in-invalid-rhs-order=" + membershipObservation + "|log:" + membershipLog);

membershipLog = "";
membershipObservation = observe(function () {
    return membershipThrowLeft() in membershipRight();
});
print("in-key-throw=" + membershipObservation + "|log:" + membershipLog);

print("in-invalid=" + [
    observe(function () { return "x" in null; }),
    observe(function () { return "x" in undefined; }),
    observe(function () { return "x" in 1; }),
    observe(function () { return "x" in Symbol("rhs"); })
].join("|"));

print("instance-basic=" + [
    show(membershipInstance instanceof membershipCtor),
    show(membershipOther instanceof membershipCtor),
    show(1 instanceof membershipCtor),
    show(membershipBoundInstance instanceof membershipCtor),
    show(membershipBoundInstance instanceof membershipBound)
].join("|"));

membershipLog = "";
print("instance-custom=" +
      show(membershipNeedle instanceof membershipCustomCtor) + "|" +
      show(membershipOther instanceof membershipCustomCtor) +
      "|log:" + membershipLog);

membershipLog = "";
print("instance-inherited-custom=" +
      show(membershipNeedle instanceof membershipInheritedHasInstanceTarget) +
      "|log:" + membershipLog);

membershipLog = "";
print("instance-accessor-custom=" +
      show(membershipNeedle instanceof membershipAccessorTarget) +
      "|log:" + membershipLog);

membershipLog = "";
print("instance-order=" +
      show(membershipInstanceLeft() instanceof membershipInstanceRight()) +
      "|log:" + membershipLog);

print("instance-null-fallback=" +
      show(membershipNullInstance instanceof membershipNullHasInstance));

membershipLog = "";
print("instance-bound-custom=" +
      show(membershipNeedle instanceof membershipCustomBound) +
      "|log:" + membershipLog);

membershipLog = "";
print("instance-prototype-accessor=" +
      show(membershipPrototypeCandidate instanceof Function.prototype) +
      "|log:" + membershipLog);

membershipLog = "";
print("instance-primitive-short=" +
      show(1 instanceof membershipPrototypeThrowTarget) +
      "|log:" + membershipLog);

print("instance-invalid=" + [
    observe(function () { return membershipNeedle instanceof 1; }),
    observe(function () { return membershipNeedle instanceof membershipInvalidTarget; }),
    observe(function () { return membershipNeedle instanceof membershipBadMethodTarget; }),
    observe(function () { return membershipNeedle instanceof membershipBadPrototype; })
].join("|"));

membershipLog = "";
var getterThrow = observe(function () {
    return membershipNeedle instanceof membershipGetterThrowTarget;
});
print("instance-getter-throw=" + getterThrow + "|log:" + membershipLog);

membershipLog = "";
var methodThrow = observe(function () {
    return membershipNeedle instanceof membershipMethodThrowTarget;
});
print("instance-method-throw=" + methodThrow + "|log:" + membershipLog);

membershipLog = "";
var prototypeThrow = observe(function () {
    return membershipNeedle instanceof membershipPrototypeThrowTarget;
});
print("instance-prototype-throw=" + prototypeThrow + "|log:" + membershipLog);

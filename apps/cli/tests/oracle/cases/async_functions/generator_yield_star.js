// mode: print
// case: async iterator selection cached next and raw yielded promise identity

var events = [];
var inner = Promise.resolve("inner");
var asyncIterator = {
    label: "async-iterator",
    get next() {
        events.push("get-next:" + this.label);
        var count = 0;
        return function (value) {
            events.push("call-next:" + this.label + ":" + String(value));
            count++;
            if (count === 1) {
                return {
                    get then() {
                        events.push("get-result-then");
                        return function (resolve) {
                            events.push("call-result-then");
                            resolve({
                                get done() {
                                    events.push("get-done-1");
                                    return false;
                                },
                                get value() {
                                    events.push("get-value-1");
                                    return inner;
                                }
                            });
                        };
                    }
                };
            }
            return Promise.resolve({
                get done() {
                    events.push("get-done-2");
                    return true;
                },
                get value() {
                    events.push("get-value-2");
                    return "delegate-complete";
                }
            });
        };
    }
};
var iterable = {
    get [Symbol.asyncIterator]() {
        events.push("get-async");
        return function () {
            events.push("call-async:" + (this === iterable));
            return asyncIterator;
        };
    },
    get [Symbol.iterator]() {
        events.push("get-sync-miss");
        throw new Error("sync iterator must not be read");
    }
};
async function* delegate() {
    events.push("before");
    var value = yield* iterable;
    events.push("after:" + value);
    return "outer:" + value;
}
var iterator = delegate();
iterator.next("ignored").then(function (result) {
    events.push("first:" + (result.value === inner) + ":" + result.done);
    return iterator.next("sent");
}).then(function (result) {
    events.push("second:" + result.value + ":" + result.done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: sync fallback uses Async-from-Sync and assimilates values

var events = [];
function thenable(label, value) {
    return {
        get then() {
            events.push("get-then:" + label);
            return function (resolve) {
                events.push("call-then:" + label);
                resolve(value);
            };
        }
    };
}
var syncIterator = {
    label: "sync-iterator",
    get next() {
        events.push("get-next:" + this.label);
        var count = 0;
        return function (value) {
            events.push("call-next:" + this.label + ":" + String(value));
            count++;
            if (count === 1) {
                return {
                    get done() {
                        events.push("get-done-1");
                        return false;
                    },
                    get value() {
                        events.push("get-value-1");
                        return thenable("yield", 42);
                    }
                };
            }
            return {
                get done() {
                    events.push("get-done-2");
                    return true;
                },
                get value() {
                    events.push("get-value-2");
                    return thenable("complete", 7);
                }
            };
        };
    }
};
var iterable = {
    get [Symbol.asyncIterator]() {
        events.push("get-async-null");
        return null;
    },
    get [Symbol.iterator]() {
        events.push("get-sync");
        return function () {
            events.push("call-sync:" + (this === iterable));
            return syncIterator;
        };
    }
};
async function* delegate() {
    var value = yield* iterable;
    events.push("after:" + value);
    return "outer:" + value;
}
var iterator = delegate();
iterator.next("ignored").then(function (result) {
    events.push("first:" + result.value + ":" + result.done);
    return iterator.next("sent");
}).then(function (result) {
    events.push("second:" + result.value + ":" + result.done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: delegated return awaits input and result and preserves FIFO requests

var events = [];
var rawYield = Promise.resolve("raw-return-yield");
var input = {
    get then() {
        events.push("get-input-then");
        return function (resolve) {
            events.push("call-input-then");
            resolve("awaited-input");
        };
    }
};
var complete = {
    get then() {
        events.push("get-complete-then");
        return function (resolve) {
            events.push("call-complete-then");
            resolve("return-complete");
        };
    }
};
var delegateIterator = {
    label: "delegate",
    next: function () {
        events.push("call-next");
        return { value: "seed", done: false };
    },
    get return() {
        events.push("get-return");
        var methodCall = ++this.returnGets;
        return function (value) {
            events.push(
                "call-return-" + methodCall + ":" + value + ":" +
                (this === delegateIterator)
            );
            return {
                get then() {
                    events.push("get-result-then-" + methodCall);
                    return function (resolve) {
                        events.push("call-result-then-" + methodCall);
                        resolve(methodCall === 1
                            ? {
                                get done() {
                                    events.push("get-return-done-1");
                                    return false;
                                },
                                get value() {
                                    events.push("get-return-value-1");
                                    return rawYield;
                                }
                            }
                            : {
                                get done() {
                                    events.push("get-return-done-2");
                                    return true;
                                },
                                get value() {
                                    events.push("get-return-value-2");
                                    return complete;
                                }
                            });
                    };
                }
            };
        };
    },
    returnGets: 0
};
var iterable = {
    [Symbol.asyncIterator]: function () {
        return delegateIterator;
    }
};
async function* generator() {
    yield* iterable;
    events.push("after-miss");
}
var iterator = generator();
iterator.next().then(function (first) {
    events.push("first:" + first.value + ":" + first.done);
    var one = iterator.return(input);
    var two = iterator.return("second-input");
    events.push("queued");
    return Promise.all([one, two]);
}).then(function (results) {
    events.push(
        "return-1:" + (results[0].value === rawYield) + ":" + results[0].done
    );
    events.push("return-2:" + results[1].value + ":" + results[1].done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: delegated throw is not input-awaited and preserves FIFO requests

var events = [];
var rawYield = Promise.resolve("raw-throw-yield");
var thrown = {
    get then() {
        events.push("unexpected-get-thrown-then");
    }
};
var delegateIterator = {
    next: function () {
        events.push("call-next");
        return { value: "seed", done: false };
    },
    get throw() {
        var methodCall = ++this.throwGets;
        events.push("get-throw-" + methodCall);
        return function (value) {
            events.push(
                "call-throw-" + methodCall + ":" + (value === thrown) + ":" +
                (this === delegateIterator)
            );
            return {
                get then() {
                    events.push("get-result-then-" + methodCall);
                    return function (resolve) {
                        events.push("call-result-then-" + methodCall);
                        resolve(methodCall === 1
                            ? {
                                get done() {
                                    events.push("get-throw-done-1");
                                    return false;
                                },
                                get value() {
                                    events.push("get-throw-value-1");
                                    return rawYield;
                                }
                            }
                            : {
                                get done() {
                                    events.push("get-throw-done-2");
                                    return true;
                                },
                                get value() {
                                    events.push("get-throw-value-2");
                                    return "throw-complete";
                                }
                            });
                    };
                }
            };
        };
    },
    throwGets: 0
};
var iterable = {
    [Symbol.asyncIterator]: function () {
        return delegateIterator;
    }
};
async function* generator() {
    var value = yield* iterable;
    events.push("after:" + value);
    return "outer:" + value;
}
var iterator = generator();
iterator.next().then(function (first) {
    events.push("first:" + first.value + ":" + first.done);
    var one = iterator.throw(thrown);
    var two = iterator.throw("second");
    events.push("queued");
    return Promise.all([one, two]);
}).then(function (results) {
    events.push(
        "throw-1:" + (results[0].value === rawYield) + ":" + results[0].done
    );
    events.push("throw-2:" + results[1].value + ":" + results[1].done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: Async-from-Sync forwards present throw and return methods

var events = [];
function thenable(label, value) {
    return {
        get then() {
            events.push("get-then:" + label);
            return function (resolve) {
                events.push("call-then:" + label);
                resolve(value);
            };
        }
    };
}
var syncIterator = {
    next: function () {
        events.push("next");
        return { value: thenable("next-value", "seed"), done: false };
    },
    get throw() {
        events.push("get-throw");
        return function (value) {
            events.push("call-throw:" + value + ":" + (this === syncIterator));
            return {
                get done() {
                    events.push("get-throw-done");
                    return false;
                },
                get value() {
                    events.push("get-throw-value");
                    return thenable("throw-value", "throw-yield");
                }
            };
        };
    },
    get return() {
        events.push("get-return");
        return function (value) {
            events.push("call-return:" + value + ":" + (this === syncIterator));
            return {
                get done() {
                    events.push("get-return-done");
                    return true;
                },
                get value() {
                    events.push("get-return-value");
                    return thenable("return-value", "return-complete");
                }
            };
        };
    }
};
var iterable = {
    [Symbol.iterator]: function () {
        return syncIterator;
    }
};
async function* generator() {
    yield* iterable;
    events.push("after-miss");
}
var iterator = generator();
iterator.next().then(function (first) {
    events.push("first:" + first.value + ":" + first.done);
    return iterator.throw("thrown");
}).then(function (thrownResult) {
    events.push("thrown:" + thrownResult.value + ":" + thrownResult.done);
    return iterator.return(thenable("return-input", "awaited-return"));
}).then(function (returnResult) {
    events.push("returned:" + returnResult.value + ":" + returnResult.done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: missing async return awaits the same non-callable then twice

var events = [];
var returned = {
    get then() {
        events.push("get-returned-then");
        return undefined;
    }
};
var delegateIterator = {
    next: function () {
        events.push("next");
        return { value: "seed", done: false };
    }
};
var iterable = {
    [Symbol.asyncIterator]: function () {
        return delegateIterator;
    }
};
async function* generator() {
    yield* iterable;
    events.push("after-miss");
}
var iterator = generator();
iterator.next().then(function (first) {
    events.push("first:" + first.value + ":" + first.done);
    return iterator.return(returned);
}).then(function (result) {
    events.push("returned:" + (result.value === returned) + ":" + result.done);
    print(events.join("|"));
}, function (error) {
    print("reject:" + error.name + ":" + error.message + "|" + events.join("|"));
});
// case: missing async throw closes without an argument and keeps close errors

var events = [];
function make(label, failClose) {
    var delegateIterator = {
        label: label,
        next: function () {
            events.push("next-" + label);
            return { value: label, done: false };
        },
        get return() {
            events.push("get-return-" + label);
            return function () {
                events.push(
                    "call-return-" + label + ":" + arguments.length + ":" +
                    (this === delegateIterator)
                );
                return {
                    get then() {
                        events.push("get-close-then-" + label);
                        return function (resolve, reject) {
                            events.push("call-close-then-" + label);
                            if (failClose) {
                                reject("close-" + label);
                            } else {
                                resolve(1);
                            }
                        };
                    }
                };
            };
        }
    };
    return (async function* () {
        yield* {
            [Symbol.asyncIterator]: function () {
                return delegateIterator;
            }
        };
    })();
}
function outcome(promise) {
    return promise.then(
        function (result) {
            return "ok:" + result.value + ":" + result.done;
        },
        function (error) {
            return "reject:" +
                (error && error.name ? error.name : String(error));
        }
    );
}
var normal = make("normal", false);
var failing = make("failing", true);
Promise.all([normal.next(), failing.next()]).then(function () {
    return Promise.all([
        outcome(normal.throw("original-normal")),
        outcome(failing.throw("original-failing"))
    ]);
}).then(function (results) {
    print(results.join("|") + "|" + events.join("|"));
});
// case: missing sync throw uses IteratorClose and close-error precedence

var events = [];
function wrap(label, failClose) {
    var syncIterator = {
        next: function () {
            events.push("next-" + label);
            return { value: label, done: false };
        },
        get return() {
            events.push("get-return-" + label);
            if (failClose) {
                throw "close-" + label;
            }
            return function () {
                events.push(
                    "call-return-" + label + ":" + arguments.length + ":" +
                    (this === syncIterator)
                );
                return {};
            };
        }
    };
    return (async function* () {
        yield* {
            [Symbol.iterator]: function () {
                return syncIterator;
            }
        };
    })();
}
function outcome(promise) {
    return promise.then(
        function (result) {
            return "ok:" + result.value + ":" + result.done;
        },
        function (error) {
            return "reject:" +
                (error && error.name ? error.name : String(error));
        }
    );
}
var normal = wrap("normal", false);
var failing = wrap("failing", true);
Promise.all([normal.next(), failing.next()]).then(function () {
    return Promise.all([
        outcome(normal.throw("original-normal")),
        outcome(failing.throw("original-failing"))
    ]);
}).then(function (results) {
    print(results.join("|") + "|" + events.join("|"));
});
// case: Async-from-Sync rejection closes next and throw but not return

var events = [];
function outcome(promise) {
    return promise.then(
        function (result) {
            return "ok:" + result.value + ":" + result.done;
        },
        function (error) {
            return "reject:" + String(error);
        }
    );
}
function wrap(syncIterator) {
    return (async function* () {
        yield* {
            [Symbol.iterator]: function () {
                return syncIterator;
            }
        };
    })();
}
var nextSync = {
    next: function () {
        events.push("next-reject-next");
        return { done: false, value: Promise.reject("next-reason") };
    },
    get return() {
        events.push("next-reject-get-close");
        throw "next-close-error";
    }
};
var throwSync = {
    next: function () {
        events.push("throw-reject-next");
        return { done: false, value: "seed" };
    },
    throw: function () {
        events.push("throw-reject-throw");
        return { done: false, value: Promise.reject("throw-reason") };
    },
    get return() {
        events.push("throw-reject-get-close");
        throw "throw-close-error";
    }
};
var returnSync = {
    next: function () {
        events.push("return-reject-next");
        return { done: false, value: "seed" };
    },
    get return() {
        events.push("return-reject-get-return");
        return function () {
            events.push("return-reject-call-return");
            return { done: false, value: Promise.reject("return-reason") };
        };
    }
};
var nextIterator = wrap(nextSync);
var throwIterator = wrap(throwSync);
var returnIterator = wrap(returnSync);
Promise.all([
    outcome(nextIterator.next()),
    throwIterator.next(),
    returnIterator.next()
]).then(function (started) {
    return Promise.all([
        Promise.resolve(started[0]),
        outcome(throwIterator.throw("sent")),
        outcome(returnIterator.return("sent"))
    ]);
}).then(function (results) {
    print(results.join("|") + "|" + events.join("|"));
});
// case: iterator acquisition and awaited result abrupt completions keep order

var events = [];
function generator(iterable) {
    return (async function* () {
        yield* iterable;
    })();
}
function outcome(label, promise) {
    return promise.then(
        function (result) {
            return label + ":ok:" + result.value + ":" + result.done;
        },
        function (error) {
            return label + ":reject:" +
                (error && error.name ? error.name : String(error));
        }
    );
}
var asyncGet = {};
Object.defineProperty(asyncGet, Symbol.asyncIterator, {
    get: function () {
        events.push("async-get");
        throw "async-get-error";
    }
});
var asyncPrimitive = {
    [Symbol.asyncIterator]: function () {
        events.push("async-call-primitive");
        return 1;
    }
};
var nextGet = {
    [Symbol.asyncIterator]: function () {
        return {
            get next() {
                events.push("next-get");
                throw "next-get-error";
            }
        };
    }
};
var thenGet = {
    [Symbol.asyncIterator]: function () {
        return {
            next: function () {
                events.push("then-next-call");
                return {
                    get then() {
                        events.push("then-get");
                        throw "then-get-error";
                    }
                };
            }
        };
    }
};
var resultPrimitive = {
    [Symbol.asyncIterator]: function () {
        return {
            next: function () {
                events.push("result-primitive-call");
                return 1;
            }
        };
    }
};
var doneGet = {
    [Symbol.asyncIterator]: function () {
        return {
            next: function () {
                events.push("done-next-call");
                return {
                    get done() {
                        events.push("done-get");
                        throw "done-get-error";
                    },
                    get value() {
                        events.push("unexpected-value-get");
                        return 1;
                    }
                };
            }
        };
    }
};
var syncGet = {};
Object.defineProperty(syncGet, Symbol.asyncIterator, {
    get: function () {
        events.push("async-null");
        return undefined;
    }
});
Object.defineProperty(syncGet, Symbol.iterator, {
    get: function () {
        events.push("sync-get");
        throw "sync-get-error";
    }
});
Promise.all([
    outcome("async-get", generator(asyncGet).next()),
    outcome("async-primitive", generator(asyncPrimitive).next()),
    outcome("next-get", generator(nextGet).next()),
    outcome("then-get", generator(thenGet).next()),
    outcome("result-primitive", generator(resultPrimitive).next()),
    outcome("done-get", generator(doneGet).next()),
    outcome("sync-get", generator(syncGet).next())
]).then(function (results) {
    print(results.join("|") + "|events=" + events.join(","));
});

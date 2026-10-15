// mode: print
function emit(label, value, cleanup) {
    print(label + "=" + value);
    for (var name of cleanup) delete globalThis[name];
}

emit("array-normal", (function () {
    return eval(`var [y]=[2];Object.prototype.hasOwnProperty.call(globalThis,"y")+"|"+typeof globalThis.y+"|"+String(globalThis.y)+"|"+typeof y+"|"+String(y)`);
})(), ["y"]);

emit("object-normal", (function () {
    return eval(`var {a:y}={get a(){return 2}};Object.prototype.hasOwnProperty.call(globalThis,"y")+"|"+typeof globalThis.y+"|"+String(globalThis.y)+"|"+typeof y+"|"+String(y)`);
})(), ["y"]);

emit("object-names", (function () {
    return eval(`var {fn=function(){},gen=function*(){},af=async function(){},arrow=()=>{},aa=async()=>{},cls=class{}}={};[fn,gen,af,arrow,aa,cls].map(function(value){return value.name}).join(",")`);
})(), ["fn", "gen", "af", "arrow", "aa", "cls"]);

emit("array-names", (function () {
    return eval(`var [fn=function(){},gen=function*(){},af=async function(){},arrow=()=>{},aa=async()=>{},cls=class{}]=[];[fn,gen,af,arrow,aa,cls].map(function(value){return value.name}).join(",")`);
})(), ["fn", "gen", "af", "arrow", "aa", "cls"]);

emit("existing", (function () {
    var x = 1;
    eval(`var {x=23,fn=function(){}}={}`);
    return x + "|" + fn.name;
})(), ["fn"]);

emit("duplicate-eval", (function () {
    eval(`var {d=function(){}}={}`);
    var first = d.name;
    eval(`var [d=function(){}]=[]`);
    return first + ">" + d.name;
})(), ["d"]);

emit("object-order", (function () {
    var log = [];
    var src = {
        get a() { log.push("get:a"); return undefined; },
        get b() { log.push("get:b"); return 42; }
    };
    function key(value) { log.push("key:" + value); return value; }
    function init(value) { log.push("init:" + value); return value === "a" ? 31 : 32; }
    var result = eval(`var {[key("a")]:oa=init("a"),[key("b")]:ob=init("b")}=src;String(oa)+","+String(ob)`);
    return log.join(">") + "|" + result;
})(), ["oa", "ob"]);

emit("array-order", (function () {
    var log = [];
    var step = 0;
    var iterable = {
        [Symbol.iterator]: function () {
            log.push("iter");
            return {
                next: function () {
                    var current = step++;
                    log.push("next:" + current);
                    return {
                        get done() { log.push("done:" + current); return false; },
                        get value() { log.push("value:" + current); return current === 0 ? undefined : 52; }
                    };
                },
                return: function () { log.push("return"); return {done:true}; }
            };
        }
    };
    function initArray() { log.push("init"); return 51; }
    var result = eval(`var [ia=initArray(),ib]=iterable;String(ia)+","+String(ib)`);
    return log.join(">") + "|" + result;
})(), ["ia", "ib"]);

emit("array-late-next", (function () {
    return eval(`var i=0;var [y]={ [Symbol.iterator](){return {next(){if(i++===0){delete y;return {value:2,done:false}}return {done:true}}}}};var own=Object.prototype.hasOwnProperty.call(globalThis,"y");var g=globalThis.y;delete globalThis.y;var after;try{after=String(y)}catch(e){after=e.name}own+"|"+g+"|"+after`);
})(), ["y"]);

emit("object-late-getter", (function () {
    return eval(`var {a:y}={get a(){delete y;return 2}};var own=Object.prototype.hasOwnProperty.call(globalThis,"y");var g=globalThis.y;delete globalThis.y;var after;try{after=String(y)}catch(e){after=e.name}own+"|"+g+"|"+after`);
})(), ["y"]);

emit("array-late-default", (function () {
    return eval(`var [y=(delete y,2)]=[undefined];var own=Object.prototype.hasOwnProperty.call(globalThis,"y");var g=globalThis.y;delete globalThis.y;var after;try{after=String(y)}catch(e){after=e.name}own+"|"+g+"|"+after`);
})(), ["y"]);

emit("object-rest-late-getter", (function () {
    return eval(`var {...y}={get a(){delete y;return 2}};var own=Object.prototype.hasOwnProperty.call(globalThis,"y");var g=globalThis.y;delete globalThis.y;var after;try{after=String(y)}catch(e){after=e.name}own+"|"+g.a+"|"+after`);
})(), ["y"]);

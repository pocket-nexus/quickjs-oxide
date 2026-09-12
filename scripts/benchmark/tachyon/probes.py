"""Focused semantic probes; these are not a conformance suite or timing cases."""
PROBES = {
    'negative-zero-mul': ("function f(a,b){return a*b;} print(Object.is(f(0,-1),-0));", 'true\n'),
    'negative-zero-div': ("function f(a,b){return a/b;} print(Object.is(f(0,-1),-0));", 'true\n'),
    'negative-zero-negate': ("function f(a){return -a;} print(Object.is(f(0),-0));", 'true\n'),
    'int32-overflow': ("function f(a,b){return a+b;} print(f(2147483647,1));", '2147483648\n'),
    'division-overflow': ("function f(a,b){return a/b;} print(f(-2147483648,-1));", '2147483648\n'),
    'unsigned-shift': ("function f(a){return a>>>0;} print(f(-1));", '4294967295\n'),
    'lone-surrogate': ("print('\\ud800'.length,'\\ud800'.charCodeAt(0));", '1 55296\n'),
    'array-frozen-length': ("var a=[1,2,3];Object.defineProperty(a,'length',{writable:false});a[3]=4;print(a.length,a[3]);", '3 undefined\n'),
    'array-frozen-length-atomic': ("var a=[1,2,3],caught=false;Object.defineProperty(a,'length',{writable:false});try{a[3]=4;}catch(e){caught=true;}print(caught,a.length,a[3]);", 'false 3 undefined\n'),
    'coercion-order': ("var events=[];var x={valueOf:function(){events.push('x');return 2;}};print(x+1,events.join(','));", '3 x\n'),
    'regexp-replace': ("print('ab12cd34'.replace(/\\d+/g,'x'));", 'abxcdx\n'),
    'strict-directive': ('"use strict"; try { __qxo_missing_global=1; print("bad"); } catch(e) { print(e.name); }', 'ReferenceError\n'),
    'sloppy-directive': ('__qxo_missing_global=1; print(__qxo_missing_global);', '1\n'),
    'strict-equality': ('print(1===1.0,NaN===NaN,0===-0);','true false true\n'),
    'prototype-array-setter': ("var n=0;Object.defineProperty(Array.prototype,'0',{set:function(v){n=v;},configurable:true});var a=[];a[0]=7;print(n,a.length);",'7 0\n'),
}

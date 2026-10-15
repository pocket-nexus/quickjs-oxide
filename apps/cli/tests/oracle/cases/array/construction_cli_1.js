// mode: print
// flags:
// flags: --strip-source
// flags: -s
// case: Array constructor invalid length keeps the call site
(function outer(){return (function inner(){return Array(-1)})()})()
// case: array literal spread error keeps the spread site
(function outer(){return (function inner(){return [...1]})()})()
// case: Array Iterator brand error keeps native and authored frames
(function outer(){function X(){};var next=[].values().next;return (function inner(){return next.call(new X)})()})()

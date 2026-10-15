// mode: print
// flags:
// flags: --strip-source
// flags: -s
// case: flat lexical declaration close call fault keeps the preceding marker
(function outer(){var it={[Symbol.iterator]:function(){return this},next:function(){return{value:1,done:false}},return:function closeFault(){throw new Error('close')}};let [x]=it})()
// case: flat lexical declaration close getter fault keeps the preceding marker
(function outer(){var it={[Symbol.iterator]:function(){return this},next:function(){return{value:1,done:false}}};Object.defineProperty(it,'return',{get:function closeGetter(){throw new Error('close')}});const [x]=it})()
// case: nested iterator acquisition fault keeps the recursive pattern marker
(function outer(){var inner={[Symbol.iterator]:function acquire(){throw new Error('inner')}};let [[x]]=[inner]})()
// case: nested normal close fault keeps the recursive pattern marker
(function outer(){var inner={[Symbol.iterator]:function(){return this},next:function(){return{value:1,done:false}},return:function close(){throw new Error('close')}};let [[x]]=[inner]})()
// case: body fault closes the iterator without replacing its origin
(function outer(){function R(v,d){this.value=v;this.done=d};function I(){};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){return new R(0,true)};function X(){};X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)(function body(){null.forOfBodyFault})()})()
// case: close fault replaces break at the return method
(function outer(){function R(v,d){this.value=v;this.done=d};function I(){};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){(function close(){null.forOfCloseFault})()};function X(){};X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)break})()
// case: next result type fault originates at loop advance
(function outer(){function I(){};I.prototype.next=function next(){return 1};function X(){};X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)value})()

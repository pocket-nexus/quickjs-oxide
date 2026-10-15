// CASES
// case: ordinary functions collect only actual trailing arguments
(function(left,...rest){
            return left+'|'+rest.length+'|'+rest[0]+'|'+rest[1]+'|'+
                Array.isArray(rest)+'|'+(Object.getPrototypeOf(rest)===Array.prototype);
        })(40,1,2)
// case: missing fixed arguments do not create rest padding
(function(left,right,...rest){
            return left+'|'+String(right)+'|'+rest.length+'|'+(0 in rest);
        })(42)
// case: function length stops before an identifier rest parameter
(function(){
            var ordinary=function(left,...rest){};
            var arrow=(left,right,...rest)=>0;
            var method=({method(left,...rest){}}).method;
            var dynamic=Function('left','...rest','');
            return ordinary.length+'|'+arrow.length+'|'+method.length+'|'+dynamic.length;
        })()
// case: rest makes sloppy arguments unmapped in both directions
(function(left,...rest){
            arguments[0]=7;arguments[1]=8;
            var before=left+'|'+rest[0];
            left=9;rest[0]=10;
            return before+'|'+left+'|'+rest[0]+'|'+arguments[0]+'|'+arguments[1];
        })(1,2)
// case: arguments snapshots raw values before rest overwrites its slot
(function(...rest){
            return arguments.length+'|'+arguments[0]+'|'+rest.length+'|'+rest[0]+'|'+
                (arguments[0]===rest);
        })(42)
// case: body var reuse retains rest while function hoists replace it afterward
(function(){
            var retained=(function(...rest){var rest;return Array.isArray(rest)+'|'+rest.length})(1,2);
            var hoisted=(function(...rest){function rest(){return 42}return typeof rest+'|'+rest()})(1);
            return retained+';'+hoisted;
        })()
// case: closures capture the initialized rest binding
(function(...rest){
            return function(){return rest[0]+rest[1]};
        })(40,2)()
// case: arrows collect their own rest while preserving lexical this
(function(){
            var arrow=(left,...rest)=>this.base+left+rest[0];
            return arrow.call({base:100},1,1);
        }).call({base:40})
// case: object methods combine dynamic this with their own rest
({base:40,method(left,...rest){
            return this.base+left+rest[0];
        }}).method(1,1)
// case: call and apply preserve identifier rest indexing
(function(){
            function collect(left,...rest){return left+'|'+rest.join(',')}
            return collect.call(null,40,1,2)+';'+collect.apply(null,[40,1,2]);
        })()
// case: direct eval observes the initialized rest binding
(function(...rest){return eval('rest[0]+rest[1]')})(40,2)
// case: Function constructor reuses ordinary rest parsing
Function('left','...rest','return left+rest[0]+rest[1]')(40,1,1)
// ERROR_CASES
// case: ordinary rest must be last
function invalid(...rest,next){}
// case: arrow rest rejects a trailing comma
(...rest,)=>0
// case: method rest rejects an initializer
({method(...rest=[]) {}})
// case: non simple ordinary parameters reject duplicates
function invalid(value,value,...rest){}
// case: non simple arrow use strict wins over duplicate
(value,...value)=>{'use strict'}
// case: non simple method use strict wins over duplicate
({method(value,...value){'use strict'}})
// case: getters cannot declare rest
({get value(...rest){}})
// case: setters cannot declare rest
({set value(...rest){}})

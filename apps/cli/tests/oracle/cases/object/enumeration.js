// VALUE_CASES
// case: ordinary order enumerable filtering inherited and symbol keys
(function(){
            function rows(values){var out=[];for(var i=0;i<values.length;i++)out[out.length]=values[i][0]+":"+values[i][1];return out.join(",")}
            var proto=Object();proto.inherited="I";
            var object=Object.create(proto),hidden=Object(),symbol=Symbol("hidden");
            object.b="B";object[2]="two";object[1]="one";object.a="A";
            hidden.value="H";hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(object,"hidden",hidden);object[symbol]="S";
            return Object.keys(object).join(",")+"|"+Object.values(object).join(",")+"|"+
                rows(Object.entries(object))+"|"+object.inherited+"|"+Object.keys(object).length;
        })()
// case: delete and readd moves a string key after retained strings
(function(){
            var object=Object();object.first=1;object.second=2;object.third=3;
            delete object.second;object.second=4;
            return Object.keys(object).join(",")+"|"+Object.values(object).join(",")+"|"+
                Object.entries(object)[2][0]+":"+Object.entries(object)[2][1];
        })()
// case: primitive ToObject families and String indices
(function(){
            function row(value){return Object.keys(value).join(",")+":"+Object.values(value).join(",")+":"+Object.entries(value).length}
            return row("ab")+"|"+row(17)+"|"+row(false)+"|"+row(19n)+"|"+row(Symbol("s"));
        })()
// case: Array and String exotic own-key surfaces
(function(){
            function rows(values){var out=[];for(var i=0;i<values.length;i++)out[out.length]=values[i][0]+":"+values[i][1];return out.join(",")}
            var array=[];array[2]="C";array[0]="A";array.extra="E";
            var hidden=Object();hidden.value="H";hidden.enumerable=false;Object.defineProperty(array,"hidden",hidden);
            var string=Object("ab");string[4]="D";string.extra="X";
            return Object.keys(array).join(",")+"|"+Object.values(array).join(",")+"|"+rows(Object.entries(array))+"|"+
                Object.keys(string).join(",")+"|"+Object.values(string).join(",")+"|"+rows(Object.entries(string));
        })()
// case: numeric strings precede ordinary strings and symbols stay excluded
(function(){
            var object=Object(),first=Symbol("first"),second=Symbol("second");
            object.z="z";object[10]="ten";object[first]="S1";object[2]="two";
            object.a="a";object[second]="S2";object[1]="one";
            return Object.keys(object).join(",")+"|"+Object.values(object).join(",")+"|"+
                Object.getOwnPropertySymbols(object).length;
        })()
// MUTATION_CASES
// case: keys skips Get while values and entries recheck later properties
(function(){
            function run(kind){
                var log="",object=Object(),hidden=Object();
                object.__defineGetter__("a",function(){
                    log+="a";delete object.b;
                    var visible=Object();visible.value="C";visible.writable=true;visible.enumerable=true;visible.configurable=true;
                    Object.defineProperty(object,"c",visible);object.late="L";return "A";
                });
                object.b="B";hidden.value="C";hidden.writable=true;hidden.enumerable=false;hidden.configurable=true;
                Object.defineProperty(object,"c",hidden);
                var result=Object[kind](object),out=[];
                for(var i=0;i<result.length;i++)out[out.length]=kind==="entries"?result[i][0]+":"+result[i][1]:result[i];
                return kind+":"+out.join(",")+":"+log+":"+object.hasOwnProperty("late");
            }
            return run("keys")+"|"+run("values")+"|"+run("entries");
        })()
// case: a getter can redefine the value source of a later snapshotted key
(function(){
            function run(kind){
                var log="",object=Object();
                object.__defineGetter__("a",function(){
                    log+="a";object.__defineGetter__("b",function(){log+="b";return "B2"});return "A";
                });
                object.b="B1";
                var result=Object[kind](object),out=[];
                for(var i=0;i<result.length;i++)out[out.length]=kind==="entries"?result[i][0]+":"+result[i][1]:result[i];
                return out.join(",")+":"+log;
            }
            return run("keys")+"|"+run("values")+"|"+run("entries");
        })()
// case: low-depth values and entries getter recursion stays on the normal path
(function(){
            var remaining=4,calls=0,object=Object(),descriptor=Object();
            descriptor.enumerable=true;
            descriptor.get=function(){
                calls++;
                if(remaining===0)return "leaf";
                remaining--;
                if(remaining%2===0)return Object.values(object)[0];
                return Object.entries(object)[0][1];
            };
            Object.defineProperty(object,"value",descriptor);
            return Object.values(object)[0]+"|"+calls;
        })()
// ERROR_CASES
// case: Object.keys missing argument
Object.keys()
// case: Object.values null
Object.values(null)
// case: Object.entries undefined
Object.entries(undefined)
// case: Object.values preserves primitive getter throw
(function(){var object=Object();object.__defineGetter__("x",function(){throw 73});return Object.values(object)})()
// case: Object.entries preserves Error getter throw
(function(){var object=Object();object.__defineGetter__("x",function(){throw new RangeError("entry")});return Object.entries(object)})()

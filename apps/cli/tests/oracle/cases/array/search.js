// VALUE_CASES
// case: dense at and search
(function(){
            var a=[10,20,30];
            return a.at(0)+"|"+a.at(-1)+"|"+a.includes(20)+"|"+a.indexOf(20)+"|"+a.lastIndexOf(20);
        })()
// case: holes differ between includes and index search
(function(){
            var a=[,1,,];
            return a.length+"|"+(a.at(0)===undefined)+"|"+(a.at(2)===undefined)+"|"+
                a.includes(undefined)+"|"+a.indexOf(undefined)+"|"+a.lastIndexOf(undefined);
        })()
// case: sparse length and a distant own element
(function(){
            var a=[];a.length=6;a[4]="x";
            return a.at(-2)+"|"+(a.at(1)===undefined)+"|"+a.includes("x")+"|"+
                a.indexOf("x")+"|"+a.lastIndexOf("x");
        })()
// case: holes continue through the prototype chain
(function(){
            Array.prototype[1]=9;var a=[0,,2];
            return a.at(1)+"|"+a.includes(9)+"|"+a.indexOf(9)+"|"+a.lastIndexOf(9);
        })()
// case: same value zero and strict equality search values
(function(){
            var symbol=Symbol("needle"),other=Symbol("needle"),a=[0/0,0,7n,symbol];
            return a.includes(0/0)+"|"+a.indexOf(0/0)+"|"+a.lastIndexOf(0/0)+"|"+
                a.includes(-0)+"|"+a.indexOf(-0)+"|"+a.includes(7n)+"|"+
                a.indexOf(7n)+"|"+a.includes(7)+"|"+a.includes(symbol)+"|"+
                a.indexOf(symbol)+"|"+a.includes(other);
        })()
// case: at integer conversion and saturation
(function(){
            function show(value){return value===undefined?"u":value}
            var a=[10,20,30];
            return show(a.at())+"|"+show(a.at(undefined))+"|"+show(a.at(0/0))+"|"+
                show(a.at(1.9))+"|"+show(a.at(-1.9))+"|"+show(a.at(1/0))+"|"+
                show(a.at(-1/0))+"|"+show(a.at(1e100))+"|"+show(a.at(-1e100));
        })()
// FROM_INDEX_CASES
// case: omitted and explicit undefined fromIndex
(function(){
            var a=[1,2,1];
            return a.includes(1)+"|"+a.includes(1,undefined)+"|"+
                a.indexOf(1)+"|"+a.indexOf(1,undefined)+"|"+
                a.lastIndexOf(1)+"|"+a.lastIndexOf(1,undefined);
        })()
// case: forward fromIndex clamp matrix
(function(){
            var a=[0,1,2,1,4];
            return a.indexOf(1,0/0)+"|"+a.indexOf(1,1/0)+"|"+a.indexOf(1,-1/0)+"|"+
                a.indexOf(1,1.9)+"|"+a.indexOf(1,2.9)+"|"+a.indexOf(1,-2.9)+"|"+
                a.indexOf(1,-1.9)+"|"+a.indexOf(1,1e100)+"|"+a.indexOf(1,-1e100)+"|"+
                a.includes(1,2.9)+"|"+a.includes(1,-2.9);
        })()
// case: reverse fromIndex clamp matrix
(function(){
            var a=[0,1,2,1,4];
            return a.lastIndexOf(1,0/0)+"|"+a.lastIndexOf(1,1/0)+"|"+
                a.lastIndexOf(1,-1/0)+"|"+a.lastIndexOf(1,2.9)+"|"+
                a.lastIndexOf(1,-2.9)+"|"+a.lastIndexOf(1,-1.9)+"|"+
                a.lastIndexOf(1,1e100)+"|"+a.lastIndexOf(1,-1e100);
        })()
// case: zero length skips fromIndex conversion
(function(){
            var index=Object(),a=[];index.valueOf=function(){throw "converted"};
            return a.includes(1,index)+"|"+a.indexOf(1,index)+"|"+a.lastIndexOf(1,index);
        })()
// ORDER_CASES
// case: length number index and element access order
(function(){
            function row(method,isAt,isReverse){
                var log="",length=Object(),index=Object(),receiver=Object();
                length.valueOf=function(){log+="N";return 3};
                index.valueOf=function(){log+="F";return isReverse?1/0:(isAt?-1:0)};
                receiver.__defineGetter__("length",function(){log+="L";return length});
                receiver.__defineGetter__("0",function(){log+="0";return "a"});
                receiver.__defineGetter__("1",function(){log+="1";return "b"});
                receiver.__defineGetter__("2",function(){log+="2";return "c"});
                var result=isAt?method.call(receiver,index):method.call(receiver,"missing",index);
                return result+":"+log;
            }
            return row(Array.prototype.at,true,false)+"|"+
                row(Array.prototype.includes,false,false)+"|"+
                row(Array.prototype.indexOf,false,false)+"|"+
                row(Array.prototype.lastIndexOf,false,true);
        })()
// case: length and conversion throws preserve order
(function(){
            function row(method,isAt,stage){
                var log="",length=Object(),index=Object(),receiver=Object();
                length.valueOf=function(){log+="N";if(stage===1)throw "number";return 1};
                index.valueOf=function(){log+="F";if(stage===2)throw "index";return 0};
                receiver.__defineGetter__("length",function(){log+="L";if(stage===0)throw "length";return length});
                receiver.__defineGetter__("0",function(){log+="0";if(stage===3)throw "element";return "x"});
                try{
                    if(isAt)return method.call(receiver,index);
                    return method.call(receiver,"missing",index);
                }catch(error){return log+":"+error}
            }
            return row(Array.prototype.at,true,0)+"|"+row(Array.prototype.at,true,1)+"|"+
                row(Array.prototype.at,true,2)+"|"+row(Array.prototype.at,true,3)+"|"+
                row(Array.prototype.includes,false,0)+"|"+row(Array.prototype.includes,false,1)+"|"+
                row(Array.prototype.includes,false,2)+"|"+row(Array.prototype.includes,false,3);
        })()
// case: index search gets present accessors and propagates their throws
(function(){
            function row(method){
                var log="",receiver=Object();receiver.length=1;
                receiver.__defineGetter__("0",function(){log+="0";throw "element"});
                try{method.call(receiver,"x")}catch(error){return log+":"+error}
            }
            return row(Array.prototype.indexOf)+"|"+row(Array.prototype.lastIndexOf);
        })()
// GENERIC_CASES
// case: ordinary object receiver
(function(){
            var receiver=Object();receiver[0]="a";receiver[2]="c";receiver.length=3;
            return Array.prototype.at.call(receiver,-1)+"|"+
                (Array.prototype.at.call(receiver,1)===undefined)+"|"+
                Array.prototype.includes.call(receiver,undefined)+"|"+
                Array.prototype.indexOf.call(receiver,undefined)+"|"+
                Array.prototype.lastIndexOf.call(receiver,"a");
        })()
// case: string receiver exposes UTF-16 indexed properties
(function(){
            var string="A\uD83D\uDCA9Z",second=Array.prototype.at.call(string,1),third=Array.prototype.at.call(string,2);
            return Array.prototype.at.call(string,-1)+"|"+second.charCodeAt(0)+"|"+third.charCodeAt(0)+"|"+
                Array.prototype.includes.call(string,"Z")+"|"+
                Array.prototype.indexOf.call(string,"Z")+"|"+
                Array.prototype.lastIndexOf.call(string,"A");
        })()
// case: number boolean bigint and symbol receivers box to zero length
(function(){
            var symbol=Symbol("receiver");
            return (Array.prototype.at.call(7,0)===undefined)+"|"+
                Array.prototype.includes.call(false,false)+"|"+
                Array.prototype.indexOf.call(7n,7n)+"|"+
                Array.prototype.lastIndexOf.call(symbol,symbol);
        })()
// case: nullish receivers are rejected
(function(){
            function row(method,value){try{method.call(value,0);return "missing"}catch(error){return error.name}}
            return row(Array.prototype.at,null)+"|"+row(Array.prototype.includes,undefined)+"|"+
                row(Array.prototype.indexOf,null)+"|"+row(Array.prototype.lastIndexOf,undefined);
        })()
// ERROR_CASES
// case: at rejects null
Array.prototype.at.call(null,0)
// case: includes rejects undefined
Array.prototype.includes.call(undefined,0)
// case: at rejects a Symbol index
Array.prototype.at.call([1],Symbol('index'))
// case: indexOf rejects a Symbol fromIndex
Array.prototype.indexOf.call([1],1,Symbol('fromIndex'))

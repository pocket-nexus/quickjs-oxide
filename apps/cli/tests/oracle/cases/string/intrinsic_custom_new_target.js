// mode: print
var log="",custom=Object(),argument=Object();
var target=(function ForeignString(){}).bind(null);
argument[Symbol.toPrimitive]=function(hint){log+="argument:"+hint+";";return "xy"};
Object.defineProperty(target,"prototype",{configurable:true,get:function(){
    log+="prototype;";return custom;
}});
var value=Reflect.construct(String,[argument],target);
print((Object.getPrototypeOf(value)===custom)+"|"+
      String.prototype.valueOf.call(value)+"|"+
      Object.getOwnPropertyNames(value).join(",")+"|"+log);
var fallbackTarget=(function ForeignFallback(){}).bind(null);
fallbackTarget.prototype=1;
var fallback=Reflect.construct(String,["z"],fallbackTarget);
print((Object.getPrototypeOf(fallback)===String.prototype)+"|"+
      fallback.valueOf()+"|"+Object.getOwnPropertyNames(fallback).join(","));

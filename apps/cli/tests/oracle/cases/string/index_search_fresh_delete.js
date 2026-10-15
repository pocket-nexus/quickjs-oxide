// mode: print
var removed=delete String.prototype.indexOf;
print(removed+'|'+('indexOf' in String.prototype)+'|'+String.prototype.indexOf);

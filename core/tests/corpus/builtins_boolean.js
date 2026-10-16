// Boolean: Boolean(v), toString, valueOf.
print(Boolean(), Boolean(0), Boolean(''), Boolean('x'), Boolean({}), Boolean(NaN), Boolean([]), Boolean(null));
print(true.toString(), false.toString(), true.valueOf(), typeof Boolean(1));
try { Boolean.prototype.toString.call(1); } catch (e) { print(e.name); }

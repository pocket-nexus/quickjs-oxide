// The error constructors and Error.prototype.toString.
var e = new Error('boom'), t = TypeError('bad'), r = new RangeError();
print(e.message, e.name, String(e), t instanceof TypeError, t instanceof Error, String(t), String(r), r.message === '');
print(Error.prototype.toString.call({ name: 'N', message: 'M' }), Error.prototype.toString.call({}), Error.prototype.toString.call({ name: '', message: 'only' }));
print(new SyntaxError('s').name, new ReferenceError('r').name, new EvalError('v').name, typeof InternalError);
try { null.x; } catch (err) { print(err instanceof TypeError, err.constructor === TypeError); }
try { Error.prototype.toString.call(1); } catch (err) { print(err.name); }

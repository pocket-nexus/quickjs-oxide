// new.target.
function Probe() { return new.target === Probe ? 'new' : 'call'; }
function Holder() { this.kind = new.target === undefined ? 'call' : 'new'; }
print(Probe(), typeof new Probe(), new Holder().kind);
function Guard() { if (!new.target) return new Guard(); this.made = true; }
print(Guard().made, new Guard().made);
var arrow = function () { return (() => new.target)(); };
print(arrow() === undefined, new arrow() === undefined);

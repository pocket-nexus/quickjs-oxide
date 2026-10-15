// mode: print
var savedConcat = String.prototype.concat;
var seed = "x".repeat(8193);
var near = seed;
for (var i = 0; i < 16; i++)
    near = near + near;
var ordinary = "A".repeat(8193) + "\ud83d" + "\ude80" + "z".repeat(513);
var peer = "A".repeat(8192) + ("A\ud83d\ude80" + "z".repeat(513));
var boundaryPeer = "A".repeat(8193) + "\ud83d" + "\ude81" + "z".repeat(513);
var prefixPeer = ordinary + "q";
var markers = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
var deep = "";
for (var j = 0; j < 70; j++)
    deep = deep + markers.charAt(j % markers.length).repeat(8193);
var power = "m";
var powers = [power];
for (var k = 1; k < 30; k++) {
    power = power + power;
    powers.push(power);
}
var exactMax = "";
for (var n = powers.length - 1; n >= 0; n--)
    exactMax = exactMax + powers[n];
print((function () {
    var caught = function (thunk) {
        try {
            thunk();
            return "ok";
        } catch (error) {
            return "throw:" + error.name + ":" + error.message;
        }
    };

    var out = "limit=" + near.length + "|" + near[0] + "|" +
              near[near.length - 1] + "|" +
              caught(function () { return near + near; });

    var equal = ordinary === peer;
    var less = ordinary < peer;
    var greater = peer < ordinary;
    var codePoint = ordinary.codePointAt(8193);
    var holder = {};
    holder[ordinary] = 17;
    out += "\nordinary=" + ordinary.length + "|" + ordinary[0] + "|" +
           ordinary[8192] + "|" + ordinary[8193].charCodeAt(0) + "|" +
           ordinary[8194].charCodeAt(0) + "|" +
           ordinary[ordinary.length - 1] + "|" + codePoint + "|" +
           equal + "|" + less + "|" + greater + "|" + holder[peer] + "|" +
           (ordinary < boundaryPeer) + "|" + (boundaryPeer < ordinary) + "|" +
           (ordinary < prefixPeer) + "|" + (prefixPeer < ordinary);

    var viaConcat = savedConcat.call("L", ordinary, "R");
    out += "\nconcat=" + viaConcat.length + "|" + viaConcat[0] + "|" +
           viaConcat[1] + "|" + viaConcat[viaConcat.length - 1] + "|" +
           viaConcat.codePointAt(8194);

    out += "\nrebalance=" + deep.length + "|" + deep[0] + "|" +
           deep[8192] + "|" + deep[8193] + "|" + deep[30 * 8193] + "|" +
           deep[59 * 8193] + "|" + deep[60 * 8193] + "|" +
           deep[61 * 8193] + "|" + deep[deep.length - 1];

    var log = "";
    var later = {
        toString: function () {
            log += "later,";
            return "later";
        }
    };
    var overflow = caught(function () {
        return savedConcat.call("", near, near, later);
    });
    out += "\nconcat-overflow=" + overflow + "|" + log + "|" + near.length;
    out += "\nmax=" + exactMax.length + "|" + (exactMax + "").length + "|" +
           caught(function () { return exactMax + "x"; });
    return out;
})());

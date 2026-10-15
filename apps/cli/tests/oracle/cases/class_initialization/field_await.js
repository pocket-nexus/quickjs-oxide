// mode: print
var aw\u0061it = 40;
async function build() {
    class Fields {
        instance = await + 1;
        static staticField = await + 2;
        #private = await + 3;
        static #staticPrivate = await + 4;
        arrow = () => await + 5;
        static escaped = aw\u0061it + 6;
        [await Promise.resolve("computed")] = 47;
        read() { return this.#private; }
        static read() { return this.#staticPrivate; }
    }
    return Fields;
}
build().then(function (Fields) {
    var instance = new Fields();
    print([
        instance.instance,
        Fields.staticField,
        instance.read(),
        Fields.read(),
        instance.arrow(),
        Fields.escaped,
        instance.computed
    ].join(","));
});

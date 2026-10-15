// mode: completion-name
// case: fixed instance constructor field
class C { constructor; }
// case: fixed quoted instance constructor field
class C { 'constructor'; }
// case: fixed static constructor field
class C { static constructor; }
// case: fixed quoted static constructor field
class C { static 'constructor'; }
// case: fixed instance prototype field
class C { prototype; }
// case: fixed quoted instance prototype field
class C { 'prototype'; }
// case: fixed static prototype field
class C { static prototype; }
// case: fixed quoted static prototype field
class C { static 'prototype'; }
// case: instance field arguments reference
class C { field = arguments; }
// case: static field arguments reference
class C { static field = arguments; }
// case: static block arguments reference
class C { static { arguments; } }
// case: instance field arrow inherits arguments restriction
class C { field = () => arguments; }
// case: static field arrow inherits arguments restriction
class C { static field = () => arguments; }
// case: static block arrow inherits arguments restriction
class C { static { (() => arguments); } }
// case: class arguments binding does not legalize an authored reference
class C { static { class arguments {} arguments; } }
// case: static block await reference
class C { static { await; } }
// case: static block lexical await binding
class C { static { let await; } }
// case: static block var await binding
class C { static { var await; } }
// case: static block function await binding
class C { static { function await() {} } }
// case: static block class await binding
class C { static { class await {} } }
// case: static block arrow default await reference
class C { static { ((value = await) => 0); } }
// case: static block arrow await parameter
class C { static { ((await) => 0); } }
// case: static block arrow object-pattern await parameter
class C { static { (({await}) => 0); } }
// case: static block arrow array-pattern await parameter
class C { static { (([await]) => 0); } }
// case: static block arrow rest await parameter
class C { static { ((...await) => 0); } }

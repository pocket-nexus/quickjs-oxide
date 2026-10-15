// case: fixed global field
Function.name
// case: computed global field
Function['name']
// case: fixed and computed identity
Function.prototype === Function['prototype']
// case: chained constructor identity
Function['prototype'].constructor === Function
// case: chained member type
typeof Function.prototype.toString
// case: fixed method receiver
Function().toString()
// case: computed method receiver
Function()['toString']()
// case: new member constructor head
new Function.prototype.constructor('return 42')()
// case: new parenthesized member constructor head
new (Function.prototype.constructor)('return 43')()
// case: string own length
'abc'.length
// case: string own indexed property
'abc'[1]

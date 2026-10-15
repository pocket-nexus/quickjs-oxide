// mode: print
// SYNTAX_CASES
// case: var for-of declaration cannot have an initializer
for(var value=1 of 'a')value
// case: let for-of declaration cannot have an initializer
for(let value=1 of 'a')value
// case: const for-of declaration cannot have an initializer
for(const value=1 of 'a')value
// case: for-of declaration has one binding
for(var first,second of 'a')first
// case: for-of requires a right operand
for(var value of)value
// case: async cannot be the bare for-of left expression
for(async of 'a')async
// case: call expression is not a for-of assignment target
for(f() of []) {}
// ARRAY_BINDING_SYNTAX_CASES
// case: let array binding requires a top-level initializer
let [value];
// case: var array binding requires a top-level initializer
var [value];
// case: const array binding requires a top-level initializer
const [value];
// case: missing initializer wins over a nested pattern frontier
let [[value]];
// case: missing initializer wins over an invalid element
let [1];
// case: missing initializer wins over a nonterminal rest element
let [...value,];
// case: missing initializer wins over a missing comma
let [first second];
// case: missing initializer lookahead balances a regexp bracket
let [value=/]/];
// case: missing initializer wins over a lookahead regexp lexer failure
let [value=/[/];
// case: array rest requires a binding target
let [...]=[]
// case: array rest cannot have a trailing comma
let [...rest,]=[]
// case: array rest cannot have a default
let [...rest=1]=[]
// case: let cannot be an array lexical binding name
let [let]=[]
// case: let cannot be a for-of array lexical binding name
for(let [let] of [[]]){}
// case: nested rest default wins before an invalid inner lexical name
let [...[let]=[]]=[[]]
// case: an array expression is not misclassified as a nested binding pattern
let [[value]+other]=[[1]]
// case: an object expression is not misclassified as a nested binding pattern
let [{value}+other]=[{}]
// case: an object expression nested in an array is not a binding pattern
let [[{value}+other]]=[[{}]]
// case: a for-of object expression is not a nested binding pattern
for(const [{value}+other] of [[{}]]){}
// case: nested object rest default wins before an invalid lexical leaf
let [...{let}=[]]=[{}]
// case: for-of nested object rest default wins before an invalid lexical leaf
for(const [...{let}=[]] of [[{}]]){}

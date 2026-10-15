// mode: print
// NO_IN_ERROR_CASES
// case: unparenthesized in in a classic-for var initializer
(function(){ for (var value = 'own' in membershipTarget; false; ); })()
// case: conditional alternate inherits the classic-for NoIn mode
(function(){ for (var value = false ? false : 'own' in membershipTarget; false; ); })()
// case: assignment right hand side inherits the classic-for NoIn mode
(function(){ var other; for (var value = other = 'own' in membershipTarget; false; ); })()
// case: expression initializer inherits the classic-for NoIn mode
(function(){ var value; for (value = 'own' in membershipTarget; false; ); })()
// PARSER_ERROR_CASES
// case: numeric adjacency before instanceof
1instanceof Function
// case: missing instanceof right operand
1 instanceof
// case: missing in right operand
'x' in
// case: escaped in is not an operator
1 \u0069n Function
// case: escaped instanceof is not an operator
1 \u0069nstanceof Function

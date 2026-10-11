# Values, strings and numeric conversion

value owns the full JS Value, the restricted primitive-constant
representation, strings and BigInt, and provides the Number algorithms and
ECMAScript value conversions. The compiler, VM, object and builtins share
these values and algorithms so that numeric and string semantics have a
single definition.

Pure numeric, string and collection-key algorithms are separate from runtime
conversions. Conversions such as ToPrimitive can read properties, call object
methods and throw arbitrary JS values, so they preserve order, context and
identity. Direct exact-Number arithmetic does not replace the rules for object
conversion or mixed BigInt operations.

Strings carry UTF-16 code-unit semantics, including lone surrogates. Latin1,
UTF-16 and rope are storage choices, and comparison and hashing give
consistent results across them; the randomly seeded collection hash and the
QuickJS content fingerprint serve different purposes.

object manages object identity and property behavior, heap manages raw
reference edges and the VM manages stack slots and value movement. value owns
values and their conversion semantics, not active execution frames.

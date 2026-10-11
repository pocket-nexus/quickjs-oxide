# Objects and property semantics

object owns object handles, property descriptors, shapes and object internal
methods, including ordinary property get, set, define and delete, and the
property behavior of exotic objects such as Array and Arguments. Changes to
Get, Set, Define or enumeration order start here, separating ordinary rules
from exotic semantics.

Ordinary property algorithms classify and locate properties through storage
probes and fetch only the data the operation needs. Accessors, distinct
receivers, the prototype chain and exotic objects follow their own semantics;
direct storage paths apply only under their stated conditions.

Shapes and dictionaries maintain layout and property order. Physical slots
can move, so cached slot positions do not survive calls that may modify the
object, and observable enumeration order is tracked separately from physical
slot order. heap performs mutations of raw records and reference edges.

The VM runs user getters, setters and Proxy traps, and builtins owns complete
built-in method algorithms. Semantic owners maintain callback resume state;
see [workspace architecture](../../../docs/architecture.md).

# Property callback activation protocol

Ordinary bytecode getters and Proxy get traps use the same authenticated callee
owner and immutable publication facts as ordinary Call. Their frame initially has
an unmaterialized active-frame token; its executable, receiver, closure owner,
arguments and fault PC live in FrameStore. General bound/proxy callable dispatch,
async/generator creation and constructors retain their existing protocols.

A property lookup finishes selecting its callable before installing a lazy child.
No heap borrow spans installation or JavaScript execution. A getter has no outgoing
argument allocation. Proxy get arguments use the execution's empty outgoing buffer;
installation transfers roots to the frame window and recycles that buffer.

Observation still materializes the complete lazy suffix before error/backtrace
construction, host callbacks, suspension or observable owner release. A callback
return alone is not an observer. Exception unwinding retains the canonical path.

A normal child return may reply directly only when its ReturnTarget identity
matches the immediately enclosing pending PropertyRead. It first takes the result,
clears the child window, retires any materialized child guard, and recycles its cold
frame. Then the property driver consumes the exact pending reply. Proxy descriptor
invariants execute unchanged after the trap; inconsistent nonconfigurable data or
accessor results still throw. Other query finishes keep their existing cold route.

Method and get continuation allocation caches contain only empty Option boxes.
Dropping a live continuation releases all roots and depth guards before caching its
allocation; caches are bounded and never retain a Runtime or property value. Pending
query boxes are execution-local and similarly cleared before reuse. Cached capacity
is not a cached JavaScript lookup result.

Native calls use an inline pending descriptor in ActiveFrames instead of a second
unmaterialized-token protocol. Its checked identity and budget charge exist from
entry, and get/last/iter include it immediately. The common synchronous tail does
not enter the records Vec; nested entry moves it there before appending the child.
Authenticated NoJS inputs skip ancestor materialization, while observable calls
and errors retain publication boundaries. This is delayed container insertion,
not absence from the logical activation registry; performance remains to be measured.

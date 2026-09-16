# Resident property mutation

A PutField site stores location metadata using the read cache's domain, realm,
shape identity, layout revision, prototype epoch and two-entry replacement
policy. A write accepts only a depth-zero writable Data slot. Array length is
excluded because changing it requires the ArraySetLength algorithm. Proxy,
module namespace and numeric exotic interception are excluded by the same
receiver admission as reads. Layout changes and dictionary revision changes
invalidate the fact; replacing a value leaves the location valid.

The immediate leaf additionally proves that both old and new values are
scalars and releasing the base cannot trigger cleanup. It commits under the
same heap borrow and consumes the base/value slots without PC publication.
Dense indexed scalar writes use the equivalent no-drain proof. Kept-receiver
reads need no base release proof; discarded string keys must keep their storage
alive, otherwise completion moves to the resident owner boundary.

Reference-bearing writes end RunSlots and publish the exact active fault PC.
A fixed tuple owns the removed operands while the existing runtime transaction
retains the incoming edge before detaching the replaced edge. Owner destruction
happens after the new short slot view is dropped. Declines restore all operands
and let canonical execution perform setters, proxy traps, coercions and errors.
The outer run frame and FrameTransaction remain resident across successful work.

DefineField admits an extensible ordinary object with a missing own key and
stores normal writable/enumerable/configurable data through shape transitions
or exclusive append. It keeps the receiver stack operand and consumes the value.
Delete admits ordinary own configurable Data slots (or an absent own key), uses
the dictionary-aware delete transaction and produces true. Nonconfigurable and
exotic cases retain canonical strict-mode and callback behavior.

## Observation protocol assessment (S15.6)

A universal pull protocol would need every error, backtrace, host reentry,
suspension and owner-release observer to reach the currently borrowed FrameStore.
The current Runtime-only observers cannot safely recover that mutable store;
adding a second registry or a hidden pointer would duplicate frame identity and
unwind ownership. This stage therefore retains explicit observation boundaries,
centralizes exit classification, and skips unchanged published PCs. S18 narrows
native ancestor publication using authenticated NoJS inputs while retaining an
inline logical native descriptor for existing observers and budget accounting.
A broader pull redesign is not needed for the implemented residency paths.

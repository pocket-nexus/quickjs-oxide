"""Check the existing WeakRef cleanup match arm with complete heap framing.

The wrapper supplies exactly its free variables (self and payload). The arm's
executable block is copied unchanged; this does not verify the enclosing loop.
"""

from source import item
import weakpass


def proof(heap_source, gc_source, object_source, value_source, arena_code, scan_code):
    scope = item(item(gc_source, r"impl Heap\b"), r"fn clear_weak\b")
    arm = item(scope, r"InternalTag::WeakRef\s*=>")
    body = arm[arm.index("{"):]
    body = body[:-1] + '''proof {
        assert(self.cleanup_frame(old(self), payload));
        assert forall|i: u32| self.payloads.internals.live_at(i)
            implies (i as int) < self.payloads.internals.records@.len() by {
            assert(old(self).payloads.internals.live_at(i));
        }
        assert(!heap_kind(kind_of(Value::UNDEFINED.0))) by(compute);
    }\n}'''
    constant = item(item(value_source, r"impl Value\b"), r"pub const UNDEFINED\b")
    initializer = constant.split("=", 1)[1].strip().removesuffix(";")
    constants = item(value_source, r"const SPECIAL_UNDEFINED\b") + '''
    impl Value {
        spec const SPEC_UNDEFINED: Value = SPEC_INITIALIZER;
        #[verifier::when_used_as_spec(SPEC_UNDEFINED)]
        exec const UNDEFINED: Value ensures Self::UNDEFINED == Self::SPEC_UNDEFINED { INITIALIZER }
    }
    '''.replace("SPEC_INITIALIZER", initializer.replace("boxed(", "boxed_spec(")).replace("INITIALIZER", initializer)
    specs = '''
        spec fn cleanup_frame(&self, previous: &Self, payload: u32) -> bool {
            *self == Self { payloads: self.payloads, ..*previous }
            && self.payloads == PayloadArenas { internals: self.payloads.internals, ..previous.payloads }
            && self.payloads.internals == Arena { records: self.payloads.internals.records, ..previous.payloads.internals }
            && self.payloads.internals.records@.len() == previous.payloads.internals.records@.len()
            && (forall|i: int| 0 <= i < self.payloads.internals.records@.len() && i != payload ==>
                self.payloads.internals.records@[i] == previous.payloads.internals.records@[i])
            && self.payloads.internals.records@[payload as int] == Internal {
                slots: self.payloads.internals.records@[payload as int].slots,
                ..previous.payloads.internals.records@[payload as int] }
        }
        fn weak_ref_arm(&mut self, payload: u32)
            requires old(self).payloads.internals.valid_marks(),
                old(self).payloads.internals.live_at(payload),
                old(self).payloads.internals.records@[payload as int].slots@.len() > 0,
            ensures final(self).cleanup_frame(old(self), payload),
                final(self).payloads.internals.valid_marks(),
                final(self).payloads.internals.records@[payload as int].slots@ ==
                    if old(self).live(old(self).payloads.internals.records@[payload as int].slots@[0]) {
                        old(self).payloads.internals.records@[payload as int].slots@
                    } else {
                        old(self).payloads.internals.records@[payload as int].slots@.update(0, Value::UNDEFINED)
                    },
                final(self).live(final(self).payloads.internals.records@[payload as int].slots@[0]),
    '''
    code = weakpass.proof(heap_source, gc_source, object_source, arena_code, scan_code,
        extra_heap=specs + body, include_scan=False, include_arena_mut=True)
    marker = 'pub fn main() {}'
    if code.count(marker) != 1:
        raise ValueError('Weak cleanup proof lost its unique composition point')
    code = code.replace(marker, constants + '\n' + marker)
    return '#[path = "kernel.rs"]\nmod gc_kernel;\nmod heap_proof {\n' + code + '\n}\nfn main() {}\n'

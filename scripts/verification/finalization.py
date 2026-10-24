"""Exact finalization entry effects and marked-or-primitive queued roots."""

from source import item, method
import weakpass


def proof(scan_source, value_source, job_code, scan_code, template):
    parts = [item(job_code, r"enum Job\b"), item(job_code, r"impl Job\b"),
             item(value_source, r"const SPECIAL_UNDEFINED\b")]
    constant = item(item(value_source, r"impl Value\b"), r"pub const UNDEFINED\b")
    initializer = constant.split("=", 1)[1].strip().removesuffix(";")
    parts.append('''impl Value {
        spec const SPEC_UNDEFINED: Value = SPEC_INITIALIZER;
        #[verifier::when_used_as_spec(SPEC_UNDEFINED)]
        exec const UNDEFINED: Value ensures Self::UNDEFINED == Self::SPEC_UNDEFINED { INITIALIZER }
    }'''.replace("SPEC_INITIALIZER", initializer.replace("boxed(", "boxed_spec(")).replace("INITIALIZER", initializer))
    parts.append(template)
    contract='''
     requires old(slots)@.len() > 0, (old(slots)@.len() - 1) % 3 == 0,
         marks.live(old(slots)@[0]),
         forall|i: int| 1 <= i && i % 3 == 1 && i + 2 < old(slots)@.len()
            && old(slots)@[i].0 != Value::EMPTY.0 ==> marks.live(old(slots)@[i + 1]),
     ensures final(slots)@.len() == old(slots)@.len(),
         forall|i: int| 0 <= i < final(slots)@.len() ==> final(slots)@[i] == cleaned(old(slots)@, mark_pred(marks), i),
         appended_calls(final(jobs)@, old(jobs)@, old(slots)@[0], finalization_args(old(slots)@, mark_pred(marks), old(slots)@.len())),
         job_roots_satisfy(final(jobs)@, old(jobs)@.len(), mark_pred(marks)),
    '''
    entry = method(scan_source, "finalize_entries", contract,bindings={'jobs.push_back(Job::Call {': {'before_enqueue': 'jobs@'}},loops={'while position < slots.len()': '''
     invariant 1 <= position <= slots@.len(), position % 3 == 1,
         slots@.len() == old(slots)@.len(), (slots@.len() - 1) % 3 == 0,
         callback == old(slots)@[0], marks.live(callback),
         forall|i: int| 1 <= i && i % 3 == 1 && i + 2 < old(slots)@.len()
            && old(slots)@[i].0 != Value::EMPTY.0 ==> marks.live(old(slots)@[i + 1]),
         forall|i: int| 0 <= i < slots@.len() ==> slots@[i] ==
            if i < position { cleaned(old(slots)@, mark_pred(marks), i) } else { old(slots)@[i] },
         appended_calls(jobs@, old(jobs)@, callback, finalization_args(old(slots)@, mark_pred(marks), position as nat)),
         job_roots_satisfy(jobs@, old(jobs)@.len(), mark_pred(marks)),
     decreases slots@.len() - position,
    '''},before={
    'while position < slots.len()': '''
     reveal_with_fuel(finalization_args, 2);
     initial_calls(jobs@, callback);
    ''',
    'position += 3;': '''
     reveal_with_fuel(finalization_args, 2);
     assert(finalization_args(old(slots)@, mark_pred(marks), (position + 3) as nat) ==
        if target.0 != Value::EMPTY.0 && !marks.live(target) {
            finalization_args(old(slots)@, mark_pred(marks), position as nat).push(held)
        } else { finalization_args(old(slots)@, mark_pred(marks), position as nat) });
     assert forall|i: int| 0 <= i < slots@.len() implies slots@[i] ==
         if i < position + 3 { cleaned(old(slots)@, mark_pred(marks), i) } else { old(slots)@[i] } by {
         if position <= i < position + 3 {
             assert((i - 1) / 3 == (position - 1) / 3);
             assert(1 + 3 * ((i - 1) / 3) == position);
         }
     }
    ''',
    'let callback = slots[0];': '''
     assert(!heap_kind(kind_of(Value::UNDEFINED.0))) by(compute);
    '''
    },after={
    'args: vec![held],\n                });': '''
     let job = jobs@.last();
     assert(jobs@ == before_enqueue.push(job));
     assert(call_view(job) == Some((callback, Value::UNDEFINED, seq![held])));
     assert(kind_of(Value::UNDEFINED.0) == Kind::Undefined) by(compute);
     assert(marks.live(Value::UNDEFINED));
     append_call(before_enqueue, old(jobs)@, callback,
         finalization_args(old(slots)@, mark_pred(marks), position as nat), job, held, mark_pred(marks));
    '''
    })
    parts.append(entry)
    marker = "pub fn main() {}"
    prelude = "use vstd::prelude::*;"
    if scan_code.count(marker) != 1 or scan_code.count(prelude) != 1:
        raise ValueError("Finalization proof composition boundary changed")
    return scan_code.replace(prelude, prelude + "\nuse std::collections::VecDeque;").replace(
        marker, "\n".join(parts) + "\n" + marker)


def heap_proof(heap_source, gc_source, object_source, arena_code, finalization_code):
    scope = item(item(gc_source, r"impl Heap\b"), r"fn clear_weak\b")
    arm = item(scope, r"InternalTag::FinalizationRegistry\s*=>")
    body = arm[arm.index("{"):]
    # Give the selected arm exactly its production free variables. All additions
    # are ghost code; its calls, snapshot construction and borrows are unchanged.
    wrapper = "fn finalization_arm(&mut self, payload: u32) " + body
    contract = '''
        requires old(self).payloads.internals.valid_marks(),
            old(self).payloads.internals.live_at(payload),
            old(self).registry_slots(payload).len() > 0,
            (old(self).registry_slots(payload).len() - 1) % 3 == 0,
            old(self).live(old(self).registry_slots(payload)[0]),
            forall|i: int| 1 <= i && i % 3 == 1 && i + 2 < old(self).registry_slots(payload).len()
                && old(self).registry_slots(payload)[i].0 != Value::EMPTY.0
                ==> old(self).live(old(self).registry_slots(payload)[i + 1]),
        ensures final(self).finalization_frame(old(self), payload),
            final(self).payloads.internals.valid_marks(),
            final(self).registry_slots(payload).len() == old(self).registry_slots(payload).len(),
            forall|i: int| 0 <= i < final(self).registry_slots(payload).len() ==>
                final(self).registry_slots(payload)[i] == cleaned(old(self).registry_slots(payload), old(self).heap_pred(), i),
            appended_calls(final(self).jobs@, old(self).jobs@, old(self).registry_slots(payload)[0],
                finalization_args(old(self).registry_slots(payload), old(self).heap_pred(), old(self).registry_slots(payload).len())),
            job_roots_satisfy(final(self).jobs@, old(self).jobs@.len(), final(self).heap_pred()),
    '''
    entry = method(wrapper, "finalization_arm", contract, before={
        "finalize_entries(": '''
            assert forall|v: Value| marks.live(v) == old(self).live(v) by {}
            equivalent_finalization_args(old(self).registry_slots(payload),
                mark_pred(&marks), old(self).heap_pred(), old(self).registry_slots(payload).len());
        ''',
    }, after={
        "&mut self.jobs,\n                    );": '''
            assert(self.finalization_frame(old(self), payload));
            assert forall|i: u32| self.payloads.internals.live_at(i)
                implies (i as int) < self.payloads.internals.records@.len() by {
                assert(old(self).payloads.internals.live_at(i));
            }
            assert forall|v: Value| self.live(v) == old(self).live(v) by {}
            assert forall|i: int| 0 <= i < self.registry_slots(payload).len() implies
                self.registry_slots(payload)[i] == cleaned(old(self).registry_slots(payload), old(self).heap_pred(), i) by {}
            assert(job_roots_satisfy(self.jobs@, old(self).jobs@.len(), self.heap_pred()));
        ''',
    })
    specs = '''
        spec fn heap_pred(&self) -> spec_fn(Value) -> bool { |v: Value| self.live(v) }
        spec fn registry_slots(&self, payload: u32) -> Seq<Value> {
            self.payloads.internals.records@[payload as int].slots@
        }
        spec fn finalization_frame(&self, previous: &Self, payload: u32) -> bool {
            *self == Self { payloads: self.payloads, jobs: self.jobs, ..*previous }
            && self.payloads == PayloadArenas { internals: self.payloads.internals, ..previous.payloads }
            && self.payloads.internals == Arena { records: self.payloads.internals.records, ..previous.payloads.internals }
            && self.payloads.internals.records@.len() == previous.payloads.internals.records@.len()
            && (forall|i: int| 0 <= i < self.payloads.internals.records@.len() && i != payload ==>
                self.payloads.internals.records@[i] == previous.payloads.internals.records@[i])
            && self.payloads.internals.records@[payload as int] == Internal {
                slots: self.payloads.internals.records@[payload as int].slots,
                ..previous.payloads.internals.records@[payload as int] }
        }
    '''
    code = weakpass.proof(heap_source, gc_source, object_source, arena_code, finalization_code,
        extra_heap=specs + entry, include_scan=False, include_arena_mut=True, include_jobs=True)
    return '#[path = "kernel.rs"]\nmod gc_kernel;\nmod heap_proof {\n' + code + '\n}\nfn main() {}\n'

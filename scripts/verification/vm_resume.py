"""Production register restoration, retaining the complete Cx/Heap/VM records."""

from records import record
from source import item, method
from vm_roots import types


def proof(heap_source, vm_source, interp_source, value_source, vm_code):
    _, _, _, heap, hp = types(heap_source, vm_source)
    cx, cp, _ = record(vm_source, "Cx", "C", {
        "heap": ("&'a mut Heap", "&'a mut " + heap),
        "static_code": ("&'a CodeStore", "&'a CCodeStore"),
        "realm": ("&'a Realm", "&'a CRealm"),
        "host": ("&'a mut Services", "&'a mut CServices"),
    })
    params = hp + ["CCodeStore", "CRealm", "CServices"] + cp
    generic = ", ".join(params)
    cx = cx.replace("pub(crate) struct", "struct").replace("Cx<'a>", "Cx<'a, " + generic + ">")
    declarations = [cx, item(value_source, r"const SPECIAL_UNDEFINED\b")]
    boxed = item(value_source, r"const fn boxed\b")
    declarations.append(boxed.replace("const fn boxed", "spec fn boxed_spec"))
    constant = item(item(value_source, r"impl Value\b"), r"pub const UNDEFINED\b")
    initializer = constant.split("=", 1)[1].strip().removesuffix(";")
    declarations.append('''impl Value {
        spec const SPEC_UNDEFINED: Value = SPEC_INITIALIZER;
        #[verifier::when_used_as_spec(SPEC_UNDEFINED)]
        exec const UNDEFINED: Value ensures Self::UNDEFINED == Self::SPEC_UNDEFINED { INITIALIZER }
    }'''.replace("SPEC_INITIALIZER", initializer.replace("boxed(", "boxed_spec(")).replace("INITIALIZER", initializer))
    declarations.append("impl<" + generic + "> Cx<'_, " + generic + "> {" + '''
        spec fn resume_frame(&self, previous: &Self) -> bool {
            *self == Self { heap: self.heap, ..*previous }
            && *self.heap == Heap { vm: self.heap.vm, ..*previous.heap }
            && self.heap.vm == VmStack { registers: self.heap.vm.registers,
                top: self.heap.vm.top, stale_from: self.heap.vm.stale_from, ..previous.heap.vm }
        }
    }''')
    clear = method(interp_source, "clear_registers", '''
        requires range.start <= range.end <= old(cx).heap.vm.registers@.len(),
        ensures final(cx).resume_frame(old(cx)),
            final(cx).heap.vm.top == old(cx).heap.vm.top,
            final(cx).heap.vm.stale_from == old(cx).heap.vm.stale_from,
            final(cx).heap.vm.registers@.len() == old(cx).heap.vm.registers@.len(),
            forall|i: int| 0 <= i < final(cx).heap.vm.registers@.len() ==>
                final(cx).heap.vm.registers@[i] == if range.start <= i < range.end {
                    Value::UNDEFINED } else { old(cx).heap.vm.registers@[i] },
    ''', loops={"while index < registers.len()": '''
        invariant index <= registers@.len(), registers@.len() == range.end - range.start,
            forall|i: int| 0 <= i < index ==> registers@[i] == Value::UNDEFINED,
        decreases registers@.len() - index,
    '''})
    raise_top = method(interp_source, "raise_top", '''
        requires old(cx).heap.vm.top <= old(cx).heap.vm.registers@.len(),
            end <= old(cx).heap.vm.registers@.len(),
        ensures final(cx).resume_frame(old(cx)), final(cx).heap.vm.top == end,
            final(cx).heap.vm.stale_from == if end > old(cx).heap.vm.top && old(cx).heap.vm.stale_from < end {
                end } else { old(cx).heap.vm.stale_from },
            final(cx).heap.vm.registers@.len() == old(cx).heap.vm.registers@.len(),
            forall|i: int| 0 <= i < final(cx).heap.vm.registers@.len() ==>
                final(cx).heap.vm.registers@[i] == if old(cx).heap.vm.top <= i < end && old(cx).heap.vm.stale_from <= i {
                    Value::UNDEFINED } else { old(cx).heap.vm.registers@[i] },
    ''')
    for name, code in (("clear_registers", clear), ("raise_top", raise_top)):
        code = code.replace("fn " + name + "(", "fn " + name + "<" + generic + ">(", 1)
        code = code.replace("Cx<'_>", "Cx<'_, " + generic + ">")
        declarations.append(code)
    # This composes the real helpers, but does not execute or replace collect.
    # The full collector must still prove that it retains the enumerated roots.
    declarations.append("fn scanned_registers_resume_safely<" + generic + ">(" +
        "cx: &mut Cx<'_, " + generic + ">, end: u32, stack: &mut Vec<Value>, Ghost(retained): Ghost<Set<Value>>)" + '''
        requires old(cx).heap.vm.top <= old(cx).heap.vm.registers@.len(),
            end <= old(cx).heap.vm.registers@.len(),
            forall|i: int| 0 <= i < old(cx).heap.vm.top
                && heap_kind(kind_of(old(cx).heap.vm.registers@[i].0)) ==>
                    retained.contains(old(cx).heap.vm.registers@[i]),
        ensures final(cx).heap.vm.top == end,
            forall|i: int| 0 <= i < end && heap_kind(kind_of(final(cx).heap.vm.registers@[i].0)) ==>
                retained.contains(final(cx).heap.vm.registers@[i]),
    {
        cx.heap.trace_vm_roots(stack);
        let ghost scanned = cx.heap.vm.registers@;
        let ghost previous_top = cx.heap.vm.top;
        let ghost boundary = cx.heap.vm.stale_from;
        raise_top(cx, end);
        proof {
            assert(!heap_kind(kind_of(Value::UNDEFINED.0))) by(compute);
            assert forall|i: int| 0 <= i < end && heap_kind(kind_of(cx.heap.vm.registers@[i].0))
                implies retained.contains(cx.heap.vm.registers@[i]) by {
                if i >= previous_top {
                    assert(boundary <= previous_top);
                    assert(cx.heap.vm.registers@[i] == Value::UNDEFINED);
                } else {
                    assert(cx.heap.vm.registers@[i] == scanned[i]);
                }
            }
        }
    }''')
    marker = "pub fn main() {}"
    if vm_code.count(marker) != 1:
        raise ValueError("VM resume proof lost its unique composition point")
    return vm_code.replace(marker, "\n".join(declarations) + "\n" + marker)

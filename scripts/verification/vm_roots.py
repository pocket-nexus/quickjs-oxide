"""Exact VM register-prefix and frame-function roots, with full record framing."""

from records import record
from source import item, method


def types(heap_source, vm_source):
    declarations = []

    def declare(source, name, prefix, fields, dependencies=()):
        declaration, parameters, _ = record(source, name, prefix, fields)
        parameters = list(dependencies) + parameters
        ty = name + ("<" + ", ".join(parameters) + ">" if parameters else "")
        declaration = declaration.replace("struct " + name, "struct " + ty, 1)
        # Restrict visibility in the closed proof module, as for other projections.
        declaration = declaration.replace("pub(crate) struct", "struct")
        declarations.append(declaration)
        return ty, parameters

    frame, fp = declare(vm_source, "Frame", "F", {"function": ("ObjectId", "ObjectId")})
    vm, vp = declare(vm_source, "VmStack", "V", {
        "registers": ("Vec<Value>", "Vec<Value>"),
        "frames": ("Vec<Frame>", "Vec<" + frame + ">"),
    }, fp)
    heap, hp = declare(heap_source, "Heap", "H", {"vm": ("VmStack", vm)}, vp)
    return declarations, frame, vm, heap, hp


def proof(heap_source, gc_source, vm_source, value_code):
    declarations, frame, _, heap, hp = types(heap_source, vm_source)
    mapper = "|activation: " + frame + "| Value((TAG_OBJECT << TAG_SHIFT) | activation.function.0 as u64)"
    specs = '''
        spec fn vm_frame(&self, previous: &Self) -> bool {
            *self == Self { vm: self.vm, ..*previous }
            && self.vm == VmStack { stale_from: self.vm.stale_from, ..previous.vm }
        }
        spec fn vm_roots(&self) -> Seq<Value> {
            self.vm.registers@.take(self.vm.top as int).filter(|v: Value| heap_kind(kind_of(v.0)))
            + self.vm.frames@.map_values(MAPPER)
        }
    '''.replace("MAPPER", mapper)
    trace = method(item(gc_source, r"impl Heap\b"), "trace_vm_roots", '''
        requires old(self).vm.top <= old(self).vm.registers@.len(),
        ensures final(self).vm_frame(old(self)),
            final(self).vm.stale_from == if old(self).vm.stale_from < old(self).vm.top {
                old(self).vm.stale_from } else { old(self).vm.top },
            final(stack)@ == old(stack)@ + old(self).vm_roots(),
    ''', loops={"while frame < self.vm.frames.len()": '''
        invariant self.vm_frame(old(self)), top == self.vm.top, top <= self.vm.registers@.len(),
            self.vm.stale_from == if old(self).vm.stale_from < old(self).vm.top {
                old(self).vm.stale_from } else { old(self).vm.top },
            frame <= self.vm.frames@.len(),
            stack@ == old(stack)@ + self.vm.registers@.take(top as int).filter(|v: Value| heap_kind(kind_of(v.0)))
                + self.vm.frames@.take(frame as int).map_values(MAPPER),
        decreases self.vm.frames@.len() - frame,
    '''.replace("MAPPER", mapper)}, before={"frame += 1;": '''
        let map = MAPPER;
        assert(self.vm.frames@.take(frame as int + 1).map_values(map) =~=
            self.vm.frames@.take(frame as int).map_values(map).push(map(self.vm.frames@[frame as int])));
    '''.replace("MAPPER", mapper)}, after={"frame += 1;\n        }": '''
        assert(self.vm.frames@.take(frame as int) =~= self.vm.frames@);
    '''})
    declarations.append("impl<" + ", ".join(hp) + "> " + heap + " {\n" + specs + trace + "\n}")
    marker = "pub fn main() {}"
    if value_code.count(marker) != 1:
        raise ValueError("VM root proof lost its unique composition point")
    return value_code.replace(marker, "\n".join(declarations) + "\n" + marker)

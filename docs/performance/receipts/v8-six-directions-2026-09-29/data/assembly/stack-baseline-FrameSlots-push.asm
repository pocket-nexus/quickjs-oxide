
/tmp/oxide-six-build-baseline/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000577010 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push>:
  577010:	push   %r15
  577012:	push   %r14
  577014:	push   %r12
  577016:	push   %rbx
  577017:	push   %rax
  577018:	mov    %rdx,%r14
  57701b:	mov    %rsi,%rbx
  57701e:	mov    0x8(%rdi),%r15
  577022:	mov    0x10(%rdi),%r12
  577026:	mov    %r15,%rdi
  577029:	mov    %r12,%rsi
  57702c:	mov    %rbx,%rdx
  57702f:	call   222250 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index>
  577034:	test   $0x1,%al
  577036:	jne    577050 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push+0x40>
  577038:	cmp    %rdx,%r12
  57703b:	jbe    57705f <quickjs_oxide::engine::vm::stack::window::FrameSlots::push+0x4f>
  57703d:	shl    $0x4,%rdx
  577041:	movups (%r14),%xmm0
  577045:	movups %xmm0,(%r15,%rdx,1)
  57704a:	incq   0x40(%rbx)
  57704e:	xor    %edx,%edx
  577050:	mov    %rdx,%rax
  577053:	add    $0x8,%rsp
  577057:	pop    %rbx
  577058:	pop    %r12
  57705a:	pop    %r14
  57705c:	pop    %r15
  57705e:	ret
  57705f:	lea    0x26be2a(%rip),%rax        # 7e2e90 <__do_global_dtors_aux_fini_array_entry+0x185d8>
  577066:	mov    %rdx,%rdi
  577069:	mov    %r12,%rsi
  57706c:	mov    %rax,%rdx
  57706f:	call   57e52 <core::panicking::panic_bounds_check>

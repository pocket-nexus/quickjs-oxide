
/tmp/oxide-six-build-baseline/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000576f20 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push>:
  576f20:	push   %r15
  576f22:	push   %r14
  576f24:	push   %r12
  576f26:	push   %rbx
  576f27:	push   %rax
  576f28:	mov    %rdx,%r14
  576f2b:	mov    %rsi,%rbx
  576f2e:	mov    0x8(%rdi),%r15
  576f32:	mov    0x10(%rdi),%r12
  576f36:	mov    %r15,%rdi
  576f39:	mov    %r12,%rsi
  576f3c:	mov    %rbx,%rdx
  576f3f:	call   222250 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index>
  576f44:	test   $0x1,%al
  576f46:	jne    576f60 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push+0x40>
  576f48:	cmp    %rdx,%r12
  576f4b:	jbe    576f6f <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push+0x4f>
  576f4d:	shl    $0x4,%rdx
  576f51:	movups (%r14),%xmm0
  576f55:	movups %xmm0,(%r15,%rdx,1)
  576f5a:	incq   0x40(%rbx)
  576f5e:	xor    %edx,%edx
  576f60:	mov    %rdx,%rax
  576f63:	add    $0x8,%rsp
  576f67:	pop    %rbx
  576f68:	pop    %r12
  576f6a:	pop    %r14
  576f6c:	pop    %r15
  576f6e:	ret
  576f6f:	lea    0x26bf1a(%rip),%rax        # 7e2e90 <__do_global_dtors_aux_fini_array_entry+0x185d8>
  576f76:	mov    %rdx,%rdi
  576f79:	mov    %r12,%rsi
  576f7c:	mov    %rax,%rdx
  576f7f:	call   57e52 <core::panicking::panic_bounds_check>

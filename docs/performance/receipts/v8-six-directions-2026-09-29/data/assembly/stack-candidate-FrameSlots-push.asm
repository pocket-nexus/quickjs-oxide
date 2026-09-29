
/home/eric/.cache/oxide-six-build-stack/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

00000000005771a0 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push>:
  5771a0:	mov    %rdi,%rax
  5771a3:	mov    0x40(%rsi),%r8
  5771a7:	mov    0x30(%rsi),%rdi
  5771ab:	mov    0x38(%rsi),%rcx
  5771af:	xor    %r9d,%r9d
  5771b2:	sub    %rdi,%rcx
  5771b5:	cmovae %rcx,%r9
  5771b9:	cmp    %r9,%r8
  5771bc:	jae    68fe0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded>
  5771c2:	mov    0x10(%rax),%rcx
  5771c6:	add    %r8,%rdi
  5771c9:	cmp    %rcx,%rdi
  5771cc:	jae    5771f3 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push+0x53>
  5771ce:	mov    0x8(%rax),%rax
  5771d2:	shl    $0x4,%rdi
  5771d6:	cmpb   $0xe,(%rax,%rdi,1)
  5771da:	jne    690d0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value>
  5771e0:	add    %rdi,%rax
  5771e3:	movups (%rdx),%xmm0
  5771e6:	movups %xmm0,(%rax)
  5771e9:	inc    %r8
  5771ec:	mov    %r8,0x40(%rsi)
  5771f0:	xor    %eax,%eax
  5771f2:	ret
  5771f3:	push   %rax
  5771f4:	lea    0x26bfdd(%rip),%rdx        # 7e31d8 <__do_global_dtors_aux_fini_array_entry+0x18920>
  5771fb:	mov    %rcx,%rsi
  5771fe:	call   57e52 <core::panicking::panic_bounds_check>

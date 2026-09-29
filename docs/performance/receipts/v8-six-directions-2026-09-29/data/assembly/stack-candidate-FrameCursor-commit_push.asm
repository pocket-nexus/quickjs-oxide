
/home/eric/.cache/oxide-six-build-stack/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

00000000005770b0 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push>:
  5770b0:	mov    %rdi,%rax
  5770b3:	mov    0x40(%rsi),%r8
  5770b7:	mov    0x30(%rsi),%rdi
  5770bb:	mov    0x38(%rsi),%rcx
  5770bf:	xor    %r9d,%r9d
  5770c2:	sub    %rdi,%rcx
  5770c5:	cmovae %rcx,%r9
  5770c9:	cmp    %r9,%r8
  5770cc:	jae    68fe0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded>
  5770d2:	mov    0x10(%rax),%rcx
  5770d6:	add    %r8,%rdi
  5770d9:	cmp    %rcx,%rdi
  5770dc:	jae    577103 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push+0x53>
  5770de:	mov    0x8(%rax),%rax
  5770e2:	shl    $0x4,%rdi
  5770e6:	cmpb   $0xe,(%rax,%rdi,1)
  5770ea:	jne    690d0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value>
  5770f0:	add    %rdi,%rax
  5770f3:	movups (%rdx),%xmm0
  5770f6:	movups %xmm0,(%rax)
  5770f9:	inc    %r8
  5770fc:	mov    %r8,0x40(%rsi)
  577100:	xor    %eax,%eax
  577102:	ret
  577103:	push   %rax
  577104:	lea    0x26c0cd(%rip),%rdx        # 7e31d8 <__do_global_dtors_aux_fini_array_entry+0x18920>
  57710b:	mov    %rcx,%rsi
  57710e:	call   57e52 <core::panicking::panic_bounds_check>

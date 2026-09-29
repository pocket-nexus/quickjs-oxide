
/home/eric/.cache/oxide-six-build-accepted/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000578190 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push>:
  578190:	48 89 f8             	mov    %rdi,%rax
  578193:	4c 8b 46 40          	mov    0x40(%rsi),%r8
  578197:	48 8b 7e 30          	mov    0x30(%rsi),%rdi
  57819b:	48 8b 4e 38          	mov    0x38(%rsi),%rcx
  57819f:	45 31 c9             	xor    %r9d,%r9d
  5781a2:	48 29 f9             	sub    %rdi,%rcx
  5781a5:	4c 0f 43 c9          	cmovae %rcx,%r9
  5781a9:	4d 39 c8             	cmp    %r9,%r8
  5781ac:	0f 83 2e 0e af ff    	jae    68fe0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded>
  5781b2:	48 8b 48 10          	mov    0x10(%rax),%rcx
  5781b6:	4c 01 c7             	add    %r8,%rdi
  5781b9:	48 39 cf             	cmp    %rcx,%rdi
  5781bc:	73 25                	jae    5781e3 <quickjs_oxide::engine::vm::execute::FrameCursor::commit_push+0x53>
  5781be:	48 8b 40 08          	mov    0x8(%rax),%rax
  5781c2:	48 c1 e7 04          	shl    $0x4,%rdi
  5781c6:	80 3c 38 0e          	cmpb   $0xe,(%rax,%rdi,1)
  5781ca:	0f 85 00 0f af ff    	jne    690d0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value>
  5781d0:	48 01 f8             	add    %rdi,%rax
  5781d3:	0f 10 02             	movups (%rdx),%xmm0
  5781d6:	0f 11 00             	movups %xmm0,(%rax)
  5781d9:	49 ff c0             	inc    %r8
  5781dc:	4c 89 46 40          	mov    %r8,0x40(%rsi)
  5781e0:	31 c0                	xor    %eax,%eax
  5781e2:	c3                   	ret
  5781e3:	50                   	push   %rax
  5781e4:	48 8d 15 ed bf 26 00 	lea    0x26bfed(%rip),%rdx        # 7e41d8 <__do_global_dtors_aux_fini_array_entry+0x18950>
  5781eb:	48 89 ce             	mov    %rcx,%rsi
  5781ee:	e8 5f fc ad ff       	call   57e52 <core::panicking::panic_bounds_check>

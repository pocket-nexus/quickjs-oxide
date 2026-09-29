
/home/eric/.cache/oxide-six-build-accepted/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000578280 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push>:
  578280:	48 89 f8             	mov    %rdi,%rax
  578283:	4c 8b 46 40          	mov    0x40(%rsi),%r8
  578287:	48 8b 7e 30          	mov    0x30(%rsi),%rdi
  57828b:	48 8b 4e 38          	mov    0x38(%rsi),%rcx
  57828f:	45 31 c9             	xor    %r9d,%r9d
  578292:	48 29 f9             	sub    %rdi,%rcx
  578295:	4c 0f 43 c9          	cmovae %rcx,%r9
  578299:	4d 39 c8             	cmp    %r9,%r8
  57829c:	0f 83 3e 0d af ff    	jae    68fe0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded>
  5782a2:	48 8b 48 10          	mov    0x10(%rax),%rcx
  5782a6:	4c 01 c7             	add    %r8,%rdi
  5782a9:	48 39 cf             	cmp    %rcx,%rdi
  5782ac:	73 25                	jae    5782d3 <quickjs_oxide::engine::vm::stack::window::FrameSlots::push+0x53>
  5782ae:	48 8b 40 08          	mov    0x8(%rax),%rax
  5782b2:	48 c1 e7 04          	shl    $0x4,%rdi
  5782b6:	80 3c 38 0e          	cmpb   $0xe,(%rax,%rdi,1)
  5782ba:	0f 85 10 0e af ff    	jne    690d0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value>
  5782c0:	48 01 f8             	add    %rdi,%rax
  5782c3:	0f 10 02             	movups (%rdx),%xmm0
  5782c6:	0f 11 00             	movups %xmm0,(%rax)
  5782c9:	49 ff c0             	inc    %r8
  5782cc:	4c 89 46 40          	mov    %r8,0x40(%rsi)
  5782d0:	31 c0                	xor    %eax,%eax
  5782d2:	c3                   	ret
  5782d3:	50                   	push   %rax
  5782d4:	48 8d 15 fd be 26 00 	lea    0x26befd(%rip),%rdx        # 7e41d8 <__do_global_dtors_aux_fini_array_entry+0x18950>
  5782db:	48 89 ce             	mov    %rcx,%rsi
  5782de:	e8 6f fb ad ff       	call   57e52 <core::panicking::panic_bounds_check>
